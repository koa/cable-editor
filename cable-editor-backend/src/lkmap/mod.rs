//! The delivery to the Leitungskataster Kanton Zürich (see docs/leitungskataster.md): one for the
//! whole network, whoever owns its parts: the delivered ducts and the Schächte where one ends, as
//! SIA405 LKMap transfer file, and the Zuständigkeitsperimeter around them, each in a ZIP.

pub mod perimeter;
pub mod xtf;

use crate::graphql::error::{ApiError, ApiResult};
use crate::{
    config::LKMAP_CONFIG,
    db::{
        entity::lkmap::{Genauigkeit, LkmapPunktObjektart},
        schema::sql_types::{GenauigkeitEnum, LkmapPunktObjektartEnum},
    },
};
use cable_editor_common::{ObjectKind, UserError};
use chrono::NaiveDate;
use diesel::{
    QueryableByName, sql_query,
    sql_types::{Array, Bool, Date, Double, Integer, Nullable, Text},
};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use perimeter::Perimeter;
use postgis_diesel::{
    sql_types::Geometry,
    types::{LineString, Point, Polygon},
};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write};
use xtf::{Delivery, LkLinie, LkPunkt};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

/// A transfer file of the delivery.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferFile {
    /// `<uid>-<content>.xtf`, the UID in lower case with `-` for `.`
    pub file_name: Box<str>,
    pub xtf: Box<[u8]>,
}

impl TransferFile {
    /// The ZIP of the same name holding the file, how the Checkservice takes it.
    pub fn zip_name(&self) -> Box<str> {
        let name = self
            .file_name
            .strip_suffix(".xtf")
            .unwrap_or(&self.file_name);
        format!("{name}.zip").into()
    }

    /// The file in a ZIP (see `zip_name`).
    pub fn zip(&self) -> zip::result::ZipResult<Box<[u8]>> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(
            &*self.file_name,
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )?;
        zip.write_all(&self.xtf)?;
        Ok(zip.finish()?.into_inner().into())
    }
}

/// The delivery: what goes into it and what can't.
#[derive(Debug, Clone, PartialEq)]
pub struct Export {
    /// UID of the Datenherr, the name of the files
    pub datenherr: Box<str>,
    /// Missing when nothing to deliver has a position
    pub files: Option<Files>,
    /// The delivered Schächte and ducts
    pub schaechte: Box<[i32]>,
    pub ducts: Box<[i32]>,
    /// Schächte without position: neither they nor their ducts can be delivered
    pub schaechte_without_position: Box<[i32]>,
    /// Delivered ducts ending at a Schacht without position
    pub ducts_without_line: Box<[i32]>,
}

/// The transfer files of the delivery.
#[derive(Debug, Clone, PartialEq)]
pub struct Files {
    /// The ducts and Schächte (`SIA405_LKMap_2015_LV95`)
    pub lkmap: TransferFile,
    /// The Zuständigkeitsperimeter (`Perimeter_LK_ZH_V2_LV95`)
    pub perimeter: TransferFile,
    /// The perimeter's outline in WGS84 (lat, lng), for the map
    pub perimeter_wgs84: Box<[(f64, f64)]>,
    /// SHA-256 of both files in hex: the same data give the same files (no export date in
    /// them), so a different checksum means a change since a delivery, deleted objects too.
    pub checksum: Box<str>,
}

impl Export {
    /// The files to deliver, or why there are none.
    pub fn deliverable(&self) -> Result<&Files, UserError> {
        self.files.as_ref().ok_or(UserError::NothingToDeliver)
    }
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
    #[diesel(sql_type = Text)]
    eigentuemer: String,
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
    #[diesel(sql_type = Text)]
    eigentuemer: String,
    #[diesel(sql_type = LkmapPunktObjektartEnum)]
    objektart: LkmapPunktObjektart,
    #[diesel(sql_type = Nullable<Integer>)]
    dimension1_mm: Option<i32>,
    #[diesel(sql_type = Nullable<Integer>)]
    dimension2_mm: Option<i32>,
}

/// The delivered ducts. `Letzte_Aenderung` is the day in Zürich, `Eigentuemer` the owner's name
/// in the delivery.
const DUCTS: &str = "
select t.id,
       v.geom                                           as line,
       t.geom is not null                               as has_course,
       sa.geom is not null and sz.geom is not null      as ends_located,
       t.lagebestimmung,
       t.breite_mm,
       (t.geaendert_am at time zone 'Europe/Zurich')::date as geaendert,
       coalesce(e.lk_name, e.name)                      as eigentuemer
from trasse t
         join trassen_mit_endpunkten v on v.id = t.id
         join schacht sa on sa.id = t.schacht_a
         join schacht sz on sz.id = t.schacht_z
         join eigentuemer e on e.id = t.eigentuemer_id
