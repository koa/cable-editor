use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{Point, schema},
        query,
    },
};
use yew::Component;
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct ListSchachtQuery {
    pub list_schacht: Vec<SchachtListEntry>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Schacht")]
pub struct SchachtListEntry {
    pub id: i32,
    pub name: String,
    pub position: Option<Point>,
}

async fn fetch_schacht_list<C: Component>(
    credentials: Option<&OAuth2Context>,
) -> Result<Box<[SchachtListEntry]>, FrontendError> {
    let schacht_list = query::<ListSchachtQuery, _>((), credentials).await?;
    Ok(schacht_list
        .data
        .map(|l| l.list_schacht)
        .unwrap_or_default()
        .into_boxed_slice())
}
