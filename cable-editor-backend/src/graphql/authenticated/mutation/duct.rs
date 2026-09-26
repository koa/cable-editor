//! Ducts (their course: graphql/duct_line.rs).

use crate::{
    db::{entity::Duct, schema},
    graphql::authenticated,
    graphql::authorization::{Role, RoleGuard},
    graphql::duct_line::{self, LineInput},
};
use async_graphql::{Context, InputObject, Object};
use diesel::{ExpressionMethods, HasQuery, QueryDsl, SelectableHelper};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use postgis_diesel::types::{GeometryContainer, LineString, Point};

#[derive(Default)]
pub struct DuctMutation;

#[Object]
impl DuctMutation {
    /// A duct between two Schächte, with the course from a file or straight.
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn create_duct(
        &self,
        ctx: &Context<'_>,
        duct: DuctInput,
        line: Option<LineInput>,
        #[graphql(default)] confirmed: bool,
    ) -> async_graphql::Result<Duct> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let description = duct.checked_description()?;
        let geom = match &line {
            Some(line) => {
                checked_line(
                    &mut connection,
                    duct.schacht_a,
                    duct.schacht_z,
                    line,
                    confirmed,
                )
                .await?
            }
            None => None,
        };
        Ok(diesel::insert_into(schema::trasse::table)
            .values((
                schema::trasse::schacht_a.eq(duct.schacht_a),
                schema::trasse::schacht_z.eq(duct.schacht_z),
                schema::trasse::description.eq(description),
                schema::trasse::geom.eq(geom),
            ))
            .returning(Duct::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// Description and Schächte; the Schächte only of a duct without cables. The course is
    /// turned (and trimmed) to fit new Schächte.
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn update_duct(
        &self,
        ctx: &Context<'_>,
        duct_id: i32,
        duct: DuctInput,
    ) -> async_graphql::Result<Duct> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let description = duct.checked_description()?;
        let stored: Duct = Duct::query()
            .filter(schema::trasse::id.eq(duct_id))
            .first(&mut connection)
            .await?;
        let mut geom = stored_line_of(&stored);
        if (stored.schacht_a, stored.schacht_z) != (duct.schacht_a, duct.schacht_z) {
            if duct_cable_count(&mut connection, duct_id).await? > 0 {
                return Err(
                    "Die Trasse enthält Kabel, ihre Schächte können nicht geändert werden".into(),
                );
            }
            if let Some(line) = geom {
                let a = duct_line::schacht_position(&mut connection, duct.schacht_a).await?;
                let z = duct_line::schacht_position(&mut connection, duct.schacht_z).await?;
                geom = duct_line::stored_line(&duct_line::fit(line.points, &a, &z)?);
            }
        }
        Ok(diesel::update(schema::trasse::table.find(duct_id))
            .set((
                schema::trasse::schacht_a.eq(duct.schacht_a),
                schema::trasse::schacht_z.eq(duct.schacht_z),
                schema::trasse::description.eq(description),
                schema::trasse::geom.eq(geom),
            ))
            .returning(Duct::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// The course from a file (see `checkDuctLine`), or none: a straight line. An end further
    /// than 10 m from its Schacht needs `confirmed`.
    #[graphql(guard = "RoleGuard(Role::Planner)")]
    async fn set_duct_line(
        &self,
        ctx: &Context<'_>,
        duct_id: i32,
        line: Option<LineInput>,
        #[graphql(default)] confirmed: bool,
    ) -> async_graphql::Result<Duct> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let stored: Duct = Duct::query()
            .filter(schema::trasse::id.eq(duct_id))
            .first(&mut connection)
            .await?;
        let geom = match &line {
            Some(line) => {
                checked_line(
                    &mut connection,
                    stored.schacht_a,
                    stored.schacht_z,
                    line,
                    confirmed,
                )
                .await?
            }
            None => None,
        };
        Ok(diesel::update(schema::trasse::table.find(duct_id))
            .set(schema::trasse::geom.eq(geom))
            .returning(Duct::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// Only a duct without cables.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn delete_duct(&self, ctx: &Context<'_>, duct_id: i32) -> async_graphql::Result<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let cables = duct_cable_count(&mut connection, duct_id).await?;
        if cables > 0 {
            return Err(format!("Durch die Trasse führen noch {cables} Kabel").into());
        }
        let deleted = diesel::delete(schema::trasse::table.find(duct_id))
            .execute(&mut connection)
            .await?;
        Ok(deleted > 0)
    }
}

/// The Schächte and description of a duct.
#[derive(Debug, Clone, PartialEq, InputObject)]
struct DuctInput {
    schacht_a: i32,
    schacht_z: i32,
    description: Option<String>,
}

impl DuctInput {
    /// Trimmed, `None` if empty; the Schächte must differ.
    fn checked_description(&self) -> async_graphql::Result<Option<String>> {
        if self.schacht_a == self.schacht_z {
            return Err("Anfangs- und Endschacht müssen verschieden sein".into());
        }
        let description = self
            .description
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty());
        // varchar(50)
        if description.is_some_and(|d| d.chars().count() > 50) {
            return Err("Die Beschreibung darf höchstens 50 Zeichen lang sein".into());
        }
        Ok(description.map(str::to_string))
    }
}

/// The line as stored, fitted to the Schächte; refused if an end is far from its Schacht and
/// not confirmed.
async fn checked_line(
    connection: &mut AsyncPgConnection,
    schacht_a: i32,
    schacht_z: i32,
    line: &LineInput,
    confirmed: bool,
) -> async_graphql::Result<Option<LineString<Point>>> {
    let (fitted, _, _) = duct_line::fit_line(connection, schacht_a, schacht_z, line).await?;
    if fitted.needs_confirmation() && !confirmed {
        return Err(format!(
            "Der Verlauf endet {:.1} m bzw. {:.1} m von den Schächten entfernt, bitte bestätigen",
            fitted.start_distance, fitted.end_distance
        )
        .into());
    }
    Ok(duct_line::stored_line(&fitted))
}

/// The stored course of the duct as line.
fn stored_line_of(duct: &Duct) -> Option<LineString<Point>> {
    match &duct.geom {
        Some(GeometryContainer::LineString(line)) => Some(line.clone()),
        _ => None,
    }
}

async fn duct_cable_count(
    connection: &mut AsyncPgConnection,
    duct_id: i32,
) -> async_graphql::Result<i64> {
    Ok(schema::kabel_trasse::table
        .filter(schema::kabel_trasse::trasse.eq(duct_id))
        .count()
        .get_result(connection)
        .await?)
}
