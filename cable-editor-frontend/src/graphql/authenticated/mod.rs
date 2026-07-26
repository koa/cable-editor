pub mod cable_details;
pub mod list_cables;
pub mod list_schacht;
pub mod list_schacht_typ;
pub mod select_duct;

#[cynic::schema("authenticated")]
mod schema {}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Point")]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
