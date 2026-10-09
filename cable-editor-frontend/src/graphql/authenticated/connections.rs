use crate::graphql::authenticated::{CableId, CableSize, PanelId, SchachtRef};
use crate::graphql::authenticated::{port_label, write_panel_path, write_port_label};
use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{PanelRef, PortSide, PortType, schema},
        mutate, query,
    },
};
use std::fmt::{Display, Formatter};
use yew_oauth2::context::OAuth2Context;

/// A panel in a plan, the variables of the panel pages' queries
#[derive(cynic::QueryVariables, Debug)]
pub struct PanelVariables {
    pub plan_id: i32,
    pub panel_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "PanelVariables")]
pub struct FetchPanelUsage {
    #[arguments(planId: $plan_id)]
    pub plan: Option<Plan>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "PanelVariables")]
pub struct Plan {
    pub id: i32,
    #[arguments(panelId: $panel_id)]
    pub panel: Option<PlannedPanel>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(variables = "PanelVariables")]
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
        Ok(
            query::<FetchPanelUsage, _>(PanelVariables { plan_id, panel_id }, credentials)
                .await?
                .plan
                .and_then(|p| p.panel),
        )
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
    #[arguments(side: "FRONT")]
    #[cynic(rename = "currentUsage")]
    pub current_front_usage: Option<PortUsageFragment>,
    #[arguments(side: "BACK")]
    #[cynic(rename = "currentUsage")]
    pub current_back_usage: Option<PortUsageFragment>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(variables = "PanelVariables")]
pub struct Panel {
    pub id: i32,
    pub schacht: Schacht,
    pub name: Option<String>,
    pub parent_chain: Vec<PanelRef>,
}
impl Display for Panel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write_panel_path(
            f,
            Some(&self.schacht.name),
            self.parent_chain
                .iter()
                .filter_map(|p| p.name.as_deref())
                .chain(self.name.as_deref()),
        )
    }
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(variables = "PanelVariables")]
pub struct Schacht {
    pub id: i32,
    pub name: String,
    pub cables: Vec<CableEnd>,
    /// Cables at ports here that don't end here (docs/datenpruefung.md), offered to take their
    /// fibers off
    #[arguments(planId: $plan_id)]
    pub stray_cables: Vec<CableEnd>,
}
impl Schacht {
    /// The cables ending here, then those at its ports that don't
    pub fn all_cables(&self) -> impl Iterator<Item = &CableEnd> {
        self.cables.iter().chain(&self.stray_cables)
    }
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(variables = "PanelVariables")]
pub struct CableEnd {
    pub cable: CableSize,
    /// Missing if the cable doesn't end in the Schacht
    pub path: Option<CablePath>,
    pub fibers: Vec<FiberOwnEnd>,
}
impl CableEnd {
    /// The Schacht at its other end, or that it doesn't end here
    pub fn destination(&self) -> &str {
        self.path
            .as_ref()
            .map_or(NOT_ENDING, |path| path.far_schacht.name.as_str())
    }
}
/// Where a cable from `Schacht.strayCables` goes
pub const NOT_ENDING: &str = "⚠ endet nicht in diesem Schacht";

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "FiberEnd", variables = "PanelVariables")]
pub struct FiberOwnEnd {
    pub bundle: i32,
    pub fiber: i32,
    pub other_end: Option<FiberOtherEnd>,
    #[arguments(planId: $plan_id)]
    pub used_port: Option<CableUsedOwnPort>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PortUsage", variables = "PanelVariables")]
pub struct CableUsedOwnPort {
    pub port: PortPanelId,
    pub modified_in_plan: bool,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "FiberEnd", variables = "PanelVariables")]
pub struct FiberOtherEnd {
    #[arguments(planId: $plan_id)]
    pub used_port: Option<CableUsedEndPort>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PortUsage", variables = "PanelVariables")]
pub struct CableUsedEndPort {
    #[arguments(planId: $plan_id)]
    pub panel_side_end_port: Option<UsedEndPort>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PortUsage")]
pub struct UsedEndPort {
    pub port: EndPort,
}

impl Display for UsedEndPort {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.port.panel)?;
        write_port_label(
            f,
            &port_label(self.port.label.as_deref(), self.port.order_number),
        )
    }
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PanelPort")]
pub struct EndPort {
    pub order_number: i32,
    pub label: Option<String>,
    pub panel: EndPortPanel,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Panel")]
pub struct EndPortPanel {
    pub id: i32,
    pub schacht: SchachtRef,
    pub name: Option<String>,
    pub parent_chain: Vec<PanelRef>,
}
impl Display for EndPortPanel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write_panel_path(
            f,
            Some(&self.schacht.name),
            self.parent_chain
                .iter()
                .filter_map(|p| p.name.as_deref())
                .chain(self.name.as_deref()),
        )
    }
}
#[derive(cynic::QueryFragment, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PanelPort")]
pub struct PortPanelId {
    pub panel: PanelId,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CablePath {
    pub far_schacht: SchachtRef,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "PortUsage")]
pub struct PortUsageFragment {
    pub fiber: Option<Fiber>,
    pub modified_in_plan: bool,
}

#[derive(cynic::QueryFragment, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fiber {
    pub bundle: i32,
    pub fiber: i32,
    pub cable: CableId,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct UpdatePortUsage {
    pub plan_id: i32,
    pub usages: Vec<PortUsageInput>,
}

impl UpdatePortUsage {
    pub async fn store(self, credentials: Option<&OAuth2Context>) -> Result<(), FrontendError> {
        mutate::<UpdatePortUsageQuery, _>(self, credentials).await?;
        Ok(())
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
    pub fiber: PortUsageUpdateAction,
}

#[derive(cynic::InputObject, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PortUsageUpdateAction {
    Remove(bool),
    Reset(bool),
    Attach(FiberKeyInput),
}

#[derive(cynic::InputObject, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct FiberKeyInput {
    pub cable_id: i32,
    pub bundle: i32,
    pub fiber: i32,
}
