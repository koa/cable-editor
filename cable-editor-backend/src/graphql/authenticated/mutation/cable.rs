//! Cables.

use crate::{
    db::{
        entity::cable::{Cable, UpdateCableChangeset},
        schema,
    },
    graphql::authenticated,
    graphql::authorization::{Role, RoleGuard},
};
use async_graphql::{Context, InputObject, Object};
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl};
use diesel_async::{AsyncConnection, RunQueryDsl};

#[derive(Default)]
pub struct CableMutation;

#[Object]
impl CableMutation {
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
}

#[derive(InputObject)]
struct UpdateCableStructure {
    bundle_count: u32,
    fiber_count: u32,
}
