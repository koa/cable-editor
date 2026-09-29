use crate::{
    config::NETBOX_CONFIG,
    db::{
        entity::panel::{Panel, PanelPort, PanelPortType},
        schema,
    },
    graphql::{
        authenticated::trace_fiber_path,
        error::{ApiError, ApiResult},
        loader::{PanelId, PanelPortId, load_one},
    },
    netbox::{
        fetch::{CurrentCircuitData, RearPort},
        fetch_device_with_ports, get_reqwest_client, query,
    },
};
use async_graphql::{ComplexObject, Context, Object, SimpleObject, Union};
use cable_editor_common::{ObjectKind, UserError, error::NetboxStep};
use diesel::{ExpressionMethods, QueryDsl};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Why a plan can't be synced to Netbox. Stored as JSON (`netbox_sync_issue`), so the
/// issues hold the ids of ports and panels, loaded when read, and a snapshot of Netbox's rear
/// ports (Netbox may be unreachable when they are shown).
#[derive(Union, Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind")]
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

impl SyncIssue {
    /// The ports and panels it refers to
    pub fn port_ids(&self) -> Box<[i32]> {
        match self {
            SyncIssue::MissingNetboxReference(MissingNetboxReferenceError { port_id })
            | SyncIssue::BlindEnd(BlindEndError { port_id })
            | SyncIssue::PortBlockedInNetbox(PortBlockedInNetboxError { port_id, .. })
            | SyncIssue::RoutingLoop(RoutingLoopError { port_id })
            | SyncIssue::InvalidTargetReference(InvalidTargetReferenceError { port_id }) => {
                Box::new([*port_id])
            }
            SyncIssue::AsymmetricDuplex(AsymmetricDuplexError { connections, .. }) => connections
                .iter()
                .flat_map(|connection| &connection.pairs)
                .flat_map(|pair| [pair.source_port_id, pair.target_port_id])
                .collect(),
            SyncIssue::NameCollision(_) | SyncIssue::MissingNetboxMasterData(_) => Box::default(),
        }
    }

    pub fn panel_ids(&self) -> Box<[i32]> {
        match self {
            SyncIssue::NameCollision(NameCollisionError { panel_id, .. }) => Box::new([*panel_id]),
            _ => Box::default(),
        }
    }
}

/// A Netbox rear port as it was when the issue was found.
#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct NetboxPortRef {
    pub id: u32,
    pub name: String,
    pub device_name: Option<String>,
    pub location_name: Option<String>,
}

