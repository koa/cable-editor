use crate::db::{
    entity::{
        panel::PortUsage,
        plan::{BASELINE_PLAN_ID, Plan},
    },
    schema,
};
use crate::graphql::error::ApiResult;
use cable_editor_common::UserError;
use diesel::{ExpressionMethods, HasQuery, QueryDsl, associations::HasTable};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

pub async fn implement_plan(plan_id: i32, conn: &mut AsyncPgConnection) -> ApiResult<Plan> {
    let plan = Plan::query()
        .for_update()
        .filter(schema::plan::id.eq(plan_id))
        .first(conn)
        .await?;
    if plan.is_baseline() {
        return Err(UserError::BaselineUnchangeable.into());
    }
    let ports_to_apply = PortUsage::query()
        .filter(schema::port_usage::plan_id.eq(plan_id))
        .load(conn)
        .await?;

    let mut usages_to_add = Vec::new();
    let mut usages_to_remove = Vec::new();

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
                plan_id: BASELINE_PLAN_ID,
                side,
                cable: Some(cable),
                fiber: Some(fiber),
                bundle: Some(bundle),
            };
            usages_to_add.push((cable, fiber, bundle, usage));
        } else {
            usages_to_remove.push((port_id, side));
        }
    }
    for (port_id, side) in usages_to_remove {
        diesel::delete(schema::port_usage::dsl::port_usage::table())
            .filter(schema::port_usage::port_id.eq(port_id))
            .filter(schema::port_usage::plan_id.eq(BASELINE_PLAN_ID))
            .filter(schema::port_usage::side.eq(side))
            .execute(conn)
            .await?;
    }
    for (cable, fiber, bundle, usage) in usages_to_add {
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
    }
    diesel::delete(schema::port_usage::table.filter(schema::port_usage::plan_id.eq(plan_id)))
        .execute(conn)
        .await?;
    diesel::delete(schema::plan::table.filter(schema::plan::id.eq(plan_id)))
        .execute(conn)
        .await?;
    // Netbox showed this plan, which the baseline now is
    if plan.netbox_active {
        diesel::update(schema::plan::table.find(BASELINE_PLAN_ID))
            .set(schema::plan::netbox_active.eq(true))
            .execute(conn)
            .await?;
    }
    Ok(plan)
}
