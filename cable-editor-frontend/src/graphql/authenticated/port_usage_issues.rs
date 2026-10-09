//! What names the port usages not fitting their cables, which the backend refers to by id.

use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{CableRef, PanelPortInfo, schema},
        query,
    },
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables, Debug)]
pub struct PortsVariables {
    port_ids: Vec<i32>,
}

/// The ports asked for, and all cables and plans (only their names, fewer than ports).
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "PortsVariables")]
pub struct PortUsageNames {
    #[arguments(portIds: $port_ids)]
    pub ports: Vec<PanelPortInfo>,
    pub list_cable: Vec<CableRef>,
    pub list_plan: Vec<PlanName>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Plan")]
pub struct PlanName {
    pub id: i32,
    pub name: String,
}

impl PortUsageNames {
    pub async fn fetch(
        credentials: Option<&OAuth2Context>,
        port_ids: Vec<i32>,
    ) -> Result<Self, FrontendError> {
        query::<Self, _>(PortsVariables { port_ids }, credentials).await
    }
}
