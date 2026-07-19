use crate::{
    db::schema::{kabel, schacht, schacht_typ, sql_types::Xml, trasse},
    graphql::{authenticated::get_connection, model},
};
use async_graphql::{Context, Object};
use diesel::{
    AsExpression, Associations, ExpressionMethods, FromSqlRow, HasQuery, Identifiable, Insertable,
    QueryDsl, deserialize,
    deserialize::FromSql,
    pg::{Pg, PgValue},
    serialize,
    serialize::{IsNull, Output, ToSql},
};
use diesel_async::RunQueryDsl;
use postgis_diesel::types::{GeometryContainer, Point};
use std::io::Write;

#[derive(Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = schacht)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Schacht {
    pub id: i32,
    pub name: Option<String>,
    pub typ: Option<i32>,
    pub geom: Option<Point>,
}

#[derive(HasQuery, Identifiable, Insertable, Associations, Debug, PartialEq)]
#[diesel(belongs_to(Schacht, foreign_key = id))]
#[diesel(table_name = schacht_typ)]
pub struct SchachtTyp {
    pub id: i32,
    pub name: Option<String>,
    pub icon: XmlDocument,
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
    pub description: Option<String>,
    pub geom: Option<GeometryContainer<Point>>,
    pub schacht_a: i32,
    pub schacht_z: i32,
}

#[Object]
impl Schacht {
    async fn name(&self) -> &str {
        self.name.as_deref().unwrap_or_default()
    }

    async fn id(&self) -> i32 {
        self.id
    }
    async fn typ(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<SchachtTyp>> {
        if let Some(typ) = self.typ {
            let mut connection = get_connection(ctx).await?;
            Ok(Some(
                SchachtTyp::query()
                    .filter(schacht_typ::id.eq(typ))
                    .get_result(&mut connection)
                    .await?,
            ))
        } else {
            Ok(None)
        }
    }
    async fn position(&self) -> Option<model::Point> {
        self.geom.map(|p| Point::into(p))
    }
}

#[Object]
impl SchachtTyp {
    async fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    async fn id(&self) -> i32 {
        self.id
    }
    async fn icon(&self) -> &str {
        self.icon.0.as_ref()
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

#[derive(Debug, Clone, FromSqlRow, AsExpression, PartialOrd, PartialEq, Hash)]
#[diesel(sql_type = Xml)]
pub struct XmlDocument(pub Box<str>);

impl FromSql<Xml, Pg> for XmlDocument {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let xml_string = std::str::from_utf8(bytes.as_bytes())?;
        Ok(XmlDocument(Box::from(xml_string)))
    }
}

impl ToSql<Xml, Pg> for XmlDocument {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        out.write_all(self.0.as_bytes())?;
        Ok(IsNull::No)
    }
}
