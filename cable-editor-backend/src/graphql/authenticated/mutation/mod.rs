pub mod implement;
pub mod sync;

use crate::db::entity::plan::BASELINE_PLAN_ID;
use crate::{
    db::{
        entity::{
            Duct,
            cable::{Cable, UpdateCableChangeset},
            panel::{InsertPanel, InsertPanelPort, PanelPort, PanelPortType, PortSide, PortUsage},
            plan::{InsertPlan, Plan},
            schacht::Schacht,
        },
        schema,
    },
    graphql::authenticated::{
        self,
        mutation::sync::{
            AsymetricTargetConnectionEntry, AsymmetricDuplexError, PortPair, SyncIssue,
            sync_plan_to_netbox,
        },
    },
    graphql::authorization::{Role, RoleGuard},
    graphql::duct_line::{self, LineInput},
    graphql::geo::{self, PositionInput},
    netbox::fetch::RearPort,
};
use async_graphql::{Context, InputObject, Object, OneofObject};
use async_recursion::async_recursion;
use diesel::{
    AsChangeset, BoolExpressionMethods, ExpressionMethods, HasQuery, OptionalExtension, QueryDsl,
    SelectableHelper, associations::HasTable, dsl::max,
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use postgis_diesel::types::{GeometryContainer, LineString, Point};
use std::collections::HashMap;

pub struct Mutation;

#[Object]
impl Mutation {
    #[graphql(guard = "RoleGuard(Role::Planner)")]
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
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn update_cable(
        &self,
        ctx: &Context<'_>,
        cable_id: i32,
        name: Option<String>,
        fibers: Option<UpdateCableStructure>,
        path: Option<Vec<i32>>,
    ) -> async_graphql::Result<Option<Cable>> {
        if path.as_ref().is_some_and(Vec::is_empty) {
            return Err("Ein Kabel braucht mindestens ein Segment".into());
        }
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
    #[graphql(guard = "RoleGuard(Role::Admin)")]
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
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn create_schacht(
        &self,
        ctx: &Context<'_>,
        schacht: SchachtInput,
    ) -> async_graphql::Result<Schacht> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let (name, typ, geom) = schacht.values(&mut connection).await?;
        Ok(diesel::insert_into(schema::schacht::table)
            .values((
                schema::schacht::name.eq(name),
                schema::schacht::typ.eq(typ),
                schema::schacht::geom.eq(geom),
            ))
            .returning(Schacht::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// Replaces name, type and position of the Schacht; its ducts follow the position (their
    /// lines start and end at their Schächte, see the view trassen_mit_endpunkten).
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn update_schacht(
        &self,
        ctx: &Context<'_>,
        schacht_id: i32,
        schacht: SchachtInput,
    ) -> async_graphql::Result<Schacht> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let (name, typ, geom) = schacht.values(&mut connection).await?;
        Ok(diesel::update(schema::schacht::table.find(schacht_id))
            .set((
                schema::schacht::name.eq(name),
                schema::schacht::typ.eq(typ),
                schema::schacht::geom.eq(geom),
            ))
            .returning(Schacht::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// Only a Schacht without panels and ducts.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn delete_schacht(
        &self,
        ctx: &Context<'_>,
        schacht_id: i32,
    ) -> async_graphql::Result<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let panels: i64 = schema::panel::table
            .filter(schema::panel::schacht_id.eq(schacht_id))
            .count()
            .get_result(&mut connection)
            .await?;
        let ducts: i64 = schema::trasse::table
            .filter(
                schema::trasse::schacht_a
                    .eq(schacht_id)
                    .or(schema::trasse::schacht_z.eq(schacht_id)),
            )
            .count()
            .get_result(&mut connection)
            .await?;
        if panels > 0 || ducts > 0 {
            return Err(format!("Der Schacht hat noch {panels} Panels und {ducts} Trassen").into());
        }
        let deleted = diesel::delete(schema::schacht::table.find(schacht_id))
            .execute(&mut connection)
            .await?;
        Ok(deleted > 0)
    }
    /// A duct between two Schächte, with the course from a file or straight.
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn create_duct(
        &self,
        ctx: &Context<'_>,
        duct: DuctInput,
        line: Option<LineInput>,
        #[graphql(default)] confirmed: bool,
    ) -> async_graphql::Result<Duct> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let description = duct.checked_description()?;
        let geom = match &line {
            Some(line) => {
                checked_line(
                    &mut connection,
                    duct.schacht_a,
                    duct.schacht_z,
                    line,
                    confirmed,
                )
                .await?
            }
            None => None,
        };
        Ok(diesel::insert_into(schema::trasse::table)
            .values((
                schema::trasse::schacht_a.eq(duct.schacht_a),
                schema::trasse::schacht_z.eq(duct.schacht_z),
                schema::trasse::description.eq(description),
                schema::trasse::geom.eq(geom),
            ))
            .returning(Duct::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// Description and Schächte; the Schächte only of a duct without cables. The course is
    /// turned (and trimmed) to fit new Schächte.
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn update_duct(
        &self,
        ctx: &Context<'_>,
        duct_id: i32,
        duct: DuctInput,
    ) -> async_graphql::Result<Duct> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let description = duct.checked_description()?;
        let stored: Duct = Duct::query()
            .filter(schema::trasse::id.eq(duct_id))
            .first(&mut connection)
            .await?;
        let mut geom = stored_line_of(&stored);
        if (stored.schacht_a, stored.schacht_z) != (duct.schacht_a, duct.schacht_z) {
            if duct_cable_count(&mut connection, duct_id).await? > 0 {
                return Err(
                    "Die Trasse enthält Kabel, ihre Schächte können nicht geändert werden".into(),
                );
            }
            if let Some(line) = geom {
                let a = duct_line::schacht_position(&mut connection, duct.schacht_a).await?;
                let z = duct_line::schacht_position(&mut connection, duct.schacht_z).await?;
                geom = duct_line::stored_line(&duct_line::fit(line.points, &a, &z)?);
            }
        }
        Ok(diesel::update(schema::trasse::table.find(duct_id))
            .set((
                schema::trasse::schacht_a.eq(duct.schacht_a),
                schema::trasse::schacht_z.eq(duct.schacht_z),
                schema::trasse::description.eq(description),
                schema::trasse::geom.eq(geom),
            ))
            .returning(Duct::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// The course from a file (see `checkDuctLine`), or none: a straight line. An end further
    /// than 10 m from its Schacht needs `confirmed`.
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn set_duct_line(
        &self,
        ctx: &Context<'_>,
        duct_id: i32,
        line: Option<LineInput>,
        #[graphql(default)] confirmed: bool,
    ) -> async_graphql::Result<Duct> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let stored: Duct = Duct::query()
            .filter(schema::trasse::id.eq(duct_id))
            .first(&mut connection)
            .await?;
        let geom = match &line {
            Some(line) => {
                checked_line(
                    &mut connection,
                    stored.schacht_a,
                    stored.schacht_z,
                    line,
                    confirmed,
                )
                .await?
            }
            None => None,
        };
        Ok(diesel::update(schema::trasse::table.find(duct_id))
            .set(schema::trasse::geom.eq(geom))
            .returning(Duct::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// Only a duct without cables.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn delete_duct(&self, ctx: &Context<'_>, duct_id: i32) -> async_graphql::Result<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let cables = duct_cable_count(&mut connection, duct_id).await?;
        if cables > 0 {
            return Err(format!("Durch die Trasse führen noch {cables} Kabel").into());
        }
        let deleted = diesel::delete(schema::trasse::table.find(duct_id))
            .execute(&mut connection)
            .await?;
        Ok(deleted > 0)
    }
    #[graphql(guard = "RoleGuard(Role::Planner)")]
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
    #[graphql(guard = "RoleGuard(Role::Planner)")]
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
    #[graphql(guard = "RoleGuard(Role::Planner)")]
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
    #[graphql(guard = "RoleGuard(Role::Planner)")]
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
    #[graphql(guard = "RoleGuard(Role::Planner)")]
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
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn set_port_usage(
        &self,
        ctx: &Context<'_>,
        plan_id: i32,
        changes: Vec<PortUsageInput>,
    ) -> async_graphql::Result<bool> {
        if plan_id == BASELINE_PLAN_ID {
            return Err("The baseline can only be changed by implementing a plan".into());
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
    #[graphql(guard = "RoleGuard(Role::Planner)")]
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
                if plan.is_baseline() {
                    return Err("The baseline can't be renamed".into());
                }
                plan.name = name;
                diesel::update(&plan).set(&plan).execute(conn).await?;
                Ok(plan)
            })
            .await
    }
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn implement_plan(&self, ctx: &Context<'_>, plan_id: i32) -> async_graphql::Result<Plan> {
        implement::implement_plan(plan_id, authenticated::get_connection(ctx).await?).await
    }

    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn sync_plan_to_netbox(
        &self,
        ctx: &Context<'_>,
        plan_id: i32,
    ) -> async_graphql::Result<Vec<SyncIssue>> {
        sync_plan_to_netbox(plan_id, authenticated::get_connection(ctx).await?).await
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
/// The Schächte and description of a duct.
#[derive(Debug, Clone, PartialEq, InputObject)]
struct DuctInput {
    schacht_a: i32,
    schacht_z: i32,
    description: Option<String>,
}

impl DuctInput {
    /// Trimmed, `None` if empty; the Schächte must differ.
    fn checked_description(&self) -> async_graphql::Result<Option<String>> {
        if self.schacht_a == self.schacht_z {
            return Err("Anfangs- und Endschacht müssen verschieden sein".into());
        }
        let description = self
            .description
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty());
        // varchar(50)
        if description.is_some_and(|d| d.chars().count() > 50) {
            return Err("Die Beschreibung darf höchstens 50 Zeichen lang sein".into());
        }
        Ok(description.map(str::to_string))
    }
}

/// The line as stored, fitted to the Schächte; refused if an end is far from its Schacht and
/// not confirmed.
async fn checked_line(
    connection: &mut AsyncPgConnection,
    schacht_a: i32,
    schacht_z: i32,
    line: &LineInput,
    confirmed: bool,
) -> async_graphql::Result<Option<LineString<Point>>> {
    let (fitted, _, _) = duct_line::fit_line(connection, schacht_a, schacht_z, line).await?;
    if fitted.needs_confirmation() && !confirmed {
        return Err(format!(
            "Der Verlauf endet {:.1} m bzw. {:.1} m von den Schächten entfernt, bitte bestätigen",
            fitted.start_distance, fitted.end_distance
        )
        .into());
    }
    Ok(duct_line::stored_line(&fitted))
}

/// The stored course of the duct as line.
fn stored_line_of(duct: &Duct) -> Option<LineString<Point>> {
    match &duct.geom {
        Some(GeometryContainer::LineString(line)) => Some(line.clone()),
        _ => None,
    }
}

async fn duct_cable_count(
    connection: &mut AsyncPgConnection,
    duct_id: i32,
) -> async_graphql::Result<i64> {
    Ok(schema::kabel_trasse::table
        .filter(schema::kabel_trasse::trasse.eq(duct_id))
        .count()
        .get_result(connection)
        .await?)
}

/// Name, type and position of a Schacht.
#[derive(Debug, Clone, PartialEq, InputObject)]
struct SchachtInput {
    name: String,
    type_id: Option<i32>,
    /// Missing: the Schacht has no position
    position: Option<PositionInput>,
}

impl SchachtInput {
    /// The column values: the name checked, the position in LV95.
    async fn values(
        self,
        connection: &mut AsyncPgConnection,
    ) -> async_graphql::Result<(String, Option<i32>, Option<Point>)> {
        let name = self.name.trim().to_string();
        if name.is_empty() {
            return Err("Der Schacht braucht einen Namen".into());
        }
        // varchar(20)
        if name.chars().count() > 20 {
            return Err("Der Name darf höchstens 20 Zeichen lang sein".into());
        }
        let geom = match self.position {
            Some(position) => Some(geo::to_lv95(connection, position).await?),
            None => None,
        };
        Ok((name, self.type_id, geom))
    }
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
