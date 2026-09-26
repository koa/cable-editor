use actix_4_jwt_auth::{
    DecodedInfo, OIDCValidationError, Oidc, OidcBiscuitValidator, OidcConfig,
    biscuit::{Validation, ValidationOptions},
};
use actix_web::{
    App, HttpMessage, HttpRequest, HttpResponse, HttpServer, Responder, get,
    guard::Post,
    http::header::{CacheControl, CacheDirective, ETag, EntityTag, IfNoneMatch},
    middleware::Logger,
    web,
    web::{Data, resource},
};
use actix_web_prometheus::PrometheusMetricsBuilder;
use async_graphql::{Response, ServerError, futures_util::future::join_all};
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};
use cable_editor_backend::{
    RunQueryDsl,
    config::CONFIG,
    db::{DB, connect, run_sync_migrations},
    graphql::{
        anonymous::{AnonymousGraphqlSchema, create_anonymous_schema},
        authenticated::{AuthenticatedGraphqlSchema, create_authenticated_schema},
        context::UserInfo,
    },
    sql_query,
};
use cached::cached;
use env_logger::Env;
use log::{info, trace, warn};
use mime_guess::from_path;
use prometheus::{HistogramVec, histogram_opts};
use reqwest::Client;
use rust_embed::RustEmbed;
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::sync::Mutex;
use tracing_actix_web::TracingLogger;

#[derive(RustEmbed)]
#[folder = "../cable-editor-frontend/dist"]
struct Assets;

async fn static_handler(req: HttpRequest) -> impl Responder {
    // Den Pfad aus der URL extrahieren (catch-all)
    let path = match req.match_info().query("filename") {
        "" => "index.html",
        path => path,
    };
    // Fallback für Yew (SPA): Wenn eine Route nicht gefunden wird,
    // liefere die index.html aus, damit der Yew-Router übernehmen kann.
    let Some((path, content)) = Assets::get(path)
        .map(|content| (path, content))
        .or_else(|| Assets::get("index.html").map(|content| ("index.html", content)))
    else {
        return HttpResponse::NotFound().body("404 Not Found");
    };
    let hash = content.metadata.sha256_hash();
    let etag = EntityTag::new_strong(hash.iter().map(|b| format!("{b:02x}")).collect());
    // Hashed files never change, others are revalidated and answered with 304 if unchanged
    let cache_control = CacheControl(if is_hashed(path) {
        vec![
            CacheDirective::Public,
            CacheDirective::MaxAge(31_536_000),
            CacheDirective::Extension("immutable".into(), None),
        ]
    } else {
        vec![CacheDirective::NoCache]
    });
    let unchanged = match req.get_header::<IfNoneMatch>() {
        Some(IfNoneMatch::Any) => true,
        Some(IfNoneMatch::Items(tags)) => tags.iter().any(|tag| tag.weak_eq(&etag)),
        None => false,
    };
    if unchanged {
        return HttpResponse::NotModified()
            .insert_header(ETag(etag))
            .insert_header(cache_control)
            .finish();
    }
    HttpResponse::Ok()
        .content_type(from_path(path).first_or_octet_stream().as_ref())
        .insert_header(ETag(etag))
        .insert_header(cache_control)
        .body(content.data.into_owned())
}

/// Trunk output with a content hash in its name, e.g. `app-525c874bfe2010a5_bg.wasm`.
fn is_hashed(path: &str) -> bool {
    let Some((stem, _)) = path.rsplit_once('.') else {
        return false;
    };
    let stem = stem.strip_suffix("_bg").unwrap_or(stem);
    !path.contains('/')
        && stem
            .rsplit_once('-')
            .is_some_and(|(_, hash)| hash.len() >= 8 && hash.chars().all(|c| c.is_ascii_hexdigit()))
}

