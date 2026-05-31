use crate::graphql::model;
use crate::{
    db::schema::{schacht, schacht_typ},
    graphql::authenticated::get_connection,
};
use async_graphql::{Context, Object};
use diesel::{Associations, ExpressionMethods, HasQuery, Identifiable, Insertable, QueryDsl};
use diesel_async::RunQueryDsl;
use postgis_diesel::types::Point;

#[derive(Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = schacht)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Schacht {
    pub id: i32,
    pub name: String,
    pub typ: i32,
    pub geom: Point,
}

#[derive(HasQuery, Identifiable, Insertable, Associations, Debug, PartialEq)]
#[diesel(belongs_to(Schacht, foreign_key = id))]
#[diesel(table_name = schacht_typ)]
pub struct SchachtTyp {
    pub id: i32,
    pub name: String,
    pub icon: String,
}

#[Object]
impl Schacht {
    async fn name(&self) -> &str {
        self.name.as_str()
    }

    async fn id(&self) -> i32 {
        self.id
    }
    async fn typ(&self, ctx: &Context<'_>) -> async_graphql::Result<SchachtTyp> {
        let mut connection = get_connection(ctx).await?;

        Ok(SchachtTyp::query()
            .filter(schacht_typ::id.eq(self.typ))
            .get_result(&mut connection)
            .await?)
    }
    async fn position(&self) -> model::Point {
        self.geom.into()
    }
}

#[Object]
impl SchachtTyp {
    async fn name(&self) -> &str {
        self.name.as_str()
    }

    async fn id(&self) -> i32 {
        self.id
    }
    async fn icon(&self) -> &str {
        self.icon.as_str()
    }
    async fn list_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Schacht>> {
        let mut connection = get_connection(ctx).await?;
        Ok(Schacht::query()
            .filter(schacht::typ.eq(self.id))
            .load(&mut connection)
            .await?)
    }
}
