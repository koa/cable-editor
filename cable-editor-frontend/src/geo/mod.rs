//! Geodata in the frontend: coordinates typed by users, lines read from files and the Leaflet
//! maps. Not components: the pages own their maps and call these helpers.

pub mod coordinates;
pub mod geo_file;
pub mod map;
