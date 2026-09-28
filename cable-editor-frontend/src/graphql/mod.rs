use crate::error::FrontendError;
use cable_editor_common::UserError;
use cynic::{
    GraphQlResponse, MutationBuilder, Operation, QueryBuilder, QueryFragment, QueryVariables,
    http::CynicReqwestError,
};
use reqwest::header::{AUTHORIZATION, HeaderMap};
use serde::{Deserialize, Serialize};
use yew_oauth2::prelude::{Authentication, OAuth2Context};

pub mod anonymous;
pub mod authenticated;

/// URL of `path` on the server the app was loaded from (`trunk serve` proxies it to the backend).
fn server_url(path: &str) -> Result<String, FrontendError> {
    let origin = web_sys::window()
        .and_then(|window| window.location().origin().ok())
        .ok_or(FrontendError::NoServerAddress)?;
    Ok(format!("{origin}{path}"))
}

pub async fn query_anonymous<Q, V>(request: V) -> Result<Q, FrontendError>
where
    Q: QueryFragment<VariablesFields = V::Fields>
        + QueryBuilder<V>
        + serde::de::DeserializeOwned
        + 'static,
    Q::SchemaType: cynic::schema::QueryRoot,
    V: QueryVariables + Serialize,
{
    run(
        &server_url("/graphql_anonymous")?,
        None,
        Q::build(request),
        FrontendError::ErrorQueryingAnonymousConnect,
        FrontendError::ErrorQueryingAnonymousTransfer,
    )
    .await
}

pub async fn query<Q, V>(
    request: V,
    credentials: Option<&OAuth2Context>,
) -> Result<Q, FrontendError>
where
    Q: QueryFragment<VariablesFields = V::Fields>
        + QueryBuilder<V>
        + serde::de::DeserializeOwned
        + 'static,
    Q::SchemaType: cynic::schema::QueryRoot,
    V: QueryVariables + Serialize,
{
    run(
        &server_url("/graphql")?,
        credentials,
        Q::build(request),
        FrontendError::ErrorQueryingAuthenticatedConnect,
        FrontendError::ErrorQueryingAuthenticatedTransfer,
    )
    .await
}

pub async fn mutate<Q, V>(
    request: V,
    credentials: Option<&OAuth2Context>,
) -> Result<Q, FrontendError>
where
    Q: QueryFragment<VariablesFields = V::Fields>
        + MutationBuilder<V>
        + serde::de::DeserializeOwned
        + 'static,
    Q::SchemaType: cynic::schema::MutationRoot,
    V: QueryVariables + Serialize,
{
    run(
        &server_url("/graphql")?,
        credentials,
        Q::build(request),
        FrontendError::ErrorQueryingAuthenticatedConnect,
        FrontendError::ErrorQueryingAuthenticatedTransfer,
    )
    .await
}

/// What the backend adds to a GraphQL error: why it refused the request (see
/// docs/fehlermeldungen.md). Read leniently, so an unknown reason (a newer backend) is a
/// technical error instead of an unreadable response.
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct ErrorExtensions {
    #[serde(default)]
    user_error: Option<serde_json::Value>,
}

impl ErrorExtensions {
    fn user_error(&self) -> Option<UserError> {
        serde_json::from_value(self.user_error.clone()?).ok()
    }
}

/// The response to `operation`, its data as JSON (like cynic's `run_graphql`, which would
/// decode the data as the query's type right away).
async fn send<Q, V: Serialize>(
    client: &reqwest::Client,
    url: &str,
    operation: &Operation<Q, V>,
) -> Result<GraphQlResponse<serde_json::Value, ErrorExtensions>, CynicReqwestError> {
    let response = client.post(url).json(operation).send().await?;
    let status = response.status();
    if status.is_success() {
        return Ok(response.json().await?);
    }
    // An error status may still carry GraphQL errors
    let text = response.text().await?;
    serde_json::from_str(&text).map_err(|_| CynicReqwestError::ErrorResponse(status, text))
}

/// Sends `operation` to `url`, with the bearer token of `credentials` if logged in, and returns
/// the data of the response. Its first error: `FrontendError::User` if the backend refused the
/// request, else `FrontendError::Graphql` with the messages; a response with neither data nor
/// errors: `FrontendError::NotFound`; data that doesn't fit the query:
/// `FrontendError::InvalidResponse`.
async fn run<Q, V>(
    url: &str,
    credentials: Option<&OAuth2Context>,
    operation: Operation<Q, V>,
    connect_error: fn(reqwest::Error) -> FrontendError,
    transfer_error: fn(CynicReqwestError) -> FrontendError,
) -> Result<Q, FrontendError>
where
    Q: serde::de::DeserializeOwned + 'static,
    V: Serialize,
{
    let mut headers = HeaderMap::new();
    if let Some(OAuth2Context::Authenticated(Authentication { access_token, .. })) = credentials {
        headers.insert(AUTHORIZATION, format!("Bearer {access_token}").parse()?);
    }
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(connect_error)?;
    // The data only as JSON first: with errors it may lack fields the query requires (the
    // backend leaves a failed field out), which must not hide the errors
    let response = send(&client, url, &operation)
        .await
        .map_err(transfer_error)?;
    match response {
        GraphQlResponse {
            errors: Some(errors),
            ..
        } => Err(
            match errors
                .iter()
                .find_map(|e| e.extensions.as_ref()?.user_error())
            {
                Some(user_error) => FrontendError::User(user_error),
                None => FrontendError::Graphql(errors.into_iter().map(|e| e.message).collect()),
            },
        ),
        GraphQlResponse {
            data: Some(data), ..
        } => serde_json::from_value(data).map_err(FrontendError::InvalidResponse),
        GraphQlResponse { data: None, .. } => Err(FrontendError::NotFound),
    }
}
