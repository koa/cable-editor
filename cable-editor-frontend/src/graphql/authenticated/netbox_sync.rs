use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, mutate},
};
use yew_oauth2::context::OAuth2Context;

#[derive(cynic::QueryVariables)]
pub struct SyncNetboxVariables {
    pub plan_id: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = SyncNetboxVariables)]
pub struct SyncNetbox {
    #[arguments(planId: $plan_id)]
    pub sync_plan_to_netbox: Vec<SyncIssue>,
}

#[derive(cynic::InlineFragments, Debug)]
pub enum SyncIssue {
    MissingNetboxReference(MissingNetboxReferenceError),
    BlindEnd(BlindEndError),
    AsymmetricDuplex(AsymmetricDuplexError),
    PortBlockedInNetbox(PortBlockedInNetboxError),
    NameCollision(NameCollisionError),
    MissingNetboxMasterData(MissingNetboxMasterDataError),
    RoutingLoop(RoutingLoopError),
    InvalidTargetReference(InvalidTargetReferenceError),
    #[cynic(fallback)]
    Unknown,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "MissingNetboxReferenceError")]
pub struct MissingNetboxReferenceError {
    pub port: PanelPortInfo,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "PanelPort")]
pub struct PanelPortInfo {
    order_number: i32,
    panel: PanelInfo,
    label: Option<String>,
}

impl PanelPortInfo {
    pub fn port_label(&self) -> String {
        let mut result = self.panel.schacht.name.clone();
        for parent in &self.panel.parent_chain {
            if let Some(name) = &parent.name {
                result.push_str(", ");
                result.push_str(name);
            }
        }
        if let Some(name) = &self.panel.name {
            result.push_str(", ");
            result.push_str(name);
        }
        if let Some(label) = &self.label {
            result.push_str(": ");
            result.push_str(label);
        } else {
            result.push_str(&format!(": Port {}", self.order_number));
        }
        result
    }
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Panel")]
pub struct PanelInfo {
    name: Option<String>,
    schacht: SchachtInfo,
    parent_chain: Vec<ParentChainPanelInfo>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Panel")]
struct ParentChainPanelInfo {
    name: Option<String>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Schacht")]
struct SchachtInfo {
    name: String,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "BlindEndError")]
pub struct BlindEndError {
    pub port: PanelPortInfo,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "AsymmetricDuplexError")]
pub struct AsymmetricDuplexError {
    pub start_netbox_port: RearPort,
    pub connections: Vec<AsymetricTargetConnectionEntry>,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "DeviceWithRearPorts")]
pub struct NetboxDevice {
    pub name: Option<String>,
    pub location_name: Option<String>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
pub struct RearPort {
    pub name: String,
    pub device: NetboxDevice,
}
impl RearPort {
    pub fn display_name(&self) -> String {
        let location = self
            .device
            .location_name
            .as_deref()
            .unwrap_or("<kein name>");
        let device = self.device.name.as_deref().unwrap_or("<kein name>");
        format!("{}, {}, {}", location, device, self.name)
    }
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
pub struct AsymetricTargetConnectionEntry {
    pub target_netbox_port: RearPort,
    pub pairs: Vec<PortPair>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
pub struct PortPair {
    pub source_port: PanelPortInfo,
    pub target_port: PanelPortInfo,
}
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "PortBlockedInNetboxError")]
pub struct PortBlockedInNetboxError {
    pub port: PanelPortInfo,
    pub netbox_port: RearPort,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "NameCollisionError")]
pub struct NameCollisionError {
    pub panel: PanelInfo,
    pub circuit_name: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "MissingNetboxMasterDataError")]
pub struct MissingNetboxMasterDataError {
    pub entity_type: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "RoutingLoopError")]
pub struct RoutingLoopError {
    pub port: PanelPortInfo,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "InvalidTargetReferenceError")]
pub struct InvalidTargetReferenceError {
    pub port: PanelPortInfo,
}

impl SyncNetbox {
    pub async fn sync_netbox(
        credentials: Option<&OAuth2Context>,
        plan_id: i32,
    ) -> Result<Vec<SyncIssue>, FrontendError> {
        Ok(
            mutate::<SyncNetbox, _>(SyncNetboxVariables { plan_id }, credentials)
                .await?
                .sync_plan_to_netbox,
        )
    }
}
