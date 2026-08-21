use std::collections::HashMap;

use actix_4_jwt_auth::{
    DecodedInfo, OIDCValidationError, Oidc, OidcBiscuitValidator, OidcConfig,
    biscuit::{Validation, ValidationOptions},
};
use actix_web::{
    App, HttpServer, get,
    guard::Post,
    middleware::Logger,
    web::{Data, resource},
};
use actix_web_prometheus::PrometheusMetricsBuilder;
use actix_web_static_files::ResourceFiles;
use async_graphql::futures_util::future::join_all;
use async_graphql::{Response, ServerError};
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};
use cable_editor_backend::db::{DB, run_sync_migrations};
use cable_editor_backend::{
    config::CONFIG,
    db::connect,
    graphql::{
        anonymous::{AnonymousGraphqlSchema, create_anonymous_schema},
        authenticated::{AuthenticatedGraphqlSchema, create_authenticated_schema},
        context::UserInfo,
    },
};
use cached::cached;
use env_logger::Env;
use log::{error, info, trace};
use prometheus::{HistogramVec, histogram_opts};
use reqwest::Client;
use static_files::Resource;
use thiserror::Error;
use tracing_actix_web::TracingLogger;
//include!(concat!(env!("OUT_DIR"), "/generated.rs"));

async fn graphql(
    context: Data<ApplicationContext>,
    user: Option<DecodedInfo>,
    request: GraphQLRequest,
) -> GraphQLResponse {
    //let user: Option<AuthenticatedUser<UserInfo>> = Some(user);
    trace!("Execute Authenticated: {user:#?}");
    let schema = &context.schema;
    let histogram = context.graphql_request_histogram.clone();
    let request = request.into_inner().data(context.pool.clone());
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
    let timer = histogram
        .with_label_values(&[
            request.operation_name.as_deref().unwrap_or_default(),
            found_user.preferred_username.as_ref(),
        ])
        .start_timer();
    let request = request.data(found_user);

    let response = schema.execute(request).await;
    timer.stop_and_record();
    response.into()
}

#[cached(ttl = 30)]
async fn fetch_user_info(access_token_str: String) -> Result<UserInfo, BackendError> {
    let client = Client::new();
    let issuer = CONFIG.auth_issuer();
    let response = client
        .get(format!("{issuer}/api/oidc/userinfo"))
        .bearer_auth(access_token_str)
        .send()
        .await?;

    Ok(response.json().await?)
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

#[get("/health")]
async fn health() -> &'static str {
    "Ok"
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
    env_logger::init_from_env(Env::default().filter_or("LOG_LEVEL", "debug"));

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
        pool: connection_pool,
    });
    let main_server = HttpServer::new(move || {
        let resources: HashMap<&str, Resource> = HashMap::new(); // generate();

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
            .service(ResourceFiles::new("/", resources).resolve_not_found_to_root())
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
    let mgmt_server = HttpServer::new(move || App::new().wrap(prometheus.clone()).service(health))
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
