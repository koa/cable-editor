//! Schächte.

use super::owner::ensure_owner_exists;
use crate::{
    db::{
        entity::{lkmap::Genauigkeit, schacht::Schacht},
        schema,
    },
    graphql::authenticated,
    graphql::authorization::{Role, RoleGuard},
    graphql::geo::{self, PositionInput},
};
use async_graphql::{Context, InputObject, Object};
use cable_editor_common::{UserError, limits::MAX_NAME};
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
        let values = schacht.values(&mut connection).await?;
        Ok(diesel::insert_into(schema::schacht::table)
            .values((
                schema::schacht::name.eq(values.name),
                schema::schacht::typ.eq(values.typ),
                schema::schacht::geom.eq(values.geom),
                schema::schacht::eigentuemer_id.eq(values.owner_id),
                schema::schacht::lagebestimmung.eq(values.lagebestimmung),
            ))
            .returning(Schacht::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// Replaces name, type, position, owner and Lagebestimmung of the Schacht; its ducts follow the position (their
    /// lines start and end at their Schächte, see the view trassen_mit_endpunkten).
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn update_schacht(
        &self,
        ctx: &Context<'_>,
        schacht_id: i32,
        schacht: SchachtInput,
    ) -> async_graphql::Result<Schacht> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let values = schacht.values(&mut connection).await?;
        Ok(diesel::update(schema::schacht::table.find(schacht_id))
            .set((
                schema::schacht::name.eq(values.name),
                schema::schacht::typ.eq(values.typ),
                schema::schacht::geom.eq(values.geom),
                schema::schacht::eigentuemer_id.eq(values.owner_id),
                schema::schacht::lagebestimmung.eq(values.lagebestimmung),
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
            return Err(UserError::SchachtReferenced { panels, ducts }.into());
        }
        let deleted = diesel::delete(schema::schacht::table.find(schacht_id))
            .execute(&mut connection)
            .await?;
        Ok(deleted > 0)
    }
}

/// Name, type and position of a Schacht, and what the delivery to the Leitungskataster takes
/// from it.
#[derive(Debug, Clone, PartialEq, InputObject)]
struct SchachtInput {
    name: String,
    type_id: Option<i32>,
    /// Missing: the Schacht has no position
    position: Option<PositionInput>,
    owner_id: i32,
    /// Accuracy of the position
    lagebestimmung: Genauigkeit,
}

/// The column values of a `SchachtInput`.
struct SchachtValues {
    name: String,
    typ: Option<i32>,
    geom: Option<Point>,
    owner_id: i32,
    lagebestimmung: Genauigkeit,
}

impl SchachtInput {
    /// The column values: the name checked, the position in LV95, the owner existing.
    async fn values(
        self,
        connection: &mut AsyncPgConnection,
    ) -> async_graphql::Result<SchachtValues> {
        let name = self.name.trim().to_string();
        if name.is_empty() {
            return Err(UserError::NameMissing.into());
        }
        if name.chars().count() > MAX_NAME {
            return Err(UserError::NameTooLong { max: MAX_NAME }.into());
        }
        let geom = match self.position {
            Some(position) => Some(geo::to_lv95(connection, position).await?),
            None => None,
        };
        ensure_owner_exists(connection, self.owner_id).await?;
        Ok(SchachtValues {
            name,
            typ: self.type_id,
            geom,
            owner_id: self.owner_id,
            lagebestimmung: self.lagebestimmung,
        })
    }
}
