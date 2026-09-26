use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, query},
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct ListDuctQuery {
    list_duct: Vec<DuctListEntry>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Duct")]
pub struct DuctListEntry {
    pub id: i32,
    pub description: Option<String>,
    pub schacht_a: DuctListSchacht,
    pub schacht_z: DuctListSchacht,
    /// Metres, missing without geometry
    pub length: Option<f64>,
    pub cables: Vec<DuctListCable>,
}

impl DuctListEntry {
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
pub struct DuctListSchacht {
    pub id: i32,
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Cable")]
pub struct DuctListCable {
    pub id: i32,
}

/// How a duct is named: its description, else the Schächte it connects.
pub fn duct_title(description: Option<&str>, schacht_a: &str, schacht_z: &str) -> String {
    match description {
        Some(description) => description.to_string(),
        None => format!("{schacht_a} – {schacht_z}"),
    }
}

pub async fn fetch_duct_list(
    credentials: Option<&OAuth2Context>,
) -> Result<Box<[DuctListEntry]>, FrontendError> {
    Ok(query::<ListDuctQuery, _>((), credentials)
        .await?
        .list_duct
        .into_boxed_slice())
}
