//! Writes an owner's Zuständigkeitsperimeter in the model `Perimeter_LK_ZH_V2_LV95` of the
//! canton (see docs/leitungskataster.md): the area its delivered ducts and Schächte lie in.

use super::xtf::{Model, interlis_date, transfer};
use chrono::NaiveDate;
use std::io;

const MODEL: Model = Model {
    name: "Perimeter_LK_ZH_V2_LV95",
    version: "2019-04-16",
    uri: "http://models.geo.zh.ch",
};
const TOPIC: &str = "Perimeter_LK_ZH_V2_LV95.Perimeter_LK_ZH";

/// The perimeter of one owner.
#[derive(Debug, Clone, PartialEq)]
pub struct Perimeter {
    pub basket_id: Box<str>,
    /// The model has no OID, the TID only needs to be unique in the file
    pub tid: Box<str>,
    /// UID of the owner
    pub datenherr: Box<str>,
    /// UID of whoever delivers (configuration)
    pub datenlieferant: Box<str>,
    /// The last change of the delivered objects
    pub letzte_aenderung: NaiveDate,
    /// The outer boundary in LV95 (east, north), closed
    pub boundary: Box<[(f64, f64)]>,
}

/// The transfer file, UTF-8.
pub fn write(perimeter: &Perimeter) -> io::Result<Box<[u8]>> {
    transfer(&MODEL, TOPIC, &perimeter.basket_id, |xtf| {
        let class = format!("{TOPIC}.Perimeter");
        xtf.start(&class, &[("TID", &perimeter.tid)])?;
        // Communication only: the system knows no other medium
        xtf.text("Medium", "Kommunikation")?;
        xtf.text("Datenherr", &perimeter.datenherr)?;
        xtf.text("Datenlieferant", &perimeter.datenlieferant)?;
        xtf.text(
            "Letzte_Aenderung",
            &interlis_date(perimeter.letzte_aenderung),
        )?;
        xtf.text("Art", "Zustaendigkeitsperimeter")?;
        xtf.start("Geometrie", &[])?;
        xtf.start("SURFACE", &[])?;
        xtf.start("BOUNDARY", &[])?;
        xtf.start("POLYLINE", &[])?;
        for point in &perimeter.boundary {
            xtf.coord(*point)?;
        }
        xtf.end("POLYLINE")?;
        xtf.end("BOUNDARY")?;
        xtf.end("SURFACE")?;
        xtf.end("Geometrie")?;
        xtf.end(&class)
    })
}

/// The ring rounded to millimetres, as the file has them, without points that fall together
/// then (INTERLIS refuses a line with the same point twice in a row).
pub fn rounded_ring(points: impl IntoIterator<Item = (f64, f64)>) -> Box<[(f64, f64)]> {
    let mut ring: Vec<(f64, f64)> = Vec::new();
    for (east, north) in points {
        let point = (
            (east * 1000.0).round() / 1000.0,
            (north * 1000.0).round() / 1000.0,
        );
        if ring.last() != Some(&point) {
            ring.push(point);
        }
    }
    ring.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_perimeter() {
        let xtf = write(&Perimeter {
            basket_id: "ch4711abp0000001".into(),
            tid: "ch4711abz0000001".into(),
            datenherr: "CHE-123.456.789".into(),
            datenlieferant: "CHE-987.654.321".into(),
            letzte_aenderung: NaiveDate::from_ymd_opt(2026, 9, 27).expect("valid date"),
            boundary: vec![(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (0.0, 0.0)].into(),
        })
        .expect("writes");
        let xtf = String::from_utf8(xtf.into_vec()).expect("UTF-8");
        assert!(xtf.contains(
            r#"<MODEL NAME="Perimeter_LK_ZH_V2_LV95" VERSION="2019-04-16" URI="http://models.geo.zh.ch"/>"#
        ));
        assert!(xtf.contains(
            r#"<Perimeter_LK_ZH_V2_LV95.Perimeter_LK_ZH.Perimeter TID="ch4711abz0000001">"#
        ));
        let order = [
            "<Medium>Kommunikation</Medium>",
            "<Datenherr>CHE-123.456.789</Datenherr>",
            "<Datenlieferant>CHE-987.654.321</Datenlieferant>",
            "<Letzte_Aenderung>20260927</Letzte_Aenderung>",
            "<Art>Zustaendigkeitsperimeter</Art>",
            "<SURFACE>",
            "<BOUNDARY>",
            "<C1>1.000</C1>",
        ];
        let mut from = 0;
        for part in order {
            let found = xtf[from..].find(part);
            assert!(
                found.is_some(),
                "{part} missing after position {from}:\n{xtf}"
            );
            from += found.unwrap_or_default() + part.len();
        }
    }

    #[test]
    fn rounds_the_ring() {
        let ring = rounded_ring([(0.0001, 0.0), (0.0004, 0.0), (1.00049, 2.0), (0.0, 0.0)]);
        assert_eq!(ring, vec![(0.0, 0.0), (1.0, 2.0), (0.0, 0.0)].into());
    }
}
