use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, query},
};
use patternfly_yew::prelude::SelectItemRenderer;
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct ListSchachtTypQuery {
    pub list_schacht_typ: Vec<SchachtTypListEntry>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "SchachtTyp")]
pub struct SchachtTypListEntry {
    pub id: i32,
    pub name: Option<String>,
    pub icon: String,
}

impl SelectItemRenderer for SchachtTypListEntry {
    type Item = i32;

    fn label(&self) -> String {
        self.name
            .as_ref()
            .map(|n| n.clone())
            .unwrap_or_else(|| format!("schacht {}", self.id))
    }
}
pub async fn fetch_schacht_type_list(
    credentials: Option<&OAuth2Context>,
) -> Result<Box<[SchachtTypListEntry]>, FrontendError> {
    let schacht_list = query::<ListSchachtTypQuery>((), credentials).await?;
    Ok(schacht_list
        .data
        .map(|l| l.list_schacht_typ)
        .unwrap_or_default()
        .into_boxed_slice())
}
