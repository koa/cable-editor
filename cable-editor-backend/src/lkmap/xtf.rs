//! Writes the transfer file of an owner's delivery in the model `SIA405_LKMap_2015_LV95`
//! (INTERLIS 2.3 XTF), see docs/leitungskataster.md. Attributes follow the order of the model;
//! the XML writer escapes names and texts.

use crate::db::entity::lkmap::{Genauigkeit, LkmapPunktObjektart};
use chrono::NaiveDate;
use quick_xml::{
    Writer,
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event},
};
use std::io;

const MODEL: &str = "SIA405_LKMap_2015_LV95";
const MODEL_VERSION: &str = "27.04.2018";
const MODEL_URI: &str = "http://www.sia.ch/405";
const TOPIC: &str = "SIA405_LKMap_2015_LV95.SIA405_LKMap";
/// Every delivered object is in operation: the Leitungskataster gets only what exists, and
/// geometries aren't planned (plans only change ports)
const STATUS: &str = "in_Betrieb";
/// The system knows only ducts in the ground
const DUCT_OBJEKTART: &str = "Kommunikation.Trasse.unterirdisch";

/// What goes into one owner's file.
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery {
    /// BID of the basket
    pub basket_id: String,
    /// UID of the owner
    pub datenherr: String,
    /// UID of whoever delivers (configuration)
    pub datenlieferant: String,
    /// `Eigentuemer` of every object: the owner's name in the delivery
    pub eigentuemer: String,
    pub schaechte: Vec<LkPunkt>,
    pub ducts: Vec<LkLinie>,
}

/// A Schacht.
#[derive(Debug, Clone, PartialEq)]
pub struct LkPunkt {
    pub oid: String,
    pub letzte_aenderung: NaiveDate,
    pub lagebestimmung: Genauigkeit,
    pub dimension1_mm: Option<i32>,
    pub dimension2_mm: Option<i32>,
    pub objektart: LkmapPunktObjektart,
    /// LV95 (east, north)
    pub position: (f64, f64),
}

/// A duct.
#[derive(Debug, Clone, PartialEq)]
pub struct LkLinie {
    pub oid: String,
    pub letzte_aenderung: NaiveDate,
    pub lagebestimmung: Genauigkeit,
    pub breite_mm: Option<i32>,
    /// LV95 (east, north), from Schacht A to Z
    pub line: Vec<(f64, f64)>,
}

/// A model of a transfer file.
pub(super) struct Model {
    pub name: &'static str,
    pub version: &'static str,
    pub uri: &'static str,
}

const LKMAP: Model = Model {
    name: MODEL,
    version: MODEL_VERSION,
    uri: MODEL_URI,
};

/// An INTERLIS 2.3 transfer file (UTF-8) of one basket of `topic`, its objects written by
/// `objects`.
pub(super) fn transfer(
    model: &Model,
    topic: &str,
    basket_id: &str,
    objects: impl FnOnce(&mut Xtf) -> io::Result<()>,
) -> io::Result<Vec<u8>> {
    let mut xtf = Xtf(Writer::new_with_indent(Vec::new(), b' ', 1));
    xtf.0
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    xtf.start(
        "TRANSFER",
        &[("xmlns", "http://www.interlis.ch/INTERLIS2.3")],
    )?;
    xtf.start(
        "HEADERSECTION",
        &[("SENDER", "cable-editor"), ("VERSION", "2.3")],
    )?;
    xtf.start("MODELS", &[])?;
    xtf.empty(
        "MODEL",
        &[
            ("NAME", model.name),
            ("VERSION", model.version),
            ("URI", model.uri),
        ],
    )?;
    xtf.end("MODELS")?;
    xtf.end("HEADERSECTION")?;
    xtf.start("DATASECTION", &[])?;
    xtf.start(topic, &[("BID", basket_id)])?;
    objects(&mut xtf)?;
    xtf.end(topic)?;
    xtf.end("DATASECTION")?;
    xtf.end("TRANSFER")?;
    let mut bytes = xtf.0.into_inner();
    bytes.push(b'\n');
    Ok(bytes)
}

