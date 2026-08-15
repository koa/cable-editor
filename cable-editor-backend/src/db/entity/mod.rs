pub mod cable;
pub mod panel;
pub mod path;
pub mod plan;
pub mod schacht;

use crate::{
    db::{
        entity::path::{DirectedDuct, DuctDirection, UnalignedDuct},
        schema,
    },
    graphql::authenticated::get_connection,
};
use async_graphql::{Context, Object};
use diesel::{
    AsExpression, FromSqlRow, HasQuery, Identifiable, Insertable, QueryDsl, QueryableByName,
    deserialize,
    deserialize::FromSql,
    dsl::sum,
    pg::{Pg, PgValue},
    serialize,
    serialize::{IsNull, Output, ToSql},
    sql_types::{Integer, Nullable},
};
use diesel_async::RunQueryDsl;
use postgis_diesel::{
    sql_types::Geometry,
    types::{GeometryContainer, Point},
};
use schacht::Schacht;
use std::io::Write;

#[derive(Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = schema::trasse)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Duct {
    pub id: i32,
    pub geom: Option<GeometryContainer<Point>>,
    pub description: Option<String>,
    pub schacht_a: i32,
    pub schacht_z: i32,
    pub eigenleistung: bool,
}

#[Object]
impl DirectedDuct<Duct, i32> {
    async fn begin_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        schacht::fetch_schacht(ctx, self.schacht_a()).await
    }
    async fn end_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        schacht::fetch_schacht(ctx, self.schacht_z()).await
    }
    async fn begin_schacht_id(&self) -> i32 {
        self.schacht_a()
    }
    async fn end_schacht_id(&self) -> i32 {
        self.schacht_z()
    }
    async fn duct(&self) -> &Duct {
        &self.duct
    }
    async fn direction(&self) -> DuctDirection {
        self.direction
    }
}

#[Object]
impl Duct {
    async fn id(&self) -> i32 {
        self.id
    }
    async fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    async fn schacht_a(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        schacht::fetch_schacht(ctx, self.schacht_a).await
    }

    async fn schacht_z(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        schacht::fetch_schacht(ctx, self.schacht_z).await
    }
    async fn length(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<f64>> {
        let mut connection = get_connection(ctx).await?;
        Ok(schema::trassen_mit_endpunkten::table
            .find(self.id)
            .select(sum(st_length(schema::trassen_mit_endpunkten::geom)))
            .first(&mut connection)
            .await?)
    }
}

impl UnalignedDuct<i32> for Duct {
    fn schacht_a(&self) -> i32 {
        self.schacht_a
    }

    fn schacht_z(&self) -> i32 {
        self.schacht_z
    }
}
impl UnalignedDuct<i32> for (Duct, i32) {
    fn schacht_a(&self) -> i32 {
        self.0.schacht_a
    }

    fn schacht_z(&self) -> i32 {
        self.0.schacht_z
    }
}

#[derive(QueryableByName, Debug, Clone)]
pub struct FiberPathNode {
    #[diesel(sql_type = Integer)]
    pub step: i32,

    #[diesel(sql_type = Integer)]
    pub from_panel: i32,
    #[diesel(sql_type = Integer)]
    pub from_port: i32,

    #[diesel(sql_type = Integer)]
    pub to_panel: i32,
    #[diesel(sql_type = Integer)]
    pub to_port: i32,

    #[diesel(sql_type = Integer)]
    pub kabel: i32,
    #[diesel(sql_type = Integer)]
    pub buendel: i32,
    #[diesel(sql_type = Integer)]
    pub faser: i32,
}

#[derive(Debug, Clone, FromSqlRow, AsExpression, PartialOrd, PartialEq, Hash)]
#[diesel(sql_type = schema::sql_types::Xml)]
pub struct XmlDocument(pub Box<str>);

impl FromSql<schema::sql_types::Xml, Pg> for XmlDocument {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let xml_string = std::str::from_utf8(bytes.as_bytes())?;
        Ok(XmlDocument(Box::from(xml_string)))
    }
}

impl ToSql<schema::sql_types::Xml, Pg> for XmlDocument {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        out.write_all(self.0.as_bytes())?;
        Ok(IsNull::No)
    }
}

diesel::define_sql_function! {
    #[sql_name = "ST_Length"]
    fn st_length(geom: Nullable<Geometry>) -> Nullable<Float8>;
}
