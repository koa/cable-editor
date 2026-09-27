//! Values of the delivery to the Leitungskataster (SIA405 LKMap), see docs/leitungskataster.md.

use async_graphql::Enum;
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
