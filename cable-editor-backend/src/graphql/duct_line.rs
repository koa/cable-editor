//! A duct's course from a file: converted to LV95, turned to run from Schacht A to Schacht Z
//! and trimmed where its ends repeat the Schächte. The stored line holds only the points
//! between the Schächte; the view trassen_mit_endpunkten puts the Schächte around them.

use crate::{
    db::{entity::st_transform, schema},
    graphql::{geo::LV95, model::GeoPoint},
};
use async_graphql::{Enum, InputObject, SimpleObject};
use diesel::{ExpressionMethods, QueryDsl};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use postgis_diesel::types::{LineString, Point};

/// An end of the line closer to its Schacht than this repeats the Schacht and is left out.
pub const SAME_POINT_METRES: f64 = 0.5;
/// An end of the line further from its Schacht than this is only stored when confirmed: the
/// file or the Schacht is probably the wrong one.
pub const CONFIRM_DISTANCE_METRES: f64 = 10.0;

/// Coordinate system of the points in a file.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateSystem {
    /// EPSG:2056
    #[graphql(name = "LV95")]
    Lv95,
    /// EPSG:21781, the older Swiss system
    #[graphql(name = "LV03")]
    Lv03,
    /// EPSG:4326
    #[graphql(name = "WGS84")]
    Wgs84,
}

impl CoordinateSystem {
    fn srid(self) -> u32 {
        match self {
            CoordinateSystem::Lv95 => LV95,
            CoordinateSystem::Lv03 => 21781,
            CoordinateSystem::Wgs84 => 4326,
        }
    }
}

/// A point as files store it: east (or longitude) first.
#[derive(InputObject, Debug, Clone, Copy, PartialEq)]
pub struct CoordinateInput {
    pub x: f64,
    pub y: f64,
}

#[derive(InputObject, Debug, Clone, PartialEq)]
pub struct LineInput {
    pub system: CoordinateSystem,
    pub points: Vec<CoordinateInput>,
}

/// What `fit` made of a line.
#[derive(Debug, Clone, PartialEq)]
pub struct FittedLine {
    /// The points between the Schächte, LV95; empty: a straight line
    pub points: Vec<Point>,
    /// The line ran from Schacht Z to Schacht A
    pub reversed: bool,
    /// Ends left out because they repeat their Schacht
    pub removed_ends: u32,
    /// Metres from the line's start to Schacht A and from its end to Schacht Z, before trimming
    pub start_distance: f64,
    pub end_distance: f64,
}

impl FittedLine {
    pub fn needs_confirmation(&self) -> bool {
        self.start_distance.max(self.end_distance) > CONFIRM_DISTANCE_METRES
    }
}

fn distance(a: &Point, b: &Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// Turns the line (LV95) to run from `a` to `z` and leaves out ends repeating them.
pub fn fit(mut points: Vec<Point>, a: &Point, z: &Point) -> Result<FittedLine, &'static str> {
    let (Some(first), Some(last)) = (points.as_slice().first(), points.as_slice().last()) else {
        return Err("Der Verlauf enthält keine Punkte");
    };
    if distance(a, z) < SAME_POINT_METRES {
        return Err("Anfangs- und Endschacht liegen am selben Ort, die Richtung ist unbestimmt");
    }
    let reversed = distance(first, z) + distance(last, a) < distance(first, a) + distance(last, z);
    if reversed {
        points.reverse();
    }
    // Not empty, checked above
    let start_distance = points
        .as_slice()
        .first()
        .map_or(0.0, |first| distance(first, a));
    let end_distance = points
        .as_slice()
        .last()
        .map_or(0.0, |last| distance(last, z));

    let before = points.len();
    let mut trimmed = points.clone();
    if trimmed.len() > 1 && end_distance < SAME_POINT_METRES {
        trimmed.pop();
    }
    if start_distance < SAME_POINT_METRES {
        trimmed.remove(0);
    }
    // A single point can't be stored as line; one that close to its Schacht doesn't hurt
    let points = if trimmed.len() == 1 { points } else { trimmed };
    let removed_ends = (before - points.len()) as u32;
    Ok(FittedLine {
        points,
        reversed,
        removed_ends,
        start_distance,
        end_distance,
    })
}

