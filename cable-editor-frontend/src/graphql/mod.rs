use crate::error::FrontendError;
use cynic::{
    GraphQlResponse, MutationBuilder, Operation, QueryBuilder, QueryFragment, QueryVariables,
    http::{CynicReqwestError, ReqwestExt},
};
use lazy_static::lazy_static;
use reqwest::header::{AUTHORIZATION, HeaderMap};
use serde::Serialize;
use yew_oauth2::prelude::{Authentication, OAuth2Context};

pub mod anonymous;
pub mod authenticated;

lazy_static! {
    static ref GRAPHQL_URL: String = format!("{}/graphql", host());
    static ref GRAPHQL_ANONYMOUS_URL: String = format!("{}/graphql_anonymous", host());
}

pub fn host() -> String {
    let location = web_sys::window().unwrap().location();
    let host = location.host().unwrap();
    let protocol = location.protocol().unwrap();
    format!("{protocol}//{host}")
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
        &GRAPHQL_ANONYMOUS_URL,
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
        &GRAPHQL_URL,
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
        &GRAPHQL_URL,
        credentials,
        Q::build(request),
        FrontendError::ErrorQueryingAuthenticatedConnect,
        FrontendError::ErrorQueryingAuthenticatedTransfer,
    )
    .await
}

/// Sends `operation` to `url`, with the bearer token of `credentials` if logged in, and returns
/// the data of the response: its errors, if any, as `FrontendError::Graphql`, and a response
/// with neither data nor errors as `FrontendError::NotFound`.
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
    let response = client
        .post(url)
        .run_graphql(operation)
        .await
        .map_err(transfer_error)?;
    match response {
        GraphQlResponse {
            errors: Some(errors),
            ..
        } => Err(FrontendError::Graphql(errors)),
        GraphQlResponse {
            data: Some(data), ..
        } => Ok(data),
        GraphQlResponse { data: None, .. } => Err(FrontendError::NotFound),
    }
}
