use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{GeoPoint, list_ducts::duct_title, schema},
        query,
    },
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables)]
struct DuctDetailsVariables {
    duct_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "DuctDetailsVariables")]
struct DuctDetailsQuery {
    #[arguments(ductId: $duct_id)]
    duct: Option<DuctDetails>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Duct")]
pub struct DuctDetails {
    pub id: i32,
    pub description: Option<String>,
    /// Metres, missing without geometry
    pub length: Option<f64>,
    /// From Schacht A to Schacht Z, missing without geometry
    pub line: Option<Vec<GeoPoint>>,
    pub schacht_a: DuctEnd,
    pub schacht_z: DuctEnd,
    pub cables: Vec<DuctCable>,
}

impl DuctDetails {
    /// The description, else the Schächte it connects.
    pub fn title(&self) -> String {
        duct_title(
            self.description.as_deref(),
            &self.schacht_a.name,
            &self.schacht_z.name,
        )
    }
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct DuctEnd {
    pub id: i32,
    pub name: String,
    pub location: Option<GeoPoint>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Cable")]
pub struct DuctCable {
    pub id: i32,
    pub name: String,
    pub bundle_count: i32,
    pub fiber_count: i32,
}

pub async fn fetch_duct_details(
    credentials: Option<&OAuth2Context>,
    duct_id: i32,
) -> Result<Option<DuctDetails>, FrontendError> {
    Ok(
        query::<DuctDetailsQuery, _>(DuctDetailsVariables { duct_id }, credentials)
            .await?
            .duct,
    )
}