/// The line's points in LV95.
pub async fn to_lv95(
    connection: &mut AsyncPgConnection,
    line: &LineInput,
) -> async_graphql::Result<Vec<Point>> {
    if line.points.is_empty() {
        return Err("Der Verlauf enthält keine Punkte".into());
    }
    let srid = line.system.srid();
    let points: Vec<Point> = line
        .points
        .iter()
        .map(|p| Point::new(p.x, p.y, Some(srid)))
        .collect();
    if srid == LV95 {
        return Ok(points);
    }
    // A LineString needs two points
    let single = points.len() == 1;
    let mut input = points;
    if single {
        input.push(input[0]);
    }
    let converted = diesel::select(st_transform(
        LineString {
            points: input,
            srid: Some(srid),
        },
        LV95 as i32,
    ))
    .get_result::<Option<LineString<Point>>>(connection)
    .await?
    .ok_or("Verlauf konnte nicht umgerechnet werden")?;
    let mut points = converted.points;
    if single {
        points.truncate(1);
    }
    Ok(points)
}

/// Position of a Schacht in LV95, which a duct's course needs to be checked.
pub async fn schacht_position(
    connection: &mut AsyncPgConnection,
    schacht_id: i32,
) -> async_graphql::Result<Point> {
    let (name, geom): (Option<String>, Option<Point>) = schema::schacht::table
        .filter(schema::schacht::id.eq(schacht_id))
        .select((schema::schacht::name, schema::schacht::geom))
        .first(connection)
        .await?;
    geom.ok_or_else(|| {
        format!(
            "Schacht {} hat keine Position, der Verlauf kann nicht geprüft werden",
            name.unwrap_or_else(|| schacht_id.to_string())
        )
        .into()
    })
}

/// Converts, turns and trims the line for the duct between the Schächte.
pub async fn fit_line(
    connection: &mut AsyncPgConnection,
    schacht_a: i32,
    schacht_z: i32,
    line: &LineInput,
) -> async_graphql::Result<(FittedLine, Point, Point)> {
    let a = schacht_position(connection, schacht_a).await?;
    let z = schacht_position(connection, schacht_z).await?;
    let points = to_lv95(connection, line).await?;
    let fitted = fit(points, &a, &z)?;
    Ok((fitted, a, z))
}

/// The line as stored (LV95, without the Schächte); `None` for a straight line.
pub fn stored_line(fitted: &FittedLine) -> Option<LineString<Point>> {
    (fitted.points.len() >= 2).then(|| LineString {
        points: fitted.points.clone(),
        srid: Some(LV95),
    })
}

/// Result of checking a line, for the preview before storing it.
#[derive(SimpleObject, Debug, Clone, PartialEq)]
pub struct DuctLineCheck {
    /// The whole course from Schacht A to Schacht Z, WGS84
    pub line: Vec<GeoPoint>,
    pub reversed: bool,
    pub removed_ends: i32,
    /// Metres from the file's line to Schacht A and to Schacht Z
    pub start_distance: f64,
    pub end_distance: f64,
    /// Metres
    pub length: f64,
    /// An end is further than 10 m from its Schacht: storing needs `confirmed`
    pub needs_confirmation: bool,
}