where t.leitungskataster
order by t.id";

/// The Schächte where a delivered duct ends.
const SCHAECHTE: &str = "
select s.id,
       s.geom,
       s.lagebestimmung,
       (s.geaendert_am at time zone 'Europe/Zurich')::date                 as geaendert,
       coalesce(e.lk_name, e.name)                                         as eigentuemer,
       coalesce(st.lkmap_objektart, 'unbekannt'::lkmap_punkt_objektart_enum) as objektart,
       st.dimension1_mm,
       st.dimension2_mm
from schacht s
         left join schacht_typ st on st.id = s.typ
         join eigentuemer e on e.id = s.eigentuemer_id
where exists (select 1
              from trasse t
              where t.leitungskataster
                and (t.schacht_a = s.id or t.schacht_z = s.id))
order by s.id";

/// The area around the delivered ducts and Schächte: their convex hull with a buffer,
/// the arcs of the buffer as straight segments (4 per quarter circle).
const PERIMETER: &str = "
select a.area, st_transform(a.area, 4326) as area_wgs84
from (select st_buffer(st_convexhull(st_collect(o.geom)), $3, 'quad_segs=4') as area
from (select v.geom from trassen_mit_endpunkten v where v.id = any($1)
      union all
      select s.geom from schacht s where s.id = any($2)) o) a";

#[derive(QueryableByName)]
struct PerimeterRow {
    #[diesel(sql_type = Nullable<Geometry>)]
    area: Option<Polygon<Point>>,
    #[diesel(sql_type = Nullable<Geometry>)]
    area_wgs84: Option<Polygon<Point>>,
}

/// The delivery: the transfer files and what they lack. Without anything to deliver it has no
/// files.
pub async fn export(connection: &mut AsyncPgConnection) -> ApiResult<Export> {
    let config = LKMAP_CONFIG.as_ref().ok_or(UserError::LkmapNotConfigured)?;
    let duct_rows: Vec<DuctRow> = sql_query(DUCTS).load(connection).await?;
    let schacht_rows: Vec<SchachtRow> = sql_query(SCHAECHTE).load(connection).await?;
    let prefix = config.oid_prefix();

    let mut ducts = Vec::new();
    let mut duct_ids = Vec::new();
    let mut ducts_without_line = Vec::new();
    for row in duct_rows {
        match row.line.filter(|_| row.ends_located) {
            Some(line) => ducts.push(LkLinie {
                oid: oid(prefix, 't', row.id)?,
                letzte_aenderung: row.geaendert,
                // A straight line between the Schächte says nothing about the real course
                lagebestimmung: if row.has_course {
                    row.lagebestimmung
                } else {
                    Genauigkeit::Unbekannt
                },
                eigentuemer: row.eigentuemer.into(),
                breite_mm: row.breite_mm,
                line: line.points.iter().map(|p| (p.x, p.y)).collect(),
            }),
            None => {
                ducts_without_line.push(row.id);
                continue;
            }
        }
        duct_ids.push(row.id);
    }
    let mut schaechte = Vec::new();
    let mut schacht_ids = Vec::new();
    let mut schaechte_without_position = Vec::new();
    for row in schacht_rows {
        match row.geom {
            Some(position) => schaechte.push(LkPunkt {
                oid: oid(prefix, 's', row.id)?,
                letzte_aenderung: row.geaendert,
                lagebestimmung: row.lagebestimmung,
                eigentuemer: row.eigentuemer.into(),
                dimension1_mm: row.dimension1_mm,
                dimension2_mm: row.dimension2_mm,
                objektart: row.objektart,
                position: (position.x, position.y),
            }),
            None => {
                schaechte_without_position.push(row.id);
                continue;
            }
        }
        schacht_ids.push(row.id);
    }
    // Nothing with position: there's no place to deliver
    let letzte_aenderung = ducts
        .iter()
        .map(|d| d.letzte_aenderung)
        .chain(schaechte.iter().map(|s| s.letzte_aenderung))
        .max();
    let datenherr = config.datenherr_uid();
    let files = match letzte_aenderung {
        Some(letzte_aenderung) => {
            let area = sql_query(PERIMETER)
                .bind::<Array<Integer>, _>(&duct_ids)
                .bind::<Array<Integer>, _>(&schacht_ids)
                .bind::<Double, _>(config.perimeter_puffer_m())
                .get_result::<PerimeterRow>(connection)
                .await?;
            let (Some(outline), Some(outline_wgs84)) = (area.area, area.area_wgs84) else {
                return Err(ApiError::failed("PostGIS", "returned no perimeter"));
            };
            let outer = outline.rings.into_iter().next().unwrap_or_default();
            let perimeter = Perimeter {
                basket_id: basket_oid(prefix, 'p'),
                tid: basket_oid(prefix, 'z'),
                datenherr: datenherr.into(),
                datenlieferant: config.datenlieferant_uid().into(),
                letzte_aenderung,
                boundary: perimeter::rounded_ring(outer.iter().map(|p| (p.x, p.y))),
            };
            let delivery = Delivery {
                basket_id: basket_oid(prefix, 'b'),
                datenherr: datenherr.into(),
                datenlieferant: config.datenlieferant_uid().into(),
                schaechte: schaechte.into(),
                ducts: ducts.into(),
            };
            let file_name = |content: &str| {
                format!(
                    "{}-{content}.xtf",
                    datenherr.to_lowercase().replace('.', "-")
                )
            };
            let lkmap = TransferFile {
                file_name: file_name("kommunikation-lkmap").into(),
                xtf: xtf::write(&delivery)?,
            };
            let perimeter = TransferFile {
                file_name: file_name("zustaendigkeit-peri").into(),
                xtf: perimeter::write(&perimeter)?,
            };
            let checksum = checksum(&[&lkmap, &perimeter]);
            Some(Files {
                lkmap,
                perimeter,
                // PostGIS keeps the longitude in x
                perimeter_wgs84: outline_wgs84
                    .rings
                    .into_iter()
                    .next()
                    .map(|ring| ring.iter().map(|p| (p.y, p.x)).collect())
                    .unwrap_or_default(),
                checksum,
            })
        }
        None => None,
    };
    Ok(Export {
        datenherr: datenherr.into(),
        files,
        schaechte: schacht_ids.into(),
        ducts: duct_ids.into(),
        schaechte_without_position: schaechte_without_position.into(),
        ducts_without_line: ducts_without_line.into(),
    })
}

