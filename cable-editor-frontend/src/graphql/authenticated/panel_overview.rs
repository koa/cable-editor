//! The connection overview of a panel and its child panels, from the fragments of the panel
//! editors (`connections.rs`)
use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{
            PanelRef,
            connections::{Panel, PanelVariables, PanelVariablesFields, PlannedPort},
            schema, write_panel_path,
        },
        query,
    },
};
use std::fmt::{Display, Formatter};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "PanelVariables")]
pub struct FetchPanelOverview {
    #[arguments(planId: $plan_id)]
    pub plan: Option<PlanOverview>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Plan", variables = "PanelVariables")]
pub struct PlanOverview {
    pub id: i32,
    pub name: String,
    #[arguments(panelId: $panel_id)]
    pub panel: Option<PlannedPanelOverview>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "PlannedPanel", variables = "PanelVariables")]
pub struct PlannedPanelOverview {
    pub panel: Panel,
    pub ports: Vec<PlannedPort>,
    pub all_children_recursive: Vec<PlannedChildPanelOverview>,
}

impl PlannedPanelOverview {
    pub async fn fetch(
        credentials: Option<&OAuth2Context>,
        plan_id: i32,
        panel_id: i32,
    ) -> Result<Option<(PlanOverview, PlannedPanelOverview)>, FrontendError> {
        let response =
            query::<FetchPanelOverview, _>(PanelVariables { plan_id, panel_id }, credentials)
                .await?;
        Ok(response.plan.and_then(|plan| {
            let panel = plan.panel.clone()?;
            Some((plan, panel))
        }))
    }
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "PlannedPanel", variables = "PanelVariables")]
pub struct PlannedChildPanelOverview {
    pub panel: ChildPanelDetail,
    pub ports: Vec<PlannedPort>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Panel")]
pub struct ChildPanelDetail {
    pub id: i32,
    pub name: Option<String>,
    pub parent_id: Option<i32>,
    pub parent_order: Option<i32>,
    pub parent_chain: Vec<PanelRef>,
}

impl Display for ChildPanelDetail {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut names = self
            .parent_chain
            .iter()
            .filter_map(|p| p.name.as_deref())
            .chain(self.name.as_deref())
            .peekable();
        if names.peek().is_none() {
            write!(f, "Panel {}", self.id)
        } else {
            write_panel_path(f, None, names)
        }
    }
}
