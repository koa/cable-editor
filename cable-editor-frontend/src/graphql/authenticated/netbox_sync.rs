use crate::graphql::authenticated::{DateTime, write_panel_path, write_port_label};
use crate::{
    error::FrontendError,
    graphql::{authenticated::schema, mutate, query},
};
use yew_oauth2::context::OAuth2Context;

/// Where the automatic sync of the plan active in Netbox stands (see docs/netbox-sync.md)
#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetboxSyncState {
    Synchron,
    Ausstehend,
    NichtSynchron,
    Fehler,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct NetboxStatusQuery {
    netbox_sync: NetboxStatus,
}

/// What everyone sees: whether Netbox is in sync
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "NetboxSync")]
pub struct NetboxStatus {
    pub state: NetboxSyncState,
}

impl NetboxStatus {
    pub async fn fetch(credentials: Option<&OAuth2Context>) -> Result<Self, FrontendError> {
        Ok(query::<NetboxStatusQuery, _>((), credentials)
            .await?
            .netbox_sync)
    }
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
struct NetboxSyncQuery {
    netbox_sync: NetboxSync,
}

/// The last run with its issues and error (admins)
#[derive(cynic::QueryFragment, Debug)]
pub struct NetboxSync {
    pub state: NetboxSyncState,
    /// Something changed that the last run didn't sync
    pub pending: bool,
    pub last_run: Option<DateTime>,
    /// After a failed run: when it is tried again
    pub retry_at: Option<DateTime>,
    pub active_plan: Option<ActivePlan>,
    pub issues: Vec<SyncIssue>,
    pub error: Option<NetboxSyncError>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "Plan")]
pub struct ActivePlan {
    pub id: i32,
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
pub struct NetboxSyncError {
    pub message: String,
    /// The extensions of the GraphQL error the run ended with, as JSON
    pub extensions: Option<String>,
}

impl NetboxSync {
    pub async fn fetch(credentials: Option<&OAuth2Context>) -> Result<Self, FrontendError> {
        Ok(query::<NetboxSyncQuery, _>((), credentials)
            .await?
            .netbox_sync)
    }

    /// Runs the sync right away (the backend's worker, after this request)
    pub async fn start(credentials: Option<&OAuth2Context>) -> Result<(), FrontendError> {
        mutate::<SyncNetboxMutation, _>((), credentials).await?;
        Ok(())
    }
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation")]
struct SyncNetboxMutation {
    #[allow(dead_code)]
    sync_netbox: bool,
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
        let panel = &self.panel;
        let label = match &self.label {
            Some(label) => label.clone(),
            None => format!("Port {}", self.order_number),
        };
        // Writing into a String can't fail
        let mut result = String::new();
        let _ = write_panel_path(
            &mut result,
            Some(&panel.schacht.name),
            panel
                .parent_chain
                .iter()
                .filter_map(|p| p.name.as_deref())
                .chain(panel.name.as_deref()),
        )
        .and_then(|()| write_port_label(&mut result, &label));
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
/// A Netbox rear port as it was when the issue was found
#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "NetboxPortRef")]
pub struct RearPort {
    pub name: String,
    pub device_name: Option<String>,
    pub location_name: Option<String>,
}
impl RearPort {
    pub fn display_name(&self) -> String {
        let location = self.location_name.as_deref().unwrap_or("<kein name>");
        let device = self.device_name.as_deref().unwrap_or("<kein name>");
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
