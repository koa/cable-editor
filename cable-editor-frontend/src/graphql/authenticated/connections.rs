use crate::graphql::authenticated::{PortSide, PortType};
use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, query},
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "Cable")]
pub struct CableInfo {
    pub id: i32,
    pub name: String,
    pub bundle_count: i32,
    pub fiber_count: i32,
}
#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "CableEnd")]
pub struct CableEndInfo {
    pub cable: CableInfo,
    pub path: CablePathInfo,
}
#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "CablePath")]
pub struct CablePathInfo {
    pub far_schacht: FarSchachtInfo,
}
#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "Schacht")]
pub struct FarSchachtInfo {
    pub name: String,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "Variables")]
struct ListCablesByCabinetQuery {
    #[arguments(schachtId: $id)]
    pub schacht: Option<SchachtData>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "Variables")]
struct ListCablesByPanelQuery {
    #[arguments(panelId: $id)]
    pub panel: Option<PanelData>,
}
#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "Panel")]
struct PanelData {
    pub schacht: SchachtData,
}

#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "Schacht")]
struct SchachtData {
    pub cables: Vec<CableEndInfo>,
}

#[derive(cynic::QueryVariables)]
struct Variables {
    id: i32,
}

impl CableEndInfo {
    pub async fn list_by_cabinet(
        credentials: Option<&OAuth2Context>,
        id: i32,
    ) -> Result<Vec<CableEndInfo>, FrontendError> {
        let response = query::<ListCablesByCabinetQuery, _>(Variables { id }, credentials).await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            Ok(response
                .data
                .and_then(|l| l.schacht)
                .map(|cabinet| cabinet.cables)
                .unwrap_or_default())
        }
    }

    pub async fn list_candidate_by_panel(
        credentials: Option<&OAuth2Context>,
        id: i32,
    ) -> Result<Vec<CableEndInfo>, FrontendError> {
        let response = query::<ListCablesByPanelQuery, _>(Variables { id }, credentials).await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            Ok(response
                .data
                .and_then(|p| p.panel)
                .map(|l| l.schacht)
                .map(|cabinet| cabinet.cables)
                .unwrap_or_default())
        }
    }
}

#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "PortUsage")]
pub struct PortUsage {
    pub side: PortSide,
    pub port: PortUsagePort,
    pub fiber: Option<PortUsageFiber>,
    pub other_side: Option<OtherSidePortUsage>,
}
#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "PortUsage")]
pub struct OtherSidePortUsage {
    pub fiber: Option<PortUsageFiber>,
}
#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "Fiber")]
pub struct PortUsageFiber {
    pub cable: CableInfo,
    pub bundle: i32,
    pub fiber: i32,
}
#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "Cable")]
pub struct PortUsageCable {
    pub id: i32,
}
#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "PanelPort")]
pub struct PortUsagePort {
    pub id: i32,
    pub port_type: PortType,
    pub panel: PanelWithId,
}
#[derive(cynic::QueryFragment, Clone, PartialEq, Debug, Hash, Eq)]
#[cynic(graphql_type = "Panel")]
pub struct PanelWithId {
    pub id: i32,
}
#[derive(cynic::QueryVariables)]
struct PortUsageOfCableVariables {
    plan_id: i32,
    panel_id: i32,
    cable_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "PortUsageOfCableVariables")]
struct PortUsageOfCableQuery {
    #[arguments(panelId: $panel_id)]
    panel: Option<PortUsagePanelQuery>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Panel", variables = "PortUsageOfCableVariables")]
struct PortUsagePanelQuery {
    schacht: PortUsageSchachtQuery,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Schacht", variables = "PortUsageOfCableVariables")]
struct PortUsageSchachtQuery {
    #[arguments(cableId: $cable_id)]
    cable: Option<CableEndQuery>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "CableEnd", variables = "PortUsageOfCableVariables")]
struct CableEndQuery {
    #[arguments(planId: $plan_id)]
    used_ports: Vec<PortUsage>,
}

impl PortUsage {
    pub async fn list_usage_of_cable(
        credentials: Option<&OAuth2Context>,
        plan_id: i32,
        panel_id: i32,
        cable_id: i32,
    ) -> Result<Vec<PortUsage>, FrontendError> {
        let response = query::<PortUsageOfCableQuery, _>(
            PortUsageOfCableVariables {
                plan_id,
                cable_id,
                panel_id,
            },
            credentials,
        )
        .await?;
        Ok(response
            .data
            .and_then(|r| r.panel)
            .and_then(|p| p.schacht.cable)
            .map(|c| {
                c.used_ports
                    .into_iter()
                    .filter(|p| p.port.panel.id == panel_id)
                    .collect()
            })
            .unwrap_or_default())
    }
}
