use crate::db::entity::panel::{Panel, PanelPort};
use async_graphql::{SimpleObject, Union};

/// Union-Typ für alle spezifischen Fehlerzustände
#[derive(Union)]
pub enum SyncIssue {
    MissingNetboxReference(MissingNetboxReferenceError),
    BlindEnd(BlindEndError),
    AsymmetricDuplex(AsymmetricDuplexError),
    PortBlockedInNetbox(PortBlockedInNetboxError),
    NameCollision(NameCollisionError),
    MissingNetboxMasterData(MissingNetboxMasterDataError),
    RoutingLoop(RoutingLoopError),
    InvalidTargetReference(InvalidTargetReferenceError),
}

#[derive(SimpleObject)]
pub struct MissingNetboxReferenceError {
    pub port: PanelPort,
}

#[derive(SimpleObject)]
pub struct BlindEndError {
    /// Der letzte Port des Traces (z.B. ein Spleiss ohne ausgehende Faser)
    pub port: PanelPort,
}

#[derive(SimpleObject)]
pub struct AsymmetricDuplexError {
    pub start_netbox_id: i32,
    pub connections: Box<[AsymetricTargetConnectionEntry]>,
}
#[derive(SimpleObject)]
pub struct AsymetricTargetConnectionEntry {
    pub target_netbox_id: i32,
    pub source_port: PanelPort,
    pub target_port: PanelPort,
}

#[derive(SimpleObject)]
pub struct PortBlockedInNetboxError {
    pub panel_id: i32,
    pub port_id: i32,
    /// Die ID des NetBox-Ports, der fälschlicherweise schon belegt ist
    pub netbox_port_id: i32,
}

#[derive(SimpleObject)]
pub struct NameCollisionError {
    /// Start-Panel des betroffenen Circuits
    pub panel: Panel,
    /// Der generierte Name, der in NetBox bereits blockiert ist
    pub circuit_name: String,
}

#[derive(SimpleObject)]
pub struct MissingNetboxMasterDataError {
    /// Identifikator, was fehlt (z.B. "Provider" oder "CircuitType")
    pub entity_type: String,
}

#[derive(SimpleObject)]
pub struct RoutingLoopError {
    /// Das Panel, an dem die Loop festgestellt wurde
    pub port: PanelPort,
}

#[derive(SimpleObject)]
pub struct InvalidTargetReferenceError {
    pub port: PanelPort,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlannedCircuit {
    pub start_netbox_id: i32,
    pub end_netbox_id: i32,
    pub start_port: PanelPort,
    pub end_port: PanelPort,
}
impl PlannedCircuit {
    pub fn order(self) -> PlannedCircuit {
        if self.start_netbox_id < self.end_netbox_id {
            self
        } else {
            PlannedCircuit {
                start_netbox_id: self.end_netbox_id,
                end_netbox_id: self.start_netbox_id,
                start_port: self.end_port,
                end_port: self.start_port,
            }
        }
    }
}
