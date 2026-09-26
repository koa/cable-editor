use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, query},
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct MapQuery {
    list_schacht: Vec<MapSchacht>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct MapSchacht {
    pub id: i32,
    pub name: String,
    /// Missing without geometry
    pub location: Option<GeoPoint>,
}

/// WGS84
#[derive(cynic::QueryFragment, Debug, Clone, Copy, PartialEq)]
pub struct GeoPoint {
    pub lat: f64,
    pub lng: f64,
}

pub async fn fetch_map_schaechte(
    credentials: Option<&OAuth2Context>,
) -> Result<Vec<MapSchacht>, FrontendError> {
    Ok(query::<MapQuery, _>((), credentials).await?.list_schacht)
}
