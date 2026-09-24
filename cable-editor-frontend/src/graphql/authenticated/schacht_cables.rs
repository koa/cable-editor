use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{cable_details::CableSegmentEndSchacht, schema},
        query,
    },
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables)]
struct Variables {
    id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "Variables")]
struct FetchSchachtCablesQuery {
    #[arguments(schachtId: $id)]
    pub schacht: Option<SchachtCables>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct SchachtCables {
    pub id: i32,
    pub name: String,
    pub cables: Vec<SchachtCableEnd>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "CableEnd")]
pub struct SchachtCableEnd {
    pub cable: SchachtCable,
    pub path: SchachtCablePath,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Cable")]
pub struct SchachtCable {
    pub id: i32,
    pub name: String,
}

/// Oriented from the requested Schacht, so far_schacht is the other end.
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "CablePath")]
pub struct SchachtCablePath {
    pub far_schacht: CableSegmentEndSchacht,
}

impl SchachtCableEnd {
    /// Cable label text: "<cable>-<destination>".
    pub fn label_text(&self) -> String {
        format!("{} - {}", self.cable.name, self.path.far_schacht.name)
    }
}

impl SchachtCables {
    pub async fn fetch(
        credentials: Option<&OAuth2Context>,
        id: i32,
    ) -> Result<SchachtCables, FrontendError> {
        let response = query::<FetchSchachtCablesQuery, _>(Variables { id }, credentials).await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            response
                .data
                .and_then(|d| d.schacht)
                .ok_or(FrontendError::NotFound)
        }
    }
}
