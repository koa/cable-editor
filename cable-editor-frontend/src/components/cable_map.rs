//! The map of the cable editor (`pages/cable/edit.rs`): all ducts and Schächte, the cable's
//! path drawn over them as it is being edited (not only as stored). While editing, a click on
//! a duct connecting to an end of the path adds it there, a click on the first or last segment
//! removes it; a cable without path starts with any duct.

use crate::{
    error::FrontendError,
    geo::map::{MapHolder, duct_hit_line, duct_line, fit_points, hover_text, schacht_marker},
    graphql::authenticated::{
        GeoPoint,
        cable_details::{CableDuct, CablePath, CableSegmentEndSchacht},
        map::{MapData, MapDuct, MapDuctEnd, fetch_map_data},
    },
    util::get_credentials,
};
use patternfly_yew::prelude::Spinner;
use wasm_bindgen::JsCast;
use yew::{
    Callback, Component, Context, Html, Properties, html, html::IntoPropValue,
    platform::spawn_local,
};

/// An end of the cable's path.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PathEnd {
    /// At `CablePath::near_schacht`
    Front,
    /// At the last segment's far Schacht
    Tail,
}

/// A change of the path clicked on the map, as the editor's messages take it.
#[derive(Clone, Debug, PartialEq)]
pub enum PathEdit {
    Append {
        end: PathEnd,
        duct: CableDuct,
        /// The duct's other end, the path's new end
        other_schacht: CableSegmentEndSchacht,
    },
    Remove(PathEnd),
    /// The first duct of a cable without path
    Start {
        schacht_a: CableSegmentEndSchacht,
        duct: CableDuct,
        schacht_z: CableSegmentEndSchacht,
    },
}

#[derive(Clone, PartialEq, Properties)]
pub struct CableMapProps {
    /// The path as currently edited
    pub path: Option<CablePath>,
    /// Clicks change the path
    pub editable: bool,
    pub onedit: Callback<PathEdit>,
}

pub struct CableMap {
    data: Option<MapData>,
    error: Option<FrontendError>,
    /// Its layers: the path and the clickable ducts, redrawn on each change of the path
    map: MapHolder,
    /// Ducts and Schächte, drawn once
    drawn_base: bool,
    /// Fit the map to the path (or everything) once, later changes keep the view
    fitted: bool,
}

pub enum Msg {
    Data(MapData),
    Error(FrontendError),
}

impl Component for CableMap {
    type Message = Msg;
    type Properties = CableMapProps;

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
            data: None,
            error: None,
            map: MapHolder::default(),
            drawn_base: false,
            fitted: false,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Data(data) => {
                self.data = Some(data);
                self.draw(ctx);
            }
            Msg::Error(error) => self.error = Some(error),
        }
        true
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        let props = ctx.props();
        if props.path != old_props.path || props.editable != old_props.editable {
            self.draw(ctx);
        }
        false
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let status = match (&self.error, &self.data) {
            (Some(error), _) => error.into_prop_value(),
            (None, None) => html!(<Spinner/>),
            (None, Some(_)) => Html::default(),
        };
        // The map's div stays the last child, so Leaflet keeps its element (see pages/map.rs)
        html! {
            <div class="cable-map">
                {status}
                <div class="map-layout__map" ref={self.map.container()}/>
            </div>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if !first_render {
            return;
        }
        match self.map.create() {
            Ok(()) => self.draw(ctx),
            Err(error) => ctx.link().send_message(Msg::Error(error)),
        }
    }
}

