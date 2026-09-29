//! Cables.

use crate::graphql::error::ApiResult;
use crate::{
    db::{
        entity::cable::{Cable, UpdateCableChangeset},
        schema,
    },
    graphql::authenticated,
    graphql::authorization::{Role, RoleGuard},
};
use async_graphql::{Context, InputObject, Object};
use cable_editor_common::{UserError, error::PlanPorts};
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, dsl::count_star};
use diesel_async::RunQueryDsl;

#[derive(Default)]
pub struct CableMutation;

#[Object]
impl CableMutation {
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn create_cable(&self, ctx: &Context<'_>, name: String) -> ApiResult<Cable> {
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
    ) -> ApiResult<Option<Cable>> {
        if path.as_ref().is_some_and(Vec::is_empty) {
            return Err(UserError::CableWithoutSegment.into());
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

        if let Some(path_ids) = path {
            diesel::delete(
                schema::kabel_trasse::table.filter(schema::kabel_trasse::kabel.eq(cable_id)),
            )
            .execute(&mut connection)
            .await?;

            for (sequenz, trasse_id) in path_ids.into_iter().enumerate() {
                diesel::insert_into(schema::kabel_trasse::table)
                    .values((
                        schema::kabel_trasse::kabel.eq(cable_id),
                        schema::kabel_trasse::trasse.eq(trasse_id),
                        schema::kabel_trasse::sequenz.eq(sequenz as i32),
                    ))
                    .execute(&mut connection)
                    .await?;
            }
        }

        Ok(if changeset.any() {
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
        })
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
                .map(|(plan, ports)| PlanPorts { plan, ports })
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

#[derive(InputObject)]
struct UpdateCableStructure {
    bundle_count: u32,
    fiber_count: u32,
}
