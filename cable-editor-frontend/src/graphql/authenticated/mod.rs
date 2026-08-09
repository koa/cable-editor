use uuid::Uuid;

pub mod cabinet_details;
pub mod cable_details;
pub mod edit_ports;
pub mod list_cables;
pub mod list_plans;
pub mod list_schacht;
pub mod list_schacht_typ;
pub mod plan_details;
pub mod select_duct;

#[cynic::schema("authenticated")]
mod schema {}

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
