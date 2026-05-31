use async_graphql::Object;
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
