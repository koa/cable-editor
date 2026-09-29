//! Shared pieces for pages with a Leaflet map (`pages/map.rs`, `pages/cabinet/properties.rs`,
//! `pages/duct/show.rs`).
//! The page owns the map: it renders an empty `div` for it (Leaflet owns its children) and
//! creates the map in `rendered`.

use crate::{
    error::FrontendError,
    graphql::authenticated::{GeoPoint, schacht_types::icon_src},
};
use gloo_timers::callback::Timeout;
use js_sys::{Array, Function, Object, Reflect};
use leaflet::{
    CircleMarker, CircleOptions, Icon, LatLng, LatLngBounds, Layer, Map, MapOptions, Marker,
    MarkerOptions, MouseEvent, MouseEvents, Polyline, PolylineOptions, TileLayer, TileLayerOptions,
    Tooltip, TooltipOptions,
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{AddEventListenerOptions, HtmlElement, TouchEvent};
use yew::NodeRef;

/// Listener remembering the chosen background
type BackgroundChoice = Closure<dyn Fn(JsValue)>;

/// A component's map: its container, the map once created and the layers drawn for the
/// component's current state, replaced as a whole. Dropping it removes the map.
#[derive(Default)]
pub struct MapHolder {
    container: NodeRef,
    map: Option<Map>,
    layers: Vec<leaflet::Layer>,
    touch: Option<TouchGestures>,
    /// Must live as long as the map
    background_choice: Option<BackgroundChoice>,
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
            let touch = is_touch_screen();
            let (map, background_choice) =
                create_map(&container, touch).map_err(FrontendError::Map)?;
            self.map = Some(map);
            self.background_choice = Some(background_choice);
            if touch {
                self.touch = Some(TouchGestures::new(container).map_err(FrontendError::Map)?);
            }
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

/// Whether the device's main pointer is a finger (phones, tablets; not a laptop with a touch
/// screen, whose mouse drags the map as usual).
fn is_touch_screen() -> bool {
    web_sys::window()
        .and_then(|window| window.match_media("(pointer: coarse)").ok().flatten())
        .is_some_and(|query| query.matches())
}

/// Shown on the map while one finger moves across it
const TOUCH_HINT: &str = "Mit zwei Fingern verschieben";

/// How long the hint stays after the finger moved, in milliseconds
const TOUCH_HINT_MS: u32 = 1500;

/// On touch screens the map doesn't take one finger (`dragging` off), so it scrolls the page
/// instead of catching it; two fingers move and zoom the map (Leaflet's touch zoom follows their
/// centre). One finger moving across the map shows a hint how to move it (`.map-touch-hint`).
struct TouchGestures {
    container: HtmlElement,
    touchmove: Closure<dyn Fn(TouchEvent)>,
}

impl TouchGestures {
    fn new(container: HtmlElement) -> Result<Self, JsValue> {
        container.set_attribute("data-touch-hint", TOUCH_HINT)?;
        let hide: Rc<RefCell<Option<Timeout>>> = Rc::default();
        let element = container.clone();
        let touchmove = Closure::<dyn Fn(TouchEvent)>::new(move |event: TouchEvent| {
            let class_list = element.class_list();
            // Only fails for an invalid class name
            if event.touches().length() == 1 {
                let _ = class_list.add_1("map-touch-hint");
                let element = element.clone();
                // Replacing the timeout cancels the one before
                *hide.borrow_mut() = Some(Timeout::new(TOUCH_HINT_MS, move || {
                    let _ = element.class_list().remove_1("map-touch-hint");
                }));
            } else {
                hide.borrow_mut().take();
                let _ = class_list.remove_1("map-touch-hint");
            }
        });
        // Passive: the page scrolls without waiting for the listener
        let options = AddEventListenerOptions::new();
        options.set_passive(true);
        container.add_event_listener_with_callback_and_add_event_listener_options(
            "touchmove",
            touchmove.as_ref().unchecked_ref(),
            &options,
        )?;
        Ok(Self {
            container,
            touchmove,
        })
    }
}

impl Drop for TouchGestures {
    fn drop(&mut self) {
        // The listener must not outlive its closure
        let _ = self.container.remove_event_listener_with_callback(
            "touchmove",
            self.touchmove.as_ref().unchecked_ref(),
        );
    }
}

/// Center of Switzerland, shown while there is nothing to show.
const SWITZERLAND: (f64, f64) = (46.8, 8.23);

/// A map on the container with swisstopo's maps as background, showing Switzerland. On a touch
/// screen one finger doesn't move it (see `TouchGestures`).
fn create_map(container: &HtmlElement, touch: bool) -> Result<(Map, BackgroundChoice), JsValue> {
    let options = MapOptions::default();
    if touch {
        options.set_dragging(false);
    }
    let map = Map::new_with_element(container, &options)?;
    let background_choice = add_background(&map)?;
    map.set_view(&LatLng::new(SWITZERLAND.0, SWITZERLAND.1), 8.0);
    Ok((map, background_choice))
}

const BACKGROUND_MAP: &str = "Karte";
const BACKGROUND_AERIAL: &str = "Luftbild";
/// Where the chosen background is kept, so the next map starts with it
const BACKGROUND_KEY: &str = "map-background";

fn swisstopo_layer(layer: &str, extension: &str, max_native_zoom: f64) -> TileLayer {
    let options = TileLayerOptions::default();
    options.set_max_zoom(20.0);
    options.set_max_native_zoom(max_native_zoom);
    options.set_attribution("© swisstopo".to_string());
    TileLayer::new_options(
        &format!(
            "https://wmts.geo.admin.ch/1.0.0/{layer}/default/current/3857/{{z}}/{{x}}/{{y}}.{extension}"
        ),
        &options,
    )
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// swisstopo's national map (its tiles end at zoom 18, further in they are enlarged) or the
/// aerial image (sharp up to zoom 20), chosen with a control on the map and remembered for the
/// next one. The cadastral map (WMS/WMTS) isn't used: the server draws its lines per tile, so
/// they are offset or cut at every tile border. Returns the listener storing the choice.
fn add_background(map: &Map) -> Result<BackgroundChoice, JsValue> {
    let national_map = swisstopo_layer("ch.swisstopo.pixelkarte-farbe", "jpeg", 18.0);
    let aerial = swisstopo_layer("ch.swisstopo.swissimage", "jpeg", 20.0);
    let chosen =
        local_storage().and_then(|storage| storage.get_item(BACKGROUND_KEY).ok().flatten());
    if chosen.as_deref() == Some(BACKGROUND_AERIAL) {
        aerial.add_to(map);
    } else {
        national_map.add_to(map);
    }

    let leaflet = Reflect::get(&js_sys::global(), &JsValue::from_str("L"))?;
    let control = Reflect::get(&leaflet, &JsValue::from_str("control"))?;
    let layers: Function = Reflect::get(&control, &JsValue::from_str("layers"))?.dyn_into()?;
    let backgrounds = Object::new();
    set_option(&backgrounds, BACKGROUND_MAP, national_map.as_ref());
    set_option(&backgrounds, BACKGROUND_AERIAL, aerial.as_ref());
    let layers_control = layers.call1(&control, &backgrounds)?;
    let add_to: Function =
        Reflect::get(&layers_control, &JsValue::from_str("addTo"))?.dyn_into()?;
    add_to.call1(&layers_control, map.as_ref())?;

    let remember = Closure::<dyn Fn(JsValue)>::new(|event: JsValue| {
        let name = Reflect::get(&event, &JsValue::from_str("name"))
            .ok()
            .and_then(|name| name.as_string());
        if let (Some(name), Some(storage)) = (name, local_storage()) {
            // Storage may be full or blocked; the choice then only lasts for this map
            let _ = storage.set_item(BACKGROUND_KEY, &name);
        }
    });
    let on: Function = Reflect::get(map.as_ref(), &JsValue::from_str("on"))?.dyn_into()?;
    on.call2(
        map.as_ref(),
        &JsValue::from_str("baselayerchange"),
        remember.as_ref(),
    )?;
    Ok(remember)
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
    div_icon_with(class, size, None)
}

/// `div_icon` with `html` inside (a string is parsed as HTML, an element is taken as it is).
fn div_icon_with(class: &str, size: f64, html: Option<&JsValue>) -> Result<Icon, JsValue> {
    let leaflet = Reflect::get(&js_sys::global(), &JsValue::from_str("L"))?;
    let factory: Function = Reflect::get(&leaflet, &JsValue::from_str("divIcon"))?.dyn_into()?;
    let options = Object::new();
    set_option(&options, "className", &JsValue::from_str(class));
    let size_value = js_sys::Array::of2(&JsValue::from_f64(size), &JsValue::from_f64(size));
    set_option(&options, "iconSize", &size_value);
    if let Some(html) = html {
        set_option(&options, "html", html);
    }
    Ok(factory.call1(&leaflet, &options)?.unchecked_into())
}

/// Size of a Schacht type's icon on the map, in pixels
const SCHACHT_ICON_SIZE: f64 = 24.0;

/// A Schacht type's icon as an `<img>` (never inline, so a script in the SVG doesn't run).
fn schacht_icon(svg: &str) -> Result<Icon, JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let img = document.create_element("img")?;
    img.set_attribute("src", &icon_src(svg))?;
    img.set_attribute("alt", "")?;
    div_icon_with("map-view__schacht-icon", SCHACHT_ICON_SIZE, Some(&img))
}

/// Zoom at most when fitting the map to what it shows; the tiles of the map end at 18.
const FIT_MAX_ZOOM: f64 = 18.0;

/// A Schacht labelled with its name, as its type's icon (`svg`) or, without one, a circle; a
/// click on either calls `on_click`. The circle's colours come from the CSS class
/// (`.map-view__schacht`): Leaflet sets them as SVG attributes, which can't take tokens.
pub fn schacht_marker(
    name: &str,
    location: GeoPoint,
    icon: Option<&str>,
    on_click: impl Fn() + 'static,
) -> Layer {
    let on_click: Box<dyn Fn(MouseEvent)> = Box::new(move |_: MouseEvent| on_click());
    // An icon that can't be created (no document) leaves the circle
    let (marker, offset): (Layer, f64) = match icon.and_then(|svg| schacht_icon(svg).ok()) {
        Some(icon) => {
            let options = MarkerOptions::default();
            options.set_icon(icon);
            let marker = Marker::new_with_options(&lat_lng(location), &options);
            marker.on_click(on_click);
            (marker.unchecked_into(), SCHACHT_ICON_SIZE / 2.0)
        }
        None => {
            let options = CircleOptions::default();
            options.set_radius(7.0);
            options.set_class_name("map-view__schacht".to_string());
            let marker = CircleMarker::new_with_options(&lat_lng(location), &options);
            marker.on_click(on_click);
            (marker.unchecked_into(), 8.0)
        }
    };

    let tooltip = TooltipOptions::default();
    tooltip.set_permanent(true);
    tooltip.set_direction("right".to_string());
    tooltip.set_offset(leaflet::Point::new(offset, 0.0));
    // Lets a click on the name open the Schacht too, easier to hit on a phone
    set_option(&tooltip, "interactive", &JsValue::TRUE);
    // The crate's bind_tooltip_with_content calls a method Leaflet doesn't have
    let tooltip = Tooltip::new(&tooltip, None);
    tooltip.set_content(&JsValue::from_str(name));
    marker.bind_tooltip(&tooltip);
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
