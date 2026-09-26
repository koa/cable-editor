use crate::{
    components::{
        links::{CableLink, SchachtLink},
        page_layout::{PageLayout, object_title},
    },
    error::FrontendError,
    geo::map::{MapHolder, duct_line, fit_points, schacht_marker},
    graphql::authenticated::duct_details::{DuctDetails, fetch_duct_details},
    pages::router::{CabinetView, PlanView},
    util::{get_credentials, navigate},
};
use patternfly_yew::prelude::{DescriptionGroup, DescriptionList, Spinner};
use wasm_bindgen::JsCast;
use yew::{Component, Context, Html, Properties, html, html::IntoPropValue, platform::spawn_local};

/// A duct: its Schächte, length and cables, and a map with its line.
pub struct ShowDuct {
    /// Missing while loading; `None` inside: the duct doesn't exist
    duct: Option<Option<DuctDetails>>,
    error: Option<FrontendError>,
    /// Its layers: what's drawn for the duct, replaced when another duct is shown
    map: MapHolder,
}

pub enum Msg {
    Data(Option<DuctDetails>),
    Error(FrontendError),
    OpenSchacht(i32),
}

#[derive(Clone, PartialEq, Properties)]
pub struct ShowDuctProps {
    pub plan_id: i32,
    pub duct_id: i32,
}

impl Component for ShowDuct {
    type Message = Msg;
    type Properties = ShowDuctProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self::fetch(ctx);
        Self {
            duct: None,
            error: None,
            map: MapHolder::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Data(duct) => {
                self.error = None;
                self.duct = Some(duct);
                self.show_duct(ctx);
                true
            }
            Msg::Error(error) => {
                self.error = Some(error);
                true
            }
            Msg::OpenSchacht(id) => {
                navigate(
                    ctx.link(),
                    ctx.props().plan_id,
                    PlanView::Cabinet {
                        id,
                        view: CabinetView::Overview,
                    },
                );
                false
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        if ctx.props().duct_id != old_props.duct_id {
            self.duct = None;
            Self::fetch(ctx);
            true
        } else {
            false
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let loaded = self.duct.as_ref().and_then(Option::as_ref);
        let title = object_title("Trasse", loaded.map(DuctDetails::title));
        let content = if let Some(error) = &self.error {
            error.into_prop_value()
        } else {
            match &self.duct {
                None => html!(<Spinner/>),
                Some(None) => (&FrontendError::NotFound).into_prop_value(),
                Some(Some(duct)) => view_details(duct),
            }
        };
        // The map's div is always there, so Leaflet keeps its element (see pages/map.rs)
        html! {
            <PageLayout {title}>
                <div class="map-layout">
                    <div>{content}</div>
                    <div class="map-layout__map" ref={self.map.container()}/>
                </div>
            </PageLayout>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if !first_render {
            return;
        }
        match self.map.create() {
            Ok(()) => self.show_duct(ctx),
            Err(error) => ctx.link().send_message(Msg::Error(error)),
        }
    }
}

impl ShowDuct {
    fn fetch(ctx: &Context<Self>) {
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        let duct_id = ctx.props().duct_id;
        spawn_local(async move {
            scope.send_message(
                match fetch_duct_details(credentials.as_ref(), duct_id).await {
                    Ok(duct) => Msg::Data(duct),
                    Err(error) => Msg::Error(error),
                },
            );
        });
    }

    /// Draws the duct and its Schächte, once both the map and the duct are there.
    fn show_duct(&mut self, ctx: &Context<Self>) {
        let (Some(map), Some(Some(duct))) = (self.map.map().cloned(), &self.duct) else {
            return;
        };
        let mut layers: Vec<leaflet::Layer> = Vec::new();
        if let Some(line) = &duct.line {
            let line = duct_line(line, "map-view__duct map-view__duct--selected");
            layers.push(line.unchecked_into());
        }
        for end in [&duct.schacht_a, &duct.schacht_z] {
            let Some(location) = end.location else {
                continue;
            };
            let id = end.id;
            let scope = ctx.link().clone();
            let marker = schacht_marker(&end.name, location, move || {
                scope.send_message(Msg::OpenSchacht(id))
            });
            layers.push(marker.unchecked_into());
        }
        let ends = [&duct.schacht_a, &duct.schacht_z]
            .into_iter()
            .filter_map(|end| end.location.as_ref());
        fit_points(&map, duct.line.iter().flatten().chain(ends));
        self.map.replace_layers(layers);
    }
}

fn view_details(duct: &DuctDetails) -> Html {
    let length = match duct.length {
        Some(length) => format!("{length:.1} m"),
        None => "unbekannt (kein Verlauf erfasst)".to_string(),
    };
    let cables = if duct.cables.is_empty() {
        html!("keine")
    } else {
        html! {
            <ul class="duct-page__cables">
                {for duct.cables.iter().map(|cable| html! {
                    <li>
                        <CableLink id={cable.id} text={cable.name.clone()}/>
                        {format!(" ({} × {} Fasern)", cable.bundle_count, cable.fiber_count)}
                    </li>
                })}
            </ul>
        }
    };
    html! {
        <DescriptionList>
            if let Some(description) = &duct.description {
                <DescriptionGroup term="Beschreibung">{description.clone()}</DescriptionGroup>
            }
            <DescriptionGroup term="Schächte">
                <SchachtLink id={duct.schacht_a.id} text={duct.schacht_a.name.clone()}/>
                {" – "}
                <SchachtLink id={duct.schacht_z.id} text={duct.schacht_z.name.clone()}/>
            </DescriptionGroup>
            <DescriptionGroup term="Länge">{length}</DescriptionGroup>
            <DescriptionGroup term="Kabel">{cables}</DescriptionGroup>
        </DescriptionList>
    }
}
