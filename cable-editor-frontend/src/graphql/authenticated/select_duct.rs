use crate::error::FrontendError;
use crate::graphql::authenticated::cable_details::CableSegmentEndSchacht;
use crate::graphql::authenticated::list_cables::CableListEntry;
use crate::graphql::authenticated::schema;
use crate::graphql::query;
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
    let response = query::<QueryDuctList, _>((), credentials).await?;
    if let Some(errors) = response.errors {
        Err(FrontendError::Graphql(errors))
    } else {
        Ok(response
            .data
            .map(|l| l.list_duct)
            .unwrap_or_default()
            .into_boxed_slice())
    }
}
