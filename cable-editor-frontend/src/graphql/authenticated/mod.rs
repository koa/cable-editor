pub mod list_schacht;
pub mod list_schacht_typ;

#[cynic::schema("authenticated")]
mod schema {}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Point")]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
