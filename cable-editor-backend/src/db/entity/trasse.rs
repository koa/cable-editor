use diesel::{HasQuery, Identifiable};
use postgis_diesel::types::{LineString, Point};

#[derive(Identifiable, HasQuery, Debug, Clone, PartialEq)]
#[diesel(table_name = crate::db::schema::trassen_mit_endpunkten)]
pub struct TrasseMitEndpunkten {
    pub id: i32,
    pub sa_id: i32,
    pub sa_name: Option<String>,
    pub sz_id: i32,
    pub sz_name: Option<String>,
    pub geom: Option<LineString<Point>>,
}