impl NetboxPortRef {
    /// The rear port with its device from Netbox
    async fn fetch(id: i32) -> ApiResult<Self> {
        let port = RearPort::fetch_by_id((id as u32).into())
            .await?
            .ok_or_else(|| rear_port_not_found(id))?;
        let device = fetch_device_with_ports(port.device.id).await?;
        Ok(NetboxPortRef {
            id: port.id.into(),
            name: port.name,
            device_name: device.as_ref().and_then(|d| d.name.clone()),
            location_name: device.and_then(|d| d.location.map(|l| l.name)),
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MissingNetboxReferenceError {
    pub port_id: i32,
}

#[Object]
impl MissingNetboxReferenceError {
    async fn port(&self, ctx: &Context<'_>) -> ApiResult<PanelPort> {
        load_one(ctx, PanelPortId(self.port_id)).await
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlindEndError {
    /// The last port of the trace (e.g. a splice without an outgoing fiber)
    pub port_id: i32,
}

#[Object]
impl BlindEndError {
    async fn port(&self, ctx: &Context<'_>) -> ApiResult<PanelPort> {
        load_one(ctx, PanelPortId(self.port_id)).await
    }
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct AsymmetricDuplexError {
    pub start_netbox_port: NetboxPortRef,
    pub connections: Box<[AsymetricTargetConnectionEntry]>,
}
#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct AsymetricTargetConnectionEntry {
    pub target_netbox_port: NetboxPortRef,
    pub pairs: Box<[PortPair]>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PortPair {
    pub source_port_id: i32,
    pub target_port_id: i32,
}

#[Object]
impl PortPair {
    async fn source_port(&self, ctx: &Context<'_>) -> ApiResult<PanelPort> {
        load_one(ctx, PanelPortId(self.source_port_id)).await
    }
    async fn target_port(&self, ctx: &Context<'_>) -> ApiResult<PanelPort> {
        load_one(ctx, PanelPortId(self.target_port_id)).await
    }
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
#[graphql(complex)]
pub struct PortBlockedInNetboxError {
    #[graphql(skip)]
    pub port_id: i32,
    pub netbox_port: NetboxPortRef,
}

#[ComplexObject]
impl PortBlockedInNetboxError {
    async fn port(&self, ctx: &Context<'_>) -> ApiResult<PanelPort> {
        load_one(ctx, PanelPortId(self.port_id)).await
    }
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
#[graphql(complex)]
pub struct NameCollisionError {
    /// The panel the circuit starts at
    #[graphql(skip)]
    pub panel_id: i32,
    /// The generated name, already taken in Netbox
    pub circuit_name: String,
}

#[ComplexObject]
impl NameCollisionError {
    async fn panel(&self, ctx: &Context<'_>) -> ApiResult<Panel> {
        load_one(ctx, PanelId(self.panel_id)).await
    }
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct MissingNetboxMasterDataError {
    /// What is missing (e.g. "Provider" or "CircuitType")
    pub entity_type: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoutingLoopError {
    /// The port where the loop was found
    pub port_id: i32,
}

#[Object]
impl RoutingLoopError {
    async fn port(&self, ctx: &Context<'_>) -> ApiResult<PanelPort> {
        load_one(ctx, PanelPortId(self.port_id)).await
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InvalidTargetReferenceError {
    pub port_id: i32,
}

#[Object]
impl InvalidTargetReferenceError {
    async fn port(&self, ctx: &Context<'_>) -> ApiResult<PanelPort> {
        load_one(ctx, PanelPortId(self.port_id)).await
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CircuitMember {
    pub start_port: PanelPort,
    pub end_port: PanelPort,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedCircuit {
    pub start_netbox_id: i32,
    pub end_netbox_id: i32,
    pub members: Vec<CircuitMember>,
    pub distance: f64,
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
    /// A circuit id (CID) for Netbox that stays the same from sync to sync
    pub fn cid(&self) -> String {
        if let Some(first) = <[_]>::first(&self.members) {
            format!("FIBER-{:05}-{:05}", first.start_port.id, first.end_port.id)
        } else {
            "FIBER-EMPTY".to_string()
        }
    }

    /// A readable description of the circuit, of all its members
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

/// Syncs the circuits of the plan to Netbox, unless the plan has issues: then it returns them
/// and writes nothing. Reads in the caller's transaction on `connection`.
pub async fn sync_plan_to_netbox(
    plan_id: i32,
    connection: &mut AsyncPgConnection,
) -> ApiResult<Box<[SyncIssue]>> {
    let conn = connection;
    let mut issues = Vec::new();

    // calculate length of all cables
    let cable_lengths_vec = schema::kabel_trasse::table
        .inner_join(schema::trassen_mit_endpunkten::table)
        .group_by(schema::kabel_trasse::kabel)
        .select((
            schema::kabel_trasse::kabel,
            diesel::dsl::sum(crate::db::entity::st_length(
                schema::trassen_mit_endpunkten::geom,
            )),
        ))
        .load::<(i32, Option<f64>)>(conn)
        .await?;

    let cable_lengths: HashMap<i32, f64> = cable_lengths_vec
        .into_iter()
        .map(|(kabel_id, len)| (kabel_id, len.unwrap_or(0.0)))
        .collect();

    // The connectors, each traced to its other end
    let mut remaining_connector_ports = schema::panel_port::table
        .filter(schema::panel_port::port_type.eq(PanelPortType::Connector))
        .load::<PanelPort>(conn)
        .await?
        .into_iter()
        .map(|port| (port.id, port))
        .collect::<HashMap<_, _>>();
    let mut port_pairs = HashMap::<_, HashMap<_, Vec<_>>>::new();
    while !remaining_connector_ports.is_empty() {
        if let Some(port) = remaining_connector_ports
            .keys()
            .copied()
            .next()
            .and_then(|k| remaining_connector_ports.remove(&k))
        {
            let mut error = false;
            let trace = trace_fiber_path(conn, port.id, plan_id).await?;
            let Some(last_node) = trace.last() else {
                continue;
            };
            let target_port_id = last_node.to_port_id;

            let mut trace_length = 0.0;
            let mut seen_cables = std::collections::HashSet::new();
            for node in &trace {
                if seen_cables.insert(node.kabel) {
                    trace_length += cable_lengths.get(&node.kabel).copied().unwrap_or(0.0);
                }
            }

            if target_port_id == port.id {
                issues.push(SyncIssue::RoutingLoop(RoutingLoopError {
                    port_id: port.id,
                }));
                error = true;
            }
            if let Some(remote_port) = remaining_connector_ports.remove(&target_port_id) {
                if port.netbox_port_id.is_none() {
                    issues.push(SyncIssue::MissingNetboxReference(
                        MissingNetboxReferenceError { port_id: port.id },
                    ));
                    error = true;
                }
                if remote_port.netbox_port_id.is_none() {
                    issues.push(SyncIssue::MissingNetboxReference(
                        MissingNetboxReferenceError {
                            port_id: remote_port.id,
                        },
                    ));
                    error = true;
                }
                if let Some(p1) = port.netbox_port_id
                    && let Some(p2) = remote_port.netbox_port_id
                {
                    port_pairs
                        .entry(p1)
                        .or_default()
                        .entry(p2)
                        .or_default()
                        .push((port.clone(), remote_port.clone(), trace_length));
                    port_pairs
                        .entry(p2)
                        .or_default()
                        .entry(p1)
                        .or_default()
                        .push((remote_port, port, trace_length));
                }
            } else {
                issues.push(SyncIssue::InvalidTargetReference(
                    InvalidTargetReferenceError {
                        port_id: target_port_id,
                    },
                ));
                error = true;
            }
            if error {
                continue;
            }
        }
    }
    // port_pairs is symmetric: a connection between the Netbox rear ports p1 and p2 is
    // in port_pairs[p1][p2] and port_pairs[p2][p1].

    // A rear port connected to more than one other: an asymmetric duplex. The ports on
    // the other side see only this one, the issue lists them all.
    for (&netbox_id, partners) in &port_pairs {
        if partners.len() > 1 {
            issues.push(create_asymetric_duplex_error(netbox_id, partners.clone()).await?);
        }
    }

    // A circuit for every pair of rear ports connected only to each other, taken from
    // the side with the smaller id, so each pair once.
    let mut planned_circuits = Vec::new();
    for (&start_netbox_id, partners) in &port_pairs {
        let mut partners = partners.iter();
        let (Some((&end_netbox_id, connections)), None) = (partners.next(), partners.next()) else {
            // more than one partner, reported above
            continue;
        };
        if start_netbox_id == end_netbox_id {
            // The fibers lead back into the same rear port
            if let Some((port, _, _)) = connections.iter().next() {
                issues.push(SyncIssue::RoutingLoop(RoutingLoopError {
                    port_id: port.id,
                }));
            }
        } else if start_netbox_id < end_netbox_id
            && port_pairs
                .get(&end_netbox_id)
                .is_some_and(|partners| partners.len() == 1)
        {
            let distance =
                connections.iter().map(|(_, _, d)| *d).sum::<f64>() / connections.len() as f64;
            planned_circuits.push(
                PlannedCircuit {
                    start_netbox_id,
                    end_netbox_id,
                    members: connections
                        .iter()
                        .map(|(start_port, end_port, _)| CircuitMember {
                            start_port: start_port.clone(),
                            end_port: end_port.clone(),
                        })
                        .collect(),
                    distance,
                }
                .order(),
            );
        }
    }
    // Issues: nothing is written to Netbox
    if !issues.is_empty() {
        return Ok(issues.into_boxed_slice());
    }

    // From here on the planned state is complete and without issues: planned_circuits
    // holds each circuit Netbox needs exactly once.

    let mut existing_circuits = query::<CurrentCircuitData, _>(()).await?.circuit_list;

    let mut to_create = Vec::new();
    let mut to_delete = Vec::new();
    let mut to_update = Vec::new();

    for planned in planned_circuits {
        let cid = planned.cid();
        let expected_ports = vec![planned.start_netbox_id, planned.end_netbox_id];

        if let Some(idx) = existing_circuits.iter().position(|c| c.cid == cid) {
            let existing = existing_circuits.remove(idx);

            let mut actual_ports = existing.connected_rear_port_ids();
            actual_ports.sort();
            let mut expected = expected_ports.clone();
            expected.sort();

            if actual_ports == expected {
                let existing_id: u32 = existing.id.into();
                to_update.push((planned, existing_id));
            } else {
                // The CID exists, but with other terminations: delete and create anew
                to_delete.push(existing);
                to_create.push(planned);
            }
        } else {
            // The circuit doesn't exist yet
            to_create.push(planned);
        }
    }
    for existing in existing_circuits {
        if existing.cid.starts_with("FIBER-") {
            to_delete.push(existing);
        }
    }

    info!("Matching circuits: {}", to_update.len());
    info!("Circuits to create: {}", to_create.len());
    info!("Circuits to delete: {}", to_delete.len());

    let client = get_reqwest_client()?;
    let rest_base_url = format!("{}api", NETBOX_CONFIG.url());

    for circuit in to_delete {
        let id: u32 = circuit.id.into();
        info!("DELETE Circuit {} (NetBox ID: {})", circuit.cid, id);

        let res = client
            .delete(format!("{}/circuits/circuits/{}/", rest_base_url, id))
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(netbox_failed(NetboxStep::DeleteCircuit, id.to_string(), res).await);
        }
    }

    for circuit in to_create {
        info!("CREATE {}: {}", circuit.cid(), circuit.description());

        // Create the circuit
        let provider_id = crate::config::NETBOX_CONFIG.provider_id();
        let type_id = crate::config::NETBOX_CONFIG.type_id();

        let circuit_payload = serde_json::json!({
            "cid": circuit.cid(),
            "description": circuit.description(),
            "provider": provider_id,
            "type": type_id,
            "status": "active",
            "distance": (circuit.distance * 100.0).round() / 100.0, // Gerundet auf 2 Nachkommastellen
            "distance_unit": "m"
        });

        let res = client
            .post(format!("{}/circuits/circuits/", rest_base_url))
            .json(&circuit_payload)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(netbox_failed(NetboxStep::CreateCircuit, circuit.cid(), res).await);
        }

        let created_circuit: serde_json::Value = res.json().await?;
        let new_circuit_id =
            created_circuit["id"]
                .as_i64()
                .ok_or_else(|| UserError::NetboxWithoutId {
                    step: NetboxStep::CreateCircuit,
                    object: circuit.cid(),
                })?;

        // The sites of the rear ports, from Netbox
        let start_rp = RearPort::fetch_by_id((circuit.start_netbox_id as u32).into())
            .await?
            .ok_or_else(|| rear_port_not_found(circuit.start_netbox_id))?;
        let end_rp = RearPort::fetch_by_id((circuit.end_netbox_id as u32).into())
            .await?
            .ok_or_else(|| rear_port_not_found(circuit.end_netbox_id))?;

        for (side, rear_port_id, site_id, existing_cable_id) in [
            (
                "A",
                circuit.start_netbox_id,
                u32::from(start_rp.device.site.id),
                start_rp.cable.map(|c| u32::from(c.id)),
            ),
            (
                "Z",
                circuit.end_netbox_id,
                u32::from(end_rp.device.site.id),
                end_rp.cable.map(|c| u32::from(c.id)),
            ),
        ] {
            // The termination at the rear port's site
            let term_payload = serde_json::json!({
                "circuit": new_circuit_id,
                "term_side": side,
                "termination_type": "dcim.site",
                "termination_id": site_id
            });

            let term_res = client
                .post(format!("{}/circuits/circuit-terminations/", rest_base_url))
                .json(&term_payload)
                .send()
                .await?;

            if !term_res.status().is_success() {
                let object = format!("{} {side}", circuit.cid());
                return Err(netbox_failed(NetboxStep::CreateTermination, object, term_res).await);
            }

            let created_term: serde_json::Value = term_res.json().await?;
            let term_id =
                created_term["id"]
                    .as_i64()
                    .ok_or_else(|| UserError::NetboxWithoutId {
                        step: NetboxStep::CreateTermination,
                        object: format!("{} {side}", circuit.cid()),
                    })?;

            // Remove a cable blocking the port, if any
            if let Some(cable_id) = existing_cable_id {
                info!("Removing cable {} blocking port {}", cable_id, rear_port_id);
                // The result is ignored on purpose: deleting the previous circuit may have
                // deleted the cable already (404), which is fine.
                let _ = client
                    .delete(format!("{}/dcim/cables/{}/", rest_base_url, cable_id))
                    .send()
                    .await;
            }

            // Patch a cable from the circuit termination to the rear port
            let cable_payload = serde_json::json!({
                "a_terminations": [{"object_type": "circuits.circuittermination", "object_id": term_id}],
                "b_terminations": [{"object_type": "dcim.rearport", "object_id": rear_port_id}],
                "status": "connected"
            });

            let cable_res = client
                .post(format!("{}/dcim/cables/", rest_base_url))
                .json(&cable_payload)
                .send()
                .await?;

            if !cable_res.status().is_success() {
                let object = format!("{} {side}", circuit.cid());
                return Err(netbox_failed(NetboxStep::PatchCable, object, cable_res).await);
            }
        }
    }
    // Update the existing circuits (length and description)
    for (circuit, netbox_id) in to_update {
        info!(
            "UPDATE {} (NetBox ID {}): {}",
            circuit.cid(),
            netbox_id,
            circuit.description()
        );

        let update_payload = serde_json::json!({
            "description": circuit.description(),
            "distance": (circuit.distance * 100.0).round() / 100.0,
            "distance_unit": "m"
        });

        // PATCH changes only the fields given
        let res = client
            .patch(format!(
                "{}/circuits/circuits/{}/",
                rest_base_url, netbox_id
            ))
            .json(&update_payload)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(netbox_failed(NetboxStep::UpdateCircuit, circuit.cid(), res).await);
        }
    }
    Ok(Box::default())
}

/// Netbox refused `step`; its answer goes along for support.
async fn netbox_failed(step: NetboxStep, object: String, response: reqwest::Response) -> ApiError {
    UserError::NetboxFailed {
        step,
        object,
        detail: response.text().await.unwrap_or_default(),
    }
    .into()
}

fn rear_port_not_found(id: i32) -> UserError {
    UserError::NotFound {
        kind: ObjectKind::NetboxRearPort,
        id: id.into(),
    }
}

async fn create_asymetric_duplex_error(
    start_netbox_id: i32,
    r1: HashMap<i32, Vec<(PanelPort, PanelPort, f64)>>,
) -> ApiResult<SyncIssue> {
    let start_netbox_port = NetboxPortRef::fetch(start_netbox_id).await?;
    let mut connections = Vec::new();
    for (target_netbox_id, port_pairs) in r1 {
        connections.push(AsymetricTargetConnectionEntry {
            target_netbox_port: NetboxPortRef::fetch(target_netbox_id).await?,
            pairs: port_pairs
                .into_iter()
                .map(|(source_port, target_port, _)| PortPair {
                    source_port_id: source_port.id,
                    target_port_id: target_port.id,
                })
                .collect(),
        });
    }
    Ok(SyncIssue::AsymmetricDuplex(AsymmetricDuplexError {
        start_netbox_port,
        connections: connections.into_boxed_slice(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stored in `netbox_sync_issue` and read back when shown
    #[test]
    fn issues_survive_storing() {
        let port = |id| NetboxPortRef {
            id,
            name: format!("RP {id}"),
            device_name: Some("ODF".into()),
            location_name: None,
        };
        let issues = [
            SyncIssue::MissingNetboxReference(MissingNetboxReferenceError { port_id: 7 }),
            SyncIssue::AsymmetricDuplex(AsymmetricDuplexError {
                start_netbox_port: port(1),
                connections: Box::new([AsymetricTargetConnectionEntry {
                    target_netbox_port: port(2),
                    pairs: Box::new([PortPair {
                        source_port_id: 3,
                        target_port_id: 4,
                    }]),
                }]),
            }),
            SyncIssue::NameCollision(NameCollisionError {
                panel_id: 5,
                circuit_name: "FIBER-00003-00004".into(),
            }),
        ];
        let stored = issues
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Box<[_]>, _>>()
            .unwrap_or_default();
        assert_eq!(stored[0]["kind"], "MissingNetboxReference");
        let read: Box<[SyncIssue]> = stored
            .into_iter()
            .filter_map(|value| serde_json::from_value(value).ok())
            .collect();
        let ids = |issues: &[SyncIssue]| {
            issues
                .iter()
                .map(|issue| (issue.port_ids(), issue.panel_ids()))
                .collect::<Box<[_]>>()
        };
        assert_eq!(ids(&read), ids(&issues));
        assert_eq!(*ids(&read)[1].0, [3, 4]);
    }
}
