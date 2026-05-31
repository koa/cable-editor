use crate::error::FrontendError;
use crate::graphql::authenticated::{Point, schema};
use crate::graphql::query;
use yew::Component;
use yew::html::Scope;

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
    pub position: Point,
}

async fn fetch_schacht_list<C: Component>(
    scope: Scope<C>,
) -> Result<Box<[SchachtListEntry]>, FrontendError> {
    let schacht_list = query::<ListSchachtQuery, _>((), scope).await?;
    Ok(schacht_list
        .data
        .map(|l| l.list_schacht)
        .unwrap_or_default()
        .into_boxed_slice())
}
