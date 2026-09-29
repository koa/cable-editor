//! Panels, their ports and the ports' usage by fibres.

use crate::db::entity::plan::BASELINE_PLAN_ID;
use crate::graphql::error::ApiResult;
use crate::{
    db::{
        entity::panel::{InsertPanel, InsertPanelPort, PanelPortType, PortSide, PortUsage},
        schema,
    },
    graphql::authenticated,
    graphql::authorization::{Role, RoleGuard},
};
use async_graphql::{Context, InputObject, Object, OneofObject};
use async_recursion::async_recursion;
use cable_editor_common::UserError;
use diesel::{
    AsChangeset, BoolExpressionMethods, ExpressionMethods, QueryDsl, associations::HasTable,
    dsl::max,
};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use std::collections::HashMap;

#[derive(Default)]
pub struct PanelMutation;

#[Object]
impl PanelMutation {
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn create_panel(
        &self,
        ctx: &Context<'_>,
        panel: CreatePanel,
        parent_panel: Option<i32>,
    ) -> ApiResult<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let conn: &mut AsyncPgConnection = &mut connection;
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
    }
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn update_panels(&self, ctx: &Context<'_>, updates: Vec<PanelUpdate>) -> ApiResult<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let conn: &mut AsyncPgConnection = &mut connection;
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
    }
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn update_cabinet_panels(
        &self,
        ctx: &Context<'_>,
        cabinet_id: i32,
        changes: Vec<FlatPanelInput>,
        deletes: Vec<i32>,
    ) -> ApiResult<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;

        let conn: &mut AsyncPgConnection = &mut connection;
        // 1. The deletions first
        if !deletes.is_empty() {
            diesel::delete(schema::panel::table.filter(schema::panel::id.eq_any(&deletes)))
                .execute(conn)
                .await?;
        }

        // The temporary ids of the frontend and the database ids they got
        let mut temp_id_map: HashMap<String, i32> = HashMap::new();

        // 2. Creations and updates (the frontend sends them in the right order)
        for change in changes {
            // The parent's id: stored, or from the map of temporary ids
            let resolved_parent_id = match change.parent_id {
                Some(p_id) => {
                    if let Some(id) = p_id.id {
                        Some(id)
                    } else if let Some(temp) = p_id.temporary {
                        // The frontend sends parents before their children
                        Some(*temp_id_map.get(&temp).ok_or(UserError::InvalidRequest)?)
                    } else {
                        None
                    }
                }
                None => None,
            };

            if let Some(panel_id) = change.id.id {
                // UPDATE: a stored panel
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
                // CREATE: a new panel
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

                // The new id, for its children that follow
                temp_id_map.insert(temp_id, inserted_id);
            } else {
                // Either id or temporary id
                return Err(UserError::InvalidRequest.into());
            }
        }

        Ok(true)
    }
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn update_panel_ports(
        &self,
        ctx: &Context<'_>,
        panel_id: i32,
        changes: Vec<FlatPortInput>,
        deletes: Vec<i32>,
    ) -> ApiResult<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;

        let conn: &mut AsyncPgConnection = &mut connection;
        // 1. The deletions first
        if !deletes.is_empty() {
            diesel::delete(
                schema::panel_port::table.filter(schema::panel_port::id.eq_any(&deletes)),
            )
            .execute(conn)
            .await?;
        }

        // 2. Creations and updates
        for change in changes {
            // An empty label from the UI is NULL
            let label_opt = if change.label.trim().is_empty() {
                None
            } else {
                Some(change.label)
            };

            if let Some(port_id) = change.id.id {
                // UPDATE: a stored port
                // Only a port of this panel, so no other panel's ports can be changed
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
                // CREATE: a new port
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
                // Either id or temporary id
                return Err(UserError::InvalidRequest.into());
            }
        }

        Ok(true)
    }
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn set_port_usage(
        &self,
        ctx: &Context<'_>,
        plan_id: i32,
        changes: Vec<PortUsageInput>,
    ) -> ApiResult<bool> {
        if plan_id == BASELINE_PLAN_ID {
            return Err(UserError::BaselineUnchangeable.into());
        }
        let mut connection = authenticated::get_connection(ctx).await?;
        let conn: &mut AsyncPgConnection = &mut connection;
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

        Ok(true)
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
) -> ApiResult<()> {
    // 1. The panel itself
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

    // 2. Its children, recursively
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
