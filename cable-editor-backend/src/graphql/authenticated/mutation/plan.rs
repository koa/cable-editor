//! Plans: creating, renaming, implementing (implement.rs) and choosing the one Netbox shows
//! (synced by the backend's worker, `netbox::auto_sync`).

use crate::graphql::error::ApiResult;
use crate::{
    db::{
        entity::plan::{InsertPlan, Plan},
        schema,
    },
    graphql::authenticated,
    graphql::authorization::{Role, RoleGuard},
};
use async_graphql::{Context, InputObject, Object};
use cable_editor_common::{ObjectKind, UserError};
use chrono::{DateTime, Utc};
use diesel::{ExpressionMethods, HasQuery, OptionalExtension, QueryDsl};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

#[derive(Default)]
pub struct PlanMutation;

#[Object]
impl PlanMutation {
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn create_plan(&self, ctx: &Context<'_>, plan: CreatePlan) -> ApiResult<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let new_plan = InsertPlan { name: plan.name };
        diesel::insert_into(schema::plan::table)
            .values(new_plan)
            .execute(&mut connection)
            .await?;
        Ok(true)
    }
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn update_plan(&self, ctx: &Context<'_>, plan_id: i32, name: String) -> ApiResult<Plan> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let conn: &mut AsyncPgConnection = &mut connection;
        let mut plan = Plan::query()
            .for_update()
            .filter(schema::plan::id.eq(plan_id))
            .first(conn)
            .await?;
        if plan.is_baseline() {
            return Err(UserError::BaselineUnchangeable.into());
        }
        plan.name = name;
        diesel::update(&plan).set(&plan).execute(conn).await?;
        Ok(plan)
    }
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn implement_plan(&self, ctx: &Context<'_>, plan_id: i32) -> ApiResult<Plan> {
        super::implement::implement_plan(plan_id, authenticated::get_connection(ctx).await?).await
    }
    /// Syncs Netbox right away (the worker runs it after the request), e.g. after a change made
    /// in Netbox by hand, also without waiting after a failed run.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn sync_netbox(&self, ctx: &Context<'_>) -> ApiResult<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;
        diesel::insert_into(schema::netbox_sync_anstoss::table)
            .default_values()
            .execute(&mut connection)
            .await?;
        // Also after a failure, e.g. once Netbox is reachable again
        diesel::update(schema::netbox_sync::table)
            .set(schema::netbox_sync::naechster_versuch.eq(None::<DateTime<Utc>>))
            .execute(&mut connection)
            .await?;
        Ok(true)
    }
    /// Netbox shows the circuits of this plan from now on, synced automatically (see
    /// docs/netbox-sync.md); the plan active so far no longer.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn set_netbox_active_plan(&self, ctx: &Context<'_>, plan_id: i32) -> ApiResult<Plan> {
        let mut connection = authenticated::get_connection(ctx).await?;
        // At most one plan is active (unique index), so the other one first
        diesel::update(schema::plan::table)
            .filter(schema::plan::netbox_active)
            .filter(schema::plan::id.ne(plan_id))
            .set(schema::plan::netbox_active.eq(false))
            .execute(&mut connection)
            .await?;
        diesel::update(schema::plan::table.find(plan_id))
            .set(schema::plan::netbox_active.eq(true))
            .get_result(&mut connection)
            .await
            .optional()?
            .ok_or_else(|| {
                UserError::NotFound {
                    kind: ObjectKind::Plan,
                    id: plan_id.into(),
                }
                .into()
            })
    }
}

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct CreatePlan {
    pub name: String,
}
