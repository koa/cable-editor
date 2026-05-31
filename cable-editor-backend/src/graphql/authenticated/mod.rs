use crate::db::entity::SchachtTyp;
use crate::db::{DB, entity::Schacht, schema::schacht::id};
use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};
use diesel::pg::Pg;
use diesel::{ExpressionMethods, HasQuery};
use diesel::{OptionalExtension, QueryDsl};
use diesel_async::{AsyncConnectionCore, RunQueryDsl};
use log::{error, info};

pub type AuthenticatedGraphqlSchema = Schema<Query, EmptyMutation, EmptySubscription>;

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
}
pub struct Mutation;

pub fn create_authenticated_schema() -> AuthenticatedGraphqlSchema {
    Schema::build(Query, EmptyMutation, EmptySubscription).finish()
}

pub async fn get_connection(
    ctx: &Context<'_>,
) -> async_graphql::Result<impl AsyncConnectionCore<Backend = Pg>> {
    let db = ctx.data::<DB>()?;
    info!("Database connection established: {:?}", db.status());

    Ok(db.get().await?)
}