async fn graphql(
    context: Data<ApplicationContext>,
    user: Option<DecodedInfo>,
    request: GraphQLRequest,
) -> GraphQLResponse {
    trace!("Execute Authenticated: {user:#?}");
    let schema = &context.schema;
    let histogram = context.graphql_request_histogram.clone();

    let found_user = if let Some(DecodedInfo { jwt, payload: _ }) = user {
        match fetch_user_info(jwt).await {
            Ok(info) => info,
            Err(error) => {
                return Response::from_errors(vec![ServerError::new(error.to_string(), None)])
                    .into();
            }
        }
    } else {
        return Response::from_errors(vec![ServerError::new("No user token found", None)]).into();
    };

    let mut connection = match context.pool.get().await {
        Ok(connection) => connection,
        Err(error) => {
            return Response::from_errors(vec![ServerError::new(error.to_string(), None)]).into();
        }
    };
    let request = request.into_inner();
    let timer = histogram
        .with_label_values(&[
            request.operation_name.as_deref().unwrap_or_default(),
            found_user.preferred_username.as_ref(),
        ])
        .start_timer();

    if let Err(e) = sql_query("BEGIN").execute(&mut connection).await {
        return Response::from_errors(vec![ServerError::new(
            format!("Failed to start transaction: {}", e),
            None,
        )])
        .into();
    }

    let shared_conn = Arc::new(Mutex::new(connection));

    let request = request.data(shared_conn.clone()).data(found_user);

    let response = schema.execute(request).await;

    let mut final_conn = shared_conn.lock().await;
    if response.errors.is_empty() {
        if let Err(e) = sql_query("COMMIT").execute(&mut *final_conn).await {
            log::error!("Failed to commit transaction: {}", e);
        }
    } else {
        if let Err(e) = sql_query("ROLLBACK").execute(&mut *final_conn).await {
            log::error!("Failed to rollback transaction: {}", e);
        }
    }

    timer.stop_and_record();

    response.into()
}
#[cached(ttl = 30)]
async fn fetch_user_info(access_token_str: String) -> Result<UserInfo, BackendError> {
    let response = Client::new()
        .get(user_info_url().await?)
        .bearer_auth(access_token_str)
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json().await?)
}

#[derive(Deserialize)]
struct OidcDiscovery {
    userinfo_endpoint: String,
}

