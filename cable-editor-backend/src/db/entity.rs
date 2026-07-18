use crate::{
    db::schema::{kabel, schacht, schacht_typ, trasse},
    graphql::{authenticated::get_connection, model},
};
use async_graphql::{Context, Object};
use diesel::{Associations, ExpressionMethods, HasQuery, Identifiable, Insertable, QueryDsl};
use diesel_async::RunQueryDsl;
use postgis_diesel::types::{LineString, Point};

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

#[derive(Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = kabel)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Cable {
    pub id: i32,
    pub name: String,
    pub buendel_anz: i32,
    pub faser_anz: i32,
}

#[derive(Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = trasse)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Duct {
    pub id: i32,
    pub description: String,
    pub geom: LineString<Point>,
    pub schacht_a: i32,
    pub schacht_z: i32,
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

#[Object]
impl Cable {
    async fn id(&self) -> i32 {
        self.id
    }
    async fn name(&self) -> &str {
        self.name.as_str()
    }
    async fn bundle_count(&self) -> u32 {
        self.buendel_anz as u32
    }
    async fn fiber_count(&self) -> u32 {
        self.faser_anz as u32
    }
    async fn cable_length(&self) -> f64 {
        0.0
    }
}
