//! Cables.

use crate::graphql::error::ApiResult;
use crate::{
    db::{
        entity::{
            cable::{Cable, UpdateCableChangeset},
            port_usage_issue::{IssueCheck, IssueScope},
        },
        schema,
    },
    graphql::authenticated,
    graphql::authorization::{Role, RoleGuard},
};
use async_graphql::{Context, InputObject, Object};
use cable_editor_common::{UserError, error::PlanPorts};
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, dsl::count_star};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

#[derive(Default)]
pub struct CableMutation;

#[Object]
impl CableMutation {
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn create_cable(
        &self,
        ctx: &Context<'_>,
        name: String,
        fibers: CableStructureInput,
        path: Vec<i32>,
    ) -> ApiResult<Cable> {
        if path.is_empty() {
            return Err(UserError::CableWithoutSegment.into());
        }
        let mut connection = authenticated::get_connection(ctx).await?;
        let cable = diesel::insert_into(schema::kabel::table)
            .values((
                schema::kabel::name.eq(name),
                schema::kabel::buendel_anz.eq(fibers.bundle_count as i32),
                schema::kabel::faser_anz.eq(fibers.fiber_count as i32),
            ))
            .get_result::<Cable>(&mut connection)
            .await?;
        set_path(&mut connection, cable.id, path).await?;
        Ok(cable)
    }
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn update_cable(
        &self,
        ctx: &Context<'_>,
        cable_id: i32,
        name: Option<String>,
        fibers: Option<CableStructureInput>,
        path: Option<Vec<i32>>,
    ) -> ApiResult<Option<Cable>> {
        if path.as_ref().is_some_and(Vec::is_empty) {
            return Err(UserError::CableWithoutSegment.into());
        }
        let mut connection = authenticated::get_connection(ctx).await?;
        let (buendel_anz, faser_anz) = if let Some(CableStructureInput {
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
        // Its path and fibers must still fit the ports its fibers are attached to
        let check = if path.is_some() || changeset.buendel_anz.is_some() {
            Some(IssueCheck::before(&mut connection, IssueScope::Cable(cable_id)).await?)
        } else {
            None
        };

        if let Some(path) = path {
            set_path(&mut connection, cable_id, path).await?;
        }

        let cable = if changeset.any() {
            diesel::update(schema::kabel::table.find(cable_id))
                .set(&changeset)
                .get_result::<Cable>(&mut connection)
                .await
                .optional()?
        } else {
            schema::kabel::table
                .find(cable_id)
                .first::<Cable>(&mut connection)
                .await
                .optional()?
        };
        if let Some(check) = check {
            check.after(&mut connection).await?;
        }
        Ok(cable)
    }
    /// Refused while its fibers are attached to ports, in the current state or in a plan.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn delete_cable(&self, ctx: &Context<'_>, cable_id: i32) -> ApiResult<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let usages: Vec<(String, i64)> = schema::port_usage::table
            .inner_join(schema::plan::table)
            .filter(schema::port_usage::cable.eq(cable_id))
            .group_by(schema::plan::name)
            .select((schema::plan::name, count_star()))
            .order_by(schema::plan::name)
            .load(&mut connection)
            .await?;
        if !usages.is_empty() {
            let plans = usages
                .into_iter()
                .map(|(plan, ports)| PlanPorts {
                    plan: plan.into(),
                    ports,
                })
                .collect();
            return Err(UserError::CableAttached { plans }.into());
        }
        diesel::delete(
            schema::kabel_trasse::table.filter(schema::kabel_trasse::kabel.eq(cable_id)),
        )
        .execute(&mut connection)
        .await?;
        diesel::delete(schema::kabel::table.filter(schema::kabel::id.eq(cable_id)))
            .execute(&mut connection)
            .await?;
        Ok(true)
    }
}

/// Replaces the ducts a cable runs through, in the order given.
async fn set_path(
    connection: &mut AsyncPgConnection,
    cable_id: i32,
    path: Vec<i32>,
) -> ApiResult<()> {
    diesel::delete(schema::kabel_trasse::table.filter(schema::kabel_trasse::kabel.eq(cable_id)))
        .execute(connection)
        .await?;
    for (sequenz, trasse_id) in path.into_iter().enumerate() {
        diesel::insert_into(schema::kabel_trasse::table)
            .values((
                schema::kabel_trasse::kabel.eq(cable_id),
                schema::kabel_trasse::trasse.eq(trasse_id),
                schema::kabel_trasse::sequenz.eq(sequenz as i32),
            ))
            .execute(connection)
            .await?;
    }
    Ok(())
}

#[derive(InputObject)]
struct CableStructureInput {
    bundle_count: u32,
    fiber_count: u32,
}
