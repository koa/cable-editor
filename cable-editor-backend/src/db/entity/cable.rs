use crate::db::entity::plan::BASELINE_PLAN_ID;
use crate::{
    db::{
        entity::{
            Duct, WGS84,
            panel::PortUsage,
            path::{DirectedDuct, DuctAlignmentError, align_ducts},
            schacht::Schacht,
        },
        schema,
    },
    graphql::{
        authenticated::get_connection,
        loader::{
            CableDucts, CableEndUsages, CableId, CableLength, SchachtId, get_loader, load_one,
        },
        model::GeoPoint,
    },
};
use async_graphql::{Context, Object};
use diesel::{
    AsChangeset, ExpressionMethods, HasQuery, Identifiable, Insertable, OptionalExtension,
    QueryDsl, QueryableByName, sql_query,
    sql_types::{Integer, Nullable},
};
use diesel_async::{AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool::Object};
use postgis_diesel::{
    sql_types::Geometry,
    types::{LineString, Point},
};

#[derive(Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq, QueryableByName)]
#[diesel(table_name = schema::kabel)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Cable {
    pub id: i32,
    pub name: String,
    pub buendel_anz: i32,
    pub faser_anz: i32,
}

#[derive(QueryableByName)]
struct CableLine {
    #[diesel(sql_type = Nullable<Geometry>)]
    line: Option<LineString<Point>>,
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
    /// The cable's path from its ducts (with their sequence), `None` without ducts.
    fn path_from_ducts(&self, ducts: Vec<(Duct, i32)>) -> async_graphql::Result<Option<CablePath>> {
        let segments = align_ducts(ducts.into_iter())
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
                cable: self.clone(),
                near_schacht: first,
                segments,
            }))
    }
    /// The path, its ducts loaded in batches with the other cables' of the request.
    async fn load_path(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<CablePath>> {
        let ducts = get_loader(ctx)?
            .load_one(CableDucts(self.id))
            .await?
            .unwrap_or_default();
        self.path_from_ducts(ducts)
    }
    async fn cable_end(
        &self,
        schacht_id: i32,
        mut connection: &mut Object<AsyncPgConnection>,
    ) -> Result<CableEnd, diesel::result::Error> {
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
    /// Metres along its ducts, missing without ducts
    async fn length(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<f64>> {
        get_loader(ctx)?.load_one(CableLength(self.id)).await
    }

    async fn path(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<CablePath>> {
        self.load_path(ctx).await
    }
    /// Course in WGS84 for the map; missing without ducts or with a gap between them
    async fn line(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Vec<GeoPoint>>> {
        let mut connection = get_connection(ctx).await?;
        // Unlike the view kabel_pfad, which fails on a gap (no LineString to cast to)
        let line: Option<CableLine> = sql_query(
            r#"
            SELECT ST_Transform(merged, $2) AS line
            FROM (SELECT ST_LineMerge(ST_Collect(t.geom ORDER BY kt.sequenz)) AS merged
                  FROM kabel_trasse kt
                  JOIN trassen_mit_endpunkten t ON t.id = kt.trasse
                  WHERE kt.kabel = $1) m
            WHERE GeometryType(merged) = 'LINESTRING'
            "#,
        )
        .bind::<Integer, _>(self.id)
        .bind::<Integer, _>(WGS84)
        .get_result(&mut connection)
        .await
        .optional()?;
        Ok(line
            .and_then(|l| l.line)
            .map(|l| l.points.into_iter().map(GeoPoint::from).collect()))
    }
    async fn end(&self, ctx: &Context<'_>, schacht_id: i32) -> async_graphql::Result<CableEnd> {
        let mut connection = get_connection(ctx).await?;
        Ok(self.cable_end(schacht_id, &mut connection).await?)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct CableEnd {
    pub cable: Cable,
    pub schacht: Schacht,
}
/// The cable's port usages at the Schacht as they are in the plan: the plan's own and the
/// current state's (plan 0) not overridden by the plan at the same port.
pub async fn cable_usages_at(
    connection: &mut AsyncPgConnection,
    plan_id: i32,
    cable_id: i32,
    schacht_id: i32,
) -> Result<Vec<PortUsage>, diesel::result::Error> {
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

    diesel::sql_query(raw_sql)
        .bind::<diesel::sql_types::Integer, _>(plan_id)
        .bind::<diesel::sql_types::Integer, _>(cable_id)
        .bind::<diesel::sql_types::Integer, _>(schacht_id)
        .load::<PortUsage>(connection)
        .await
}

impl CableEnd {
    /// `cable_usages_at`, loaded in batches with the request's other cable ends.
    async fn usages(
        &self,
        ctx: &Context<'_>,
        plan_id: i32,
    ) -> async_graphql::Result<Vec<PortUsage>> {
        Ok(get_loader(ctx)?
            .load_one(CableEndUsages {
                cable: self.cable.id,
                schacht: self.schacht.id,
                plan: plan_id,
            })
            .await?
            .unwrap_or_default())
    }
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
        let path = self.cable.load_path(ctx).await?.ok_or_else(|| {
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
        self.usages(ctx, plan_id).await
    }
    async fn fibers(&self) -> Vec<FiberEnd> {
        (1..=self.cable.buendel_anz)
            .flat_map(|bundle| {
                (1..=self.cable.faser_anz).map(move |fiber| FiberEnd {
                    cable: self.clone(),
                    bundle,
                    fiber,
                })
            })
            .collect()
    }
}

struct FiberEnd {
    pub cable: CableEnd,
    pub bundle: i32,
    pub fiber: i32,
}
#[Object]
impl FiberEnd {
    async fn cable(&self) -> &CableEnd {
        &self.cable
    }
    async fn bundle(&self) -> i32 {
        self.bundle
    }
    async fn fiber(&self) -> i32 {
        self.fiber
    }
    /// The port the fiber ends at in the plan, if any; a usage of the plan wins over one of the
    /// current state.
    async fn used_port(
        &self,
        ctx: &Context<'_>,
        plan_id: i32,
    ) -> async_graphql::Result<Option<PortUsage>> {
        let usages = self.cable.usages(ctx, plan_id).await?;
        Ok(fiber_usage(&usages, self.bundle, self.fiber).cloned())
    }
    async fn other_end(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<FiberEnd>> {
        let Some(path) = self.cable.cable.load_path(ctx).await? else {
            return Ok(None);
        };
        let other_schacht_id = if path.near_schacht == self.cable.schacht.id {
            path.far_schacht_id()
        } else {
            path.near_schacht
        };
        let schacht = load_one(ctx, SchachtId(other_schacht_id)).await?;
        Ok(Some(FiberEnd {
            cable: CableEnd {
                cable: self.cable.cable.clone(),
                schacht,
            },
            bundle: self.bundle,
            fiber: self.fiber,
        }))
    }
}

/// The fiber's usage among a cable end's (`cable_usages_at`): the plan's wins over the
/// current state's.
fn fiber_usage(usages: &[PortUsage], bundle: i32, fiber: i32) -> Option<&PortUsage> {
    let of_fiber = || {
        usages
            .iter()
            .filter(move |u| u.bundle == Some(bundle) && u.fiber == Some(fiber))
    };
    of_fiber()
        .find(|u| u.plan_id != BASELINE_PLAN_ID)
        .or_else(|| of_fiber().next())
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
        load_one(ctx, CableId(self.cable)).await
    }
}

pub struct CablePath {
    cable: Cable,
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
                cable: self.cable,
                near_schacht: next_schacht,
                segments: new_segments,
            }
        }
    }
    fn far_schacht_id(&self) -> i32 {
        self.segments
            .last()
            .map(|s| s.far_schacht)
            .unwrap_or(self.near_schacht)
    }
}

#[Object]
impl CablePath {
    async fn near_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        load_one(ctx, SchachtId(self.near_schacht)).await
    }
    async fn near_end(&self, ctx: &Context<'_>) -> async_graphql::Result<CableEnd> {
        let schacht = load_one(ctx, SchachtId(self.near_schacht)).await?;
        Ok(CableEnd {
            cable: self.cable.clone(),
            schacht,
        })
    }
    async fn segments(&self) -> &[CablePathSegment] {
        self.segments.as_ref()
    }
    async fn far_schacht(&self, ctx: &Context<'_>) -> async_graphql::Result<Schacht> {
        load_one(ctx, SchachtId(self.far_schacht_id())).await
    }
    async fn far_end(&self, ctx: &Context<'_>) -> async_graphql::Result<CableEnd> {
        let schacht = load_one(ctx, SchachtId(self.far_schacht_id())).await?;
        Ok(CableEnd {
            cable: self.cable.clone(),
            schacht,
        })
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
        load_one(ctx, SchachtId(self.far_schacht)).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entity::panel::PortSide;

    fn usage(port_id: i32, plan_id: i32, bundle: i32, fiber: i32) -> PortUsage {
        PortUsage {
            port_id,
            plan_id,
            side: PortSide::Front,
            cable: Some(11),
            bundle: Some(bundle),
            fiber: Some(fiber),
        }
    }

    #[test]
    fn finds_the_fibers_usage() {
        let usages = [usage(1, 0, 1, 1), usage(2, 0, 1, 2)];
        assert_eq!(fiber_usage(&usages, 1, 2).map(|u| u.port_id), Some(2));
        assert_eq!(fiber_usage(&usages, 2, 1), None);
    }

    #[test]
    fn the_plans_usage_wins() {
        // Moved in the plan from port 1 to port 5; the current state's usage comes first
        let usages = [usage(1, 0, 1, 1), usage(5, 3, 1, 1)];
        assert_eq!(fiber_usage(&usages, 1, 1).map(|u| u.port_id), Some(5));
    }
}
