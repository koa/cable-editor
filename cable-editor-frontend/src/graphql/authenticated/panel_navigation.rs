use crate::error::FrontendError;
use crate::graphql::authenticated::{ParentChainPanel, schema};
use crate::graphql::query;
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables, Debug)]
struct FetchPanelVariables {
    panel_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "FetchPanelVariables")]
struct FetchPanelQuery {
    #[arguments(panelId: $panel_id)]
    panel: Option<PanelHierarchy>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Panel")]
pub struct PanelHierarchy {
    pub id: i32,
    pub name: Option<String>,
    pub parent_chain: Vec<ParentChainPanel>,
    pub children: Vec<ChildPanelNav>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Panel")]
pub struct ChildPanelNav {
    pub id: i32,
    pub name: Option<String>,
}

impl PanelHierarchy {
    pub async fn fetch(
        credentials: Option<&OAuth2Context>,
        panel_id: i32,
    ) -> Result<PanelHierarchy, FrontendError> {
        query::<FetchPanelQuery, _>(FetchPanelVariables { panel_id }, credentials)
            .await?
            .data
            .and_then(|d| d.panel)
            .ok_or(FrontendError::NotFound)
    }
}
