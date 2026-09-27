//! The delivery to the Leitungskataster Kanton Zürich (see docs/leitungskataster.md): per owner
//! with UID the delivered ducts and the Schächte where one ends, as SIA405 LKMap transfer file.

pub mod xtf;

use crate::{
    config::LKMAP_CONFIG,
    db::{
        entity::{
            eigentuemer::Eigentuemer,
            lkmap::{Genauigkeit, LkmapPunktObjektart},
        },
        schema::{self, sql_types::GenauigkeitEnum, sql_types::LkmapPunktObjektartEnum},
    },
};
use cable_editor_common::{ObjectKind, UserError};
use chrono::NaiveDate;
use diesel::{
    ExpressionMethods, HasQuery, OptionalExtension, QueryDsl, QueryableByName, sql_query,
    sql_types::{Bool, Date, Integer, Nullable},
};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use postgis_diesel::{
    sql_types::Geometry,
    types::{LineString, Point},
};
use xtf::{Delivery, LkLinie, LkPunkt};

/// An owner's transfer file and what couldn't go into it.
#[derive(Debug, Clone, PartialEq)]
pub struct Export {
    /// `<uid>-kommunikation-lkmap.xtf`, the UID in lower case with `-` for `.`
    pub file_name: String,
    pub xtf: Vec<u8>,
    pub schacht_count: usize,
    pub duct_count: usize,
    /// Schächte without position: neither they nor their ducts can be delivered
    pub schaechte_without_position: Vec<i32>,
    /// Delivered ducts ending at a Schacht without position
    pub ducts_without_line: Vec<i32>,
}

#[derive(QueryableByName)]
struct DuctRow {
    #[diesel(sql_type = Integer)]
    id: i32,
    /// From Schacht A through the course to Schacht Z (`trassen_mit_endpunkten`)
    #[diesel(sql_type = Nullable<Geometry>)]
    line: Option<LineString<Point>>,
    #[diesel(sql_type = Bool)]
    has_course: bool,
    #[diesel(sql_type = Bool)]
    ends_located: bool,
    #[diesel(sql_type = GenauigkeitEnum)]
    lagebestimmung: Genauigkeit,
    #[diesel(sql_type = Nullable<Integer>)]
    breite_mm: Option<i32>,
    #[diesel(sql_type = Date)]
    geaendert: NaiveDate,
}

#[derive(QueryableByName)]
struct SchachtRow {
    #[diesel(sql_type = Integer)]
    id: i32,
    #[diesel(sql_type = Nullable<Geometry>)]
    geom: Option<Point>,
    #[diesel(sql_type = GenauigkeitEnum)]
    lagebestimmung: Genauigkeit,
    #[diesel(sql_type = Date)]
    geaendert: NaiveDate,
    #[diesel(sql_type = LkmapPunktObjektartEnum)]
    objektart: LkmapPunktObjektart,
    #[diesel(sql_type = Nullable<Integer>)]
    dimension1_mm: Option<i32>,
    #[diesel(sql_type = Nullable<Integer>)]
    dimension2_mm: Option<i32>,
}

/// The owner's delivered ducts. `Letzte_Aenderung` is the day in Zürich.
const DUCTS: &str = "
select t.id,
       v.geom                                           as line,
       t.geom is not null                               as has_course,
       sa.geom is not null and sz.geom is not null      as ends_located,
       t.lagebestimmung,
       t.breite_mm,
       (t.geaendert_am at time zone 'Europe/Zurich')::date as geaendert
from trasse t
         join trassen_mit_endpunkten v on v.id = t.id
         join schacht sa on sa.id = t.schacht_a
         join schacht sz on sz.id = t.schacht_z
where t.leitungskataster
  and t.eigentuemer_id = $1
order by t.id";

/// The owner's Schächte where a delivered duct ends, whoever owns the duct.
const SCHAECHTE: &str = "
select s.id,
       s.geom,
       s.lagebestimmung,
       (s.geaendert_am at time zone 'Europe/Zurich')::date                 as geaendert,
       coalesce(st.lkmap_objektart, 'unbekannt'::lkmap_punkt_objektart_enum) as objektart,
       st.dimension1_mm,
       st.dimension2_mm
from schacht s
         left join schacht_typ st on st.id = s.typ
where s.eigentuemer_id = $1
  and exists (select 1
              from trasse t
              where t.leitungskataster
                and (t.schacht_a = s.id or t.schacht_z = s.id))
order by s.id";

