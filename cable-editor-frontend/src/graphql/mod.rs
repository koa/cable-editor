use crate::error::FrontendError;
use cynic::{GraphQlResponse, QueryBuilder, QueryFragment, QueryVariables, http::ReqwestExt};
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

pub async fn query_anonymous<Q, V>(request: V) -> Result<GraphQlResponse<Q>, FrontendError>
where
    Q: QueryFragment<VariablesFields = V::Fields>
        + QueryBuilder<V>
        + serde::de::DeserializeOwned
        + 'static,
    Q::SchemaType: cynic::schema::QueryRoot,
    V: QueryVariables + Serialize,
{
    let client = reqwest::Client::builder()
        .build()
        .map_err(FrontendError::ErrorQueryingAnonymousConnect)?;
    let response = client
        .post(GRAPHQL_ANONYMOUS_URL.as_str())
        .run_graphql(Q::build(request))
        .await
        .map_err(FrontendError::ErrorQueryingAnonymousTransfer)?;
    Ok(response)
}

pub async fn query<Q, V>(
    request: V,
    credentials: Option<&OAuth2Context>,
) -> Result<GraphQlResponse<Q>, FrontendError>
where
    Q: QueryFragment<VariablesFields = V::Fields>
        + QueryBuilder<V>
        + serde::de::DeserializeOwned
        + 'static,
    Q::SchemaType: cynic::schema::QueryRoot,
    V: QueryVariables + Serialize,
{
    let mut headers = HeaderMap::new();
    if let Some(OAuth2Context::Authenticated(Authentication { access_token, .. })) =
        credentials.as_ref()
    {
        headers.insert(AUTHORIZATION, format!("Bearer {access_token}").parse()?);
    }
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(FrontendError::ErrorQueryingAuthenticatedConnect)?;

    let response = client
        .post(GRAPHQL_URL.as_str())
        .run_graphql(Q::build(request))
        .await
        .map_err(FrontendError::ErrorQueryingAuthenticatedTransfer)?;
    Ok(response)
}
