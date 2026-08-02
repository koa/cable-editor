pub mod path;
use crate::{
    db::{
        entity::path::{
            DirectedDuct, DuctAlignmentError, DuctDirection, UnalignedDuct, align_ducts,
        },
        schema::{
            kabel, kabel_trasse, panel, panel_port, plan, schacht, schacht_typ,
            sql_types::{PlanStatusEnum, PortTypeEnum, Xml},
            trasse, trassen_mit_endpunkten,
        },
    },
    graphql::authenticated::planned::PlannedPanel,
    graphql::{authenticated::get_connection, model},
};
use async_graphql::{Context, Enum, Object, SimpleObject};
use diesel::{
    AsChangeset, AsExpression, Associations, BoolExpressionMethods, ExpressionMethods, FromSqlRow,
    HasQuery, Identifiable, Insertable, QueryDsl, QueryableByName, deserialize,
    deserialize::FromSql,
    dsl::not,
    dsl::sum,
    pg::{Pg, PgValue},
    serialize,
    serialize::{IsNull, Output, ToSql},
    sql_query,
    sql_types::Integer,
    sql_types::Nullable,
};
use diesel_async::{AsyncConnection, RunQueryDsl};
use diesel_derive_enum::DbEnum;
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

#[derive(QueryableByName, Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = panel)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Panel {
    pub id: i32,
    pub name: Option<String>,
    pub schacht_id: i32,
    pub parent_panel: Option<i32>,
    pub parent_order: Option<i32>,
}

#[derive(QueryableByName, Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = plan)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Plan {
    pub id: i32,
    pub name: String,
    pub status: PlanStatusType,
}

#[derive(Debug, Clone, PartialEq, Copy, Eq, DbEnum, Enum, Hash, PartialOrd, Ord)]
#[ExistingTypePath = "crate::db::schema::sql_types::PlanStatusEnum"]
pub enum PlanStatusType {
    #[db_rename = "Open"]
    Open,

    #[db_rename = "Implemented"]
    Implemented,

    #[db_rename = "Rejected"]
    Rejected,
}

#[derive(Insertable)]
#[diesel(table_name = panel)]
pub struct InsertPanel {
    pub name: Option<String>,
    pub schacht_id: i32,
    pub parent_panel: Option<i32>,
    pub parent_order: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = panel_port)]
pub struct InsertPanelPort {
    pub panel_id: i32,
    pub port_number: i32,
    pub port_type: PanelPortType,
    pub label: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = plan)]
pub struct InsertPlan {
    pub name: String,
}
#[derive(
    Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq, Hash, PartialOrd, Ord, Eq,
)]
#[diesel(table_name = panel_port)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(panel_id, port_number))]
pub struct PanelPort {
    pub panel_id: i32,
    pub port_number: i32,
    pub label: Option<String>,
    pub port_type: PanelPortType,
    pub f1_kabel_id: Option<i32>,
    pub f1_buendel: Option<i32>,
    pub f1_faser: Option<i32>,

    pub f2_kabel_id: Option<i32>,
    pub f2_buendel: Option<i32>,
    pub f2_faser: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Copy, Eq, DbEnum, Enum, Hash, PartialOrd, Ord)]
#[ExistingTypePath = "PortTypeEnum"]
pub enum PanelPortType {
    Splice,
    Connector,
}

#[Object]
impl Plan {
    async fn id(&self) -> i32 {
        self.id
    }
    async fn name(&self) -> &str {
        self.name.as_str()
    }
    async fn status(&self) -> PlanStatusType {
        self.status
    }

