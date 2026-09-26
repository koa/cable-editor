//! Schächte.

use crate::{
    db::{entity::schacht::Schacht, schema},
    graphql::authenticated,
    graphql::authorization::{Role, RoleGuard},
    graphql::geo::{self, PositionInput},
};
use async_graphql::{Context, InputObject, Object};
use diesel::{BoolExpressionMethods, ExpressionMethods, QueryDsl, SelectableHelper};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use postgis_diesel::types::Point;

#[derive(Default)]
pub struct SchachtMutation;

#[Object]
impl SchachtMutation {
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn create_schacht(
        &self,
        ctx: &Context<'_>,
        schacht: SchachtInput,
    ) -> async_graphql::Result<Schacht> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let (name, typ, geom) = schacht.values(&mut connection).await?;
        Ok(diesel::insert_into(schema::schacht::table)
            .values((
                schema::schacht::name.eq(name),
                schema::schacht::typ.eq(typ),
                schema::schacht::geom.eq(geom),
            ))
            .returning(Schacht::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// Replaces name, type and position of the Schacht; its ducts follow the position (their
    /// lines start and end at their Schächte, see the view trassen_mit_endpunkten).
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn update_schacht(
        &self,
        ctx: &Context<'_>,
        schacht_id: i32,
        schacht: SchachtInput,
    ) -> async_graphql::Result<Schacht> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let (name, typ, geom) = schacht.values(&mut connection).await?;
        Ok(diesel::update(schema::schacht::table.find(schacht_id))
            .set((
                schema::schacht::name.eq(name),
                schema::schacht::typ.eq(typ),
                schema::schacht::geom.eq(geom),
            ))
            .returning(Schacht::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// Only a Schacht without panels and ducts.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn delete_schacht(
        &self,
        ctx: &Context<'_>,
        schacht_id: i32,
    ) -> async_graphql::Result<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let panels: i64 = schema::panel::table
            .filter(schema::panel::schacht_id.eq(schacht_id))
            .count()
            .get_result(&mut connection)
            .await?;
        let ducts: i64 = schema::trasse::table
            .filter(
                schema::trasse::schacht_a
                    .eq(schacht_id)
                    .or(schema::trasse::schacht_z.eq(schacht_id)),
            )
            .count()
            .get_result(&mut connection)
            .await?;
        if panels > 0 || ducts > 0 {
            return Err(format!("Der Schacht hat noch {panels} Panels und {ducts} Trassen").into());
        }
        let deleted = diesel::delete(schema::schacht::table.find(schacht_id))
            .execute(&mut connection)
            .await?;
        Ok(deleted > 0)
    }
}

/// Name, type and position of a Schacht.
#[derive(Debug, Clone, PartialEq, InputObject)]
struct SchachtInput {
    name: String,
    type_id: Option<i32>,
    /// Missing: the Schacht has no position
    position: Option<PositionInput>,
}

impl SchachtInput {
    /// The column values: the name checked, the position in LV95.
    async fn values(
        self,
        connection: &mut AsyncPgConnection,
    ) -> async_graphql::Result<(String, Option<i32>, Option<Point>)> {
        let name = self.name.trim().to_string();
        if name.is_empty() {
            return Err("Der Schacht braucht einen Namen".into());
        }
        // varchar(20)
        if name.chars().count() > 20 {
            return Err("Der Name darf höchstens 20 Zeichen lang sein".into());
        }
        let geom = match self.position {
            Some(position) => Some(geo::to_lv95(connection, position).await?),
            None => None,
        };
        Ok((name, self.type_id, geom))
    }
}
