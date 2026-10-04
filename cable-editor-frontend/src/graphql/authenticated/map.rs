use crate::graphql::authenticated::{CableRef, SchachtRef};
use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{GeoPoint, schacht_types::SchachtTypIcon, schema},
        query,
    },
};
use yew_oauth2::context::OAuth2Context;

/// Everything the map shows.
#[derive(cynic::QueryFragment, Debug, PartialEq)]
#[cynic(graphql_type = "Query")]
pub struct MapData {
    #[cynic(rename = "listSchacht")]
    pub schaechte: Vec<MapSchacht>,
    #[cynic(rename = "listDuct")]
    pub ducts: Vec<MapDuct>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct MapSchacht {
    pub id: i32,
    pub name: String,
    /// Missing without geometry
    pub location: Option<GeoPoint>,
    /// Its icon on the map
    pub typ: Option<SchachtTypIcon>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Duct")]
pub struct MapDuct {
    pub id: i32,
    pub description: Option<String>,
    /// From Schacht A to Schacht Z, missing without geometry
    pub line: Option<Vec<GeoPoint>>,
    /// Metres, missing without geometry
    pub length: Option<f64>,
    pub schacht_a: SchachtRef,
    pub schacht_z: SchachtRef,
    pub cables: Vec<CableRef>,
}

pub async fn fetch_map_data(credentials: Option<&OAuth2Context>) -> Result<MapData, FrontendError> {
    query::<MapData, _>((), credentials).await
}
