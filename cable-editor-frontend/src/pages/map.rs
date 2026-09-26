use crate::{
    components::page_layout::PageLayout,
    error::FrontendError,
    graphql::authenticated::map::{MapSchacht, fetch_map_schaechte},
    pages::router::{AppRoute, CabinetView, PlanView},
    util::get_credentials,
};
use js_sys::{Array, Object, Reflect};
use leaflet::{
    CircleMarker, CircleOptions, LatLng, LatLngBounds, MapOptions, MouseEvent, MouseEvents,
    TileLayer, TileLayerOptions, TileLayerWms, TileLayerWmsOptions, Tooltip, TooltipOptions,
};
use patternfly_yew::prelude::{Alert, AlertType, Spinner};
use wasm_bindgen::JsValue;
use web_sys::HtmlElement;
use yew::{
    Callback, Component, Context, Html, NodeRef, Properties, html, html::IntoPropValue,
    platform::spawn_local,
};
use yew_nested_router::prelude::RouterContext;

/// Center of Switzerland, shown while no Schacht has a position.
const SWITZERLAND: (f64, f64) = (46.8, 8.23);

/// Map of the plan's objects: for now the Schächte with a position, labelled with their name;
/// a click opens the Schacht's overview.
pub struct Map {
    container: NodeRef,
    map: Option<leaflet::Map>,
    schaechte: Option<Vec<MapSchacht>>,
    error: Option<FrontendError>,
}

pub enum Msg {
    Data(Vec<MapSchacht>),
    Error(FrontendError),
    Open(i32),
}

#[derive(Clone, PartialEq, Properties)]
pub struct MapProps {
    pub plan_id: i32,
}

impl Component for Map {
    type Message = Msg;
    type Properties = MapProps;

    fn create(ctx: &Context<Self>) -> Self {
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        spawn_local(async move {
            scope.send_message(match fetch_map_schaechte(credentials.as_ref()).await {
                Ok(data) => Msg::Data(data),
                Err(error) => Msg::Error(error),
            });
        });
        Self {
            container: NodeRef::default(),
            map: None,
            schaechte: None,
            error: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Data(schaechte) => {
                if let Some(map) = &self.map {
                    show_schaechte(ctx, map, &schaechte);
                }
                self.schaechte = Some(schaechte);
                true
            }
            Msg::Error(error) => {
                self.error = Some(error);
                true
            }
            Msg::Open(id) => {
                if let Some((router, _)) = ctx
                    .link()
                    .context::<RouterContext<AppRoute>>(Callback::noop())
                {
                    router.push(AppRoute::Plan {
                        plan_id: ctx.props().plan_id,
                        view: PlanView::Cabinet {
                            id,
                            view: CabinetView::Overview,
                        },
                    });
                }
                false
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let status = if let Some(error) = &self.error {
            error.into_prop_value()
        } else {
            match &self.schaechte {
                None => html!(<Spinner/>),
                Some(list) if list.iter().all(|s| s.location.is_none()) => html! {
                    <Alert inline=true title="Kein Schacht hat eine Position" r#type={AlertType::Info}/>
                },
                Some(_) => Html::default(),
            }
        };
        // Leaflet owns the container's children, so it must not get any from Yew
        html! {
            <PageLayout title="Karte">
                {status}
                <div class="map-view" ref={self.container.clone()}/>
            </PageLayout>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if !first_render {
            return;
        }
        let Some(container) = self.container.cast::<HtmlElement>() else {
            return;
        };
        match leaflet::Map::new_with_element(&container, &MapOptions::default()) {
            Ok(map) => {
                add_background(&map);
                map.set_view(&LatLng::new(SWITZERLAND.0, SWITZERLAND.1), 8.0);
                if let Some(schaechte) = &self.schaechte {
                    show_schaechte(ctx, &map, schaechte);
                }
                self.map = Some(map);
            }
            Err(error) => {
                ctx.link()
                    .send_message(Msg::Error(FrontendError::Map(error)));
            }
        }
    }

    fn destroy(&mut self, _ctx: &Context<Self>) {
        if let Some(map) = self.map.take() {
            map.remove();
        }
    }
}

/// swisstopo's national map, from zoom 17 on the cadastral map (official survey).
fn add_background(map: &leaflet::Map) {
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

fn show_schaechte(ctx: &Context<Map>, map: &leaflet::Map, schaechte: &[MapSchacht]) {
    let corners = Array::new();
    for schacht in schaechte {
        let Some(location) = schacht.location else {
            continue;
        };
        let position = LatLng::new(location.lat, location.lng);
        corners.push(&position);

        let options = CircleOptions::default();
        options.set_radius(7.0);
        options.set_class_name("map-view__schacht".to_string());
        let marker = CircleMarker::new_with_options(&position, &options);

        let tooltip = TooltipOptions::default();
        tooltip.set_permanent(true);
        tooltip.set_direction("right".to_string());
        tooltip.set_offset(leaflet::Point::new(8.0, 0.0));
        // Lets a click on the name open the Schacht too, easier to hit on a phone
        set_option(&tooltip, "interactive", &JsValue::TRUE);
        // The crate's bind_tooltip_with_content calls a method Leaflet doesn't have
        let tooltip = Tooltip::new(&tooltip, None);
        tooltip.set_content(&JsValue::from_str(&schacht.name));
        marker.bind_tooltip(&tooltip);

        let id = schacht.id;
        let scope = ctx.link().clone();
        marker.on_click(Box::new(move |_: MouseEvent| {
            scope.send_message(Msg::Open(id))
        }));
        marker.add_to(map);
    }
    if corners.length() > 0 {
        // A single Schacht would otherwise be shown at the deepest zoom
        let options = Object::new();
        set_option(&options, "maxZoom", &JsValue::from_f64(18.0));
        map.fit_bounds_with_options(&LatLngBounds::new_from_list(&corners), &options);
    }
}

/// Sets an option the crate has no setter for.
fn set_option(options: &Object, name: &str, value: &JsValue) {
    // Only fails on a frozen object or a throwing setter, which plain options aren't
    let _ = Reflect::set(options, &JsValue::from_str(name), value);
}