/// The transfer file of the owner (with UID) and what it lacks.
pub async fn export(
    connection: &mut AsyncPgConnection,
    owner_id: i32,
) -> async_graphql::Result<Export> {
    let config = LKMAP_CONFIG.as_ref().ok_or(UserError::LkmapNotConfigured)?;
    let owner: Eigentuemer = Eigentuemer::query()
        .filter(schema::eigentuemer::id.eq(owner_id))
        .first(connection)
        .await
        .optional()?
        .ok_or(UserError::NotFound {
            kind: ObjectKind::Owner,
            id: owner_id.into(),
        })?;
    let uid = owner
        .uid
        .clone()
        .ok_or_else(|| UserError::OwnerWithoutUid {
            owner: owner.name.clone(),
        })?;
    let duct_rows: Vec<DuctRow> = sql_query(DUCTS)
        .bind::<Integer, _>(owner_id)
        .load(connection)
        .await?;
    let schacht_rows: Vec<SchachtRow> = sql_query(SCHAECHTE)
        .bind::<Integer, _>(owner_id)
        .load(connection)
        .await?;
    if duct_rows.is_empty() && schacht_rows.is_empty() {
        return Err(UserError::NothingToDeliver { owner: owner.name }.into());
    }
    let prefix = config.oid_prefix();

    let mut ducts = Vec::new();
    let mut ducts_without_line = Vec::new();
    for row in duct_rows {
        match row.line.filter(|_| row.ends_located) {
            Some(line) => ducts.push(LkLinie {
                oid: oid(prefix, ObjectKind::Duct, row.id)?,
                letzte_aenderung: row.geaendert,
                // A straight line between the Schächte says nothing about the real course
                lagebestimmung: if row.has_course {
                    row.lagebestimmung
                } else {
                    Genauigkeit::Unbekannt
                },
                breite_mm: row.breite_mm,
                line: line.points.iter().map(|p| (p.x, p.y)).collect(),
            }),
            None => ducts_without_line.push(row.id),
        }
    }
    let mut schaechte = Vec::new();
    let mut schaechte_without_position = Vec::new();
    for row in schacht_rows {
        match row.geom {
            Some(position) => schaechte.push(LkPunkt {
                oid: oid(prefix, ObjectKind::Schacht, row.id)?,
                letzte_aenderung: row.geaendert,
                lagebestimmung: row.lagebestimmung,
                dimension1_mm: row.dimension1_mm,
                dimension2_mm: row.dimension2_mm,
                objektart: row.objektart,
                position: (position.x, position.y),
            }),
            None => schaechte_without_position.push(row.id),
        }
    }
    let delivery = Delivery {
        basket_id: oid(prefix, ObjectKind::Owner, owner.id)?,
        datenherr: uid.clone(),
        datenlieferant: config.datenlieferant_uid().to_string(),
        eigentuemer: owner.delivered_name().to_string(),
        schaechte,
        ducts,
    };
    Ok(Export {
        file_name: format!(
            "{}-kommunikation-lkmap.xtf",
            uid.to_lowercase().replace('.', "-")
        ),
        xtf: xtf::write(&delivery)?,
        schacht_count: delivery.schaechte.len(),
        duct_count: delivery.ducts.len(),
        schaechte_without_position,
        ducts_without_line,
    })
}

/// The STANDARDOID of an object: the configured prefix (8 characters), a letter for its kind
/// and its id in 7 digits; also `OBJ_ID`. A basket (the delivery of an owner) is `b` + the
/// owner's id.
fn oid(prefix: &str, kind: ObjectKind, id: i32) -> Result<String, UserError> {
    let letter = match kind {
        ObjectKind::Schacht => 's',
        ObjectKind::Duct => 't',
        _ => 'b',
    };
    if !(0..10_000_000).contains(&id) {
        return Err(UserError::LkmapIdTooLarge { kind, id });
    }
    Ok(format!("{prefix}{letter}{id:07}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oids() {
        assert_eq!(
            oid("ch4711ab", ObjectKind::Schacht, 42),
            Ok("ch4711abs0000042".into())
        );
        assert_eq!(
            oid("ch4711ab", ObjectKind::Duct, 9_999_999),
            Ok("ch4711abt9999999".into())
        );
        assert_eq!(
            oid("ch4711ab", ObjectKind::Owner, 1),
            Ok("ch4711abb0000001".into())
        );
        assert_eq!(
            oid("ch4711ab", ObjectKind::Duct, 10_000_000),
            Err(UserError::LkmapIdTooLarge {
                kind: ObjectKind::Duct,
                id: 10_000_000
            })
        );
    }
}
