use crate::util::get_backdrop;
use crate::{
    error::FrontendError,
    graphql::authenticated::list_schacht_typ::{SchachtTypListEntry, fetch_schacht_type_list},
    util::GuardAppHandle,
};
use base64::{Engine, engine};
use gloo_utils::document;
use leaflet::{
    LatLng, Map, MapOptions, Marker, MarkerOptions, MouseEvent, MouseEvents, Popup, PopupOptions,
    TileLayerWms, TileLayerWmsOptions,
};
use log::info;
use patternfly_yew::prelude::{
    ActionGroup, Backdrop, Backdropper, Bullseye, Button, Form, Menu, MenuAction, Modal,
    SelectItemRenderer, ToggleGroup, ToggleGroupItem,
};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, HtmlElement, Node, window};
use yew::{
    Callback, Component, Context, Html, Properties, function_component, html, html_nested,
    platform::spawn_local,
};
use yew_oauth2::prelude::OAuth2Context;

pub enum Msg {
    AddMarker(LatLng),
    ClearAllMarkers,
    RemoveMarker(Marker),
    SetSchachtTypeList(Box<[SchachtTypListEntry]>),
    SetError(FrontendError),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Point(pub f64, pub f64);

#[derive(PartialEq, Properties, Clone)]
pub struct Props {
    pub center: Point,
}

impl MapComponent {
    fn render_map(&self) -> Html {
        let node: &Node = &self.container.clone().into();
        Html::VRef(node.clone())
    }
}

pub fn add_tile_layer(map: &Map) {
    let av_options = TileLayerWmsOptions::default();
    av_options.set_layers("ch.kantone.cadastralwebmap-farbe".to_string());
    av_options.set_format("image/png".to_string());
    //av_options.set_detect_retina(true);
    av_options.set_transparent(true);
    av_options.set_version("1.3.0".to_string());
    av_options.set_max_zoom(20.0);
    TileLayerWms::new_options("https://wms.geo.admin.ch/", &av_options).add_to(map);
    let basemap_options = TileLayerWmsOptions::default();
    basemap_options.set_layers("ch.swisstopo.pixelkarte-farbe".to_string());
    basemap_options.set_format("image/jpeg".to_string());
    basemap_options.set_detect_retina(true);
    basemap_options.set_max_zoom(17.0);
    TileLayerWms::new_options("https://wms.geo.admin.ch/", &basemap_options).add_to(map);
}

pub struct MapComponent {
    map: Map,
    center: Point,
    container: HtmlElement,
    markers: Vec<Marker>,
    error: Option<FrontendError>,
    schacht_typ_list: Box<[SchachtTypListEntry]>,
}

impl Component for MapComponent {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let container: Element = document().create_element("div").unwrap();
        let container: HtmlElement = container.dyn_into().unwrap();
        container.set_class_name("map");
        let options = MapOptions::default();
        let map = Map::new_with_element(&container, &options).unwrap();
        map.set_max_zoom(20.0);