/// The transfer file of the delivery.
pub fn write(delivery: &Delivery) -> io::Result<Vec<u8>> {
    transfer(&LKMAP, TOPIC, &delivery.basket_id, |xtf| {
        write_objects(xtf, delivery)
    })
}

fn write_objects(xtf: &mut Xtf, delivery: &Delivery) -> io::Result<()> {
    for schacht in &delivery.schaechte {
        let class = format!("{TOPIC}.LKPunkt");
        xtf.start(&class, &[("TID", &schacht.oid)])?;
        xtf.lk_objekt(
            delivery,
            &schacht.oid,
            schacht.letzte_aenderung,
            schacht.lagebestimmung,
        )?;
        if let Some(dimension) = schacht.dimension1_mm {
            xtf.text("Dimension1", &dimension.to_string())?;
        }
        if let Some(dimension) = schacht.dimension2_mm {
            xtf.text("Dimension2", &dimension.to_string())?;
        }
        xtf.text("Objektart", schacht.objektart.transfer_value())?;
        xtf.start("SymbolPos", &[])?;
        xtf.coord(schacht.position)?;
        xtf.end("SymbolPos")?;
        xtf.end(&class)?;
    }
    for duct in &delivery.ducts {
        let class = format!("{TOPIC}.LKLinie");
        xtf.start(&class, &[("TID", &duct.oid)])?;
        xtf.lk_objekt(
            delivery,
            &duct.oid,
            duct.letzte_aenderung,
            duct.lagebestimmung,
        )?;
        if let Some(breite) = duct.breite_mm {
            xtf.text("Breite", &breite.to_string())?;
        }
        xtf.start("Linie", &[])?;
        xtf.start("POLYLINE", &[])?;
        for point in &duct.line {
            xtf.coord(*point)?;
        }
        xtf.end("POLYLINE")?;
        xtf.end("Linie")?;
        xtf.text("Objektart", DUCT_OBJEKTART)?;
        xtf.end(&class)?;
    }
    Ok(())
}

/// `Letzte_Aenderung` (`INTERLIS_1_DATE`)
pub(super) fn interlis_date(date: NaiveDate) -> String {
    date.format("%Y%m%d").to_string()
}

pub(super) struct Xtf(Writer<Vec<u8>>);

impl Xtf {
    pub(super) fn start(&mut self, name: &str, attributes: &[(&str, &str)]) -> io::Result<()> {
        let start = BytesStart::new(name).with_attributes(attributes.iter().copied());
        self.0.write_event(Event::Start(start))
    }
    pub(super) fn empty(&mut self, name: &str, attributes: &[(&str, &str)]) -> io::Result<()> {
        let empty = BytesStart::new(name).with_attributes(attributes.iter().copied());
        self.0.write_event(Event::Empty(empty))
    }
    pub(super) fn end(&mut self, name: &str) -> io::Result<()> {
        self.0.write_event(Event::End(BytesEnd::new(name)))
    }
    pub(super) fn text(&mut self, name: &str, text: &str) -> io::Result<()> {
        self.0
            .create_element(name)
            .write_text_content(BytesText::new(text))?;
        Ok(())
    }
    /// LV95 in metres, to the millimetre
    pub(super) fn coord(&mut self, (east, north): (f64, f64)) -> io::Result<()> {
        self.start("COORD", &[])?;
        self.text("C1", &format!("{east:.3}"))?;
        self.text("C2", &format!("{north:.3}"))?;
        self.end("COORD")
    }
    /// The attributes of `SIA405_BaseClass` and `LKObjekt`.
    fn lk_objekt(
        &mut self,
        delivery: &Delivery,
        oid: &str,
        letzte_aenderung: NaiveDate,
        lagebestimmung: Genauigkeit,
    ) -> io::Result<()> {
        self.text("OBJ_ID", oid)?;
        self.start("Metaattribute", &[])?;
        self.start("SIA405_Base_LV95.Metaattribute", &[])?;
        self.text("Datenherr", &delivery.datenherr)?;
        self.text("Datenlieferant", &delivery.datenlieferant)?;
        self.text("Letzte_Aenderung", &interlis_date(letzte_aenderung))?;
        self.end("SIA405_Base_LV95.Metaattribute")?;
        self.end("Metaattribute")?;
        self.text("Eigentuemer", &delivery.eigentuemer)?;
        self.text("Lagebestimmung", lagebestimmung.transfer_value())?;
        self.text("Status", STATUS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 7).expect("valid date")
    }

