//! The delivery to the Leitungskataster (see docs/leitungskataster.md), for admins: where the
//! delivery of the whole network stands, the download of its files, what can't be delivered and
//! the deliveries so far; below a map of the perimeter and what is delivered.

use crate::{
    components::{
        links::{DuctLink, SchachtLink},
        page_layout::PageLayout,
    },
    error::FrontendError,
    geo::map::{MapHolder, duct_line, fit_points, hover_text, lat_lng, schacht_marker},
    graphql::authenticated::{
        lkmap::{
            DeliveryState, LkmapDelivery, LkmapExport, days_until, download_lkmap,
            fetch_lkmap_export, set_lkmap_delivered,
        },
        local_day,
        schacht_types::type_icon,
    },
    pages::router::{CabinetView, PlanView},
    util::{get_credentials, navigate, save_file, toast_error, toast_success},
};
use js_sys::{Array, Date};
use leaflet::{Polygon, PolylineOptions};
use patternfly_yew::prelude::{
    Alert, AlertType, Button, ButtonVariant, Color, Icon, Label, Level, Spinner, Title,
};
use wasm_bindgen::{JsCast, JsValue};
use yew::platform::spawn_local;
use yew::{Callback, Component, Context, Html, Properties, html, html::IntoPropValue};

/// The Checkservice takes the transfer files as ZIPs.
const ZIP: &str = "application/zip";

/// How many days before the end of a quarter a missing delivery becomes a warning.
const QUARTER_WARNING_DAYS: i32 = 14;

/// What the page does with the delivery.
#[derive(Clone, PartialEq, Debug)]
pub enum Action {
    Download,
    /// The delivery reached the Checkservice, or takes that back
    SetDelivered(i32, bool),
}

/// The loaded delivery with where it stands.
struct Loaded {
    export: LkmapExport,
    state: DeliveryState,
}

impl Loaded {
    fn view(&self, ctx: &Context<Leitungskataster>) -> Html {
        let export = &self.export;
        let onaction = ctx.link().callback(Msg::Action);
        html! {
            <div class="lkmap__details">
                <div class="lkmap__summary">
                    <span>
                        {state_label(&self.state)}
                        <br/>
                        <span class="lkmap__subtle">
                            {format!("{}, Datenherr {}",
                                content(export.ducts.len(), export.schaechte.len()),
                                export.datenherr)}
                        </span>
                        if let Some(pending) = export.pending() {
                            <br/>
                            <span class="lkmap__subtle">
                                {format!("heruntergeladen {}, nicht bestätigt", pending.created_at.local())}
                            </span>
                        }
                    </span>
                    if export.checksum.is_some() {
                        <Button
                            label="Herunterladen"
                            icon={Icon::Download}
                            variant={ButtonVariant::Primary}
                            onclick={onaction.reform(|_| Action::Download)}
                        />
                    }
                </div>
                if !export.schaechte_without_position.is_empty() {
                    <Alert inline=true r#type={AlertType::Warning}
                        title={format!("{} ohne Position, {} nicht geliefert",
                            count(export.schaechte_without_position.len(), "Schacht", "Schächte"),
                            if export.schaechte_without_position.len() == 1 { "wird" } else { "werden" })}>
                        {links(export.schaechte_without_position.iter().map(|schacht| html! {
                            <SchachtLink id={schacht.id} text={schacht.name.clone()}/>
                        }))}
                    </Alert>
                }
                if !export.ducts_without_line.is_empty() {
                    <Alert inline=true r#type={AlertType::Warning}
                        title={format!("{} an einem Schacht ohne Position, {} nicht geliefert",
                            count(export.ducts_without_line.len(), "Trasse", "Trassen"),
                            if export.ducts_without_line.len() == 1 { "wird" } else { "werden" })}>
                        {links(export.ducts_without_line.iter().map(|duct| html! {
                            <DuctLink id={duct.id} text={duct.title()}/>
                        }))}
                    </Alert>
                }
                <Title level={Level::H2}>{"Lieferungen"}</Title>
                if export.deliveries.is_empty() {
                    <p class="lkmap__subtle">{"Noch nichts heruntergeladen."}</p>
                } else {
                    {self.view_deliveries(&onaction)}
                }
            </div>
        }
    }

