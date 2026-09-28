//! Values of the delivery to the Leitungskataster (SIA405 LKMap), see docs/leitungskataster.md.

use crate::db::schema;
use async_graphql::{Enum, Object};
use chrono::{DateTime, Utc};
use diesel::HasQuery;
use diesel_derive_enum::DbEnum;

/// `Lagebestimmung` of a Schacht or duct (SIA405 `Genauigkeit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, DbEnum, Enum)]
#[ExistingTypePath = "crate::db::schema::sql_types::GenauigkeitEnum"]
pub enum Genauigkeit {
    /// ±10 cm (from different measurements ±30 cm)
    #[db_rename = "genau"]
    Genau,
    #[db_rename = "ungenau"]
    Ungenau,
    #[db_rename = "unbekannt"]
    Unbekannt,
}

impl Genauigkeit {
    /// The value of `Lagebestimmung` in the transfer file.
    pub fn transfer_value(self) -> &'static str {
        match self {
            Genauigkeit::Genau => "genau",
            Genauigkeit::Ungenau => "ungenau",
            Genauigkeit::Unbekannt => "unbekannt",
        }
    }
}

/// `Objektart` of a Schacht type's Schächte as `LKPunkt` (`Kommunikation.…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, DbEnum, Enum)]
#[ExistingTypePath = "crate::db::schema::sql_types::LkmapPunktObjektartEnum"]
pub enum LkmapPunktObjektart {
    #[db_rename = "Schacht_rund"]
    SchachtRund,
    #[db_rename = "Schacht_rechteckig"]
    SchachtRechteckig,
    #[db_rename = "Bauwerk"]
    Bauwerk,
    #[db_rename = "Tragwerk"]
    Tragwerk,
    #[db_rename = "unbekannt"]
    Unbekannt,
}

impl LkmapPunktObjektart {
    /// The value of `LKPunkt.Objektart` in the transfer file.
    pub fn transfer_value(self) -> &'static str {
        match self {
            LkmapPunktObjektart::SchachtRund => "Kommunikation.Schacht.rund",
            LkmapPunktObjektart::SchachtRechteckig => "Kommunikation.Schacht.rechteckig",
            LkmapPunktObjektart::Bauwerk => "Kommunikation.Bauwerk",
            LkmapPunktObjektart::Tragwerk => "Kommunikation.Tragwerk",
            LkmapPunktObjektart::Unbekannt => "Kommunikation.unbekannt",
        }
    }
}

/// A delivery of an owner's transfer files (`lk_lieferung`): logged when they are downloaded,
/// marked when they reached the Checkservice.
#[derive(HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = schema::lk_lieferung)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct LkLieferung {
    pub id: i32,
    pub eigentuemer_id: i32,
    pub erstellt_am: DateTime<Utc>,
    pub erstellt_von: String,
    pub anzahl_schaechte: i32,
    pub anzahl_trassen: i32,
    pub pruefsumme: String,
    pub geliefert_am: Option<DateTime<Utc>>,
}

#[Object(name = "LkmapDelivery")]
impl LkLieferung {
    async fn id(&self) -> i32 {
        self.id
    }
    /// When the files were downloaded
    async fn created_at(&self) -> DateTime<Utc> {
        self.erstellt_am
    }
    /// Who downloaded them (user name)
    async fn created_by(&self) -> &str {
        &self.erstellt_von
    }
    async fn schacht_count(&self) -> i32 {
        self.anzahl_schaechte
    }
    async fn duct_count(&self) -> i32 {
        self.anzahl_trassen
    }
    /// SHA-256 of the files in hex, equal to the export's while nothing changed
    async fn checksum(&self) -> &str {
        &self.pruefsumme
    }
    /// When the files reached the Checkservice, missing while not confirmed
    async fn delivered_at(&self) -> Option<DateTime<Utc>> {
        self.geliefert_am
    }
}
