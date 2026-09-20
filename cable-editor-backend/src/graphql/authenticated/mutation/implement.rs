use crate::db::{
    entity::{
        panel::PortUsage,
        plan::{Plan, PlanStatusType},
    },
    schema,
};
use diesel::{ExpressionMethods, HasQuery, QueryDsl, associations::HasTable};
use diesel_async::{
    AsyncConnection, AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool::Object,
};
use tokio::sync::MutexGuard;

pub async fn implement_plan(
    plan_id: i32,
    mut connection: MutexGuard<'_, Object<AsyncPgConnection>>,
) -> Result<Plan, async_graphql::Error> {
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
                        plan_id: 0,
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
                    .filter(schema::port_usage::plan_id.eq(0))
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
            diesel::delete(
                schema::port_usage::table.filter(schema::port_usage::plan_id.eq(plan_id)),
            )
            .execute(conn)
            .await?;
            diesel::delete(schema::plan::table.filter(schema::plan::id.eq(plan_id)))
                .execute(conn)
                .await?;
            plan.status = PlanStatusType::Implemented;
            Ok(plan)
        })
        .await
}
