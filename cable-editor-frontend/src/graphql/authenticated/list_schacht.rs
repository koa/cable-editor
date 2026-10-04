use crate::graphql::authenticated::PanelRef;
use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, query},
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct ListSchachtQuery {
    pub list_schacht: Vec<SchachtListEntry>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Schacht")]
pub struct SchachtListEntry {
    pub id: i32,
    pub name: String,
    pub root_panels: Vec<PanelRef>,
}

pub async fn fetch_schacht_list(
    credentials: Option<&OAuth2Context>,
) -> Result<Box<[SchachtListEntry]>, FrontendError> {
    Ok(query::<ListSchachtQuery, _>((), credentials)
        .await?
        .list_schacht
        .into_boxed_slice())
}
