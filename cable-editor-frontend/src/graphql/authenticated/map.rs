use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, query},
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
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Duct")]
pub struct MapDuct {
    pub id: i32,
    pub description: Option<String>,
    /// From Schacht A to Schacht Z, missing without geometry
    pub line: Option<Vec<GeoPoint>>,
    pub schacht_a: MapDuctEnd,
    pub schacht_z: MapDuctEnd,
    pub cables: Vec<MapCable>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct MapDuctEnd {
    pub id: i32,
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Cable")]
pub struct MapCable {
    pub id: i32,
    pub name: String,
}

/// WGS84
#[derive(cynic::QueryFragment, Debug, Clone, Copy, PartialEq)]
pub struct GeoPoint {
    pub lat: f64,
    pub lng: f64,
}

pub async fn fetch_map_data(credentials: Option<&OAuth2Context>) -> Result<MapData, FrontendError> {
    query::<MapData, _>((), credentials).await
}
