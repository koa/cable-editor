//! Types of Schächte (see docs/stammdaten.md); Admin only, as the type gives the Objektart of
//! its Schächte in the delivery to the Leitungskataster.

use crate::graphql::error::ApiResult;
use crate::{
    db::{
        entity::{XmlDocument, lkmap::LkmapPunktObjektart, schacht::SchachtTyp},
        schema,
    },
    graphql::authenticated,
    graphql::authorization::{Role, RoleGuard},
};
use async_graphql::{Context, InputObject, Object};
use cable_editor_common::{
    ObjectKind, UserError,
    limits::{MAX_ICON_BYTES, MAX_MILLIMETRES, MAX_NAME},
};
use diesel::{
    ExpressionMethods, OptionalExtension, QueryDsl, QueryableByName, SelectableHelper, sql_query,
    sql_types::{Bool, Text},
};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

#[derive(Default)]
pub struct SchachtTypMutation;

#[Object]
impl SchachtTypMutation {
    /// Without icon: a plain circle.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn create_schacht_typ(
        &self,
        ctx: &Context<'_>,
        typ: SchachtTypInput,
    ) -> ApiResult<SchachtTyp> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let typ = typ.checked(&mut connection, None).await?;
        let icon = typ.icon.unwrap_or_else(|| DEFAULT_ICON.to_string());
        Ok(diesel::insert_into(schema::schacht_typ::table)
            .values((
                schema::schacht_typ::name.eq(typ.name),
                schema::schacht_typ::icon.eq(XmlDocument(icon.into_boxed_str())),
                schema::schacht_typ::lkmap_objektart.eq(typ.lkmap_objektart),
                schema::schacht_typ::dimension1_mm.eq(typ.dimension1_mm),
                schema::schacht_typ::dimension2_mm.eq(typ.dimension2_mm),
            ))
            .returning(SchachtTyp::as_returning())
            .get_result(&mut connection)
            .await?)
    }
    /// Without icon the stored one stays. A new Objektart or new dimensions change its
    /// Schächte for the Leitungskataster.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn update_schacht_typ(
        &self,
        ctx: &Context<'_>,
        typ_id: i32,
        typ: SchachtTypInput,
    ) -> ApiResult<SchachtTyp> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let typ = typ.checked(&mut connection, Some(typ_id)).await?;
        let values = (
            schema::schacht_typ::name.eq(typ.name),
            schema::schacht_typ::lkmap_objektart.eq(typ.lkmap_objektart),
            schema::schacht_typ::dimension1_mm.eq(typ.dimension1_mm),
            schema::schacht_typ::dimension2_mm.eq(typ.dimension2_mm),
        );
        let target = schema::schacht_typ::table.find(typ_id);
        let updated = match typ.icon {
            Some(icon) => {
                diesel::update(target)
                    .set((
                        values,
                        schema::schacht_typ::icon.eq(XmlDocument(icon.into_boxed_str())),
                    ))
                    .returning(SchachtTyp::as_returning())
                    .get_result(&mut connection)
                    .await
            }
            None => {
                diesel::update(target)
                    .set(values)
                    .returning(SchachtTyp::as_returning())
                    .get_result(&mut connection)
                    .await
            }
        };
        updated.optional()?.ok_or_else(|| {
            UserError::NotFound {
                kind: ObjectKind::SchachtTyp,
                id: typ_id.into(),
            }
            .into()
        })
    }
    /// Only a type without Schächte.
    #[graphql(guard = "RoleGuard(Role::Admin)")]
    async fn delete_schacht_typ(&self, ctx: &Context<'_>, typ_id: i32) -> ApiResult<bool> {
        let mut connection = authenticated::get_connection(ctx).await?;
        let schaechte: i64 = schema::schacht::table
            .filter(schema::schacht::typ.eq(typ_id))
            .count()
            .get_result(&mut connection)
            .await?;
        if schaechte > 0 {
            return Err(UserError::SchachtTypReferenced { schaechte }.into());
        }
        let deleted = diesel::delete(schema::schacht_typ::table.find(typ_id))
            .execute(&mut connection)
            .await?;
        Ok(deleted > 0)
    }
}

/// The icon of a type created without one.
const DEFAULT_ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100"><circle cx="50" cy="50" r="42" stroke="#4d5258" stroke-width="8" fill="#f0f0f0"/></svg>"##;

