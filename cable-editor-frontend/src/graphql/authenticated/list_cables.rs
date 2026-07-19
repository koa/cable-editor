use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, query},
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct ListCablesQuery {
    pub list_cable: Vec<CableListEntry>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Cable")]
pub struct CableListEntry {
    pub id: i32,
    pub name: String,
    pub bundle_count: i32,
    pub fiber_count: i32,
    pub length: Option<f64>,
}

pub async fn fetch_cables_list(
    credentials: Option<&OAuth2Context>,
) -> Result<Box<[CableListEntry]>, FrontendError> {
    let response = query::<ListCablesQuery, _>((), credentials).await?;
    if let Some(errors) = response.errors {
        Err(FrontendError::Graphql(errors))
    } else {
        Ok(response
            .data
            .map(|l| l.list_cable)
            .unwrap_or_default()
            .into_boxed_slice())
    }
}
