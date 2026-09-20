use crate::{
    error::FrontendError,
    graphql::{
        authenticated::{ParentChainPanel, PortSide, PortType, schema},
        query,
    },
};
use std::fmt::{Display, Formatter};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables, Debug)]
pub struct FetchPanelOverviewVariables {
    pub plan_id: i32,
    pub panel_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "FetchPanelOverviewVariables")]
pub struct FetchPanelOverview {
    #[arguments(planId: $plan_id)]
    pub plan: Option<PlanOverview>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Plan", variables = "FetchPanelOverviewVariables")]
pub struct PlanOverview {
    pub id: i32,
    pub name: String,
    #[arguments(panelId: $panel_id)]
    pub panel: Option<PlannedPanelOverview>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "PlannedPanel", variables = "FetchPanelOverviewVariables")]
pub struct PlannedPanelOverview {
    pub panel: PanelOverviewDetail,
    pub ports: Vec<PlannedPortOverview>,
    pub all_children_recursive: Vec<PlannedChildPanelOverview>,
}

impl PlannedPanelOverview {
    pub async fn fetch(
        credentials: Option<&OAuth2Context>,
        plan_id: i32,
        panel_id: i32,
    ) -> Result<Option<(PlanOverview, PlannedPanelOverview)>, FrontendError> {
        let response = query::<FetchPanelOverview, _>(
            FetchPanelOverviewVariables { plan_id, panel_id },
            credentials,
        )
        .await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else if let Some(plan) = response.data.and_then(|d| d.plan) {
            let panel_opt = plan.panel.clone();
            Ok(panel_opt.map(|panel| (plan, panel)))
        } else {
            Ok(None)
        }
    }
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "PlannedPanel", variables = "FetchPanelOverviewVariables")]
pub struct PlannedChildPanelOverview {
    pub panel: ChildPanelDetail,
    pub ports: Vec<PlannedPortOverview>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Panel", variables = "FetchPanelOverviewVariables")]
pub struct PanelOverviewDetail {
    pub id: i32,
    pub name: Option<String>,
    pub schacht: SchachtOverview,
    pub parent_chain: Vec<ParentChainPanel>,
}

impl Display for PanelOverviewDetail {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.schacht.name)?;
        let mut device_written = false;
        for name in self
            .parent_chain
            .iter()
            .filter_map(|p| p.name.as_deref())
            .chain(self.name.as_deref())
        {
            if device_written {
                f.write_str(" > ")?;
            } else {
                f.write_str(": ")?;
                device_written = true;
            }
            f.write_str(name)?;
        }
        Ok(())
    }
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Panel")]
pub struct ChildPanelDetail {
    pub id: i32,
    pub name: Option<String>,
    pub parent_id: Option<i32>,
    pub parent_order: Option<i32>,
    pub parent_chain: Vec<ParentChainPanel>,
}

impl Display for ChildPanelDetail {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for name in self
            .parent_chain
            .iter()
            .filter_map(|p| p.name.as_deref())
            .chain(self.name.as_deref())
        {
            if !first {
                f.write_str(" > ")?;
            } else {
                first = false;
            }
            f.write_str(name)?;
        }
        if first {
            write!(f, "Panel {}", self.id)?;
        }
        Ok(())
    }
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "Schacht", variables = "FetchPanelOverviewVariables")]
pub struct SchachtOverview {
    pub id: i32,
    pub name: String,
    pub cables: Vec<CableEndOverview>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "CableEnd", variables = "FetchPanelOverviewVariables")]
pub struct CableEndOverview {
    pub cable: CableOverviewInfo,
    pub path: CablePathOverview,
    pub fibers: Vec<FiberOwnEndOverview>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Cable")]
pub struct CableOverviewInfo {
    pub id: i32,
    pub name: String,
    pub bundle_count: i32,
    pub fiber_count: i32,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "CablePath")]
pub struct CablePathOverview {
    pub far_schacht: RemoteSchachtOverview,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Schacht")]
pub struct RemoteSchachtOverview {
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "FiberEnd", variables = "FetchPanelOverviewVariables")]
pub struct FiberOwnEndOverview {
    pub bundle: i32,
    pub fiber: i32,
    pub other_end: Option<FiberOtherEndOverview>,
    #[arguments(planId: $plan_id)]
    pub used_port: Option<CableUsedOwnPortOverview>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PortUsage", variables = "FetchPanelOverviewVariables")]
pub struct CableUsedOwnPortOverview {
    pub port: PortPanelIdOverview,
    pub modified_in_plan: bool,
}

#[derive(cynic::QueryFragment, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PanelPort")]
pub struct PortPanelIdOverview {
    pub panel: PanelIdOverview,
}

#[derive(cynic::QueryFragment, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Panel")]
pub struct PanelIdOverview {
    pub id: i32,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "FiberEnd", variables = "FetchPanelOverviewVariables")]
pub struct FiberOtherEndOverview {
    #[arguments(planId: $plan_id)]
    pub used_port: Option<CableUsedEndPortOverview>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PortUsage", variables = "FetchPanelOverviewVariables")]
pub struct CableUsedEndPortOverview {
    #[arguments(planId: $plan_id)]
    pub panel_side_end_port: Option<UsedEndPortOverview>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PortUsage")]
pub struct UsedEndPortOverview {
    pub port: EndPortOverview,
}

impl Display for UsedEndPortOverview {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.port.panel)?;
        if let Some(name) = self.port.label.as_deref() {
            write!(f, " : {name}")?;
        }
        Ok(())
    }
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "PanelPort")]
pub struct EndPortOverview {
    pub label: Option<String>,
    pub panel: EndPortPanelOverview,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Panel")]
pub struct EndPortPanelOverview {
    pub id: i32,
    pub name: Option<String>,
    pub schacht: EndPortSchachtOverview,
    pub parent_chain: Vec<ParentChainPanel>,
}

impl Display for EndPortPanelOverview {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.schacht.name)?;
        let mut device_written = false;
        for name in self
            .parent_chain
            .iter()
            .filter_map(|p| p.name.as_deref())
            .chain(self.name.as_deref())
        {
            if device_written {
                f.write_str(" > ")?;
            } else {
                f.write_str(": ")?;
                device_written = true;
            }
            f.write_str(name)?;
        }
        Ok(())
    }
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Schacht")]
pub struct EndPortSchachtOverview {
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "PlannedPort")]
pub struct PlannedPortOverview {
    pub id: i32,
    pub label: Option<String>,
    pub order_number: i32,
    pub port_type: PortType,
    #[arguments(side: "FRONT")]
    #[cynic(rename = "usage")]
    pub front_usage: Option<PortUsageOverview>,
    #[arguments(side: "BACK")]
    #[cynic(rename = "usage")]
    pub back_usage: Option<PortUsageOverview>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(graphql_type = "PortUsage")]
pub struct PortUsageOverview {
    pub fiber: Option<FiberOverview>,
    pub modified_in_plan: bool,
}

#[derive(cynic::QueryFragment, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Fiber")]
pub struct FiberOverview {
    pub bundle: i32,
    pub fiber: i32,
    pub cable: CableIdOverview,
}

#[derive(cynic::QueryFragment, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Cable")]
pub struct CableIdOverview {
    pub id: i32,
}