        Self {
            map,
            container,
            center: props.center,
            markers: vec![],
            error: None,
            schacht_typ_list: Box::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::AddMarker(lng) => {
                info!("Clicked on map {lng:?}");
                let marker_options = MarkerOptions::new();
                marker_options.set_draggable(true);
                /*if let Some(entry)=self.schacht_typ_list.first(){
                //let icon_options=IconOptions::default();
                    let encoded = engine::general_purpose::STANDARD.encode(entry.icon.as_bytes());
                    icon_options.set_icon_url(format!("data:image/svg+xml;base64,{encoded}"));
                    icon_options.set_icon_anchor(leaflet::Point::new(15f64,15f64));
                    icon_options.set_icon_size(leaflet::Point::new(30f64,30f64));
                //marker_options.set_icon(Icon::new(&icon_options));
                }*/
                let marker = Marker::new_with_options(&lng, &marker_options);
                let map = self.map.clone();
                let marker_clone = marker.clone();
                let scope = ctx.link().clone();
                marker.on_click(Box::from(move |event: MouseEvent| {
                    let popup_options = PopupOptions::default();
                    popup_options.set_min_width(200.0);
                    let popup = Popup::new_with_lat_lng(&event.lat_lng(), &popup_options);
                    let div_container: Element = window()
                        .unwrap()
                        .document()
                        .unwrap()
                        .create_element("div")
                        .unwrap();
                    let remove_marker_callback: Callback<()> = {
                        let marker_clone = marker_clone.clone();
                        let popup = popup.clone();
                        let scope = scope.clone();
                        Callback::from(move |_| {
                            popup.remove();
                            scope.send_message(Msg::RemoveMarker(marker_clone.clone()));
                        })
                    };
                    let close_popup_callback: Callback<()> = {
                        let popup = popup.clone();
                        Callback::from(move |_| {
                            popup.remove();
                        })
                    };
                    let guard: GuardAppHandle<_> =
                        yew::Renderer::<MarkerPopup>::with_root_and_props(
                            div_container.clone(),
                            MarkerPopupProps {
                                entries: vec![
                                    MenuEntry {
                                        text: html!("Marker entfernen"),
                                        callback: remove_marker_callback,
                                    },
                                    MenuEntry {
                                        text: html!("Schliessen"),
                                        callback: close_popup_callback,
                                    },
                                ],
                            },
                        )
                        .render()
                        .into();
                    let value: JsValue = div_container.into();
                    popup.set_content(&value).open_on(&map);
                    info!("Clicked on marker {event:?}");
                }));

                if let Some((backdropper)) = get_backdrop(ctx.link()) {
                    let marker_clone = marker.clone();
                    let onclose = {
                        let scope = ctx.link().clone();
                        let backdropper = backdropper.clone();
                        Callback::from(move |_| {
                            backdropper.close();
                            scope.send_message(Msg::RemoveMarker(marker_clone.clone()));
                        })
                    };
                    let toggle_group_items = self.schacht_typ_list.iter().map(|entry| {
                        let encoded = engine::general_purpose::STANDARD.encode(entry.icon.as_bytes());
                        let url = format!("data:image/svg+xml;base64,{encoded}");
                        html_nested! {
                            <ToggleGroupItem text={entry.label()} icon={html!{<img src={url} width="24" height="24"/>}}/>
                        }
                    });
                    backdropper.open(Backdrop::new(html! {
                            <Bullseye>
                                <Modal title="Schacht setzen" onclose={onclose}>
                                    <Form>
                                        <ToggleGroup>{for toggle_group_items}</ToggleGroup>
                                        <ActionGroup>
                                            <Button label="Schacht setzen" />
                                        </ActionGroup>
                                    </Form>
                                </Modal>
                            </Bullseye>
                    }));
                }
                marker.add_to(&self.map);
                self.markers.push(marker);
                false
            }
            Msg::ClearAllMarkers => {
                for marker in self.markers.iter() {
                    marker.remove();
                }
                self.markers.clear();
                false
            }
            Msg::RemoveMarker(marker) => {
                marker.remove();
                self.markers.retain(|m| m != &marker);
                false
            }
            Msg::SetSchachtTypeList(schacht_typ_list) => {
                self.schacht_typ_list = schacht_typ_list;
                true
            }
            Msg::SetError(e) => {
                self.error = Some(e);
                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        let props = ctx.props();

        if self.center == props.center {
            false
        } else {
            self.center = props.center;
            self.map
                .set_view(&LatLng::new(self.center.0, self.center.1), 11.0);
            true
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let onclick = ctx.link().callback(|_| Msg::ClearAllMarkers);

        html! {
            <>
            <Button {onclick}>{"Clear all markers"}</Button>
            <div class="map-container component-container">
                {self.render_map()}
            </div>
            </>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            self.map
                .set_view(&LatLng::new(self.center.0, self.center.1), 15.0);
            add_tile_layer(&self.map);
            {
                let scope = ctx.link().clone();
                self.map.on_mouse_click(Box::from(move |event: MouseEvent| {
                    let lng = event.lat_lng();
                    scope.send_message(Msg::AddMarker(lng));
                }));
            }
            let scope = ctx.link().clone();
            let credentials = scope
                .context::<OAuth2Context>(Callback::noop())
                .map(|r| r.0);

            spawn_local(async move {
                match fetch_schacht_type_list(credentials.as_ref()).await {
                    Ok(list) => {
                        scope.send_message(Msg::SetSchachtTypeList(list));
                    }
                    Err(e) => {
                        scope.send_message(Msg::SetError(e));
                    }
                }
            })
        }
    }
}

#[derive(Properties, Clone, PartialEq)]
struct MarkerPopupProps {
    entries: Vec<MenuEntry>,
}
#[derive(Properties, Clone, PartialEq)]
struct MenuEntry {
    text: Html,
    callback: Callback<()>,
}

#[function_component(MarkerPopup)]
fn marker_component(props: &MarkerPopupProps) -> Html {
    let entries = props.entries.iter().map(|entry|{
        html_nested!(<MenuAction onclick={entry.callback.clone()}>{entry.text.clone()}</MenuAction>)
    });
    html! {
        <Menu>
            {for entries}
        </Menu>
    }
}

fn void_callback<E>(remove_marker_callback: Callback<()>) -> Callback<E> {
    Callback::from(move |_| remove_marker_callback.emit(()))
}
