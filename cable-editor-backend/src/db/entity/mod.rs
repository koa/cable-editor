pub mod cable;
pub mod eigentuemer;
pub mod lkmap;
pub mod panel;
pub mod path;
pub mod plan;
pub mod schacht;
pub mod trasse;

use crate::graphql::error::ApiResult;
use crate::{
    db::{
        entity::path::{DirectedDuct, DuctDirection, UnalignedDuct},
        schema,
    },
    graphql::{
        loader::{
            DuctCables, DuctLength, DuctLine, EigentuemerId, SchachtId, get_loader, load_one,
        },
        model::GeoPoint,
    },
};
use async_graphql::{Context, Object};
use cable::Cable;
use chrono::{DateTime, Utc};
use diesel::{
    AsExpression, FromSqlRow, HasQuery, Identifiable, Insertable, deserialize,
    deserialize::FromSql,
    pg::{Pg, PgValue},
    serialize,
    serialize::{IsNull, Output, ToSql},
    sql_types::{Integer, Nullable},
};
use eigentuemer::Eigentuemer;
use lkmap::Genauigkeit;
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
    pub eigentuemer_id: i32,
    pub leitungskataster: bool,
    pub lagebestimmung: Genauigkeit,
    pub breite_mm: Option<i32>,
    pub geaendert_am: DateTime<Utc>,
}

#[Object]
impl DirectedDuct<Duct, i32> {
    async fn begin_schacht(&self, ctx: &Context<'_>) -> ApiResult<Schacht> {
        load_one(ctx, SchachtId(self.schacht_a())).await
    }
    async fn end_schacht(&self, ctx: &Context<'_>) -> ApiResult<Schacht> {
        load_one(ctx, SchachtId(self.schacht_z())).await
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
    async fn cables(&self, ctx: &Context<'_>) -> ApiResult<Box<[Cable]>> {
        Ok(get_loader(ctx)?
            .load_one(DuctCables(self.id))
            .await?
            .unwrap_or_default())
    }
    /// Line in WGS84 for the map, from Schacht A to Schacht Z
    async fn line(&self, ctx: &Context<'_>) -> ApiResult<Option<Box<[GeoPoint]>>> {
        get_loader(ctx)?.load_one(DuctLine(self.id)).await
    }
    async fn schacht_a(&self, ctx: &Context<'_>) -> ApiResult<Schacht> {
        load_one(ctx, SchachtId(self.schacht_a)).await
    }

    async fn schacht_z(&self, ctx: &Context<'_>) -> ApiResult<Schacht> {
        load_one(ctx, SchachtId(self.schacht_z)).await
    }
    async fn owner(&self, ctx: &Context<'_>) -> ApiResult<Eigentuemer> {
        load_one(ctx, EigentuemerId(self.eigentuemer_id)).await
    }
    /// Delivered to the Leitungskataster (crosses property boundaries)
    async fn leitungskataster(&self) -> bool {
        self.leitungskataster
    }
    /// Accuracy of the course
    async fn lagebestimmung(&self) -> Genauigkeit {
        self.lagebestimmung
    }
    /// Width in millimetres, optional
    async fn width_mm(&self) -> Option<i32> {
        self.breite_mm
    }
    /// Last change of the duct, its course or its Schächte' positions (`Letzte_Aenderung`)
    async fn changed_at(&self) -> DateTime<Utc> {
        self.geaendert_am
    }
    /// Metres, missing without geometry
    async fn length(&self, ctx: &Context<'_>) -> ApiResult<Option<f64>> {
        get_loader(ctx)?.load_one(DuctLength(self.id)).await
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

/// SRID of WGS84, what the map (Leaflet) takes.
pub const WGS84: i32 = 4326;

diesel::define_sql_function! {
    #[sql_name = "ST_Transform"]
    fn st_transform(geom: Nullable<Geometry>, srid: Integer) -> Nullable<Geometry>;
}

diesel::define_sql_function! {
    #[sql_name = "ST_Length"]
    fn st_length(geom: Nullable<Geometry>) -> Nullable<Float8>;
}
