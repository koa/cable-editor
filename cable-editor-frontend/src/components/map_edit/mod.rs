pub mod marker;

use crate::components::map::Point;
use futures::lock::Mutex;
use gloo_utils::document;
use leaflet::{LatLng, Map, MapOptions, Marker, MouseEvent, MouseEvents, TileLayerWms};
use marker::{MarkerLoader, NoDynamicMarkerLayer};
use std::{collections::HashMap, fmt::Debug, hash::Hash, sync::Arc};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, Node};
use yew::{Component, Context, Html, Properties, html, platform::spawn_local};

pub struct MapEditor<ML: MarkerLoader = NoDynamicMarkerLayer> {
    map: Map,
    container: HtmlElement,
    layers: Box<[Layer<ML>]>,
    center: Point,
    zoom: f64,
    markers: Arc<
        Mutex<
            HashMap<
                ML,
                HashMap<<<ML as MarkerLoader>::Data as ReferencedData>::Key, (ML::Data, Marker)>,
            >,
        >,
    >,
    active_layer: Option<ActiveLayer<ML>>,
}
#[derive(Clone, PartialEq, Debug)]
pub enum ActiveLayer<ML: MarkerLoader> {
    PointLayer(ML),
}
pub enum Msg<ML: MarkerLoader> {
    Moved,
    AddMarker(Marker),
    RemoveMarker(Marker),
    Clicked(LatLng),
    SetEditLayer(ActiveLayer<ML>),
}
#[derive(Debug, Clone, PartialEq, Properties)]
pub struct MapEditorProps<Loader: MarkerLoader = NoDynamicMarkerLayer> {
    pub center: Point,
    pub zoom: f64,
    pub layers: Box<[Layer<Loader>]>,
}
#[derive(Clone, PartialEq, Debug)]
pub enum Layer<ML: MarkerLoader> {
    PassiveTileLayer(TileLayerWms),
    DynamicPointLayer(ML),
}
pub trait ReferencedData: PartialEq {
    type Key: Debug + Clone + PartialEq + Hash + Eq;
    fn key_of(&self) -> Self::Key;
}
#[derive(PartialEq)]
pub enum NoData {}
impl ReferencedData for NoData {
    type Key = ();
    fn key_of(&self) -> Self::Key {}
}

impl<ML: MarkerLoader + 'static> Component for MapEditor<ML> {
    type Message = Msg<ML>;
    type Properties = MapEditorProps<ML>;

    fn create(ctx: &Context<Self>) -> Self {
        let props = ctx.props();

        let container: Element = document().create_element("div").unwrap();
        let container: HtmlElement = container
            .dyn_into()
            .expect("Failed to convert Element to HtmlElement");
        container.set_class_name("map");
        let options = MapOptions::default();
        let map = Map::new_with_element(&container, &options).expect("Failed to create map");
        map.set_max_zoom(20.0);
        let scope = ctx.link().clone();
        map.on_move_end(Box::from(move |_| {
            scope.send_message(Msg::Moved);
        }));

        Self {
            map,
            container,
            layers: props.layers.clone(),
            center: props.center,
            zoom: props.zoom,
            markers: Default::default(),
            active_layer: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Moved => {
                let bounds = self.map.get_bounds();
                let scope = ctx.link().clone();
                let markers = self.markers.clone();
                let layers = self.layers.clone();

                spawn_local(async move {
                    let mut markers = markers.lock().await;
                    for layer in layers {
                        match layer {
                            Layer::PassiveTileLayer(_) => {}
                            Layer::DynamicPointLayer(layer_key) => {
                                let mut old_entries =
                                    markers.remove(&layer_key).unwrap_or_default();
                                let mut new_entries = HashMap::new();
                                for marker_key in ML::list_points(&layer_key, &bounds).await {
                                    if let Some(existing) = old_entries.remove(&marker_key) {
                                        new_entries.insert(marker_key, existing);
                                    } else {
                                        if let Some(data) = layer_key.fetch_data(&marker_key).await
                                        {
                                            let marker = layer_key.render(&data);
                                            scope.send_message(Msg::AddMarker(marker.clone()));
                                            new_entries.insert(marker_key, (data, marker));
                                        }
                                    }
                                }
                                for (_, marker) in old_entries.into_values() {
                                    scope.send_message(Msg::RemoveMarker(marker));
                                }
                                markers.insert(layer_key, new_entries);
                            }
                        }
                    }
                });
                false
            }
            Msg::AddMarker(marker) => {
                marker.add_to(&self.map);
                false
            }
            Msg::RemoveMarker(marker) => {
                marker.remove_from(&self.map);
                false
            }
            Msg::Clicked(position) => {
                if let Some(layer) = self
                    .layers
                    .iter()
                    .filter_map(|layer| match layer {
                        Layer::PassiveTileLayer(_) => None,
                        Layer::DynamicPointLayer(l) => Some(l.clone()),
                    })
                    .next()
                {
                    let scope = ctx.link().clone();
                    spawn_local(async move {
                        let entry = layer.create_entry(&position).await;
                        scope.send_message(Msg::Moved);
                    })
                }
                false
            }
            Msg::SetEditLayer(l) => {
                if self.active_layer.as_ref() != Some(&l) {
                    let old_active_layer = self.active_layer.clone();
                    let scope = ctx.link().clone();
                    let markers = self.markers.clone();
                    let map = self.map.clone();
                    spawn_local(async move {
                        let mut markers = markers.lock().await;
                        match &old_active_layer {
                            None => {}
                            Some(ActiveLayer::PointLayer(l)) => {
                                if let Some(old_markers) = markers.get_mut(l) {
                                    for (data, marker) in old_markers.values() {
                                        marker.remove();
                                        let new_marker = l.render(data);
                                        new_marker.add_to(&map);
                                    }
                                }
                            }
                        };
                    });
                }
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <>
                <div class="map-container component-container">
                    {self.render_map()}
                </div>
            </>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            self.map
                .set_view(&LatLng::new(self.center.0, self.center.1), self.zoom);
            for layer in &self.layers {
                match layer {
                    Layer::PassiveTileLayer(l) => {
                        l.add_to(&self.map);
                    }
                    Layer::DynamicPointLayer(l) => ctx
                        .link()
                        .send_message(Msg::SetEditLayer(ActiveLayer::PointLayer(l.clone()))),
                }
            }
            {
                let scope = ctx.link().clone();
                self.map.on_mouse_click(Box::from(move |event: MouseEvent| {
                    let lng = event.lat_lng();
                    scope.send_message(Msg::Clicked(lng));
                }));
            }
        }
    }
}
impl<ML: MarkerLoader + 'static> MapEditor<ML> {
    fn render_map(&self) -> Html {
        let node: &Node = &self.container.clone().into();
        Html::VRef(node.clone())
    }
}
