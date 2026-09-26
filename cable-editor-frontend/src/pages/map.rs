use crate::{
    components::{
        links::{CableLink, SchachtLink},
        page_layout::PageLayout,
    },
    error::FrontendError,
    geo::map::{create_map, lat_lng, set_option},
    graphql::authenticated::{
        GeoPoint,
        map::{MapData, MapDuct, fetch_map_data},
    },
    pages::router::{AppRoute, CabinetView, PlanView},
    util::get_credentials,
};
use js_sys::{Array, Object};
use leaflet::{
    CircleMarker, CircleOptions, LatLngBounds, MouseEvent, MouseEvents, Polyline, PolylineOptions,
    Tooltip, TooltipOptions,
};
use patternfly_yew::prelude::{
    Alert, AlertType, Button, ButtonVariant, Card, CardBody, CardHeader, CardHeaderActionsObject,
    CardSize, CardTitle, DescriptionGroup, DescriptionList, Icon, Spinner,
};
use wasm_bindgen::JsValue;
use web_sys::HtmlElement;
use yew::{
    Callback, Component, Context, Html, NodeRef, Properties, html, html::IntoPropValue,
    platform::spawn_local,
};
use yew_nested_router::prelude::RouterContext;

/// Map of the plan's objects: the Schächte with a position, labelled with their name (a click
/// opens the Schacht's overview), and the ducts (a click selects one and shows its Schächte and
/// cables as links).
pub struct Map {
    container: NodeRef,
    map: Option<leaflet::Map>,
    data: Option<MapData>,
    error: Option<FrontendError>,
    /// Id of the duct whose details are shown
    selected_duct: Option<i32>,
    /// The selected duct drawn above the others
    highlight: Option<Polyline>,
}