    fn delivery() -> Delivery {
        Delivery {
            basket_id: "ch4711abb0000001".into(),
            datenherr: "CHE-123.456.789".into(),
            datenlieferant: "CHE-987.654.321".into(),
            eigentuemer: "Müller & <Söhne>".into(),
            schaechte: vec![LkPunkt {
                oid: "ch4711abs0000042".into(),
                letzte_aenderung: date(),
                lagebestimmung: Genauigkeit::Genau,
                dimension1_mm: Some(1200),
                dimension2_mm: None,
                objektart: LkmapPunktObjektart::SchachtRund,
                position: (2709223.5604, 1253098.3196),
            }],
            ducts: vec![LkLinie {
                oid: "ch4711abt0000007".into(),
                letzte_aenderung: date(),
                lagebestimmung: Genauigkeit::Unbekannt,
                breite_mm: None,
                line: vec![(2709223.56, 1253098.32), (2709228.27, 1253125.88)],
            }],
        }
    }

    fn xtf() -> String {
        String::from_utf8(write(&delivery()).expect("writes")).expect("UTF-8")
    }

    #[test]
    fn header_and_basket() {
        let xtf = xtf();
        assert!(xtf.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xtf.contains(
            r#"<MODEL NAME="SIA405_LKMap_2015_LV95" VERSION="27.04.2018" URI="http://www.sia.ch/405"/>"#
        ));
        assert!(xtf.contains(r#"<SIA405_LKMap_2015_LV95.SIA405_LKMap BID="ch4711abb0000001">"#));
    }

    #[test]
    fn objects_in_model_order() {
        let xtf = xtf();
        let order = [
            r#"<SIA405_LKMap_2015_LV95.SIA405_LKMap.LKPunkt TID="ch4711abs0000042">"#,
            "<OBJ_ID>ch4711abs0000042</OBJ_ID>",
            "<Datenherr>CHE-123.456.789</Datenherr>",
            "<Datenlieferant>CHE-987.654.321</Datenlieferant>",
            "<Letzte_Aenderung>20260907</Letzte_Aenderung>",
            "<Lagebestimmung>genau</Lagebestimmung>",
            "<Status>in_Betrieb</Status>",
            "<Dimension1>1200</Dimension1>",
            "<Objektart>Kommunikation.Schacht.rund</Objektart>",
            "<C1>2709223.560</C1>",
            "<C2>1253098.320</C2>",
            r#"<SIA405_LKMap_2015_LV95.SIA405_LKMap.LKLinie TID="ch4711abt0000007">"#,
            "<Lagebestimmung>unbekannt</Lagebestimmung>",
            "<POLYLINE>",
            "<C1>2709228.270</C1>",
            "<Objektart>Kommunikation.Trasse.unterirdisch</Objektart>",
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
        assert!(!xtf.contains("<Dimension2>"));
        assert!(!xtf.contains("<Breite>"));
    }

    #[test]
    fn escapes_names() {
        assert!(xtf().contains("<Eigentuemer>Müller &amp; &lt;Söhne&gt;</Eigentuemer>"));
    }
}