pub async fn check(
    connection: &mut AsyncPgConnection,
    schacht_a: i32,
    schacht_z: i32,
    line: &LineInput,
) -> async_graphql::Result<DuctLineCheck> {
    let (fitted, a, z) = fit_line(connection, schacht_a, schacht_z, line).await?;
    let mut course = vec![a];
    course.extend(fitted.points.iter().copied());
    course.push(z);
    let length = course.windows(2).map(|w| distance(&w[0], &w[1])).sum();
    let wgs84 = diesel::select(st_transform(
        LineString {
            points: course,
            srid: Some(LV95),
        },
        crate::db::entity::WGS84,
    ))
    .get_result::<Option<LineString<Point>>>(connection)
    .await?
    .ok_or("Verlauf konnte nicht umgerechnet werden")?;
    Ok(DuctLineCheck {
        line: wgs84.points.into_iter().map(GeoPoint::from).collect(),
        reversed: fitted.reversed,
        removed_ends: fitted.removed_ends as i32,
        start_distance: fitted.start_distance,
        end_distance: fitted.end_distance,
        length,
        needs_confirmation: fitted.needs_confirmation(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y, Some(LV95))
    }
    const A: (f64, f64) = (2_700_000.0, 1_200_000.0);
    const Z: (f64, f64) = (2_700_100.0, 1_200_000.0);

    fn fit_xy(points: &[(f64, f64)]) -> FittedLine {
        let points = points.iter().map(|&(x, y)| p(x, y)).collect();
        fit(points, &p(A.0, A.1), &p(Z.0, Z.1)).expect("fits")
    }

    fn xs(line: &FittedLine) -> Vec<f64> {
        line.points.iter().map(|p| p.x - A.0).collect()
    }

    #[test]
    fn keeps_a_line_in_the_right_direction() {
        let line = fit_xy(&[(A.0 + 2.0, A.1), (A.0 + 50.0, A.1 + 5.0), (Z.0 - 2.0, Z.1)]);
        assert!(!line.reversed);
        assert_eq!(xs(&line), vec![2.0, 50.0, 98.0]);
        assert_eq!(line.removed_ends, 0);
        assert!((line.start_distance - 2.0).abs() < 1e-9);
        assert!(!line.needs_confirmation());
    }

    #[test]
    fn reverses_a_line_from_z_to_a() {
        let line = fit_xy(&[(Z.0 - 2.0, Z.1), (A.0 + 50.0, A.1 + 5.0), (A.0 + 2.0, A.1)]);
        assert!(line.reversed);
        assert_eq!(xs(&line), vec![2.0, 50.0, 98.0]);
    }

    #[test]
    fn leaves_out_ends_repeating_the_schaechte() {
        let line = fit_xy(&[
            (A.0 + 0.1, A.1),
            (A.0 + 30.0, A.1 + 5.0),
            (A.0 + 60.0, A.1 + 5.0),
            (Z.0, Z.1 + 0.2),
        ]);
        assert_eq!(xs(&line), vec![30.0, 60.0]);
        assert_eq!(line.removed_ends, 2);
    }

    #[test]
    fn a_line_of_only_the_schaechte_is_straight() {
        let line = fit_xy(&[(Z.0, Z.1), (A.0, A.1)]);
        assert!(line.reversed);
        assert!(line.points.is_empty());
        assert_eq!(line.removed_ends, 2);
    }

    #[test]
    fn keeps_an_end_rather_than_a_single_point() {
        let line = fit_xy(&[(A.0 + 0.25, A.1), (A.0 + 50.0, A.1 + 5.0)]);
        assert_eq!(xs(&line), vec![0.25, 50.0]);
        assert_eq!(line.removed_ends, 0);
    }

    #[test]
    fn far_ends_need_confirmation() {
        let line = fit_xy(&[(A.0 + 30.0, A.1), (Z.0, Z.1)]);
        assert!((line.start_distance - 30.0).abs() < 1e-9);
        assert!(line.needs_confirmation());
    }

    #[test]
    fn refuses_schaechte_at_the_same_place() {
        let a = p(A.0, A.1);
        let error = fit(vec![p(A.0 + 5.0, A.1)], &a, &a);
        assert!(error.is_err());
    }
}
