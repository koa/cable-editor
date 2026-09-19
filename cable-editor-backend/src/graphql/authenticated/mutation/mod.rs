pub mod sync;

use crate::config::NETBOX_CONFIG;
use crate::graphql::authenticated::mutation::sync::{
    AsymetricTargetConnectionEntry, CircuitMember, InvalidTargetReferenceError, PortPair,
};
use crate::netbox::fetch::{CurrentCircuitData, RearPort};
use crate::netbox::{get_reqwest_client, query};
use crate::{
    db::{
        entity::{
            cable::{Cable, UpdateCableChangeset},
            panel::{
                InsertPanel, InsertPanelPort, Panel, PanelPort, PanelPortType, PortSide, PortUsage,
            },
            plan::{InsertPlan, Plan, PlanStatusType},
        },
        schema,
    },
    graphql::authenticated::{
        self,
        mutation::sync::{
            AsymmetricDuplexError, BlindEndError, MissingNetboxReferenceError, PlannedCircuit,
            RoutingLoopError, SyncIssue,
        },
        trace_fiber_path,
    },
};
use async_graphql::{Context, InputObject, Object, OneofObject};
use async_recursion::async_recursion;
use diesel::{
    AsChangeset, BoolExpressionMethods, ExpressionMethods, HasQuery, OptionalExtension, QueryDsl,
    associations::HasTable, dsl::max,
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use log::info;
use std::collections::HashMap;

pub struct Mutation;

#[Object]
impl Mutation {
    async fn create_cable(&self, ctx: &Context<'_>, name: String) -> async_graphql::Result<Cable> {
        let mut connection = authenticated::get_connection(ctx).await?;
        Ok(diesel::insert_into(schema::kabel::table)
            .values((
                schema::kabel::name.eq(name),
                schema::kabel::buendel_anz.eq(1),
                schema::kabel::faser_anz.eq(12),
            ))
            .get_result::<Cable>(&mut connection)
            .await?)
    }
    async fn update_cable(
        &self,
        ctx: &Context<'_>,
        cable_id: i32,
        name: Option<String>,
        fibers: Option<UpdateCableStructure>,
        path: Option<Vec<i32>>,
    ) -> async_graphql::Result<Option<Cable>> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let (buendel_anz, faser_anz) = if let Some(UpdateCableStructure {
            bundle_count,
            fiber_count,
        }) = fibers
        {
            (Some(bundle_count as i32), Some(fiber_count as i32))
        } else {
            (None, None)
        };

        let changeset = UpdateCableChangeset {
            name,
            buendel_anz,
            faser_anz,
        };

        let updated_db_cable = connection
            .transaction(async move |conn| {
                if let Some(ref path_ids) = path {
                    diesel::delete(
                        schema::kabel_trasse::table
                            .filter(schema::kabel_trasse::kabel.eq(cable_id)),
                    )
                    .execute(conn)
                    .await?;

                    for (sequenz, &trasse_id) in path_ids.iter().enumerate() {
                        diesel::insert_into(schema::kabel_trasse::table)
                            .values((
                                schema::kabel_trasse::kabel.eq(cable_id),
                                schema::kabel_trasse::trasse.eq(trasse_id),
                                schema::kabel_trasse::sequenz.eq(sequenz as i32),
                            ))
                            .execute(conn)
                            .await?;
                    }
                }

                let updated = if changeset.any() {
                    diesel::update(schema::kabel::table.find(cable_id))
                        .set(&changeset)
                        .get_result::<Cable>(conn)
                        .await
                        .optional()?
                } else {
                    schema::kabel::table
                        .find(cable_id)
                        .first::<Cable>(conn)
                        .await
                        .optional()?
                };

                Ok::<Option<Cable>, diesel::result::Error>(updated)
            })
            .await?;

        Ok(updated_db_cable)
    }
    async fn delete_cable(&self, ctx: &Context<'_>, cable_id: i32) -> async_graphql::Result<bool> {
        authenticated::get_connection(ctx)
            .await?
            .transaction(async move |conn| {
                diesel::delete(
                    schema::kabel_trasse::table.filter(schema::kabel_trasse::kabel.eq(cable_id)),
                )
                .execute(conn)
                .await?;
                diesel::delete(schema::kabel::table.filter(schema::kabel::id.eq(cable_id)))
                    .execute(conn)
                    .await?;
                Ok(true)
            })
            .await
    }
    async fn create_panel(
        &self,
        ctx: &Context<'_>,
        panel: CreatePanel,
        parent_panel: Option<i32>,
    ) -> async_graphql::Result<bool> {
        authenticated::get_connection(ctx)
            .await?
            .transaction(async move |conn| {
                let parent_order = if let Some(parent_id) = parent_panel {
                    let max_order: Option<i32> = schema::panel::table
                        .filter(schema::panel::parent_panel.eq(parent_id))
                        .select(max(schema::panel::parent_order))
                        .first(conn)
                        .await?;
                    Some(max_order.unwrap_or(0) + 1)
                } else {
                    None
                };
                insert_panel_tree_recursive(conn, panel, parent_panel, parent_order).await?;
                Ok(true)
            })
            .await
    }
    async fn update_panels(
        &self,
        ctx: &Context<'_>,
        updates: Vec<PanelUpdate>,
    ) -> async_graphql::Result<bool> {
        authenticated::get_connection(ctx)
            .await?
            .transaction(async move |conn| {
                for PanelUpdate {
                    panel_id,
                    name,
                    order,
                    parent,
                    netbox_device_id,
                } in updates
                {
                    diesel::update(schema::panel::table)
                        .filter(schema::panel::id.eq(panel_id))
                        .set(UpdatePanelChangeset {
                            name: name.map(|n| n.value),
                            parent_panel: order.map(|o| Some(o.order)),
                            parent_order: parent.map(|p| p.parent),
                            netbox_device_id: netbox_device_id.map(|n| n.device_id),
                        })
                        .execute(conn)
                        .await?;
                }

                Ok(true)
            })
            .await
    }
    async fn create_plan(
        &self,
        ctx: &Context<'_>,
        plan: CreatePlan,
    ) -> async_graphql::Result<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let new_plan = InsertPlan { name: plan.name };
        diesel::insert_into(schema::plan::table)
            .values(new_plan)
            .execute(&mut connection)
            .await?;
        Ok(true)
    }
    async fn update_cabinet_panels(
        &self,
        ctx: &Context<'_>,
        cabinet_id: i32,
        changes: Vec<FlatPanelInput>,
        deletes: Vec<i32>,
    ) -> async_graphql::Result<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;

        connection
            .transaction(async move |conn| {
                // 1. Zuerst Löschungen verarbeiten
                if !deletes.is_empty() {
                    diesel::delete(schema::panel::table.filter(schema::panel::id.eq_any(&deletes)))
                        .execute(conn)
                        .await?;
                }

                // Mapping von temporären Frontend-UUIDs zu echten Datenbank-IDs
                let mut temp_id_map: HashMap<String, i32> = HashMap::new();

                // 2. Erstellungen und Updates verarbeiten (Reihenfolge ist dank Frontend korrekt)
                for change in changes {
                    // Parent-ID auflösen (entweder echte ID oder aus der Mapping-Tabelle)
                    let resolved_parent_id = match change.parent_id {
                        Some(p_id) => {
                            if let Some(id) = p_id.id {
                                Some(id)
                            } else if let Some(temp) = p_id.temporary {
                                Some(*temp_id_map.get(&temp).ok_or_else(|| {
                                    async_graphql::Error::new(
                                        "Parent temporary ID not found in mapping",
                                    )
                                })?)
                            } else {
                                None
                            }
                        }
                        None => None,
                    };

                    if let Some(panel_id) = change.id.id {
                        // UPDATE: Bestehendes Panel
                        diesel::update(schema::panel::table.find(panel_id))
                            .set((
                                schema::panel::name.eq(change.name),
                                schema::panel::parent_panel.eq(resolved_parent_id),
                                schema::panel::parent_order.eq(change.order),
                                schema::panel::netbox_device_id.eq(change.netbox_device_id),
                            ))
                            .execute(conn)
                            .await?;
                    } else if let Some(temp_id) = change.id.temporary {
                        // CREATE: Neues Panel
                        let new_panel = InsertPanel {
                            name: change.name,
                            schacht_id: cabinet_id,
                            parent_panel: resolved_parent_id,
                            parent_order: Some(change.order), // Das Schema erwartet Option<i32>
                        };

                        let inserted_id: i32 = diesel::insert_into(schema::panel::table)
                            .values(new_panel)
                            .returning(schema::panel::id)
                            .get_result(conn)
                            .await?;

                        // Die neue DB-ID für potenziell folgende Kinder-Panels merken
                        temp_id_map.insert(temp_id, inserted_id);
                    } else {
                        return Err(async_graphql::Error::new(
                            "Change must have either id or temporary id",
                        ));
                    }
                }

                Ok::<bool, async_graphql::Error>(true)
            })
            .await?;

        Ok(true)
    }
    async fn update_panel_ports(
        &self,
        ctx: &Context<'_>,
        panel_id: i32,
        changes: Vec<FlatPortInput>,
        deletes: Vec<i32>,
    ) -> async_graphql::Result<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;

        connection
            .transaction(async move |conn| {
                // 1. Zuerst Löschungen verarbeiten
                if !deletes.is_empty() {
                    diesel::delete(
                        schema::panel_port::table.filter(schema::panel_port::id.eq_any(&deletes)),
                    )
                    .execute(conn)
                    .await?;
                }

                // 2. Erstellungen und Updates verarbeiten
                for change in changes {
                    // Leere Strings aus dem UI in echte SQL-NULL Werte umwandeln
                    let label_opt = if change.label.trim().is_empty() {
                        None
                    } else {
                        Some(change.label)
                    };

                    if let Some(port_id) = change.id.id {
                        // UPDATE: Bestehender Port
                        // Wir prüfen zur Sicherheit panel_id mit, damit niemand fremde Ports manipuliert
                        diesel::update(
                            schema::panel_port::table.filter(
                                schema::panel_port::id
                                    .eq(port_id)
                                    .and(schema::panel_port::panel_id.eq(panel_id)),
                            ),
                        )
                        .set((
                            schema::panel_port::port_order.eq(change.order),
                            schema::panel_port::label.eq(label_opt),
                            schema::panel_port::port_type.eq(change.port_type),
                            schema::panel_port::netbox_port_id.eq(change.netbox_port_id),
                        ))
                        .execute(conn)
                        .await?;
                    } else if change.id.temporary.is_some() {
                        // CREATE: Neuer Port
                        let new_port = InsertPanelPort {
                            panel_id,
                            port_order: change.order,
                            port_type: change.port_type,
                            label: label_opt,
                            netbox_port_id: change.netbox_port_id,
                        };

                        diesel::insert_into(schema::panel_port::table)
                            .values(new_port)
                            .execute(conn)
                            .await?;
                    } else {
                        return Err(async_graphql::Error::new(
                            "Change must have either id or temporary id",
                        ));
                    }
                }

                Ok::<bool, async_graphql::Error>(true)
            })
            .await?;

        Ok(true)
    }
    async fn set_port_usage(
        &self,
        ctx: &Context<'_>,
        plan_id: i32,
        changes: Vec<PortUsageInput>,
    ) -> async_graphql::Result<bool> {
        if plan_id <= 0 {
            return Err(format!("Cannot manipulate plan {plan_id} directly").into());
        }
        let mut connection = authenticated::get_connection(ctx).await?;
        connection
            .transaction(async move |conn| {
                for PortUsageInput {
                    port_id,
                    side,
                    fiber,
                } in changes
                {
                    let (port_update, remove_plan) = match fiber {
                        PortUsageUpdateAction::Remove(_) => (
                            Some(PortUsage {
                                port_id,
                                plan_id,
                                side,
                                cable: None,
                                fiber: None,
                                bundle: None,
                            }),
                            false,
                        ),
                        PortUsageUpdateAction::Reset(_) => (None, true),
                        PortUsageUpdateAction::Attach(FiberKeyInput {
                            cable_id,
                            bundle,
                            fiber,
                        }) => (
                            Some(PortUsage {
                                port_id,
                                plan_id,
                                side,
                                cable: Some(cable_id),
                                fiber: Some(fiber),
                                bundle: Some(bundle),
                            }),
                            false,
                        ),
                    };
                    if let Some(usage) = port_update {
                        diesel::insert_into(schema::port_usage::dsl::port_usage::table())
                            .values(&usage)
                            .on_conflict((
                                schema::port_usage::port_id,
                                schema::port_usage::plan_id,
                                schema::port_usage::side,
                            ))
                            .do_update()
                            .set((
                                schema::port_usage::cable.eq(usage.cable),
                                schema::port_usage::fiber.eq(usage.fiber),
                                schema::port_usage::bundle.eq(usage.bundle),
                            ))
                            .execute(conn)
                            .await?;
                    }
                    if remove_plan {
                        diesel::delete(schema::port_usage::dsl::port_usage::table())
                            .filter(schema::port_usage::port_id.eq(port_id))
                            .filter(schema::port_usage::plan_id.eq(plan_id))
                            .filter(schema::port_usage::side.eq(side))
                            .execute(conn)
                            .await?;
                    }
                }

                Ok::<bool, async_graphql::Error>(true)
            })
            .await
    }
    async fn update_plan(
        &self,
        ctx: &Context<'_>,
        plan_id: i32,
        name: String,
    ) -> async_graphql::Result<Plan> {
        let mut connection = authenticated::get_connection(ctx).await?;
        connection
            .transaction::<_, async_graphql::Error, _>(async move |conn| {
                let mut plan = Plan::query()
                    .for_update()
                    .filter(schema::plan::id.eq(plan_id))
                    .first(conn)
                    .await?;
                plan.name = name;
                diesel::update(&plan).set(&plan).execute(conn).await?;
                Ok(plan)
            })
            .await
    }
    async fn implement_plan(&self, ctx: &Context<'_>, plan_id: i32) -> async_graphql::Result<Plan> {
        if plan_id <= 0 {
            return Err(
                format!("Cannot implement plan {plan_id}, it is already implemented").into(),
            );
        }
        let mut connection = authenticated::get_connection(ctx).await?;
        connection
            .transaction::<_, async_graphql::Error, _>(async move |conn| {
                let mut plan = Plan::query()
                    .for_update()
                    .filter(schema::plan::id.eq(plan_id))
                    .first(conn)
                    .await?;
                if plan.status != PlanStatusType::Open {
                    return Err(async_graphql::Error::new(format!(
                        "Invalid status of plan {:?}",
                        plan.status
                    )));
                }
                let ports_to_apply = PortUsage::query()
                    .filter(schema::port_usage::plan_id.eq(plan_id))
                    .load(conn)
                    .await?;
                for PortUsage {
                    port_id,
                    plan_id: _,
                    side,
                    cable,
                    fiber,
                    bundle,
                } in ports_to_apply
                {
                    if let (Some(cable), Some(bundle), Some(fiber)) = (cable, bundle, fiber) {
                        let usage = PortUsage {
                            port_id,
                            plan_id: 0,
                            side,
                            cable: Some(cable),
                            fiber: Some(fiber),
                            bundle: Some(bundle),
                        };
                        diesel::insert_into(schema::port_usage::dsl::port_usage::table())
                            .values(&usage)
                            .on_conflict((
                                schema::port_usage::port_id,
                                schema::port_usage::plan_id,
                                schema::port_usage::side,
                            ))
                            .do_update()
                            .set((
                                schema::port_usage::cable.eq(cable),
                                schema::port_usage::fiber.eq(fiber),
                                schema::port_usage::bundle.eq(bundle),
                            ))
                            .execute(conn)
                            .await?;
                    } else {
                        diesel::delete(schema::port_usage::dsl::port_usage::table())
                            .filter(schema::port_usage::port_id.eq(port_id))
                            .filter(schema::port_usage::plan_id.eq(0))
                            .filter(schema::port_usage::side.eq(side))
                            .execute(conn)
                            .await?;
                    }
                }
                plan.status = PlanStatusType::Implemented;
                diesel::update(&plan).set(&plan).execute(conn).await?;
                Ok(plan)
            })
            .await
    }
    async fn sync_plan_to_netbox(
        &self,
        ctx: &Context<'_>,
        plan_id: i32,
    ) -> async_graphql::Result<Vec<SyncIssue>> {
        let mut connection = authenticated::get_connection(ctx).await?;

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
                        if trace.is_empty() {
                            continue;
                        }
                        let last_node = trace.last().unwrap();
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
                                    .push((remote_port, port,trace_length));
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
                let mut planned_circuits = Vec::new();

                while !port_pairs.is_empty() {
                    if let Some((start_netbox_id, r1)) = port_pairs
                        .keys()
                        .next()
                        .copied()
                        .and_then(|k| port_pairs.remove(&k).map(|e| (k, e)))
                    {
                        if r1.len() > 1 {
                            issues.push(create_asymetric_duplex_error(start_netbox_id, r1).await?)
                        } else if let Some((end_netbox_id, connections)) = r1.into_iter().next() {
                            if let Some(r2) = port_pairs.remove(&end_netbox_id) {
                                if r2.len() > 1 {
                                    issues.push(
                                        create_asymetric_duplex_error(end_netbox_id, r2).await?,
                                    )
                                }
                                let distance = connections.iter().map(|(_,_,d)|*d).sum::<f64>()/connections.len() as f64;
                                planned_circuits.push(
                                    PlannedCircuit {
                                        start_netbox_id,
                                        end_netbox_id,
                                        members: connections
                                            .into_iter()
                                            .map(|(start_port, end_port, _)| CircuitMember {
                                                start_port,
                                                end_port,
                                            })
                                            .collect(),
                                        distance,
                                    }
                                    .order(),
                                );
                            } else {
                                panic!("Not possible, I swear :-)")
                            }
                        }
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
                let rest_base_url = format!("{}api",NETBOX_CONFIG
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
                    let new_circuit_id = created_circuit["id"].as_i64().unwrap();

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
                        let term_id = created_term["id"].as_i64().unwrap();

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
                    }                }
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
#[derive(Debug, Clone, PartialEq, InputObject, Copy)]
struct PortUsageInput {
    port_id: i32,
    side: PortSide,
    fiber: PortUsageUpdateAction,
}
#[derive(Debug, Clone, PartialEq, OneofObject, Copy)]
enum PortUsageUpdateAction {
    Remove(bool),
    Reset(bool),
    Attach(FiberKeyInput),
}

#[derive(Debug, Clone, PartialEq, InputObject, Copy)]
pub struct FiberKeyInput {
    cable_id: i32,
    bundle: i32,
    fiber: i32,
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct CreatePanel {
    pub name: Option<String>,
    pub schacht_id: i32,
    pub children: Box<[CreatePanel]>,
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct CreatePort {
    pub label: Option<String>,
    pub port_type: PanelPortType,
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct PanelUpdate {
    panel_id: i32,
    name: Option<PanelUpdateSetName>,
    order: Option<PanelUpdateSetOrder>,
    parent: Option<PanelUpdateSetParent>,
    netbox_device_id: Option<PanelUpdateSetNetboxDeviceId>,
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct PanelUpdateSetName {
    value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct PanelUpdateSetOrder {
    order: i32,
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct PanelUpdateSetParent {
    parent: Option<i32>,
}
#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct PanelUpdateSetNetboxDeviceId {
    device_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct CreatePlan {
    pub name: String,
}

#[derive(InputObject)]
struct UpdateCableStructure {
    bundle_count: u32,
    fiber_count: u32,
}

#[derive(AsChangeset)]
#[diesel(table_name = schema::panel)]
struct UpdatePanelChangeset {
    name: Option<Option<String>>,
    parent_panel: Option<Option<i32>>,
    parent_order: Option<Option<i32>>,
    netbox_device_id: Option<Option<i32>>,
}

#[async_recursion]
async fn insert_panel_tree_recursive(
    conn: &mut AsyncPgConnection,
    node: CreatePanel,
    parent_id: Option<i32>,
    parent_order_val: Option<i32>,
) -> async_graphql::Result<()> {
    // 1. Das aktuelle Panel speichern
    let new_panel = InsertPanel {
        name: node.name.clone(),
        schacht_id: node.schacht_id,
        parent_panel: parent_id,
        parent_order: parent_order_val,
    };

    let inserted_panel_id: i32 = diesel::insert_into(schema::panel::table)
        .values(new_panel)
        .returning(schema::panel::id)
        .get_result(conn)
        .await?;

    // 2. Rekursiv alle Kinder dieses Panels speichern
    for (index, child) in node.children.into_iter().enumerate() {
        insert_panel_tree_recursive(
            conn,
            child,
            Some(inserted_panel_id),
            Some((index + 1) as i32), // parent_order startet bei 1
        )
        .await?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct IdOrNewInput {
    pub id: Option<i32>,
    pub temporary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct FlatPanelInput {
    pub id: IdOrNewInput,
    pub name: Option<String>,
    pub netbox_device_id: Option<i32>,
    pub parent_id: Option<IdOrNewInput>,
    pub order: i32,
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct FlatPortInput {
    pub id: IdOrNewInput,
    pub order: i32,
    pub label: String,
    pub port_type: PanelPortType,
    pub netbox_port_id: Option<i32>,
}
