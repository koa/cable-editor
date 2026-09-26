//! Coordinates typed by users: parsing, checks with correction proposals and formatting.
//! Converting between the systems is left to the backend (`convertPoint`).

use crate::graphql::authenticated::schacht_properties::{GeoPointInput, Lv95Input, PositionInput};
use std::ops::RangeInclusive;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoordinateSystem {
    #[default]
    Lv95,
    Wgs84,
}

impl CoordinateSystem {
    pub fn title(self) -> &'static str {
        match self {
            CoordinateSystem::Lv95 => "LV95",
            CoordinateSystem::Wgs84 => "WGS84",
        }
    }
    /// Labels of the two fields
    pub fn labels(self) -> (&'static str, &'static str) {
        match self {
            CoordinateSystem::Lv95 => ("Ost (E)", "Nord (N)"),
            CoordinateSystem::Wgs84 => ("Breite", "Länge"),
        }
    }
    pub fn placeholders(self) -> (&'static str, &'static str) {
        match self {
            CoordinateSystem::Lv95 => ("2 600 000.00", "1 200 000.00"),
            CoordinateSystem::Wgs84 => ("46.95108", "7.43864"),
        }
    }
    /// The two values as they are shown in the fields.
    pub fn format(self, first: f64, second: f64) -> (String, String) {
        match self {
            CoordinateSystem::Lv95 => (format_lv95(first), format_lv95(second)),
            // 6 decimals: about 0.1 m
            CoordinateSystem::Wgs84 => (format!("{first:.6}"), format!("{second:.6}")),
        }
    }
    pub fn input(self, first: f64, second: f64) -> PositionInput {
        match self {
            CoordinateSystem::Lv95 => PositionInput::Lv95(Lv95Input {
                e: first,
                n: second,
            }),
            CoordinateSystem::Wgs84 => PositionInput::Wgs84(GeoPointInput {
                lat: first,
                lng: second,
            }),
        }
    }
}

/// LV95 with a space as thousands separator and cm precision: `2 709 812.45`.
pub fn format_lv95(value: f64) -> String {
    let text = format!("{value:.2}");
    let (int, frac) = text.split_once('.').unwrap_or((&text, "00"));
    let mut grouped = String::new();
    for (i, digit) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 && digit.is_ascii_digit() {
            grouped.push(' ');
        }
        grouped.push(digit);
    }
    format!("{grouped}.{frac}")
}

/// A number as users type it: thousands separated by `'`, `’` or spaces, a decimal point or
/// comma.
pub fn parse_number(text: &str) -> Option<f64> {
    let cleaned: String = text
        .trim()
        .chars()
        .filter(|c| !matches!(c, '\'' | '’' | ' ' | '\u{a0}' | '\u{202f}'))
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    if cleaned.is_empty() {
        None
    } else {
        cleaned.parse().ok().filter(|v: &f64| v.is_finite())
    }
}

/// Both values pasted into one field, e.g. `47.41963, 8.88611` or `2709812 / 1262154`.
pub fn split_pair(text: &str) -> Option<(String, String)> {
    ["/", ";", "\t", ", "].iter().find_map(|separator| {
        let (first, second) = text.split_once(separator)?;
        parse_number(first)?;
        parse_number(second)?;
        Some((first.trim().to_string(), second.trim().to_string()))
    })
}

const LV95_EAST: RangeInclusive<f64> = 2_480_000.0..=2_840_000.0;
const LV95_NORTH: RangeInclusive<f64> = 1_070_000.0..=1_300_000.0;
const LV03_EAST: RangeInclusive<f64> = 480_000.0..=840_000.0;
const LV03_NORTH: RangeInclusive<f64> = 70_000.0..=300_000.0;
const LATITUDE: RangeInclusive<f64> = 45.7..=47.9;
const LONGITUDE: RangeInclusive<f64> = 5.8..=10.6;

/// Outcome of checking two typed values.
#[derive(Debug, Clone, PartialEq)]
pub enum Check {
    /// Plausible for Switzerland
    Valid(f64, f64),
    /// Not plausible, but probably meant as these values
    Proposal {
        reason: &'static str,
        first: f64,
        second: f64,
    },
    Invalid(&'static str),
}

pub fn check(system: CoordinateSystem, first: f64, second: f64) -> Check {
    match system {
        CoordinateSystem::Lv95 => {
            if LV95_EAST.contains(&first) && LV95_NORTH.contains(&second) {
                Check::Valid(first, second)
            } else if LV95_EAST.contains(&second) && LV95_NORTH.contains(&first) {
                Check::Proposal {
                    reason: "Ost und Nord scheinen vertauscht",
                    first: second,
                    second: first,
                }
            } else if LV03_EAST.contains(&first) && LV03_NORTH.contains(&second) {
                // LV95 = LV03 + 2 000 000 / 1 000 000, up to about 1.5 m off
                Check::Proposal {
                    reason: "Das sind LV03-Koordinaten (ohne führende 2 bzw. 1)",
                    first: first + 2_000_000.0,
                    second: second + 1_000_000.0,
                }
            } else if LATITUDE.contains(&first) && LONGITUDE.contains(&second) {
                Check::Invalid("Das sind WGS84-Koordinaten, bitte auf WGS84 umschalten")
            } else {
                Check::Invalid("Die Position liegt nicht in der Schweiz")
            }
        }
        CoordinateSystem::Wgs84 => {
            if LATITUDE.contains(&first) && LONGITUDE.contains(&second) {
                Check::Valid(first, second)
            } else if LATITUDE.contains(&second) && LONGITUDE.contains(&first) {
                Check::Proposal {
                    reason: "Breite und Länge scheinen vertauscht",
                    first: second,
                    second: first,
                }
            } else if LV95_EAST.contains(&first) || LV03_EAST.contains(&first) {
                Check::Invalid("Das sind LV95-Koordinaten, bitte auf LV95 umschalten")
            } else {
                Check::Invalid("Die Position liegt nicht in der Schweiz")
            }
        }
    }
}
