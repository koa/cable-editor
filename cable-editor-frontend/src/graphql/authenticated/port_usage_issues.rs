//! Port usages not fitting their cables (docs/datenpruefung.md): how many there are, the list
//! for admins, deleting them, and the names of the ones an error refers to by id.

use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{CableRef, PanelPortInfo, PortSide, schema},
        mutate, query,
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

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
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

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct BrokenCountQuery {
    broken_port_usage_count: i32,
}

/// How many port usages don't fit their cables.
pub async fn fetch_broken_count(credentials: Option<&OAuth2Context>) -> Result<i32, FrontendError> {
    Ok(query::<BrokenCountQuery, _>((), credentials)
        .await?
        .broken_port_usage_count)
}

#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortUsageProblem {
    CableNotEnding,
    FiberOutOfRange,
    FiberTwice,
}

impl From<PortUsageProblem> for cable_editor_common::error::PortUsageProblem {
    fn from(problem: PortUsageProblem) -> Self {
        match problem {
            PortUsageProblem::CableNotEnding => Self::CableNotEnding,
            PortUsageProblem::FiberOutOfRange => Self::FiberOutOfRange,
            PortUsageProblem::FiberTwice => Self::FiberTwice,
        }
    }
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct IssuesQuery {
    port_usage_issues: Vec<PortUsageIssue>,
}

/// A problem of a port usage, a usage can have several.
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
pub struct PortUsageIssue {
    pub plan: PlanName,
    pub port: PanelPortInfo,
    pub side: PortSide,
    pub cable: CableRef,
    pub bundle: i32,
    pub fiber: i32,
    pub problem: PortUsageProblem,
}

impl PortUsageIssue {
    pub fn key(&self) -> PortUsageKeyInput {
        PortUsageKeyInput {
            port_id: self.port.id,
            plan_id: self.plan.id,
            side: self.side,
        }
    }
}

/// All port usages not fitting their cables (admins).
pub async fn fetch_issues(
    credentials: Option<&OAuth2Context>,
) -> Result<Vec<PortUsageIssue>, FrontendError> {
    Ok(query::<IssuesQuery, _>((), credentials)
        .await?
        .port_usage_issues)
}

#[derive(cynic::InputObject, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortUsageKeyInput {
    pub port_id: i32,
    pub plan_id: i32,
    pub side: PortSide,
}

#[derive(cynic::QueryVariables, Debug)]
struct DeleteVariables {
    usages: Vec<PortUsageKeyInput>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "DeleteVariables")]
struct DeleteMutation {
    #[arguments(usages: $usages)]
    remove_broken_port_usages: i32,
}

/// Deletes the usages that still don't fit their cables, returns how many.
pub async fn delete_broken(
    credentials: Option<&OAuth2Context>,
    usages: Vec<PortUsageKeyInput>,
) -> Result<i32, FrontendError> {
    Ok(
        mutate::<DeleteMutation, _>(DeleteVariables { usages }, credentials)
            .await?
            .remove_broken_port_usages,
    )
}
