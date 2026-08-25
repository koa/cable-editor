use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{PortSide, PortType, schema},
        mutate, query,
    },
};
use std::fmt::{Display, Formatter};
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
    #[arguments(side: "FRONT")]
    #[cynic(rename = "currentUsage")]
    pub current_front_usage: Option<PortUsageFragment>,
    #[arguments(side: "BACK")]
    #[cynic(rename = "currentUsage")]
    pub current_back_usage: Option<PortUsageFragment>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(variables = "FetchPanelUsageVariables")]
pub struct Panel {
    pub schacht: Schacht,
    pub name: Option<String>,
    pub parent_chain: Vec<EndPortParentPanel>,
}
impl Display for Panel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.schacht.name,)?;
        let mut device_written = false;
        for name in self
            .parent_chain
            .iter()
            .filter_map(|p| p.name.as_deref())
            .chain(self.name.as_deref())
        {
            if device_written {
                f.write_str(" ")?;
            } else {
                f.write_str(":")?;
                device_written = true;
            }
            f.write_str(name)?;
        }
        Ok(())
    }
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(variables = "FetchPanelUsageVariables")]
pub struct Schacht {
    pub cables: Vec<CableEnd>,
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(variables = "FetchPanelUsageVariables")]
pub struct CableEnd {
    pub cable: Cable,
    pub path: CablePath,
    pub fibers: Vec<FiberOwnEnd>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "FiberEnd", variables = "FetchPanelUsageVariables")]
pub struct FiberOwnEnd {
    pub bundle: i32,
    pub fiber: i32,
    pub other_end: Option<FiberOtherEnd>,
    #[arguments(planId: $plan_id)]
    pub used_port: Option<CableUsedOwnPort>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PortUsage", variables = "FetchPanelUsageVariables")]
pub struct CableUsedOwnPort {
    pub port: PortPanelId,
    pub modified_in_plan: bool,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "FiberEnd", variables = "FetchPanelUsageVariables")]
pub struct FiberOtherEnd {
    #[arguments(planId: $plan_id)]
    pub used_port: Option<CableUsedEndPort>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PortUsage", variables = "FetchPanelUsageVariables")]
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
        if let Some(name) = self.port.label.as_deref() {
            write!(f, ":{name}",)?;
        }
        Ok(())
    }
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PanelPort")]
pub struct EndPort {
    pub label: Option<String>,
    pub panel: EndPortPanel,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Panel")]
pub struct EndPortPanel {
    pub schacht: EndPortSchacht,
    pub name: Option<String>,
    pub parent_chain: Vec<EndPortParentPanel>,
}
impl Display for EndPortPanel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.schacht.name,)?;
        let mut device_written = false;
        for name in self
            .parent_chain
            .iter()
            .filter_map(|p| p.name.as_deref())
            .chain(self.name.as_deref())
        {
            if device_written {
                f.write_str(" ")?;
            } else {
                f.write_str(":")?;
                device_written = true;
            }
            f.write_str(name)?;
        }
        Ok(())
    }
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Panel")]
pub struct EndPortParentPanel {
    pub name: Option<String>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Schacht")]
pub struct EndPortSchacht {
    pub name: String,
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
    pub modified_in_plan: bool,
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
