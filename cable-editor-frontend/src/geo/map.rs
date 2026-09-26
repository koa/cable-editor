//! Shared pieces for pages with a Leaflet map (`pages/map.rs`, `pages/cabinet/properties.rs`,
//! `pages/duct/show.rs`).
//! The page owns the map: it renders an empty `div` for it (Leaflet owns its children) and
//! creates the map in `rendered`.

use crate::{error::FrontendError, graphql::authenticated::GeoPoint};
use js_sys::{Array, Function, Object, Reflect};
use leaflet::{
    CircleMarker, CircleOptions, Icon, LatLng, LatLngBounds, Map, MapOptions, MouseEvent,
    MouseEvents, Polyline, PolylineOptions, TileLayer, TileLayerOptions, TileLayerWms,
    TileLayerWmsOptions, Tooltip, TooltipOptions,
};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::HtmlElement;
use yew::NodeRef;

/// A component's map: its container, the map once created and the layers drawn for the
/// component's current state, replaced as a whole. Dropping it removes the map.
#[derive(Default)]
pub struct MapHolder {
    container: NodeRef,
    map: Option<Map>,
    layers: Vec<leaflet::Layer>,
}

impl MapHolder {
    /// For the empty `div` the map fills; it must stay the same element (Yew matches unkeyed
    /// siblings from the end, so keep optional siblings before it or always rendered).
    pub fn container(&self) -> NodeRef {
        self.container.clone()
    }

    /// Creates the map in the rendered container, on the component's first render.
    pub fn create(&mut self) -> Result<(), FrontendError> {
        if let Some(container) = self.container.cast::<HtmlElement>() {
            self.map = Some(create_map(&container).map_err(FrontendError::Map)?);
        }
        Ok(())
    }

    pub fn map(&self) -> Option<&Map> {
        self.map.as_ref()
    }

    /// Removes the layers drawn before and draws these instead (none: just removes).
    pub fn replace_layers(&mut self, layers: Vec<leaflet::Layer>) {
        for layer in self.layers.drain(..) {
            layer.remove();
        }
        if let Some(map) = &self.map {
            for layer in &layers {
                layer.add_to(map);
            }
            self.layers = layers;
        }
    }
}

impl Drop for MapHolder {
    fn drop(&mut self) {
        if let Some(map) = self.map.take() {
            map.remove();
        }
    }
}

/// Center of Switzerland, shown while there is nothing to show.
const SWITZERLAND: (f64, f64) = (46.8, 8.23);

/// A map on the container with swisstopo's maps as background, showing Switzerland.
fn create_map(container: &HtmlElement) -> Result<Map, JsValue> {
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

/// Zoom at most when fitting the map to what it shows; the cadastral map starts at 17.
const FIT_MAX_ZOOM: f64 = 18.0;

/// A Schacht as a circle labelled with its name; a click on either calls `on_click`. The
/// colours come from the CSS class (`.map-view__schacht`): Leaflet sets them as SVG attributes,
/// which can't take tokens.
pub fn schacht_marker(
    name: &str,
    location: GeoPoint,
    on_click: impl Fn() + 'static,
) -> CircleMarker {
    let options = CircleOptions::default();
    options.set_radius(7.0);
    options.set_class_name("map-view__schacht".to_string());
    let marker = CircleMarker::new_with_options(&lat_lng(location), &options);

    let tooltip = TooltipOptions::default();
    tooltip.set_permanent(true);
    tooltip.set_direction("right".to_string());
    tooltip.set_offset(leaflet::Point::new(8.0, 0.0));
    // Lets a click on the name open the Schacht too, easier to hit on a phone
    set_option(&tooltip, "interactive", &JsValue::TRUE);
    // The crate's bind_tooltip_with_content calls a method Leaflet doesn't have
    let tooltip = Tooltip::new(&tooltip, None);
    tooltip.set_content(&JsValue::from_str(name));
    marker.bind_tooltip(&tooltip);

    marker.on_click(Box::new(move |_: MouseEvent| on_click()));
    marker
}

/// A duct's line, not clickable; `class` sets the look (`.map-view__duct`, `--selected`).
pub fn duct_line(line: &[GeoPoint], class: &str) -> Polyline {
    let options = PolylineOptions::default();
    options.set_class_name(class.to_string());
    options.set_interactive(false);
    Polyline::new_with_options(&points(line), &options)
}

/// A wide invisible line over a duct that takes the clicks, the visible one is hard to hit on
/// a phone. The click doesn't reach the map. `class` adds to `map-view__duct-hit`, e.g. a
/// colour on hover.
pub fn duct_hit_line(line: &[GeoPoint], class: &str, on_click: impl Fn() + 'static) -> Polyline {
    let options = PolylineOptions::default();
    options.set_class_name(format!("map-view__duct-hit {class}"));
    options.set_weight(20.0);
    options.set_bubbling_mouse_events(false);
    let hit = Polyline::new_with_options(&points(line), &options);
    hit.on_click(Box::new(move |_: MouseEvent| on_click()));
    hit
}

/// Shows all the points (if any).
pub fn fit_points<'a>(map: &Map, points: impl IntoIterator<Item = &'a GeoPoint>) {
    let corners: Array = points
        .into_iter()
        .map(|point| JsValue::from(lat_lng(*point)))
        .collect();
    if corners.length() > 0 {
        let options = Object::new();
        set_option(&options, "maxZoom", &JsValue::from_f64(FIT_MAX_ZOOM));
        map.fit_bounds_with_options(&LatLngBounds::new_from_list(&corners), &options);
    }
}

fn points(line: &[GeoPoint]) -> Array {
    line.iter()
        .map(|point| JsValue::from(lat_lng(*point)))
        .collect()
}

/// A text following the pointer over the layer, e.g. what a click does.
pub fn hover_text(layer: &leaflet::Layer, text: &str) {
    let options = TooltipOptions::default();
    options.set_sticky(true);
    options.set_direction("top".to_string());
    let tooltip = Tooltip::new(&options, None);
    tooltip.set_content(&JsValue::from_str(text));
    layer.bind_tooltip(&tooltip);
}
