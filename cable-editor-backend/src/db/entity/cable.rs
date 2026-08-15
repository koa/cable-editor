use crate::db::entity::panel::{Panel, PanelPort, PortUsage};
use crate::db::entity::path::DirectedDuct;
use crate::db::entity::schacht;
use crate::db::entity::schacht::Schacht;
use crate::{
    db::{
        entity::{
            Duct,
            path::{DuctAlignmentError, align_ducts},
            st_length,
        },
        schema,
    },
    graphql::authenticated::get_connection,
};
use async_graphql::{Context, Object};
use diesel::{
    AsChangeset, ExpressionMethods, HasQuery, Identifiable, Insertable, QueryDsl, Queryable,
    QueryableByName, dsl::sum,
};
use diesel_async::pooled_connection::deadpool::Object;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

#[derive(Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq, QueryableByName)]
#[diesel(table_name = schema::kabel)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Cable {
    pub id: i32,
    pub name: String,
    pub buendel_anz: i32,
    pub faser_anz: i32,
}

#[derive(Insertable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = schema::kabel_trasse)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CableDuct {
    pub kabel: i32,
    pub trasse: i32,
    pub sequenz: i32,
}
impl Cable {
    async fn build_cable_path(
        &self,
        mut connection: &mut Object<AsyncPgConnection>,
    ) -> async_graphql::Result<Option<CablePath>> {
        let vec = schema::trasse::table
            .inner_join(schema::kabel_trasse::table)
            .filter(schema::kabel_trasse::kabel.eq(self.id))
            .order(schema::kabel_trasse::sequenz.asc())
            .select((schema::trasse::all_columns, schema::kabel_trasse::sequenz))
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
        Ok(schema::trassen_mit_endpunkten::table
            .inner_join(schema::kabel_trasse::table)
            .filter(schema::kabel_trasse::kabel.eq(self.id))
            .select(sum(st_length(schema::trassen_mit_endpunkten::geom)))
            .first(&mut connection)
            .await?)
    }

    async fn path(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<CablePath>> {
        let mut connection = get_connection(ctx).await?;
        self.build_cable_path(&mut connection).await
    }
    async fn end(&self, ctx: &Context<'_>, schacht_id: i32) -> async_graphql::Result<CableEnd> {
        let mut connection = get_connection(ctx).await?;
        let schacht = Schacht::query()
            .filter(schema::schacht::id.eq(schacht_id))
            .first(&mut connection)
            .await?;
        Ok(CableEnd {
            cable: self.clone(),
            schacht,
        })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct CableEnd {
    pub cable: Cable,
    pub schacht: Schacht,
}

#[Object]
impl CableEnd {
    async fn cable(&self) -> &Cable {
        &self.cable
    }
    async fn schacht(&self) -> &Schacht {
        &self.schacht
    }
    async fn path(&self, ctx: &Context<'_>) -> async_graphql::Result<CablePath> {
        let mut connection = get_connection(ctx).await?;

        let path = self
            .cable
            .build_cable_path(&mut connection)
            .await?
            .ok_or_else(|| {
                async_graphql::Error::new(format!(
                    "invalid cable end on duct {} for cable {}",
                    self.schacht.id, self.cable.id
                ))
            })?;
        Ok(if path.near_schacht == self.schacht.id {
            path
        } else {
            path.reverse()
        })
    }
    async fn used_ports(
        &self,
        ctx: &Context<'_>,
        plan_id: i32,
    ) -> async_graphql::Result<Vec<PortUsage>> {
        let mut connection = get_connection(ctx).await?;
        let raw_sql = r#"
        -- 1. Echte Belegungen für dieses Kabel im aktuellen Plan, direkt auf den Schacht gefiltert
        SELECT u.*
        FROM port_usage u
        JOIN panel_port pp ON u.port_id = pp.id
        JOIN panel p ON pp.panel_id = p.id
        WHERE u.plan_id = $1
          AND u.cable = $2
          AND p.schacht_id = $3

        UNION ALL

        -- 2. Belegungen für dieses Kabel aus der Baseline, direkt auf den Schacht gefiltert...
        SELECT p0.*
        FROM port_usage p0
        JOIN panel_port pp ON p0.port_id = pp.id
        JOIN panel p ON pp.panel_id = p.id
        WHERE p0.plan_id = 0
          AND $1 != 0
          AND p0.cable = $2
          AND p.schacht_id = $3
          -- ...die im aktuellen Plan an exakt diesem Port nicht überschrieben wurden
          AND NOT EXISTS (
              SELECT 1
              FROM port_usage px
              WHERE px.plan_id = $1
                AND px.port_id = p0.port_id
                AND px.side = p0.side
          )
    "#;

        Ok(diesel::sql_query(raw_sql)
            .bind::<diesel::sql_types::Integer, _>(plan_id)
            .bind::<diesel::sql_types::Integer, _>(self.cable.id)
            .bind::<diesel::sql_types::Integer, _>(self.schacht.id)
            .load::<PortUsage>(&mut connection)
            .await?)
    }
}

pub struct PotentialPathSegment {
    pub duct: Duct,
    pub schacht: Schacht,
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

#[derive(Clone, PartialEq, Hash, Ord, PartialOrd, Eq, Debug)]
struct FiberPathSegment {
    fiber: Fiber,
    next_port: PanelPort,
    plan_id: i32,
}

#[Object]
impl FiberPathSegment {
    async fn fiber(&self) -> &Fiber {
        &self.fiber
    }
    async fn next_port(&self) -> &PanelPort {
        &self.next_port
    }
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
            .filter(schema::kabel::id.eq(self.cable))
            .first(&mut connection)
            .await?)
    }
}

pub struct CablePath {
    near_schacht: i32,
    segments: Vec<CablePathSegment>,
}

impl CablePath {
    fn reverse(self) -> CablePath {
        if self.segments.is_empty() {
            self
        } else {
            let mut next_schacht = self.near_schacht;
            let mut new_segments = Vec::with_capacity(self.segments.len());
            for path_segment in self.segments.into_iter() {
                new_segments.push(CablePathSegment {
                    segment: path_segment.segment.reverse(),
                    far_schacht: next_schacht,
                });
                next_schacht = path_segment.far_schacht;
            }
            new_segments.reverse();
            CablePath {
                near_schacht: next_schacht,
                segments: new_segments,
            }
        }
    }
}

#[Object]
impl CablePath {
    async fn near_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        schacht::fetch_schacht(ctx, self.near_schacht).await
    }
    async fn segments(&self) -> &[CablePathSegment] {
        self.segments.as_ref()
    }
    async fn far_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        let schacht_id = self
            .segments
            .last()
            .ok_or_else(|| async_graphql::Error::new("Empty path is invalid"))?
            .far_schacht;
        schacht::fetch_schacht(ctx, schacht_id).await
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
        schacht::fetch_schacht(ctx, self.far_schacht).await
    }
    async fn sequence(&self) -> i32 {
        self.segment.duct.1
    }
}

#[derive(AsChangeset)]
#[diesel(table_name = schema::kabel)]
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
