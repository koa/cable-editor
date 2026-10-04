use crate::graphql::authenticated::SchachtRef;
use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, mutate, query},
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
    pub path: Option<CablePathDescription>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "CablePath")]
pub struct CablePathDescription {
    pub near_schacht: SchachtRef,
    pub far_schacht: SchachtRef,
}

pub async fn fetch_cables_list(
    credentials: Option<&OAuth2Context>,
) -> Result<Box<[CableListEntry]>, FrontendError> {
    Ok(query::<ListCablesQuery, _>((), credentials)
        .await?
        .list_cable
        .into_boxed_slice())
}
pub async fn delete_cable(
    credentials: Option<&OAuth2Context>,
    cable_id: i32,
) -> Result<(), FrontendError> {
    mutate::<DeleteCableMutation, _>(DeleteCableMutationVariables { cable_id }, credentials)
        .await?;
    Ok(())
}

#[derive(cynic::QueryVariables)]
struct DeleteCableMutationVariables {
    cable_id: i32,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "DeleteCableMutationVariables")]
struct DeleteCableMutation {
    #[arguments( cableId: $cable_id)]
    #[allow(unused)]
    delete_cable: bool,
}
