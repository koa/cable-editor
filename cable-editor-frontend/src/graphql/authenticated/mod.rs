use std::fmt::{self, Write};
use uuid::Uuid;

pub mod cabinet_details;
pub mod cable_details;
pub mod connections;
pub mod current_user;
pub mod duct_details;
pub mod duct_properties;
pub mod edit_cabinet;
pub mod edit_ports;
pub mod list_cables;
pub mod list_ducts;
pub mod list_plans;
pub mod list_schacht;
pub mod map;
pub mod netbox_sync;
pub mod owners;
pub mod panel_navigation;
pub mod panel_overview;
pub mod plan_details;
pub mod schacht_cables;
pub mod schacht_properties;
pub mod schacht_types;

#[cynic::schema("authenticated")]
mod schema {}

/// Writes the path of a panel as users see it, "Schacht: Parent > Panel" (`panels` are the names
/// from the root panel down, unnamed ones left out). Without a Schacht it starts with the first
/// panel.
pub fn write_panel_path<'a>(
    out: &mut impl Write,
    schacht: Option<&str>,
    panels: impl IntoIterator<Item = &'a str>,
) -> fmt::Result {
    let mut separator = match schacht {
        Some(schacht) => {
            out.write_str(schacht)?;
            ": "
        }
        None => "",
    };
    for panel in panels {
        out.write_str(separator)?;
        out.write_str(panel)?;
        separator = " > ";
    }
    Ok(())
}

/// Writes a port after the path of its panel: " : label".
pub fn write_port_label(out: &mut impl Write, label: &str) -> fmt::Result {
    write!(out, " : {label}")
}

/// WGS84, for the map
#[derive(cynic::QueryFragment, Debug, Clone, Copy, PartialEq)]
pub struct GeoPoint {
    pub lat: f64,
    pub lng: f64,
}

/// LV95 (EPSG:2056), east and north in metres
#[derive(cynic::QueryFragment, Debug, Clone, Copy, PartialEq)]
pub struct Lv95Point {
    pub e: f64,
    pub n: f64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum IdOrNew {
    Id(i32),
    Temporary(Uuid),
}

impl From<i32> for IdOrNew {
    fn from(value: i32) -> Self {
        IdOrNew::Id(value)
    }
}

impl Default for IdOrNew {
    fn default() -> Self {
        IdOrNew::Temporary(Uuid::new_v4())
    }
}

#[derive(cynic::InputObject, Debug, Clone)]
#[cynic(graphql_type = "IdOrNewInput")]
pub struct IdOrNewInput {
    pub id: Option<i32>,
    pub temporary: Option<String>,
}

// Praktischer Helfer für die Konvertierung
impl From<IdOrNew> for IdOrNewInput {
    fn from(val: IdOrNew) -> Self {
        match val {
            IdOrNew::Id(id) => IdOrNewInput {
                id: Some(id),
                temporary: None,
            },
            IdOrNew::Temporary(uuid) => IdOrNewInput {
                id: None,
                temporary: Some(uuid.to_string()),
            },
        }
    }
}

/// `Lagebestimmung` of a Schacht or duct in the Leitungskataster (SIA405 `Genauigkeit`);
/// `Display` and `FromStr` are the value of its `FormSelect` option.
#[derive(
    Clone, Copy, PartialEq, Eq, Debug, Hash, cynic::Enum, strum::Display, strum::EnumString,
)]
pub enum Genauigkeit {
    Genau,
    Ungenau,
    Unbekannt,
}

impl Genauigkeit {
    pub const ALL: [Genauigkeit; 3] = [
        Genauigkeit::Genau,
        Genauigkeit::Ungenau,
        Genauigkeit::Unbekannt,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Genauigkeit::Genau => "genau (±10 cm)",
            Genauigkeit::Ungenau => "ungenau",
            Genauigkeit::Unbekannt => "unbekannt",
        }
    }
}

/// A point in time as the backend sends it (RFC 3339).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct DateTime(pub String);
cynic::impl_scalar!(DateTime, schema::DateTime);

impl DateTime {
    /// Date and time in the browser's time zone, e.g. "27.9.2026, 15:28:24".
    pub fn local(&self) -> String {
        let date = js_sys::Date::new(&self.0.as_str().into());
        if date.get_time().is_nan() {
            return self.0.clone();
        }
        date.to_locale_string("de-CH", &wasm_bindgen::JsValue::UNDEFINED)
            .into()
    }
}

// Das Enum für den Typ
#[derive(Clone, Copy, PartialEq, Eq, Debug, strum::Display, cynic::Enum, Hash, Ord, PartialOrd)]
#[cynic(graphql_type = "PanelPortType")]
pub enum PortType {
    Splice,
    Connector,
    Loop,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug, strum::Display, cynic::Enum, Hash, Ord, PartialOrd)]
#[cynic(graphql_type = "PortSide")]
pub enum PortSide {
    FRONT,
    BACK,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq, Eq, Hash)]
#[cynic(graphql_type = "Panel")]
pub struct ParentChainPanel {
    pub id: i32,
    pub name: Option<String>,
}
