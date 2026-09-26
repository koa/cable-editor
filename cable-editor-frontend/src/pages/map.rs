use crate::{
    components::{
        links::{CableLink, DuctLink, SchachtLink},
        page_layout::PageLayout,
    },
    error::FrontendError,
    geo::map::{MapHolder, duct_hit_line, duct_line, fit_points, schacht_marker},
    graphql::authenticated::{
        list_ducts::duct_title,
        map::{MapData, MapDuct, fetch_map_data},
    },
    pages::router::{CabinetView, PlanView},
    util::{get_credentials, navigate},
};
use leaflet::MouseEvent;
use patternfly_yew::prelude::{
    Alert, AlertType, Button, ButtonVariant, Card, CardBody, CardHeader, CardHeaderActionsObject,
    CardSize, CardTitle, DescriptionGroup, DescriptionList, Icon, Spinner,
};
use wasm_bindgen::JsCast;
use yew::{Component, Context, Html, Properties, html, html::IntoPropValue, platform::spawn_local};

/// Map of the plan's objects: the Schächte with a position, labelled with their name (a click
/// opens the Schacht's overview), and the ducts (a click selects one and shows its Schächte and
/// cables as links).
pub struct Map {
    /// Its layers: the selected duct drawn above the others
    map: MapHolder,
    data: Option<MapData>,
    error: Option<FrontendError>,
    /// Id of the duct whose details are shown
    selected_duct: Option<i32>,
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
            map: MapHolder::default(),
            data: None,
            error: None,
            selected_duct: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Data(data) => {
                if let Some(map) = self.map.map() {
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
                self.selected_duct = id;
                let highlight = self
                    .selected()
                    .and_then(|duct| duct.line.as_deref())
                    .map(|line| duct_line(line, "map-view__duct map-view__duct--selected"));
                self.map
                    .replace_layers(highlight.into_iter().map(JsCast::unchecked_into).collect());
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
                    <div class="map-view__map" ref={self.map.container()}/>
                    <div class="map-view__details">{details}</div>
                </div>
            </PageLayout>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if !first_render {
            return;
        }
        if let Err(error) = self.map.create() {
            ctx.link().send_message(Msg::Error(error));
            return;
        }
        if let Some(map) = self.map.map() {
            // A click on a duct doesn't reach the map (bubbling_mouse_events)
            let scope = ctx.link().clone();
            map.on_mouse_click(Box::new(move |_: MouseEvent| {
                scope.send_message(Msg::SelectDuct(None))
            }));
            if let Some(data) = &self.data {
                show_data(ctx, map, data);
            }
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
    let title = duct_title(
        duct.description.as_deref(),
        &duct.schacht_a.name,
        &duct.schacht_z.name,
    );
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
                <CardTitle><DuctLink id={duct.id} text={title}/></CardTitle>
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
    // Ducts first, so the Schächte lie above them
    for duct in &data.ducts {
        let Some(line) = &duct.line else {
            continue;
        };
        duct_line(line, "map-view__duct").add_to(map);
        let id = duct.id;
        let scope = ctx.link().clone();
        duct_hit_line(line, "", move || {
            scope.send_message(Msg::SelectDuct(Some(id)))
        })
        .add_to(map);
    }
    for schacht in &data.schaechte {
        let Some(location) = schacht.location else {
            continue;
        };
        let id = schacht.id;
        let scope = ctx.link().clone();
        schacht_marker(&schacht.name, location, move || {
            scope.send_message(Msg::OpenSchacht(id))
        })
        .add_to(map);
    }
    let duct_points = data
        .ducts
        .iter()
        .flat_map(|duct| duct.line.iter().flatten());
    let schacht_points = data.schaechte.iter().filter_map(|s| s.location.as_ref());
    fit_points(map, duct_points.chain(schacht_points));
}
