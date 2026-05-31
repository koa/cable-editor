use crate::error::FrontendError;
use cynic::{GraphQlResponse, QueryBuilder, QueryFragment, QueryVariables, http::ReqwestExt};
use lazy_static::lazy_static;
use reqwest::header::{HeaderMap, AUTHORIZATION};
use yew::{Callback, Component};
use yew::html::Scope;
use yew::platform::spawn_local;
use yew_oauth2::context::OAuth2Context;
use yew_oauth2::prelude::Authentication;
use yew_oauth2::prelude::OAuth2Context::Authenticated;

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

// Send Graphql-Query to server
pub async fn query_anonymous<Q, Variables>(
    request: Variables,
) -> Result<GraphQlResponse<Q>, FrontendError>
where
    Variables: QueryVariables + cynic::serde::Serialize,
    Q: QueryFragment
        + QueryFragment<VariablesFields = Variables::Fields>
        + serde::de::DeserializeOwned
        + 'static,
    Q::SchemaType: cynic::schema::QueryRoot,
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

pub async fn query<Q, Variables, C>(request: Variables, scope: Scope<C>) -> Result<GraphQlResponse<Q>, FrontendError>
where
    Variables: QueryVariables + cynic::serde::Serialize,
    Q: QueryFragment
        + QueryFragment<VariablesFields = Variables::Fields>
        + serde::de::DeserializeOwned
        + 'static,
    Q::SchemaType: cynic::schema::QueryRoot,
    C: Component
{
    let credentials = scope
        .context::<OAuth2Context>(Callback::noop())
        .map(|r| r.0);

    let mut headers = HeaderMap::new();
    if let Some(Authenticated(Authentication { access_token, .. })) = credentials.as_ref() {
        headers.insert(AUTHORIZATION, format!("Bearer {access_token}").parse()?);
    }
    if let Some((auth_context,handle)) = scope.context::<OAuth2Context>(Callback::noop()){
        auth_context.access_token();

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