    fn view_deliveries(&self, onaction: &Callback<Action>) -> Html {
        let checksum = self.export.checksum.as_deref();
        let rows = self.export.deliveries.iter().map(|delivery| {
            let delivered = delivery.delivered_at.is_some();
            let id = delivery.id;
            let onclick = onaction.reform(move |_| Action::SetDelivered(id, !delivered));
            let current = Some(delivery.checksum.as_str()) == checksum;
            html! {
                <li class="lkmap__delivery">
                    <span>
                        {format!("{} von {}", delivery.created_at.local(), delivery.created_by)}
                        <br/>
                        <span class="lkmap__subtle">
                            {content(delivery.duct_count as usize, delivery.schacht_count as usize)}
                            {", Prüfsumme "}
                            <span title={delivery.checksum.clone()}>{short_checksum(delivery)}</span>
                        </span>
                        if current {
                            {" "}<Label label="aktueller Stand" compact=true color={Color::Blue}/>
                        }
                    </span>
                    <span>
                        {delivery.delivered_at.as_ref().map_or_else(
                            || "nicht als geliefert bestätigt".to_string(),
                            |at| format!("geliefert {}", at.local()),
                        )}
                    </span>
                    <span>
                        if delivered {
                            <Button label="Bestätigung zurücknehmen" variant={ButtonVariant::Link} {onclick}/>
                        } else {
                            <Button label="Als geliefert bestätigen" variant={ButtonVariant::Secondary} {onclick}/>
                        }
                    </span>
                </li>
            }
        });
        html!(<ul class="lkmap__deliveries">{for rows}</ul>)
    }
}

/// The first 8 characters, enough to tell deliveries apart.
fn short_checksum(delivery: &LkmapDelivery) -> String {
    delivery.checksum.chars().take(8).collect()
}

/// "1 Trasse", "3 Trassen"
fn count(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}

/// "2 Trassen, 4 Schächte"
fn content(ducts: usize, schaechte: usize) -> String {
    format!(
        "{}, {}",
        count(ducts, "Trasse", "Trassen"),
        count(schaechte, "Schacht", "Schächte")
    )
}

fn links(links: impl Iterator<Item = Html>) -> Html {
    html!(<ul class="lkmap__links">{for links.map(|link| html!(<li>{link}</li>))}</ul>)
}

/// Where the delivery stands and until when it is due, as a label.
fn state_label(state: &DeliveryState) -> Html {
    let now = Date::new_0();
    let (text, color) = match state {
        DeliveryState::NothingToDeliver => ("nichts zu liefern".to_string(), Color::Grey),
        DeliveryState::NeverDelivered => ("noch nie geliefert".to_string(), Color::Orange),
        DeliveryState::Changed { due: Some(due) } if days_until(&now, due) < 0 => (
            format!("geändert, Frist abgelaufen am {}", local_day(due)),
            Color::Red,
        ),
        DeliveryState::Changed { due: Some(due) } => (
            format!("geändert, liefern bis {}", local_day(due)),
            Color::Orange,
        ),
        DeliveryState::Changed { due: None } => ("geändert, liefern".to_string(), Color::Orange),
        DeliveryState::QuarterDue { due } => (
            format!("in diesem Quartal nicht geliefert, bis {}", local_day(due)),
            if days_until(&now, due) <= QUARTER_WARNING_DAYS {
                Color::Orange
            } else {
                Color::Grey
            },
        ),
        DeliveryState::UpToDate => ("aktuell".to_string(), Color::Green),
    };
    html!(<Label label={text} compact=true {color}/>)
}

/// The delivery of the whole network to the Leitungskataster.
pub struct Leitungskataster {
    /// `None` while loading
    export: Option<Result<Loaded, FrontendError>>,
    map: MapHolder,
}

