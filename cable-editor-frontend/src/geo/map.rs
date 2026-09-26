//! Shared pieces for pages with a Leaflet map (`pages/map.rs`, `pages/cabinet/properties.rs`).
//! The page owns the map: it renders an empty `div` for it (Leaflet owns its children) and
//! creates the map in `rendered`.

use crate::graphql::authenticated::GeoPoint;
use js_sys::{Function, Object, Reflect};
use leaflet::{
    Icon, LatLng, Map, MapOptions, TileLayer, TileLayerOptions, TileLayerWms, TileLayerWmsOptions,
};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::HtmlElement;

/// Center of Switzerland, shown while there is nothing to show.
const SWITZERLAND: (f64, f64) = (46.8, 8.23);

/// A map on the container with swisstopo's maps as background, showing Switzerland.
pub fn create_map(container: &HtmlElement) -> Result<Map, JsValue> {
    let map = Map::new_with_element(container, &MapOptions::default())?;
    add_background(&map);
    map.set_view(&LatLng::new(SWITZERLAND.0, SWITZERLAND.1), 8.0);
    Ok(map)
}

/// swisstopo's national map, from zoom 17 on the cadastral map (official survey).
fn add_background(map: &Map) {
    let options = TileLayerOptions::default();
    options.set_max_zoom(20.0);
    options.set_max_native_zoom(18.0);
    options.set_attribution("© swisstopo".to_string());
    TileLayer::new_options(
        "https://wmts.geo.admin.ch/1.0.0/ch.swisstopo.pixelkarte-farbe/default/current/3857/{z}/{x}/{y}.jpeg",
        &options,
    )
    .add_to(map);

    let options = TileLayerWmsOptions::default();
    options.set_layers("ch.kantone.cadastralwebmap-farbe".to_string());
    options.set_format("image/png".to_string());
    options.set_transparent(true);
    options.set_version("1.3.0".to_string());
    options.set_min_zoom(17.0);
    options.set_max_zoom(20.0);
    options.set_attribution("© Kantone, swisstopo".to_string());
    TileLayerWms::new_options("https://wms.geo.admin.ch/", &options).add_to(map);
}

pub fn lat_lng(point: GeoPoint) -> LatLng {
    LatLng::new(point.lat, point.lng)
}

/// Sets an option the crate has no setter for.
pub fn set_option(options: &Object, name: &str, value: &JsValue) {
    // Only fails on a frozen object or a throwing setter, which plain options aren't
    let _ = Reflect::set(options, &JsValue::from_str(name), value);
}

/// An icon drawn by CSS (the class) instead of an image. The crate's `DivIcon::new` creates an
/// `L.Icon` (it binds the wrong constructor), so this calls `L.divIcon` itself.
pub fn div_icon(class: &str, size: f64) -> Result<Icon, JsValue> {
    let leaflet = Reflect::get(&js_sys::global(), &JsValue::from_str("L"))?;
    let factory: Function = Reflect::get(&leaflet, &JsValue::from_str("divIcon"))?.dyn_into()?;
    let options = Object::new();
    set_option(&options, "className", &JsValue::from_str(class));
    let size_value = js_sys::Array::of2(&JsValue::from_f64(size), &JsValue::from_f64(size));
    set_option(&options, "iconSize", &size_value);
    Ok(factory.call1(&leaflet, &options)?.unchecked_into())
}
