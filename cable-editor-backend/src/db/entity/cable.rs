use crate::db::entity::schacht::Schacht;
use crate::{
    db::{
        entity::{
            CablePath, CablePathSegment, Duct,
            path::{DuctAlignmentError, align_ducts},
            st_length,
        },
        schema,
    },
    graphql::authenticated::get_connection,
};
use async_graphql::{Context, Object};
use diesel::{ExpressionMethods, HasQuery, Identifiable, Insertable, QueryDsl, dsl::sum};
use diesel_async::pooled_connection::deadpool::Object;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

#[derive(Identifiable, Insertable, HasQuery, Debug, Clone, PartialEq)]
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
}