/// Configured userinfo endpoint, else the one from the issuer's discovery document.
#[cached]
async fn user_info_url() -> Result<String, BackendError> {
    if let Some(url) = CONFIG.user_info_url() {
        return Ok(url.to_string());
    }
    let issuer = CONFIG.auth_issuer();
    let discovery: OidcDiscovery = Client::new()
        .get(format!("{issuer}/.well-known/openid-configuration"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    info!("Userinfo endpoint: {}", discovery.userinfo_endpoint);
    Ok(discovery.userinfo_endpoint)
}

async fn graphql_anonymous(
    context: Data<ApplicationContext>,
    request: GraphQLRequest,
) -> GraphQLResponse {
    let schema = &context.anonymous_schema;
    let histogram = context.graphql_request_histogram.clone();
    let request = request.into_inner();
    let timer = histogram
        .with_label_values(&[request.operation_name.as_deref().unwrap_or_default(), ""])
        .start_timer();

    let response = schema.execute(request).await;
    timer.stop_and_record();
    response.into()
}

/// Liveness: the process answers. Doesn't check the database, restarting doesn't fix it.
#[get("/health")]
async fn health() -> &'static str {
    "Ok"
}

/// Readiness: requests can be served, i.e. the database answers within a second (the pool has
/// no wait timeout of its own, the probe's timeout is longer).
#[get("/ready")]
async fn ready(pool: Data<DB>) -> HttpResponse {
    let check = async {
        let mut connection = pool.get().await.map_err(|e| e.to_string())?;
        sql_query("SELECT 1")
            .execute(&mut connection)
            .await
            .map_err(|e| e.to_string())
    };
    match tokio::time::timeout(Duration::from_secs(1), check).await {
        Ok(Ok(_)) => HttpResponse::Ok().body("Ok"),
        Ok(Err(e)) => {
            warn!("Database not reachable: {e}");
            HttpResponse::ServiceUnavailable().body("Database not reachable")
        }
        Err(_) => {
            warn!("Database didn't answer within a second");
            HttpResponse::ServiceUnavailable().body("Database not reachable")
        }
    }
}

#[derive(Clone)]
struct ApplicationContext {
    graphql_request_histogram: HistogramVec,
    schema: AuthenticatedGraphqlSchema,
    anonymous_schema: AnonymousGraphqlSchema,
    pool: DB,
}

#[derive(Error, Debug)]
enum BackendError {
    #[error("An IO Error happened {0}")]
    IO(#[from] std::io::Error),
    #[error("An Error from prometheus {0}")]
    Prometheus(#[from] prometheus::Error),
    #[error("An Error from prometheus {0}")]
    ActixWebPrometheus(#[from] actix_web_prometheus::error::Error),
    #[error("Error on OIDC Validation {0}")]
    OidcValidationError(#[from] OIDCValidationError),
    #[error("Error from backend {0:?}")]
    Backend(#[from] cable_editor_backend::error::BackendError),
    #[error("Cannot fetch http data {0}")]
    Reqwest(#[from] reqwest::Error),
}

#[actix_web::main]
async fn main() -> Result<(), BackendError> {
    env_logger::init_from_env(Env::default().filter_or("LOG_LEVEL", "info"));

    run_sync_migrations();

    let connection_pool = connect().await?;

    info!(
        "Database connection established: {:?}",
        connection_pool.status()
    );

    let bind_addr = CONFIG.server_bind_address();
    let api_port = CONFIG.server_port();
    let mgmt_port = CONFIG.server_mgmt_port();

    let mut labels = HashMap::new();
    labels.insert("server".to_string(), "api".to_string());

    let graphql_request_histogram = HistogramVec::new(
        histogram_opts!("graphql_request", "Measure graphql queries"),
        &["name", "user"],
    )?;
    let prometheus = PrometheusMetricsBuilder::new("")
        .const_labels(labels)
        .build()?;

    let registry = prometheus.registry.clone();
    registry.register(Box::new(graphql_request_histogram.clone()))?;

    let schema = create_authenticated_schema();
    let anonymous_schema = create_anonymous_schema();

    let issuer = CONFIG.auth_issuer().to_string();
    info!("Issuer: {issuer}");
    let oidc = Oidc::new(OidcConfig::Issuer(issuer.clone().into())).await?;

    let biscuit_validator = OidcBiscuitValidator {
        options: ValidationOptions {
            issuer: Validation::Validate(issuer),
            ..ValidationOptions::default()
        },
    };

    let data = Data::new(ApplicationContext {
        graphql_request_histogram,
        schema,
        anonymous_schema,
        pool: connection_pool.clone(),
    });
    let main_server = HttpServer::new(move || {
        App::new()
            .wrap(prometheus.clone())
            .wrap(TracingLogger::default())
            .wrap(Logger::default())
            .app_data(data.clone())
            .app_data(oidc.clone())
            .service(
                resource("/graphql")
                    .guard(Post())
                    .wrap(biscuit_validator.clone())
                    .to(graphql),
            )
            .service(
                resource("/graphql_anonymous")
                    .guard(Post())
                    .to(graphql_anonymous),
            )
            // workaround for proxy troubles
            .service(
                resource("/graphql/")
                    .guard(Post())
                    .wrap(biscuit_validator.clone())
                    .to(graphql),
            )
            .service(
                resource("/graphql_anonymous/")
                    .guard(Post())
                    .to(graphql_anonymous),
            )
            .route("/{filename:.*}", web::get().to(static_handler))
    })
    .bind((bind_addr, api_port))?
    .run();
    let mut labels = HashMap::new();
    labels.insert("server".to_string(), "mgmt".to_string());

    let prometheus = PrometheusMetricsBuilder::new("")
        .const_labels(labels)
        .registry(registry)
        .endpoint("/metrics")
        .build()
        .unwrap();
    let pool = Data::new(connection_pool);
    let mgmt_server = HttpServer::new(move || {
        App::new()
            .wrap(prometheus.clone())
            .app_data(pool.clone())
            .service(health)
            .service(ready)
    })
    .bind((bind_addr, mgmt_port))?
    .workers(2)
    .run();
    if let Some(e) = join_all(vec![main_server, mgmt_server])
        .await
        .into_iter()
        .flat_map(|r| r.err())
        .next()
    {
        return Err(e.into());
    }
    Ok(())
}
