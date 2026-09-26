use async_graphql::{Object, SimpleObject};
use postgis_diesel::types;

pub struct Point(types::Point);
#[Object]
impl Point {
    async fn x(&self) -> f64 {
        self.0.x
    }
    async fn y(&self) -> f64 {
        self.0.y
    }
}
impl From<types::Point> for Point {
    fn from(point: types::Point) -> Self {
        Self(point)
    }
}

/// WGS84 (EPSG:4326), for the map. The database stores LV95 (EPSG:2056), queries convert it
/// with `ST_Transform(geom, WGS84)`.
#[derive(SimpleObject, Clone, Copy, Debug, PartialEq)]
pub struct GeoPoint {
    pub lat: f64,
    pub lng: f64,
}
impl From<types::Point> for GeoPoint {
    fn from(point: types::Point) -> Self {
        // PostGIS keeps the longitude in x
        Self {
            lat: point.y,
            lng: point.x,
        }
    }
}

/// LV95 (EPSG:2056), what the database stores: east and north in metres.
#[derive(SimpleObject, Clone, Copy, Debug, PartialEq)]
pub struct Lv95Point {
    pub e: f64,
    pub n: f64,
}
impl From<types::Point> for Lv95Point {
    fn from(point: types::Point) -> Self {
        Self {
            e: point.x,
            n: point.y,
        }
    }
}
