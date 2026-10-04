use crate::graphql::authenticated::{CableId, SchachtRef};
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
    pub schacht_a: SchachtRef,
    pub schacht_z: SchachtRef,
    /// Metres, missing without geometry
    pub length: Option<f64>,
    pub cables: Vec<CableId>,
    pub owner: DuctListOwner,
    /// Delivered to the Leitungskataster
    pub leitungskataster: bool,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Owner")]
pub struct DuctListOwner {
    pub name: String,
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
