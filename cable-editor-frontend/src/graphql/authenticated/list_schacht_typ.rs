use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, query},
};
use patternfly_yew::prelude::SelectItemRenderer;
use yew::{Component, html::Scope};

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct ListSchachtTypQuery {
    pub list_schacht_typ: Vec<SchachtTypListEntry>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "SchachtTyp")]
pub struct SchachtTypListEntry {
    pub id: i32,
    pub name: String,
    pub icon: String,
}

impl SelectItemRenderer for SchachtTypListEntry {
    type Item = i32;

    fn label(&self) -> String {
        self.name.clone()
    }
}
pub async fn fetch_schacht_type_list<C: Component>(
    scope: Scope<C>,
) -> Result<Box<[SchachtTypListEntry]>, FrontendError> {
    let schacht_list = query::<ListSchachtTypQuery, _>((), scope).await?;
    Ok(schacht_list
        .data
        .map(|l| l.list_schacht_typ)
        .unwrap_or_default()
        .into_boxed_slice())
}
