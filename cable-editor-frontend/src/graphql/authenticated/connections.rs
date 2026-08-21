use crate::graphql::authenticated::{PortSide, PortType};
use crate::graphql::mutate;
use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, query},
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables, Debug)]
pub struct FetchPanelUsageVariables {
    pub plan_id: i32,
    pub panel_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "FetchPanelUsageVariables")]
pub struct FetchPanelUsage {
    #[arguments(planId: $plan_id)]
    pub plan: Option<Plan>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "FetchPanelUsageVariables")]
pub struct Plan {
    pub id: i32,
    #[arguments(panelId: $panel_id)]
    pub panel: Option<PlannedPanel>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(variables = "FetchPanelUsageVariables")]
pub struct PlannedPanel {
    pub panel: Panel,
    pub ports: Vec<PlannedPort>,
}
impl PlannedPanel {
    pub async fn fetch_situation(
        credentials: Option<&OAuth2Context>,
        plan_id: i32,
        panel_id: i32,
    ) -> Result<Option<PlannedPanel>, FrontendError> {
        let response = query::<FetchPanelUsage, _>(
            FetchPanelUsageVariables { plan_id, panel_id },
            credentials,
        )
        .await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            Ok(response.data.and_then(|p| p.plan).and_then(|p| p.panel))
        }
    }
}

#[derive(cynic::QueryFragment, Debug, Clone)]
pub struct PlannedPort {
    pub id: i32,
    pub label: Option<String>,
    pub order_number: i32,
    pub port_type: PortType,
    #[arguments(side: "FRONT")]
    #[cynic(rename = "usage")]
    pub front_usage: Option<PortUsageFragment>,
    #[arguments(side: "BACK")]
    #[cynic(rename = "usage")]
    pub back_usage: Option<PortUsageFragment>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(variables = "FetchPanelUsageVariables")]
pub struct Panel {
    pub schacht: Schacht,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(variables = "FetchPanelUsageVariables")]
pub struct Schacht {
    pub cables: Vec<CableEnd>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(variables = "FetchPanelUsageVariables")]
pub struct CableEnd {
    pub cable: Cable,
    pub path: CablePath,
    #[arguments(planId: $plan_id)]
    pub used_ports: Vec<CableUsedPort>,
}
#[derive(cynic::QueryFragment, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PortUsage")]
pub struct CableUsedPort {
    pub port: PortPanelId,
    pub fiber: Option<Fiber>,
}

#[derive(cynic::QueryFragment, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PanelPort")]
pub struct PortPanelId {
    pub panel: PanelId,
}
#[derive(cynic::QueryFragment, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Panel")]
pub struct PanelId {
    pub id: i32,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CablePath {
    pub far_schacht: RemoteSchacht,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Schacht")]
pub struct RemoteSchacht {
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cable {
    pub id: i32,
    pub name: String,
    pub bundle_count: i32,
    pub fiber_count: i32,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "PortUsage")]
pub struct PortUsageFragment {
    pub fiber: Option<Fiber>,
}

#[derive(cynic::QueryFragment, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fiber {
    pub bundle: i32,
    pub fiber: i32,
    pub cable: CableId,
}

#[derive(cynic::QueryFragment, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Cable")]
pub struct CableId {
    pub id: i32,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct UpdatePortUsage {
    pub plan_id: i32,
    pub usages: Vec<PortUsageInput>,
}

impl UpdatePortUsage {
    pub async fn store(self, credentials: Option<&OAuth2Context>) -> Result<(), FrontendError> {
        let response = mutate::<UpdatePortUsageQuery, _>(self, credentials).await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            Ok(())
        }
    }
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "UpdatePortUsage")]
struct UpdatePortUsageQuery {
    #[arguments(changes: $usages, planId: $plan_id)]
    #[allow(unused)]
    set_port_usage: bool,
}

#[derive(cynic::InputObject, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PortUsageInput {
    pub port_id: i32,
    pub side: PortSide,
    pub fiber: Option<FiberKeyInput>,
}

#[derive(cynic::InputObject, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct FiberKeyInput {
    pub cable_id: i32,
    pub bundle: i32,
    pub fiber: i32,
}
