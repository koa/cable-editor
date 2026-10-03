use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{ParentChainPanel, PortType, schema},
        query,
    },
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables, Debug)]
pub struct FetchWorkOrderVariables {
    pub plan_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "FetchWorkOrderVariables")]
pub struct FetchWorkOrder {
    #[arguments(planId: $plan_id)]
    pub plan: Option<WorkOrderPlan>,
}

/// A plan with the ports it changes (`Plan.changedPorts`).
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Plan")]
pub struct WorkOrderPlan {
    pub name: String,
    pub is_baseline: bool,
    pub changed_ports: Vec<PortChange>,
}

impl WorkOrderPlan {
    pub async fn fetch(
        credentials: Option<&OAuth2Context>,
        plan_id: i32,
    ) -> Result<Option<WorkOrderPlan>, FrontendError> {
        let response =
            query::<FetchWorkOrder, _>(FetchWorkOrderVariables { plan_id }, credentials).await?;
        Ok(response.plan)
    }
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
pub struct PortChange {
    pub port: ChangedPort,
    pub current_front: Option<WorkOrderFiber>,
    pub current_back: Option<WorkOrderFiber>,
    pub planned_front: Option<WorkOrderFiber>,
    pub planned_back: Option<WorkOrderFiber>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "PanelPort")]
pub struct ChangedPort {
    pub id: i32,
    pub order_number: i32,
    pub label: Option<String>,
    pub port_type: PortType,
    pub panel: ChangedPortPanel,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Panel")]
pub struct ChangedPortPanel {
    pub id: i32,
    pub name: Option<String>,
    /// From the root panel down to the parent
    pub parent_chain: Vec<ParentChainPanel>,
    pub schacht: WorkOrderSchacht,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
pub struct WorkOrderSchacht {
    pub id: i32,
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Fiber")]
pub struct WorkOrderFiber {
    pub cable: WorkOrderCable,
    pub bundle: i32,
    pub fiber: i32,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Cable")]
pub struct WorkOrderCable {
    pub id: i32,
    pub name: String,
}
