use std::fmt::{self, Write};
use uuid::Uuid;

pub mod cabinet_details;
pub mod cable_details;
pub mod connections;
pub mod current_user;
pub mod edit_cabinet;
pub mod edit_ports;
pub mod list_cables;
pub mod list_plans;
pub mod list_schacht;
pub mod list_schacht_typ;
pub mod map;
pub mod netbox_sync;
pub mod panel_navigation;
pub mod panel_overview;
pub mod plan_details;
pub mod schacht_cables;
pub mod select_duct;

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

#[derive(cynic::QueryFragment, Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cynic(graphql_type = "Point")]
pub struct Point {
    pub x: f64,
    pub y: f64,
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
