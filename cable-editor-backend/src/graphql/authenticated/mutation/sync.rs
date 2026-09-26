use crate::{
    config::NETBOX_CONFIG,
    db::{
        entity::panel::{Panel, PanelPort, PanelPortType},
        schema,
    },
    graphql::authenticated::trace_fiber_path,
    netbox::{
        fetch::{CurrentCircuitData, RearPort},
        get_reqwest_client, query,
    },
};
use async_graphql::{SimpleObject, Union};
use diesel::{ExpressionMethods, HasQuery, QueryDsl};
use diesel_async::pooled_connection::deadpool::Object;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};

use log::info;
use std::collections::HashMap;
use tokio::sync::MutexGuard;

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
    /// Generiert eine über Synchronisationen hinweg stabile Circuit ID (CID) für NetBox
    pub fn cid(&self) -> String {
        if let Some(first) = <[_]>::first(&self.members) {
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

pub async fn sync_plan_to_netbox(
    plan_id: i32,
    mut connection: MutexGuard<'_, Object<AsyncPgConnection>>,
) -> Result<Vec<SyncIssue>, async_graphql::Error> {
    let issues = connection
        .transaction::<_, async_graphql::Error, _>(async move |conn| {
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

            // DB-Daten laden
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
                            port: port.clone(),
                        }));
                        error = true;
                    }
                    if let Some(remote_port) = remaining_connector_ports.remove(&target_port_id)
                    {
                        if port.netbox_port_id.is_none() {
                            issues.push(SyncIssue::MissingNetboxReference(
                                MissingNetboxReferenceError { port: port.clone() },
                            ));
                            error = true;
                        }
                        if remote_port.netbox_port_id.is_none() {
                            issues.push(SyncIssue::MissingNetboxReference(
                                MissingNetboxReferenceError {
                                    port: remote_port.clone(),
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
                        let port = PanelPort::query()
                            .filter(schema::panel_port::id.eq(target_port_id))
                            .first(conn)
                            .await?;
                        issues.push(SyncIssue::InvalidTargetReference(
                            InvalidTargetReferenceError { port },
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
                let (Some((&end_netbox_id, connections)), None) = (partners.next(), partners.next())
                else {
                    // more than one partner, reported above
                    continue;
                };
                if start_netbox_id == end_netbox_id {
                    // The fibers lead back into the same rear port
                    if let Some((port, _, _)) = connections.iter().next() {
                        issues.push(SyncIssue::RoutingLoop(RoutingLoopError {
                            port: port.clone(),
                        }));
                    }
                } else if start_netbox_id < end_netbox_id
                    && port_pairs
                        .get(&end_netbox_id)
                        .is_some_and(|partners| partners.len() == 1)
                {
                    let distance = connections.iter().map(|(_, _, d)| *d).sum::<f64>()
                        / connections.len() as f64;
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
            // 4. Abbruch, falls fachliche Fehler gefunden wurden
            if !issues.is_empty() {
                return Ok(issues);
            }

            // --- AB HIER: SOLL-ZUSTAND IST DEFINIERT UND FEHLERFREI ---
            // planned_circuits enthält nun dedupliziert exakt die Circuits, die NetBox benötigt.

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
                        // CID existiert, aber die Terminations sind falsch -> löschen und neu anlegen
                        to_delete.push(existing);
                        to_create.push(planned);
                    }
                } else {
                    // Circuit existiert noch gar nicht
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

            let client = get_reqwest_client()
                .map_err(|e| async_graphql::Error::new(e.to_string()))?;
            let rest_base_url = format!("{}api", NETBOX_CONFIG
                .url()
            );

            for circuit in to_delete {
                let id: u32 = circuit.id.into();
                info!("DELETE Circuit {} (NetBox ID: {})", circuit.cid, id);

                let res = client.delete(format!("{}/circuits/circuits/{}/", rest_base_url, id))
                    .send()
                    .await
                    .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                if !res.status().is_success() {
                    return Err(async_graphql::Error::new(format!(
                        "Fehler beim Löschen von Circuit {}: {:?}",
                        id, res.text().await.unwrap_or_default()
                    )));
                }
            }

            for circuit in to_create {
                info!("CREATE {}: {}", circuit.cid(), circuit.description());

                // --- 5.1 Circuit erstellen ---
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

                let res = client.post(format!("{}/circuits/circuits/", rest_base_url))
                    .json(&circuit_payload)
                    .send()
                    .await
                    .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                if !res.status().is_success() {
                    return Err(async_graphql::Error::new(format!(
                        "Fehler beim Erstellen des Circuits {}: {:?}",
                        circuit.cid(), res.text().await.unwrap_or_default()
                    )));
                }

                let created_circuit: serde_json::Value = res.json().await
                    .map_err(|e| async_graphql::Error::new(e.to_string()))?;
                let new_circuit_id = created_circuit["id"].as_i64().ok_or_else(|| {
                    async_graphql::Error::new(format!(
                        "Netbox returned no id for the circuit {}: {created_circuit}",
                        circuit.cid()
                    ))
                })?;

                // --- 5.2 Dynamische Site-IDs für die RearPorts aus Netbox abfragen ---
                let start_rp = RearPort::fetch_by_id((circuit.start_netbox_id as u32).into())
                    .await?
                    .ok_or_else(|| async_graphql::Error::new(format!("Start RearPort {} nicht gefunden", circuit.start_netbox_id)))?;
                let end_rp = RearPort::fetch_by_id((circuit.end_netbox_id as u32).into())
                    .await?
                    .ok_or_else(|| async_graphql::Error::new(format!("End RearPort {} nicht gefunden", circuit.end_netbox_id)))?;

                for (side, rear_port_id, site_id, existing_cable_id) in [
                    ("A", circuit.start_netbox_id, u32::from(start_rp.device.site.id), start_rp.cable.map(|c| u32::from(c.id))),
                    ("Z", circuit.end_netbox_id, u32::from(end_rp.device.site.id), end_rp.cable.map(|c| u32::from(c.id))),
                ] {
                    // 5.2.1 Termination mit dynamischer Site-ID erstellen
                    let term_payload = serde_json::json!({
                            "circuit": new_circuit_id,
                            "term_side": side,
                            "termination_type": "dcim.site",
                            "termination_id": site_id
                        });

                    let term_res = client.post(format!("{}/circuits/circuit-terminations/", rest_base_url))
                        .json(&term_payload)
                        .send()
                        .await
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    if !term_res.status().is_success() {
                        return Err(async_graphql::Error::new(format!(
                            "Fehler beim Erstellen der Termination Seite {}: {:?}",
                            side, term_res.text().await.unwrap_or_default()
                        )));
                    }

                    let created_term: serde_json::Value = term_res.json().await
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;
                    let term_id = created_term["id"].as_i64().ok_or_else(|| {
                        async_graphql::Error::new(format!(
                            "Netbox returned no id for the termination {side}: {created_term}"
                        ))
                    })?;

                    // 5.2.2 Eventuell vorhandenes Kabel entfernen, das den Port blockiert
                    if let Some(cable_id) = existing_cable_id {
                        info!("Entferne blockierendes Kabel {} an Port {}", cable_id, rear_port_id);
                        // Den Rückgabewert ignorieren wir bewusst: Falls es bereits durch das Löschen
                        // des vorherigen Circuits kaskadierend mitgelöscht wurde (404), ist das für uns völlig ok.
                        let _ = client.delete(format!("{}/dcim/cables/{}/", rest_base_url, cable_id))
                            .send()
                            .await;
                    }

                    // 5.2.3 Kabel vom Circuit-Termination zum RearPort patchen
                    let cable_payload = serde_json::json!({
                            "a_terminations": [{"object_type": "circuits.circuittermination", "object_id": term_id}],
                            "b_terminations": [{"object_type": "dcim.rearport", "object_id": rear_port_id}],
                            "status": "connected"
                        });

                    let cable_res = client.post(format!("{}/dcim/cables/", rest_base_url))
                        .json(&cable_payload)
                        .send()
                        .await
                        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                    if !cable_res.status().is_success() {
                        return Err(async_graphql::Error::new(format!(
                            "Fehler beim Patchen des Kabels Seite {}: {:?}",
                            side, cable_res.text().await.unwrap_or_default()
                        )));
                    }
                }
            }
            // 6. BESTEHENDE CIRCUITS AKTUALISIEREN (Länge und Beschreibung)
            for (circuit, netbox_id) in to_update {
                info!("UPDATE {} (NetBox ID {}): {}", circuit.cid(), netbox_id, circuit.description());

                let update_payload = serde_json::json!({
                        "description": circuit.description(),
                        "distance": (circuit.distance * 100.0).round() / 100.0,
                        "distance_unit": "m"
                    });

                // PATCH aktualisiert nur die mitgegebenen Felder
                let res = client.patch(format!("{}/circuits/circuits/{}/", rest_base_url, netbox_id))
                    .json(&update_payload)
                    .send()
                    .await
                    .map_err(|e| async_graphql::Error::new(e.to_string()))?;

                if !res.status().is_success() {
                    return Err(async_graphql::Error::new(format!(
                        "Fehler beim Aktualisieren des Circuits {}: {:?}",
                        circuit.cid(), res.text().await.unwrap_or_default()
                    )));
                }
            }
            Ok(Vec::default())
        })
        .await?;
    Ok(issues)
}

async fn create_asymetric_duplex_error(
    start_netbox_id: i32,
    r1: HashMap<i32, Vec<(PanelPort, PanelPort, f64)>>,
) -> async_graphql::Result<SyncIssue> {
    let start_netbox_port = RearPort::fetch_by_id((start_netbox_id as u32).into())
        .await?
        .ok_or_else(|| {
            async_graphql::Error::new(format!("Netbox RearPort {} not found", start_netbox_id))
        })?;

    let mut connections = Vec::new();
    for (target_netbox_id, port_pairs) in r1 {
        let target_netbox_port = RearPort::fetch_by_id((target_netbox_id as u32).into())
            .await?
            .ok_or_else(|| {
                async_graphql::Error::new(format!("Netbox RearPort {} not found", target_netbox_id))
            })?;

        let pairs: Vec<PortPair> = port_pairs
            .into_iter()
            .map(|(source_port, target_port, _)| PortPair {
                source_port,
                target_port,
            })
            .collect();

        connections.push(AsymetricTargetConnectionEntry {
            target_netbox_port,
            pairs: pairs.into_boxed_slice(),
        });
    }

    Ok(SyncIssue::AsymmetricDuplex(AsymmetricDuplexError {
        start_netbox_port,
        connections: connections.into_boxed_slice(),
    }))
}
