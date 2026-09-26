//! Lines read from GeoJSON and GPX files, e.g. a duct's course exported from QGIS or recorded
//! with a GPS device. Only reading: converting and fitting the line to its Schächte is left to
//! the backend (`checkDuctLine`).

use crate::graphql::authenticated::duct_properties::{CoordinateInput, CoordinateSystem};
use serde_json::Value;
use web_sys::{DomParser, Element, SupportedType};

/// A line in the file, with the name to choose it by.
#[derive(Debug, Clone, PartialEq)]
pub struct FileLine {
    pub name: String,
    /// East (or longitude) first, as in the file
    pub points: Vec<CoordinateInput>,
}

/// The lines of a file and the coordinate system it uses (declared or guessed).
#[derive(Debug, Clone, PartialEq)]
pub struct GeoFile {
    pub lines: Vec<FileLine>,
    pub system: CoordinateSystem,
}

/// Reads a `.gpx` file as GPX, everything else as GeoJSON.
pub fn read_geo_file(file_name: &str, text: &str) -> Result<GeoFile, String> {
    let file = if file_name.to_lowercase().ends_with(".gpx") {
        read_gpx(text)?
    } else {
        read_geojson(text)?
    };
    if file.lines.is_empty() {
        Err("Die Datei enthält keine Linie".to_string())
    } else {
        Ok(file)
    }
}

fn read_geojson(text: &str) -> Result<GeoFile, String> {
    let json: Value =
        serde_json::from_str(text).map_err(|error| format!("Kein gültiges GeoJSON: {error}"))?;
    let mut lines = Vec::new();
    collect_geojson(&json, "Linie", &mut lines);
    let system = declared_system(&json)
        .or_else(|| lines.first()?.points.first().map(guess_system))
        // RFC 7946: without other declaration GeoJSON is WGS84
        .unwrap_or(CoordinateSystem::Wgs84);
    Ok(GeoFile { lines, system })
}

/// The system of the `crs` member (GeoJSON 2008, still written by QGIS for other systems).
fn declared_system(json: &Value) -> Option<CoordinateSystem> {
    let name = json.pointer("/crs/properties/name")?.as_str()?;
    if name.ends_with("2056") {
        Some(CoordinateSystem::Lv95)
    } else if name.ends_with("21781") {
        Some(CoordinateSystem::Lv03)
    } else if name.ends_with("4326") || name.ends_with("CRS84") {
        Some(CoordinateSystem::Wgs84)
    } else {
        None
    }
}

/// Guesses the system from the size of the coordinates.
fn guess_system(point: &CoordinateInput) -> CoordinateSystem {
    if point.x > 2_000_000.0 {
        CoordinateSystem::Lv95
    } else if point.x > 180.0 {
        CoordinateSystem::Lv03
    } else {
        CoordinateSystem::Wgs84
    }
}

fn collect_geojson(json: &Value, name: &str, lines: &mut Vec<FileLine>) {
    match json.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => {
            for (index, feature) in json
                .get("features")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
            {
                collect_geojson(feature, &format!("Linie {}", index + 1), lines);
            }
        }
        Some("Feature") => {
            let properties = json.get("properties");
            let name = ["name", "Name", "description", "bezeichnung", "id"]
                .iter()
                .find_map(|key| properties?.get(*key))
                .map(|value| match value {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_else(|| name.to_string());
            if let Some(geometry) = json.get("geometry") {
                collect_geojson(geometry, &name, lines);
            }
        }
        Some("GeometryCollection") => {
            for geometry in json
                .get("geometries")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                collect_geojson(geometry, name, lines);
            }
        }
        Some("LineString") => {
            if let Some(points) = json.get("coordinates").and_then(positions) {
                lines.push(FileLine {
                    name: name.to_string(),
                    points,
                });
            }
        }
        Some("MultiLineString") => {
            let parts: Vec<Vec<CoordinateInput>> = json
                .get("coordinates")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(positions)
                .collect();
            // Parts following each other are one line, others are offered one by one
            match join_parts(&parts) {
                Some(points) => lines.push(FileLine {
                    name: name.to_string(),
                    points,
                }),
                None => {
                    for (index, points) in parts.into_iter().enumerate() {
                        lines.push(FileLine {
                            name: format!("{name} (Teil {})", index + 1),
                            points,
                        });
                    }
                }
            }
        }
        _ => {}
    }
}

/// The parts as one line, if each starts where the one before ends.
fn join_parts(parts: &[Vec<CoordinateInput>]) -> Option<Vec<CoordinateInput>> {
    let mut joined: Vec<CoordinateInput> = Vec::new();
    for part in parts {
        match (joined.last(), part.first()) {
            (Some(end), Some(start)) if end != start => return None,
            (Some(_), Some(_)) => joined.extend_from_slice(&part[1..]),
            _ => joined.extend_from_slice(part),
        }
    }
    Some(joined)
}

/// GeoJSON positions `[x, y, (z)]`.
fn positions(coordinates: &Value) -> Option<Vec<CoordinateInput>> {
    coordinates
        .as_array()?
        .iter()
        .map(|position| {
            let position = position.as_array()?;
            Some(CoordinateInput {
                x: position.first()?.as_f64()?,
                y: position.get(1)?.as_f64()?,
            })
        })
        .collect()
}

/// Tracks (all their segments in order) and routes; GPX is always WGS84.
fn read_gpx(text: &str) -> Result<GeoFile, String> {
    let invalid = |detail: String| format!("Kein gültiges GPX: {detail}");
    let parser = DomParser::new().map_err(|error| invalid(format!("{error:?}")))?;
    let document = parser
        .parse_from_string(text, SupportedType::ApplicationXml)
        .map_err(|error| invalid(format!("{error:?}")))?;
    if let Some(error) = document.get_elements_by_tag_name("parsererror").item(0) {
        return Err(invalid(error.text_content().unwrap_or_default()));
    }
    let mut lines = Vec::new();
    for (kind, point_tag) in [("trk", "trkpt"), ("rte", "rtept")] {
        let elements = document.get_elements_by_tag_name(kind);
        for index in 0..elements.length() {
            let Some(element) = elements.item(index) else {
                continue;
            };
            let name = child_text(&element, "name").unwrap_or_else(|| {
                let kind = if kind == "trk" { "Track" } else { "Route" };
                format!("{kind} {}", index + 1)
            });
            let points_found = element.get_elements_by_tag_name(point_tag);
            let points = (0..points_found.length())
                .filter_map(|i| points_found.item(i))
                .filter_map(|point| {
                    Some(CoordinateInput {
                        x: point.get_attribute("lon")?.trim().parse().ok()?,
                        y: point.get_attribute("lat")?.trim().parse().ok()?,
                    })
                })
                .collect::<Vec<_>>();
            if !points.is_empty() {
                lines.push(FileLine { name, points });
            }
        }
    }
    Ok(GeoFile {
        lines,
        system: CoordinateSystem::Wgs84,
    })
}

/// Text of the element's direct child with that tag (a track's own name, not a point's).
fn child_text(element: &Element, tag: &str) -> Option<String> {
    let children = element.children();
    (0..children.length())
        .filter_map(|i| children.item(i))
        .find(|child| child.tag_name().eq_ignore_ascii_case(tag))
        .and_then(|child| child.text_content())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}
