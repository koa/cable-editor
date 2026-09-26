//! Plans: creating, renaming, implementing (implement.rs) and syncing them to NetBox (sync.rs).

use crate::{
    db::{
        entity::plan::{InsertPlan, Plan},
        schema,
    },
    graphql::authenticated::{
        self,
        mutation::sync::{SyncIssue, sync_plan_to_netbox},
    },
    graphql::authorization::{Role, RoleGuard},
};
use async_graphql::{Context, InputObject, Object};
use diesel::{ExpressionMethods, HasQuery, QueryDsl};
use diesel_async::{AsyncConnection, RunQueryDsl};

#[derive(Default)]
pub struct PlanMutation;

#[Object]
impl PlanMutation {
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
        super::implement::implement_plan(plan_id, authenticated::get_connection(ctx).await?).await
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

#[derive(Debug, Clone, PartialEq, InputObject)]
pub struct CreatePlan {
    pub name: String,
}
