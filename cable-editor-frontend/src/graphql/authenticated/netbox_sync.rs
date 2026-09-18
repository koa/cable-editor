use crate::error::FrontendError;
use crate::graphql::authenticated::schema;
use crate::graphql::mutate;
use std::fmt;
use std::fmt::Write;
use std::fmt::write;
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

#[derive(cynic::QueryFragment, Debug)]
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
            write!(&mut result, ": Port {}", self.order_number).expect("Strange error");
        }
        result
    }
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Panel")]
struct PanelInfo {
    name: Option<String>,
    schacht: SchachtInfo,
    parent_chain: Vec<ParentChainPanelInfo>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Panel")]
struct ParentChainPanelInfo {
    name: Option<String>,
}
#[derive(cynic::QueryFragment, Debug)]
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
    pub start_netbox_id: i32,
    pub connections: Vec<AsymetricTargetConnectionEntry>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "AsymetricTargetConnectionEntry")]
pub struct AsymetricTargetConnectionEntry {
    pub target_netbox_id: i32,
    pub source_port: PanelPortInfo,
    pub target_port: PanelPortInfo,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "PortBlockedInNetboxError")]
pub struct PortBlockedInNetboxError {
    pub panel_id: i32,
    pub port_id: i32,
    pub netbox_port_id: i32,
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
        let response =
            mutate::<SyncNetbox, _>(SyncNetboxVariables { plan_id }, credentials).await?;
        if let Some(errors) = response.errors {
            Err(FrontendError::Graphql(errors))
        } else {
            response
                .data
                .map(|d| d.sync_plan_to_netbox)
                .ok_or(FrontendError::NotFound)
        }
    }
}
