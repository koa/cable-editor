pub mod path;
use crate::db::entity::path::{
    DirectedDuct, DuctAlignmentError, DuctDirection, UnalignedDuct, align_ducts,
};
use crate::{
    db::schema::{
        kabel, kabel_trasse, schacht, schacht_typ, sql_types::Xml, trasse, trassen_mit_endpunkten,
    },
    graphql::{authenticated::get_connection, model},
};
use async_graphql::{Context, Object};
use diesel::BoolExpressionMethods;
use diesel::{
    AsChangeset, AsExpression, Associations, ExpressionMethods, FromSqlRow, HasQuery, Identifiable,
    Insertable, QueryDsl, deserialize,
    deserialize::FromSql,
    dsl::sum,
    pg::{Pg, PgValue},
    serialize,
    serialize::{IsNull, Output, ToSql},
    sql_types::Nullable,
};
use diesel_async::RunQueryDsl;
use postgis_diesel::{
    sql_types::Geometry,
    types::{GeometryContainer, Point},
};
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
    pub geom: Option<GeometryContainer<Point>>,
    pub description: Option<String>,
    pub schacht_a: i32,
    pub schacht_z: i32,
    pub eigenleistung: bool,
}

#[derive(Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = kabel_trasse)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CableDuct {
    pub kabel: i32,
    pub trasse: i32,
    pub sequenz: i32,
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
        self.geom.map(Point::into)
    }
    async fn connecting_duct(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<PotentialPathSegment>> {
        let mut connection = get_connection(ctx).await?;

        let ducts: Vec<Duct> = Duct::query()
            .filter(
                trasse::schacht_a
                    .eq(self.id)
                    .or(trasse::schacht_z.eq(self.id)),
            )
            .load(&mut connection)
            .await?;

        let mut results = Vec::new();
        for duct in ducts {
            let other_schacht_id = if duct.schacht_a == self.id {
                duct.schacht_z
            } else {
                duct.schacht_a
            };

            let other_schacht = Schacht::query()
                .filter(schacht::id.eq(other_schacht_id))
                .get_result(&mut connection)
                .await?;
            results.push(PotentialPathSegment {
                duct,
                schacht: other_schacht,
            });
        }

        Ok(results)
    }
}

struct PotentialPathSegment {
    duct: Duct,
    schacht: Schacht,
}

#[Object]
impl PotentialPathSegment {
    async fn duct(&self) -> &Duct {
        &self.duct
    }
    async fn schacht(&self) -> &Schacht {
        &self.schacht
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
    async fn length(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<f64>> {
        let mut connection = get_connection(ctx).await?;
        Ok(trassen_mit_endpunkten::table
            .inner_join(kabel_trasse::table)
            .filter(kabel_trasse::kabel.eq(self.id))
            .select(sum(st_length(trassen_mit_endpunkten::geom)))
            .first(&mut connection)
            .await?)
    }

    async fn path(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<CablePath>> {
        let mut connection = get_connection(ctx).await?;
        let vec = trasse::table
            .inner_join(kabel_trasse::table)
            .filter(kabel_trasse::kabel.eq(self.id))
            .order(kabel_trasse::sequenz.asc())
            .select((trasse::all_columns, kabel_trasse::sequenz))
            .load::<(Duct, i32)>(&mut connection)
            .await?;

        let segments = align_ducts(vec.into_iter())
            .map(|r| {
                r.map(|segment| CablePathSegment {
                    far_schacht: segment.schacht_z(),
                    segment,
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| match error {
                DuctAlignmentError::NoConnectionFoundOnPair { first, second } => {
                    async_graphql::Error::new(format!(
                        "Duct {} and {} are not connected",
                        first.0.id, second.0.id
                    ))
                }
                DuctAlignmentError::NoConnectionFoundForSchacht { last_schacht, duct } => {
                    async_graphql::Error::new(format!(
                        "Duct {} don't contain schacht {}",
                        duct.0.id, last_schacht
                    ))
                }
            })?;
        Ok(segments
            .as_slice()
            .first()
            .map(|s| s.segment.schacht_a())
            .map(|first| CablePath {
                near_schacht: first,
                segments,
            }))
    }
}
pub struct CablePath {
    near_schacht: i32,
    segments: Vec<CablePathSegment>,
}
#[Object]
impl CablePath {
    async fn near_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        fetch_schacht(ctx, self.near_schacht).await
    }
    async fn segments(&self) -> &[CablePathSegment] {
        self.segments.as_ref()
    }
}
struct CablePathSegment {
    segment: DirectedDuct<(Duct, i32), i32>,
    far_schacht: i32,
}
#[Object]
impl CablePathSegment {
    async fn duct(&self) -> &Duct {
        &self.segment.duct.0
    }
    async fn far_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        fetch_schacht(ctx, self.far_schacht).await
    }
    async fn sequence(&self) -> i32 {
        self.segment.duct.1
    }
}
#[Object]
impl DirectedDuct<Duct, i32> {
    async fn begin_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        fetch_schacht(ctx, self.schacht_a()).await
    }
    async fn end_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        fetch_schacht(ctx, self.schacht_z()).await
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
#[derive(AsChangeset)]
#[diesel(table_name = kabel)]
pub struct UpdateCableChangeset {
    pub name: Option<String>,
    pub buendel_anz: Option<i32>,
    pub faser_anz: Option<i32>,
}

impl UpdateCableChangeset {
    pub fn any(&self) -> bool {
        self.name.is_some() || self.buendel_anz.is_some() || self.faser_anz.is_some()
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
        fetch_schacht(ctx, self.schacht_a).await
    }

    async fn schacht_z(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        fetch_schacht(ctx, self.schacht_z).await
    }
    async fn length(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<f64>> {
        let mut connection = get_connection(ctx).await?;
        Ok(trassen_mit_endpunkten::table
            .find(self.id)
            .select(sum(st_length(trassen_mit_endpunkten::geom)))
            .first(&mut connection)
            .await?)
    }
}
async fn fetch_schacht(ctx: &Context<'_>, id: i32) -> async_graphql::Result<Schacht> {
    let mut connection = get_connection(ctx).await?;
    Ok(Schacht::query()
        .filter(schacht::id.eq(id))
        .get_result(&mut connection)
        .await?)
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

diesel::define_sql_function! {
    #[sql_name = "ST_Length"]
    fn st_length(geom: Nullable<Geometry>) -> Nullable<Float8>;
}
