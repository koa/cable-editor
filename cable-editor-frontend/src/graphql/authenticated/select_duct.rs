use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{cable_details::CableSegmentEndSchacht, schema},
        query,
    },
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct QueryDuctList {
    list_duct: Vec<DuctListEntry>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Duct")]
pub struct DuctListEntry {
    pub id: i32,
    pub description: Option<String>,
    pub schacht_a: CableSegmentEndSchacht,
    pub schacht_z: CableSegmentEndSchacht,
    pub length: Option<f64>,
}
pub async fn list_all_ducts(
    credentials: Option<&OAuth2Context>,
) -> Result<Box<[DuctListEntry]>, FrontendError> {
    Ok(query::<QueryDuctList, _>((), credentials)
        .await?
        .list_duct
        .into_boxed_slice())
}