impl CableMap {
    /// Draws what's missing: the base once, the path and the clickable ducts anew.
    fn draw(&mut self, ctx: &Context<Self>) {
        let (Some(map), Some(data)) = (self.map.map().cloned(), &self.data) else {
            return;
        };
        if !self.drawn_base {
            self.drawn_base = true;
            for line in data.ducts.iter().filter_map(|duct| duct.line.as_deref()) {
                duct_line(line, "map-view__duct").add_to(&map);
            }
            for schacht in &data.schaechte {
                if let Some(location) = schacht.location {
                    // No navigation from the editor, it would drop unsaved changes
                    schacht_marker(&schacht.name, location, || {}).add_to(&map);
                }
            }
        }
        let props = ctx.props();
        let path_ducts: Vec<i32> = props
            .path
            .iter()
            .flat_map(|path| path.duct_sequence())
            .collect();
        let find = |id: i32| data.ducts.iter().find(|duct| duct.id == id);

        let mut layers: Vec<leaflet::Layer> = Vec::new();
        let mut path_points: Vec<GeoPoint> = Vec::new();
        for line in path_ducts
            .iter()
            .filter_map(|&id| find(id)?.line.as_deref())
        {
            let drawn = duct_line(line, "map-view__duct map-view__duct--cable");
            layers.push(drawn.unchecked_into());
            path_points.extend(line);
        }

        if props.editable {
            for (duct, edit, text) in clickable(props.path.as_ref(), &path_ducts, data) {
                let Some(line) = duct.line.as_deref() else {
                    continue;
                };
                let class = if matches!(edit, PathEdit::Remove(_)) {
                    "map-view__duct-hit--remove"
                } else {
                    "map-view__duct-hit--add"
                };
                let onedit = props.onedit.clone();
                let hit = duct_hit_line(line, class, move || onedit.emit(edit.clone()));
                let hit: leaflet::Layer = hit.unchecked_into();
                hover_text(&hit, text);
                layers.push(hit);
            }
        }
        self.map.replace_layers(layers);

        if !self.fitted {
            self.fitted = true;
            if path_points.is_empty() {
                let all = data
                    .ducts
                    .iter()
                    .flat_map(|duct| duct.line.iter().flatten());
                fit_points(&map, all);
            } else {
                fit_points(&map, &path_points);
            }
        }
    }
}

/// The ducts a click changes the path with, what it does and the text telling it.
fn clickable<'a>(
    path: Option<&CablePath>,
    path_ducts: &[i32],
    data: &'a MapData,
) -> Vec<(&'a MapDuct, PathEdit, &'static str)> {
    let Some(path) = path.filter(|path| !path.segments.is_empty()) else {
        return data
            .ducts
            .iter()
            .map(|duct| {
                let edit = PathEdit::Start {
                    schacht_a: end_schacht(&duct.schacht_a),
                    duct: cable_duct(duct),
                    schacht_z: end_schacht(&duct.schacht_z),
                };
                (duct, edit, "Kabelweg hier beginnen")
            })
            .collect();
    };
    let front = path.near_schacht.id;
    let tail = path
        .segments
        .last()
        .map_or(front, |segment| segment.far_schacht.id);
    let mut result = Vec::new();
    for duct in &data.ducts {
        if let (Some(&first), Some(&last)) = (path_ducts.first(), path_ducts.last()) {
            // A single segment is removed at the front (the editor keeps the last one at the tail)
            if duct.id == first {
                result.push((duct, PathEdit::Remove(PathEnd::Front), "Segment entfernen"));
                continue;
            }
            if duct.id == last {
                result.push((duct, PathEdit::Remove(PathEnd::Tail), "Segment entfernen"));
                continue;
            }
        }
        if path_ducts.contains(&duct.id) {
            continue;
        }
        // A duct touching both ends closes a ring; it's added at the tail
        for (end, at) in [(PathEnd::Tail, tail), (PathEnd::Front, front)] {
            let other = if duct.schacht_a.id == at {
                &duct.schacht_z
            } else if duct.schacht_z.id == at {
                &duct.schacht_a
            } else {
                continue;
            };
            let edit = PathEdit::Append {
                end,
                duct: cable_duct(duct),
                other_schacht: end_schacht(other),
            };
            result.push((duct, edit, "Segment hinzufügen"));
            break;
        }
    }
    result
}

fn cable_duct(duct: &MapDuct) -> CableDuct {
    CableDuct {
        id: duct.id,
        description: duct.description.clone(),
        length: duct.length,
    }
}

fn end_schacht(end: &MapDuctEnd) -> CableSegmentEndSchacht {
    CableSegmentEndSchacht {
        id: end.id,
        name: end.name.clone(),
    }
}