pub enum Msg {
    Load,
    Loaded(Result<LkmapExport, FrontendError>),
    Action(Action),
    /// A change is stored: reloads and confirms
    Changed(&'static str),
    OpenSchacht(i32),
    MapError(FrontendError),
}

#[derive(Clone, PartialEq, Properties)]
pub struct LeitungskatasterProps {
    pub plan_id: i32,
}

impl Component for Leitungskataster {
    type Message = Msg;
    type Properties = LeitungskatasterProps;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Load);
        Self {
            export: None,
            map: MapHolder::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let scope = ctx.link().clone();
        match msg {
            Msg::Load => {
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    scope.send_message(Msg::Loaded(fetch_lkmap_export(credentials.as_ref()).await));
                });
                false
            }
            Msg::Loaded(export) => {
                let now = Date::new_0();
                self.export = Some(export.map(|export| Loaded {
                    state: export.state(&now),
                    export,
                }));
                self.show_on_map(ctx);
                true
            }
            Msg::Action(Action::Download) => {
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    let saved = download_lkmap(credentials.as_ref())
                        .await
                        .and_then(|files| {
                            files
                                .iter()
                                .try_for_each(|file| save_file(&file.file_name, &file.content, ZIP))
                        });
                    match saved {
                        Ok(()) => scope.send_message(Msg::Changed("Dateien heruntergeladen")),
                        Err(error) => toast_error(
                            &scope,
                            "Dateien konnten nicht heruntergeladen werden",
                            error,
                        ),
                    }
                });
                false
            }
            Msg::Action(Action::SetDelivered(delivery_id, delivered)) => {
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    match set_lkmap_delivered(credentials.as_ref(), delivery_id, delivered).await {
                        Ok(()) => scope.send_message(Msg::Changed(if delivered {
                            "Lieferung bestätigt"
                        } else {
                            "Bestätigung zurückgenommen"
                        })),
                        Err(error) => {
                            toast_error(&scope, "Lieferung konnte nicht geändert werden", error)
                        }
                    }
                });
                false
            }
            Msg::Changed(message) => {
                toast_success(ctx.link(), message);
                ctx.link().send_message(Msg::Load);
                false
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
            Msg::MapError(error) => {
                self.export = Some(Err(error));
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let content = match &self.export {
            None => html!(<Spinner/>),
            Some(Err(error)) => error.into_prop_value(),
            Some(Ok(loaded)) => loaded.view(ctx),
        };
        // The map's div is always there, so Leaflet keeps its element (see pages/map.rs)
        html! {
            <PageLayout title="Leitungskataster">
                <div class="lkmap">
                    <p class="lkmap__subtle">
                        {"Die Dateien für das ganze Netz herunterladen, beim Checkservice von infoGrips \
                          hochladen und danach die Lieferung bestätigen. Zu liefern ist innerhalb einer \
                          Woche nach einer Änderung, mindestens aber jedes Quartal."}
                    </p>
                    {content}
                    <Title level={Level::H2}>{"Karte"}</Title>
                    <div class="lkmap__map" ref={self.map.container()}/>
                </div>
            </PageLayout>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if !first_render {
            return;
        }
        match self.map.create() {
            Ok(()) => self.show_on_map(ctx),
            Err(error) => ctx.link().send_message(Msg::MapError(error)),
        }
    }
}

impl Leitungskataster {
    /// Draws the perimeter and what is delivered, once both the map and the data are there.
    fn show_on_map(&mut self, ctx: &Context<Self>) {
        let (Some(map), Some(Ok(loaded))) = (self.map.map().cloned(), &self.export) else {
            return;
        };
        let export = &loaded.export;
        let mut layers: Vec<leaflet::Layer> = Vec::new();
        let mut points = Vec::new();
        if let Some(area) = &export.perimeter_area {
            let options = PolylineOptions::default();
            options.set_class_name("lkmap__perimeter".to_string());
            let corners: Array = area.iter().map(|p| JsValue::from(lat_lng(*p))).collect();
            let polygon = Polygon::new_with_options(&corners, &options);
            let layer: leaflet::Layer = polygon.unchecked_into();
            hover_text(&layer, "Zuständigkeitsperimeter");
            layers.push(layer);
            points.extend(area.iter().copied());
        }
        for line in export.ducts.iter().filter_map(|duct| duct.line.as_ref()) {
            layers.push(duct_line(line, "map-view__duct").unchecked_into());
            points.extend(line.iter().copied());
        }
        for schacht in &export.schaechte {
            let Some(location) = schacht.location else {
                continue;
            };
            let id = schacht.id;
            let scope = ctx.link().clone();
            let marker = schacht_marker(
                &schacht.name,
                location,
                type_icon(&schacht.typ),
                move || scope.send_message(Msg::OpenSchacht(id)),
            );
            layers.push(marker.unchecked_into());
            points.push(location);
        }
        fit_points(&map, points.iter());
        self.map.replace_layers(layers);
    }
}