    async fn root_panels(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<PlannedPanel>> {
        {
            let mut connection = get_connection(ctx).await?;
            let raw_sql = r#"
WITH RECURSIVE affected_panels AS (
    -- 1. Basisfall (Anchor):
    -- Finde alle Panels, die direkt mindestens einen Port in dieser plan_id haben
    SELECT p.id, p.parent_panel
    FROM panel p
    WHERE EXISTS (
        SELECT 1
        FROM panel_port pp
        WHERE pp.panel_id = p.id AND pp.plan_id = $1 -- Hier die plan_id übergeben
    )

    UNION
    -- Wichtig: UNION (ohne ALL) entfernt Duplikate, falls mehrere Kinder denselben Parent haben

    -- 2. Rekursiver Schritt:
    -- Klettere von den gefundenen Panels immer einen Parent nach oben
    SELECT parent.id, parent.parent_panel
    FROM panel parent
    INNER JOIN affected_panels child ON child.parent_panel = parent.id
)
-- 3. Finale Ausgabe:
-- Filtere aus allen berührten Panels nur jene heraus, die keinen Parent haben (Root)
-- und lade deren komplette Daten.
SELECT p.id, p.name, p.schacht_id, p.parent_panel, p.parent_order
FROM affected_panels a
    JOIN panel p ON a.id = p.id
WHERE a.parent_panel IS NULL;
    "#;

            Ok(sql_query(raw_sql)
                .bind::<Integer, _>(self.id)
                .load::<Panel>(&mut connection)
                .await
                .map(|panels| {
                    panels
                        .into_iter()
                        .map(|panel| PlannedPanel {
                            panel,
                            plan: self.clone(),
                        })
                        .collect()
                })?)
        }
    }
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
    async fn root_panels(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Panel>> {
        let mut connection = get_connection(ctx).await?;
        Ok(Panel::query()
            .filter(panel::schacht_id.eq(self.id).and(panel::parent_panel.eq(0)))
            .load(&mut connection)
            .await?)
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

#[Object]
impl Panel {
    async fn id(&self) -> i32 {
        self.id
    }
    async fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    async fn schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        let mut connection = get_connection(ctx).await?;
        let schacht = Schacht::query()
            .filter(schacht::id.eq(self.schacht_id))
            .first(&mut connection)
            .await?;
        Ok(schacht)
    }
    async fn parent_id(&self) -> Option<i32> {
        self.parent_panel
    }
    async fn parent_order(&self) -> Option<i32> {
        self.parent_order
    }
    async fn parent(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Panel>> {
        if let Some(parent_panel_id) = self.parent_panel {
            let mut connection = get_connection(ctx).await?;
            Ok(Some(
                Panel::query()
                    .filter(panel::id.eq(parent_panel_id))
                    .first(&mut connection)
                    .await?,
            ))
        } else {
            Ok(None)
        }
    }
    async fn children(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Panel>> {
        let mut connection = get_connection(ctx).await?;
        Ok(Panel::query()
            .filter(panel::parent_panel.eq(self.id))
            .order(panel::parent_order.asc())
            .load(&mut connection)
            .await?)
    }
    async fn all_children_recursive(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Panel>> {
        let mut connection = get_connection(ctx).await?;
        let raw_sql = r#"
        WITH RECURSIVE panel_tree AS (
            SELECT
                id, name, schacht_id, parent_panel, parent_order,
                1 as level
            FROM panel
            WHERE parent_panel = $1

            UNION ALL

            SELECT
                p.id, p.name, p.schacht_id, p.parent_panel, p.parent_order,
                pt.level + 1 as level
            FROM panel p
            INNER JOIN panel_tree pt ON p.parent_panel = pt.id
        )
        SELECT
            id, name, schacht_id, parent_panel, parent_order
        FROM panel_tree
        ORDER BY level, parent_order;
    "#;

        Ok(sql_query(raw_sql)
            .bind::<Integer, _>(self.id)
            .load::<Panel>(&mut connection)
            .await?)
    }
}

impl PanelPort {
    fn fiber1(&self) -> Option<Fiber> {
        if let (Some(cable), Some(bundle), Some(fiber)) =
            (self.f1_faser, self.f1_buendel, self.f1_faser)
        {
            Some(Fiber {
                cable,
                bundle,
                fiber,
            })
        } else {
            None
        }
    }
    fn fiber2(&self) -> Option<Fiber> {
        if let (Some(cable), Some(bundle), Some(fiber)) =
            (self.f2_faser, self.f2_buendel, self.f2_faser)
        {
            Some(Fiber {
                cable,
                bundle,
                fiber,
            })
        } else {
            None
        }
    }
    fn fibers(&self) -> impl Iterator<Item = Fiber> {
        self.fiber1().into_iter().chain(self.fiber2())
    }
}

#[Object]
impl PanelPort {
    async fn order_number(&self) -> i32 {
        self.port_number
    }
    async fn panel(&self, ctx: &Context<'_>) -> async_graphql::Result<Panel> {
        let mut connection = get_connection(ctx).await?;
        Ok(Panel::query()
            .filter(panel::id.eq(self.panel_id))
            .first(&mut connection)
            .await?)
    }
    async fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    async fn connected_fibers(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<FiberPathSegment>> {
        let mut connection = get_connection(ctx).await?;
        connection
            .transaction(async move |conn| {
                let mut path = Vec::with_capacity(2);
                for fiber in self.fibers() {
                    PanelPort::query()
                        .filter(
                            ((panel_port::f1_faser
                                .eq(fiber.fiber)
                                .and(panel_port::f1_buendel.eq(fiber.bundle))
                                .and(panel_port::f1_kabel_id.eq(fiber.cable)))
                            .or(panel_port::f2_faser
                                .eq(fiber.fiber)
                                .and(panel_port::f2_buendel.eq(fiber.bundle))
                                .and(panel_port::f2_kabel_id.eq(fiber.cable))))
                            .and(not(panel_port::panel_id
                                .eq(self.panel_id)
                                .and(panel_port::port_number.eq(self.port_number)))),
                        )
                        .load::<PanelPort>(conn)
                        .await?
                        .into_iter()
                        .map(|next_port| FiberPathSegment { fiber, next_port })
                        .for_each(|segment| path.push(segment));
                }
                Ok(path)
            })
            .await
    }
}
#[derive(Clone, PartialEq, Hash, Ord, PartialOrd, Eq, Debug, SimpleObject)]
struct FiberPathSegment {
    pub fiber: Fiber,
    pub next_port: PanelPort,
}

#[derive(Copy, Clone, PartialEq, Hash, Ord, PartialOrd, Eq, Debug)]
pub struct Fiber {
    pub cable: i32,
    pub bundle: i32,
    pub fiber: i32,
}
#[Object]
impl Fiber {
    async fn bundle(&self) -> i32 {
        self.bundle
    }
    async fn fiber(&self) -> i32 {
        self.fiber
    }
    async fn cable(&self, ctx: &Context<'_>) -> async_graphql::Result<Cable> {
        let mut connection = get_connection(ctx).await?;
        Ok(Cable::query()
            .filter(kabel::id.eq(self.cable))
            .first(&mut connection)
            .await?)
    }
}
#[derive(Copy, Clone, PartialEq, Hash, Ord, PartialOrd, Eq, Debug)]
pub struct PortId {
    pub panel_id: i32,
    pub port_number: i32,
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
    async fn far_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        let schacht_id = self
            .segments
            .last()
            .expect("Empty path is invalid")
            .far_schacht;
        fetch_schacht(ctx, schacht_id).await
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
