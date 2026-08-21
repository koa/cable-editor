use crate::{
    db::{
        entity::{
            cable::{Cable, UpdateCableChangeset},
            panel::{InsertPanel, InsertPanelPort, PanelPortType, PortSide, PortUsage},
            plan::InsertPlan,
        },
        schema::{self, kabel, kabel_trasse, panel, panel_port, plan, port_usage::dsl::port_usage},
    },
    graphql::authenticated,
};
use async_graphql::{Context, InputObject, Object};
use async_recursion::async_recursion;
use diesel::{
    AsChangeset, BoolExpressionMethods, ExpressionMethods, OptionalExtension, QueryDsl,
    associations::HasTable, dsl::max,
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use std::collections::HashMap;

pub struct Mutation;

#[Object]
impl Mutation {
    async fn create_cable(&self, ctx: &Context<'_>, name: String) -> async_graphql::Result<Cable> {
        let mut connection = authenticated::get_connection(ctx).await?;
        Ok(diesel::insert_into(kabel::table)
            .values((
                kabel::name.eq(name),
                kabel::buendel_anz.eq(1),
                kabel::faser_anz.eq(12),
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
                    diesel::delete(kabel_trasse::table.filter(kabel_trasse::kabel.eq(cable_id)))
                        .execute(conn)
                        .await?;

                    for (sequenz, &trasse_id) in path_ids.iter().enumerate() {
                        diesel::insert_into(kabel_trasse::table)
                            .values((
                                kabel_trasse::kabel.eq(cable_id),
                                kabel_trasse::trasse.eq(trasse_id),
                                kabel_trasse::sequenz.eq(sequenz as i32),
                            ))
                            .execute(conn)
                            .await?;
                    }
                }

                let updated = if changeset.any() {
                    diesel::update(kabel::table.find(cable_id))
                        .set(&changeset)
                        .get_result::<Cable>(conn)
                        .await
                        .optional()?
                } else {
                    kabel::table
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
                diesel::delete(kabel_trasse::table.filter(kabel_trasse::kabel.eq(cable_id)))
                    .execute(conn)
                    .await?;
                diesel::delete(kabel::table.filter(kabel::id.eq(cable_id)))
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
                    let max_order: Option<i32> = panel::table
                        .filter(panel::parent_panel.eq(parent_id))
                        .select(max(panel::parent_order))
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
                } in updates
                {
                    diesel::update(panel::table)
                        .filter(panel::id.eq(panel_id))
                        .set(UpdatePanelChangeset {
                            name: name.map(|n| n.value),
                            parent_panel: order.map(|o| Some(o.order)),
                            parent_order: parent.map(|p| p.parent),
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
        diesel::insert_into(plan::table)
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
                    diesel::delete(panel::table.filter(panel::id.eq_any(&deletes)))
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
                        diesel::update(panel::table.find(panel_id))
                            .set((
                                panel::name.eq(change.name),
                                panel::parent_panel.eq(resolved_parent_id),
                                panel::parent_order.eq(change.order),
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

                        let inserted_id: i32 = diesel::insert_into(panel::table)
                            .values(new_panel)
                            .returning(panel::id)
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
                    diesel::delete(panel_port::table.filter(panel_port::id.eq_any(&deletes)))
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
                            panel_port::table.filter(
                                panel_port::id
                                    .eq(port_id)
                                    .and(panel_port::panel_id.eq(panel_id)),
                            ),
                        )
                        .set((
                            panel_port::port_order.eq(change.order),
                            panel_port::label.eq(label_opt),
                            panel_port::port_type.eq(change.port_type),
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
                        };

                        diesel::insert_into(panel_port::table)
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
        let mut connection = authenticated::get_connection(ctx).await?;
        connection
            .transaction(async move |conn| {
                for PortUsageInput {
                    port_id,
                    side,
                    fiber,
                } in changes
                {
                    let usage = PortUsage {
                        port_id,
                        plan_id,
                        side,
                        cable: fiber.map(|f| f.cable_id),
                        fiber: fiber.map(|f| f.fiber),
                        bundle: fiber.map(|f| f.bundle),
                    };

                    diesel::insert_into(port_usage::table())
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

                Ok::<bool, async_graphql::Error>(true)
            })
            .await
    }
}

#[derive(Debug, Clone, PartialEq, InputObject, Copy)]
struct PortUsageInput {
    port_id: i32,
    side: PortSide,
    fiber: Option<FiberKeyInput>,
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
pub struct CreatePlan {
    pub name: String,
}

#[derive(InputObject)]
struct UpdateCableStructure {
    bundle_count: u32,
    fiber_count: u32,
}

#[derive(AsChangeset)]
#[diesel(table_name = panel)]
struct UpdatePanelChangeset {
    name: Option<Option<String>>,
    parent_panel: Option<Option<i32>>,
    parent_order: Option<Option<i32>>,
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

    let inserted_panel_id: i32 = diesel::insert_into(panel::table)
        .values(new_panel)
        .returning(panel::id)
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
    pub parent_id: Option<IdOrNewInput>,
    pub order: i32,
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct FlatPortInput {
    pub id: IdOrNewInput,
    pub order: i32,
    pub label: String,
    pub port_type: PanelPortType,
}
