use crate::error::FrontendError;
use crate::graphql::authenticated::schema;
use crate::graphql::{mutate, query};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables)]
struct Variables {
    id: i32,
}

#[derive(cynic::InputObject, Clone)]
#[cynic(graphql_type = "UpdateCableStructure")]
pub struct UpdateCableStructure {
    pub bundle_count: i32,
    pub fiber_count: i32,
}

#[derive(cynic::QueryVariables)]
struct UpdateCableMutationVariables {
    cable_id: i32,
    name: Option<String>,
    fibers: Option<UpdateCableStructure>,
    path: Option<Vec<i32>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "UpdateCableMutationVariables")]
struct UpdateCableMutation {
    #[arguments(cableId: $cable_id, name: $name, fibers: $fibers, path: $path)]
    pub update_cable: Option<CableDetails>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "Variables")]
struct FetchCableDetailsQuery {
    #[arguments(cableId: $id)]
    pub cable: Option<CableDetails>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "Variables")]
struct FetchAvailableDuctFromSchacht {
    #[arguments(schachtId: $id)]
    pub schacht: Option<AvailableDuctsOnSchacht>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Duct")]
pub struct CableDuct {
    pub id: i32,
    pub description: Option<String>,
    pub length: Option<f64>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "CablePath")]
pub struct CablePath {
    pub near_schacht: CableSegmentEndSchacht,
    pub segments: Vec<CablePathSegment>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "CablePathSegment")]
pub struct CablePathSegment {
    pub duct: CableDuct,
    pub far_schacht: CableSegmentEndSchacht,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct CableSegmentEndSchacht {
    pub id: i32,
    pub name: String,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct AvailableDuctsOnSchacht {
    pub connecting_duct: Vec<PotentialDuct>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "PotentialPathSegment")]
pub struct PotentialDuct {
    pub duct: CableDuct,
    pub schacht: CableSegmentEndSchacht,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Cable")]
pub struct CableDetails {
    pub id: i32,
    pub name: String,
    pub bundle_count: i32,
    pub fiber_count: i32,
    pub length: Option<f64>,
    pub path: Option<CablePath>,
}

impl CableDetails {
    pub async fn fetch(
        credentials: Option<&OAuth2Context>,
        id: i32,
    ) -> Result<Option<CableDetails>, FrontendError> {
        let response = query::<FetchCableDetailsQuery, _>(Variables { id }, credentials).await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            Ok(response.data.and_then(|l| l.cable))
        }
    }

    pub async fn update_cable(
        credentials: Option<&OAuth2Context>,
        cable_id: i32,
        name: Option<String>,
        fibers: Option<UpdateCableStructure>,
        path: Option<Vec<i32>>,
    ) -> Result<Option<CableDetails>, FrontendError> {
        let response = mutate::<UpdateCableMutation, _>(
            UpdateCableMutationVariables {
                cable_id,
                name,
                fibers,
                path,
            },
            credentials,
        )
        .await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            Ok(response.data.and_then(|m| m.update_cable))
        }
    }
}

impl CablePath {
    pub fn duct_sequence(&self) -> impl Iterator<Item = i32> {
        self.segments.iter().map(|s| s.duct.id)
    }
}

impl CableSegmentEndSchacht {
    pub async fn fetch_connected_ducts(
        &self,
        credentials: Option<&OAuth2Context>,
    ) -> Result<Vec<PotentialDuct>, FrontendError> {
        let response =
            query::<FetchAvailableDuctFromSchacht, _>(Variables { id: self.id }, credentials)
                .await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            Ok(response
                .data
                .and_then(|l| l.schacht)
                .map(|s| s.connecting_duct)
                .unwrap_or_default())
        }
    }
}