pub enum Msg {
    Data(MapData),
    Error(FrontendError),
    OpenSchacht(i32),
    SelectDuct(Option<i32>),
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
            scope.send_message(match fetch_map_data(credentials.as_ref()).await {
                Ok(data) => Msg::Data(data),
                Err(error) => Msg::Error(error),
            });
        });
        Self {
            container: NodeRef::default(),
            map: None,
            data: None,
            error: None,
            selected_duct: None,
            highlight: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Data(data) => {
                if let Some(map) = &self.map {
                    show_data(ctx, map, &data);
                }
                self.data = Some(data);
                true
            }
            Msg::Error(error) => {
                self.error = Some(error);
                true
            }
            Msg::SelectDuct(id) => {
                if let Some(highlight) = self.highlight.take() {
                    highlight.remove();
                }
                self.selected_duct = id;
                if let (Some(map), Some(duct)) = (&self.map, self.selected()) {
                    self.highlight = duct.line.as_deref().map(|line| {
                        let options = duct_options("map-view__duct map-view__duct--selected");
                        options.set_interactive(false);
                        let highlight = Polyline::new_with_options(&points(line), &options);
                        highlight.add_to(map);
                        highlight
                    });
                }
                true
            }
            Msg::OpenSchacht(id) => {
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

    fn view(&self, ctx: &Context<Self>) -> Html {
        let status = if let Some(error) = &self.error {
            error.into_prop_value()
        } else {
            match &self.data {
                None => html!(<Spinner/>),
                Some(data) if data.schaechte.iter().all(|s| s.location.is_none()) => html! {
                    <Alert inline=true title="Kein Schacht hat eine Position" r#type={AlertType::Info}/>
                },
                Some(_) => Html::default(),
            }
        };
        let details = self.selected().map(|duct| view_duct(ctx, duct));
        // Leaflet owns the map container's children, so it must not get any from Yew. The
        // details' div is always there: Yew matches unkeyed siblings from the end, so a div
        // appearing after the map would take over the map's element.
        html! {
            <PageLayout title="Karte">
                {status}
                <div class="map-view">
                    <div class="map-view__map" ref={self.container.clone()}/>
                    <div class="map-view__details">{details}</div>
                </div>
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
        match create_map(&container) {
            Ok(map) => {
                // A click on a duct doesn't reach the map (bubbling_mouse_events)
                let scope = ctx.link().clone();
                map.on_mouse_click(Box::new(move |_: MouseEvent| {
                    scope.send_message(Msg::SelectDuct(None))
                }));
                if let Some(data) = &self.data {
                    show_data(ctx, &map, data);
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
        self.highlight = None;
        if let Some(map) = self.map.take() {
            map.remove();
        }
    }
}

impl Map {
    fn selected(&self) -> Option<&MapDuct> {
        let id = self.selected_duct?;
        self.data.as_ref()?.ducts.iter().find(|duct| duct.id == id)
    }
}

/// The selected duct's details in a card above the map.
fn view_duct(ctx: &Context<Map>, duct: &MapDuct) -> Html {
    let title = duct
        .description
        .clone()
        .unwrap_or_else(|| format!("Trasse {} – {}", duct.schacht_a.name, duct.schacht_z.name));
    let actions = CardHeaderActionsObject {
        actions: html! {
            <Button
                variant={ButtonVariant::Plain}
                icon={Icon::Times}
                aria_label="Schliessen"
                onclick={ctx.link().callback(|_| Msg::SelectDuct(None))}
            />
        },
        has_no_offset: false,
        class: Default::default(),
    };
    let cables = if duct.cables.is_empty() {
        html!("keine")
    } else {
        html! {
            <ul class="map-view__cables">
                {for duct.cables.iter().map(|cable| html! {
                    <li><CableLink id={cable.id} text={cable.name.clone()}/></li>
                })}
            </ul>
        }
    };
    html! {
        <Card size={CardSize::Compact}>
            <CardHeader actions={Some(actions)}>
                <CardTitle>{title}</CardTitle>
            </CardHeader>
            <CardBody>
                <DescriptionList compact=true>
                    <DescriptionGroup term="Schächte">
                        <SchachtLink id={duct.schacht_a.id} text={duct.schacht_a.name.clone()}/>
                        {" – "}
                        <SchachtLink id={duct.schacht_z.id} text={duct.schacht_z.name.clone()}/>
                    </DescriptionGroup>
                    <DescriptionGroup term="Kabel">{cables}</DescriptionGroup>
                </DescriptionList>
            </CardBody>
        </Card>
    }
}

fn show_data(ctx: &Context<Map>, map: &leaflet::Map, data: &MapData) {
    let corners = Array::new();
    // Ducts first, so the Schächte lie above them
    for duct in &data.ducts {
        let Some(line) = &duct.line else {
            continue;
        };
        for point in line {
            corners.push(&lat_lng(*point));
        }
        let options = duct_options("map-view__duct");
        options.set_interactive(false);
        Polyline::new_with_options(&points(line), &options).add_to(map);
        // A wide invisible line takes the clicks, the visible one is hard to hit on a phone
        let options = duct_options("map-view__duct-hit");
        options.set_weight(20.0);
        options.set_bubbling_mouse_events(false);
        let hit = Polyline::new_with_options(&points(line), &options);
        let id = duct.id;
        let scope = ctx.link().clone();
        hit.on_click(Box::new(move |_: MouseEvent| {
            scope.send_message(Msg::SelectDuct(Some(id)))
        }));
        hit.add_to(map);
    }
    for schacht in &data.schaechte {
        let Some(location) = schacht.location else {
            continue;
        };
        let position = lat_lng(location);
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
            scope.send_message(Msg::OpenSchacht(id))
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

/// The colours come from the CSS class, see `.map-view__schacht`.
fn duct_options(class: &str) -> PolylineOptions {
    let options = PolylineOptions::default();
    options.set_class_name(class.to_string());
    options
}

fn points(line: &[GeoPoint]) -> Array {
    line.iter()
        .map(|point| JsValue::from(lat_lng(*point)))
        .collect()
}