/// SHA-256 over the files in hex.
fn checksum(files: &[&TransferFile]) -> Box<str> {
    let mut hasher = Sha256::new();
    for file in files {
        hasher.update(&file.xtf);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The STANDARDOID of an object: the configured prefix (8 characters), a letter for its kind
/// and its id in 7 digits; also `OBJ_ID`. `s`: Schacht, `t`: duct.
fn oid(prefix: &str, letter: char, id: i32) -> Result<Box<str>, UserError> {
    if !(0..10_000_000).contains(&id) {
        let kind = match letter {
            's' => ObjectKind::Schacht,
            _ => ObjectKind::Duct,
        };
        return Err(UserError::LkmapIdTooLarge { kind, id });
    }
    Ok(format!("{prefix}{letter}{id:07}").into())
}

/// The OID of what the delivery has once: `b` the LKMap basket, `p` the perimeter's basket, `z`
/// the perimeter.
fn basket_oid(prefix: &str, letter: char) -> Box<str> {
    format!("{prefix}{letter}0000001").into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oids() {
        assert_eq!(oid("ch4711ab", 's', 42), Ok("ch4711abs0000042".into()));
        assert_eq!(
            oid("ch4711ab", 't', 9_999_999),
            Ok("ch4711abt9999999".into())
        );
        assert_eq!(basket_oid("ch4711ab", 'b'), "ch4711abb0000001".into());
        assert_eq!(
            oid("ch4711ab", 't', 10_000_000),
            Err(UserError::LkmapIdTooLarge {
                kind: ObjectKind::Duct,
                id: 10_000_000
            })
        );
    }

    #[test]
    fn zipped() {
        let file = TransferFile {
            file_name: "che-123-456-789-kommunikation-lkmap.xtf".into(),
            xtf: b"<TRANSFER/>".repeat(100).into(),
        };
        assert_eq!(
            file.zip_name(),
            "che-123-456-789-kommunikation-lkmap.zip".into()
        );
        let mut archive = zip::ZipArchive::new(Cursor::new(file.zip().unwrap())).unwrap();
        let mut entry = archive.by_index(0).unwrap();
        assert_eq!(entry.name(), &*file.file_name);
        let mut xtf = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut xtf).unwrap();
        assert_eq!(*xtf, *file.xtf);
    }

    #[test]
    fn checksums() {
        let file = |xtf: &str| TransferFile {
            file_name: Box::default(),
            xtf: xtf.as_bytes().into(),
        };
        assert_eq!(
            checksum(&[&file("a"), &file("b")]),
            // sha256("ab")
            "fb8e20fc2e4c3f248c60c39bd652f3c1347298bb977b8b4d5903b85055620603".into()
        );
        assert_ne!(
            checksum(&[&file("a"), &file("b")]),
            checksum(&[&file("a"), &file("c")])
        );
    }
}
