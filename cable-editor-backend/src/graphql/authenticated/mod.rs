use crate::db::entity::UpdateCableChangeset;
use crate::db::schema::kabel;
use crate::db::{
    DB,
    entity::{Cable, Schacht, SchachtTyp},
    schema::schacht::id,
};
use async_graphql::{Context, EmptySubscription, InputObject, Object, Schema};
use diesel::{ExpressionMethods, HasQuery, OptionalExtension, QueryDsl, pg::Pg};
use diesel_async::{AsyncConnectionCore, RunQueryDsl};
use log::info;

pub type AuthenticatedGraphqlSchema = Schema<Query, Mutation, EmptySubscription>;

pub struct Query;

#[Object]
impl Query {
    async fn list_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Schacht>> {
        //let mut connection = pool.get().await?;
        let mut connection = get_connection(ctx).await?;
        let query = Schacht::query();
        let list = query.load(&mut connection).await?;
        Ok(list)
    }
    async fn schacht(
        &self,
        ctx: &Context<'_>,
        schacht_id: i32,
    ) -> async_graphql::Result<Option<Schacht>> {
        let mut connection = get_connection(ctx).await?;

        let schacht = Schacht::query()
            .filter(id.eq(schacht_id))
            .first(&mut connection)
            .await
            .optional()?;
        Ok(schacht)
    }
    async fn list_schacht_typ(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<SchachtTyp>> {
        let mut connection = get_connection(ctx).await?;
        let query = SchachtTyp::query();
        let list = query.load(&mut connection).await?;
        Ok(list)
    }
    async fn list_cable(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Cable>> {
        let mut connection = get_connection(ctx).await?;
        let query = Cable::query();
        let list = query.load(&mut connection).await?;
        Ok(list)
    }
    async fn cable(
        &self,
        ctx: &Context<'_>,
        cable_id: u32,
    ) -> async_graphql::Result<Option<Cable>> {
        let mut connection = get_connection(ctx).await?;
        Ok(kabel::table
            .find(cable_id as i32)
            .first::<Cable>(&mut connection)
            .await
            .optional()?)
    }
}
pub struct Mutation;

#[derive(InputObject)]
struct UpdateCableStructure {
    bundle_count: u32,
    fiber_count: u32,
}

#[Object]
impl Mutation {
    async fn update_cable(
        &self,
        ctx: &Context<'_>,
        cable_id: i32,
        name: Option<String>,
        fibers: Option<UpdateCableStructure>,
    ) -> async_graphql::Result<Option<Cable>> {
        let mut connection = get_connection(ctx).await?;

        let mut changeset = UpdateCableChangeset {
            name,
            buendel_anz: None,
            faser_anz: None,
        };
        if let Some(f) = fibers {
            changeset.buendel_anz = Some(f.bundle_count as i32);
            changeset.faser_anz = Some(f.fiber_count as i32);
        }
        let updated_db_cable = diesel::update(kabel::table.find(cable_id))
            .set(&changeset)
            .get_result::<Cable>(&mut connection)
            .await
            .optional()?;

        Ok(updated_db_cable)
    }
}

pub fn create_authenticated_schema() -> AuthenticatedGraphqlSchema {
    Schema::build(Query, Mutation, EmptySubscription).finish()
}

pub async fn get_connection(
    ctx: &Context<'_>,
) -> async_graphql::Result<impl AsyncConnectionCore<Backend = Pg>> {
    let db = ctx.data::<DB>()?;
    info!("Database connection established: {:?}", db.status());

    Ok(db.get().await?)
}
