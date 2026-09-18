use crate::db::entity::panel::{Panel, PanelPort};
use crate::netbox::fetch::RearPort;
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
    pub start_netbox_port: RearPort,
    pub connections: Box<[AsymetricTargetConnectionEntry]>,
}
#[derive(SimpleObject)]
pub struct AsymetricTargetConnectionEntry {
    pub target_netbox_port: RearPort,
    pub pairs: Box<[PortPair]>,
}
#[derive(SimpleObject)]
pub struct PortPair {
    pub source_port: PanelPort,
    pub target_port: PanelPort,
}

#[derive(SimpleObject)]
pub struct PortBlockedInNetboxError {
    pub port: PanelPort,
    pub netbox_port: RearPort,
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
pub struct CircuitMember {
    pub start_port: PanelPort,
    pub end_port: PanelPort,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlannedCircuit {
    pub start_netbox_id: i32,
    pub end_netbox_id: i32,
    pub members: Vec<CircuitMember>,
}
impl PlannedCircuit {
    pub fn order(mut self) -> PlannedCircuit {
        if self.start_netbox_id > self.end_netbox_id {
            std::mem::swap(&mut self.start_netbox_id, &mut self.end_netbox_id);
            for member in &mut self.members {
                std::mem::swap(&mut member.start_port, &mut member.end_port);
            }
        }

        self.members
            .sort_by_key(|m| (m.start_port.id, m.end_port.id));
        self
    }
    /// Generiert eine über Synchronisationen hinweg stabile Circuit ID (CID) für NetBox
    pub fn cid(&self) -> String {
        if let Some(first) = self.members.first() {
            format!("FIBER-{:05}-{:05}", first.start_port.id, first.end_port.id)
        } else {
            "FIBER-EMPTY".to_string()
        }
    }

    /// Generiert eine lesbare, aggregierte Beschreibung für den Circuit
    pub fn description(&self) -> String {
        let starts = self
            .members
            .iter()
            .map(|m| m.start_port.create_label().into_owned())
            .collect::<Vec<_>>()
            .join(", ");
        let ends = self
            .members
            .iter()
            .map(|m| m.end_port.create_label().into_owned())
            .collect::<Vec<_>>()
            .join(", ");

        format!("LWL Crossconnect: [{}] ↔ [{}]", starts, ends)
    }
}
