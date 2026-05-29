use crate::error::FrontendError;
use cynic::{GraphQlResponse, QueryBuilder, QueryFragment, QueryVariables, http::ReqwestExt};
use lazy_static::lazy_static;
use yew::Component;
use yew::html::Scope;
pub mod anonymous;

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
pub async fn query_anonymous<Q, Variables, S>(
    request: Variables,
) -> Result<GraphQlResponse<Q>, FrontendError>
where
    S: Component,
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
