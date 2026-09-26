//! Positions entered by users, in LV95 or WGS84. The database stores LV95 (EPSG:2056); PostGIS
//! does every conversion, so there is only one.

use crate::{
    db::entity::{WGS84, st_transform},
    graphql::model::{GeoPoint, Lv95Point},
};
use async_graphql::{InputObject, OneofObject, SimpleObject};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use postgis_diesel::types::Point;

/// SRID of LV95, what the database stores.
pub const LV95: u32 = 2056;

/// Range of LV95 coordinates in Switzerland (and Liechtenstein), with some margin.
const EAST: std::ops::RangeInclusive<f64> = 2_480_000.0..=2_840_000.0;
const NORTH: std::ops::RangeInclusive<f64> = 1_070_000.0..=1_300_000.0;

#[derive(InputObject, Debug, Clone, Copy, PartialEq)]
pub struct Lv95Input {
    pub e: f64,
    pub n: f64,
}

#[derive(InputObject, Debug, Clone, Copy, PartialEq)]
pub struct GeoPointInput {
    pub lat: f64,
    pub lng: f64,
}

#[derive(OneofObject, Debug, Clone, Copy, PartialEq)]
pub enum PositionInput {
    Lv95(Lv95Input),
    Wgs84(GeoPointInput),
}

/// A position in both systems.
#[derive(SimpleObject, Debug, Clone, Copy, PartialEq)]
pub struct ConvertedPoint {
    pub lv95: Lv95Point,
    pub wgs84: GeoPoint,
}

/// The position as LV95 point, refused outside of Switzerland.
pub async fn to_lv95(
    connection: &mut AsyncPgConnection,
    position: PositionInput,
) -> async_graphql::Result<Point> {
    let point = match position {
        PositionInput::Lv95(Lv95Input { e, n }) => Point::new(e, n, Some(LV95)),
        PositionInput::Wgs84(GeoPointInput { lat, lng }) => {
            let wgs84 = Point::new(lng, lat, Some(WGS84 as u32));
            diesel::select(st_transform(wgs84, LV95 as i32))
                .get_result::<Option<Point>>(connection)
                .await?
                .ok_or("Position konnte nicht umgerechnet werden")?
        }
    };
    if EAST.contains(&point.x) && NORTH.contains(&point.y) {
        Ok(point)
    } else {
        Err(format!(
            "Position E {:.2} / N {:.2} liegt nicht in der Schweiz",
            point.x, point.y
        )
        .into())
    }
}

/// An LV95 point in WGS84.
pub async fn to_wgs84(
    connection: &mut AsyncPgConnection,
    point: Point,
) -> async_graphql::Result<GeoPoint> {
    Ok(diesel::select(st_transform(point, WGS84))
        .get_result::<Option<Point>>(connection)
        .await?
        .ok_or("Position konnte nicht umgerechnet werden")?
        .into())
}

pub async fn convert(
    connection: &mut AsyncPgConnection,
    position: PositionInput,
) -> async_graphql::Result<ConvertedPoint> {
    let lv95 = to_lv95(connection, position).await?;
    Ok(ConvertedPoint {
        lv95: lv95.into(),
        wgs84: to_wgs84(connection, lv95).await?,
    })
}