/// Name, icon, Objektart and dimensions of a type of Schacht.
#[derive(Debug, Clone, PartialEq, InputObject)]
struct SchachtTypInput {
    name: String,
    /// SVG; missing: the stored one (a new type: a plain circle)
    icon: Option<String>,
    /// `Objektart` of its Schächte in the Leitungskataster
    lkmap_objektart: LkmapPunktObjektart,
    /// Larger inner dimension in millimetres
    dimension1_mm: Option<i32>,
    /// Smaller inner dimension in millimetres, only with the larger one
    dimension2_mm: Option<i32>,
}

#[derive(QueryableByName)]
struct WellFormed {
    #[diesel(sql_type = Bool)]
    well_formed: bool,
}

impl SchachtTypInput {
    /// Refuses what the table would refuse and a name another type has (the
    /// Schacht's page chooses by name); `typ_id` is the type being changed.
    async fn checked(
        mut self,
        connection: &mut AsyncPgConnection,
        typ_id: Option<i32>,
    ) -> ApiResult<SchachtTypInput> {
        self.name = self.name.trim().to_string();
        if self.name.is_empty() {
            return Err(UserError::NameMissing.into());
        }
        if self.name.chars().count() > MAX_NAME {
            return Err(UserError::NameTooLong { max: MAX_NAME }.into());
        }
        let mut others = schema::schacht_typ::table
            .filter(schema::schacht_typ::name.eq(&self.name))
            .select(schema::schacht_typ::id)
            .into_boxed();
        if let Some(typ_id) = typ_id {
            others = others.filter(schema::schacht_typ::id.ne(typ_id));
        }
        let taken = others.first::<i32>(connection).await.optional()?;
        if taken.is_some() {
            return Err(UserError::NameTaken {
                kind: ObjectKind::SchachtTyp,
                name: self.name.into(),
            }
            .into());
        }
        for (dimension, value) in [(1, self.dimension1_mm), (2, self.dimension2_mm)] {
            if let Some(value) = value
                && !(0..=MAX_MILLIMETRES).contains(&value)
            {
                return Err(UserError::DimensionOutOfRange {
                    dimension,
                    max: MAX_MILLIMETRES,
                }
                .into());
            }
        }
        match (self.dimension1_mm, self.dimension2_mm) {
            (None, Some(_)) => {
                return Err(UserError::Dimension2WithoutDimension1.into());
            }
            (Some(larger), Some(smaller)) if smaller > larger => {
                return Err(UserError::DimensionsSwapped.into());
            }
            _ => {}
        }
        if let Some(icon) = &mut self.icon {
            *icon = icon.trim().to_string();
            if icon.len() > MAX_ICON_BYTES {
                return Err(UserError::IconTooLarge {
                    max_bytes: MAX_ICON_BYTES,
                }
                .into());
            }
            let well_formed = sql_query("select xml_is_well_formed_document($1) as well_formed")
                .bind::<Text, _>(icon.as_str())
                .get_result::<WellFormed>(connection)
                .await?
                .well_formed;
            if !well_formed || !is_svg(icon) {
                return Err(UserError::IconNotSvg.into());
            }
        }
        Ok(self)
    }
}

/// Whether the document's root element is `<svg>` (after an XML declaration, comments or a
/// doctype).
fn is_svg(document: &str) -> bool {
    let mut rest = document;
    loop {
        rest = rest.trim_start();
        let skipped = if rest.starts_with("<?") {
            rest.find("?>").map(|end| end + 2)
        } else if rest.starts_with("<!--") {
            rest.find("-->").map(|end| end + 3)
        } else if rest.starts_with("<!") {
            rest.find('>').map(|end| end + 1)
        } else {
            return rest.starts_with("<svg") || rest.starts_with("<svg:svg");
        };
        match skipped {
            Some(end) => rest = &rest[end..],
            None => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_svg;

    #[test]
    fn svg_root() {
        assert!(is_svg("<svg xmlns=\"http://www.w3.org/2000/svg\"/>"));
        assert!(is_svg(
            "<?xml version=\"1.0\"?>\n<!-- Inkscape -->\n<!DOCTYPE svg>\n<svg/>"
        ));
        assert!(!is_svg("<html><svg/></html>"));
        assert!(!is_svg("<?xml version=\"1.0\""));
        assert!(!is_svg("svg"));
    }
}
