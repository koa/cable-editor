//! The delivery to the Leitungskataster (see docs/leitungskataster.md), for admins: a row per
//! owner with where its delivery stands and the download of its files, opened its report and
//! deliveries; below a map of the perimeters and what is delivered.

use crate::{
    components::{
        links::{DuctLink, SchachtLink},
        page_layout::PageLayout,
        table::ListModel,
    },
    error::FrontendError,
    geo::map::{MapHolder, duct_line, fit_points, hover_text, lat_lng, schacht_marker},
    graphql::authenticated::{
        lkmap::{
            DeliveryState, LkmapDelivery, LkmapExport, days_until, download_lkmap,
            fetch_lkmap_exports, set_lkmap_delivered,
        },
        local_day,
    },
    pages::router::{CabinetView, PlanView},
    util::{get_credentials, navigate, save_file, toast_error, toast_success},
};
use js_sys::{Array, Date};
use leaflet::{Polygon, PolylineOptions};
use patternfly_yew::prelude::{
    Alert, AlertType, Button, ButtonVariant, Cell, CellContext, Color, ExpansionState, Icon, Label,
    Level, MemoizedTableModel, Span, Spinner, Table, TableColumn, TableEntryRenderer,
    TableGridMode, TableHeader, TableMode, Title,
};
use std::{cell::RefCell, collections::HashMap, collections::hash_map::Entry, rc::Rc};
use wasm_bindgen::{JsCast, JsValue};
use yew::platform::spawn_local;
use yew::{Callback, Component, Context, Html, Properties, html, html::IntoPropValue};

/// The Checkservice takes the transfer files as ZIPs.
const ZIP: &str = "application/zip";

/// How many days before the end of a quarter a missing delivery becomes a warning.
const QUARTER_WARNING_DAYS: i32 = 14;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Columns {
    Owner,
    Content,
    State,
    Actions,
}

/// What a row does with its owner's delivery.
#[derive(Clone, PartialEq, Debug)]
pub enum Action {
    Download(i32),
    /// The delivery reached the Checkservice, or takes that back
    SetDelivered(i32, bool),
}

#[derive(Clone, PartialEq)]
struct DeliveryRow {
    export: LkmapExport,
    state: DeliveryState,
    onaction: Callback<Action>,
}

impl TableEntryRenderer<Columns> for DeliveryRow {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        let export = &self.export;
        match context.column {
            // One element per cell: on phones the cell lays its children out as a grid
            Columns::Owner => Cell::new(html! {
                <span>
                    {export.owner.name.clone()}
                    <br/>
                    <span class="lkmap__subtle">
                        {export.owner.uid.clone().unwrap_or_else(|| "keine UID".to_string())}
                    </span>
                </span>
            }),
            Columns::Content => {
                Cell::new(html!(content(export.ducts.len(), export.schaechte.len())))
            }
            Columns::State => Cell::new(html! {
                <span>
                    {state_label(&self.state)}
                    if let Some(pending) = export.pending() {
                        <br/>
                        <span class="lkmap__subtle">
                            {format!("heruntergeladen {}, nicht bestätigt", pending.created_at.local())}
                        </span>
                    }
                </span>
            }),
            Columns::Actions => Cell::new(if export.checksum.is_some() {
                let owner_id = export.owner.id;
                let onclick = self.onaction.reform(move |_| Action::Download(owner_id));
                html! {
                    <Button
                        label="Herunterladen"
                        icon={Icon::Download}
                        variant={ButtonVariant::Secondary}
                        {onclick}
                    />
                }
            } else {
                Html::default()
            }),
        }
    }

    fn is_full_width_details(&self) -> Option<bool> {
        Some(true)
    }

    fn render_details(&self) -> Vec<Span> {
        vec![Span::max(self.view_details())]
    }
}

impl DeliveryRow {
    fn view_details(&self) -> Html {
        let export = &self.export;
        html! {
            <div class="lkmap__details">
                if export.owner.uid.is_none() {
                    <Alert inline=true r#type={AlertType::Warning} title="Ohne UID (Datenherr) wird nichts geliefert">
                        {"Die UID wird beim Eigentümer erfasst, für Private eine fiktive UID der \
                          Geschäftsstelle (ZHE-…)."}
                    </Alert>
                }
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
                <Title level={Level::H3}>{"Lieferungen"}</Title>
                if export.deliveries.is_empty() {
                    <p class="lkmap__subtle">{"Noch nichts heruntergeladen."}</p>
                } else {
                    {self.view_deliveries()}
                }
            </div>
        }
    }

    fn view_deliveries(&self) -> Html {
        let checksum = self.export.checksum.as_deref();
        let rows = self.export.deliveries.iter().map(|delivery| {
            let delivered = delivery.delivered_at.is_some();
            let id = delivery.id;
            let onclick = self
                .onaction
                .reform(move |_| Action::SetDelivered(id, !delivered));
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
        DeliveryState::WithoutUid => ("keine UID, wird nicht geliefert".to_string(), Color::Orange),
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

/// The owners' deliveries to the Leitungskataster.
pub struct Leitungskataster {
    /// `None` while loading
    exports: Option<Result<Rc<Vec<DeliveryRow>>, FrontendError>>,
    /// Which rows are opened
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<Columns>>>>,
    map: MapHolder,
}

pub enum Msg {
    Load,
    Loaded(Result<Vec<LkmapExport>, FrontendError>),
    Toggle(usize, ExpansionState<Columns>),
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
            exports: None,
            table_state: Rc::default(),
            map: MapHolder::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let scope = ctx.link().clone();
        match msg {
            Msg::Load => {
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    scope
                        .send_message(Msg::Loaded(fetch_lkmap_exports(credentials.as_ref()).await));
                });
                false
            }
            Msg::Loaded(exports) => {
                let onaction = ctx.link().callback(Msg::Action);
                let now = Date::new_0();
                self.exports = Some(exports.map(|exports| {
                    Rc::new(
                        exports
                            .into_iter()
                            .map(|export| DeliveryRow {
                                state: export.state(&now),
                                export,
                                onaction: onaction.clone(),
                            })
                            .collect(),
                    )
                }));
                self.show_on_map(ctx);
                true
            }
            Msg::Toggle(key, state) => {
                match self.table_state.borrow_mut().entry(key) {
                    Entry::Occupied(entry) if entry.get() == &state => {
                        entry.remove();
                    }
                    Entry::Occupied(mut entry) => {
                        entry.insert(state);
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(state);
                    }
                }
                true
            }
            Msg::Action(Action::Download(owner_id)) => {
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    let saved = download_lkmap(credentials.as_ref(), owner_id)
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
                self.exports = Some(Err(error));
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let content = match &self.exports {
            None => html!(<Spinner/>),
            Some(Err(error)) => error.into_prop_value(),
            Some(Ok(rows)) if rows.is_empty() => html! {
                <p>{"Keine Trasse ist zur Lieferung an den Leitungskataster markiert."}</p>
            },
            Some(Ok(rows)) => self.view_rows(ctx, rows),
        };
        // The map's div is always there, so Leaflet keeps its element (see pages/map.rs)
        html! {
            <PageLayout title="Leitungskataster">
                <div class="lkmap">
                    <p class="lkmap__subtle">
                        {"Pro Eigentümer die Dateien herunterladen, beim Checkservice von infoGrips \
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
    fn view_rows(&self, ctx: &Context<Self>, rows: &Rc<Vec<DeliveryRow>>) -> Html {
        let header = yew::html_nested! {
            <TableHeader<Columns>>
                <TableColumn<Columns> label="Eigentümer" index={Columns::Owner}/>
                <TableColumn<Columns> label="Lieferung" index={Columns::Content}/>
                <TableColumn<Columns> label="Zustand" index={Columns::State}/>
                <TableColumn<Columns> index={Columns::Actions}/>
            </TableHeader<Columns>>
        };
        let entries = ListModel::new(
            MemoizedTableModel::new(rows.clone()),
            self.table_state.clone(),
        );
        let onexpand = ctx.link().callback(|(key, state)| Msg::Toggle(key, state));
        html! {
            <Table<Columns, ListModel<Columns, MemoizedTableModel<DeliveryRow>>>
                mode={TableMode::CompactExpandable}
                grid={TableGridMode::Medium}
                {header}
                {entries}
                {onexpand}
            />
        }
    }

    /// Draws the perimeters and what is delivered, once both the map and the data are there.
    fn show_on_map(&mut self, ctx: &Context<Self>) {
        let (Some(map), Some(Ok(rows))) = (self.map.map().cloned(), &self.exports) else {
            return;
        };
        let mut layers: Vec<leaflet::Layer> = Vec::new();
        let mut points = Vec::new();
        for row in rows.iter() {
            let export = &row.export;
            if let Some(area) = &export.perimeter_area {
                let options = PolylineOptions::default();
                options.set_class_name("lkmap__perimeter".to_string());
                let corners: Array = area.iter().map(|p| JsValue::from(lat_lng(*p))).collect();
                let polygon = Polygon::new_with_options(&corners, &options);
                let layer: leaflet::Layer = polygon.unchecked_into();
                hover_text(
                    &layer,
                    &format!("Zuständigkeitsperimeter {}", export.owner.name),
                );
                layers.push(layer);
                points.extend(area.iter().copied());
            }
            // Without files (no UID) they aren't delivered
            let class = if export.checksum.is_some() {
                "map-view__duct"
            } else {
                "map-view__duct lkmap__undelivered"
            };
            for line in export.ducts.iter().filter_map(|duct| duct.line.as_ref()) {
                layers.push(duct_line(line, class).unchecked_into());
                points.extend(line.iter().copied());
            }
            for schacht in &export.schaechte {
                let Some(location) = schacht.location else {
                    continue;
                };
                let id = schacht.id;
                let scope = ctx.link().clone();
                let marker = schacht_marker(&schacht.name, location, move || {
                    scope.send_message(Msg::OpenSchacht(id))
                });
                layers.push(marker.unchecked_into());
                points.push(location);
            }
        }
        fit_points(&map, points.iter());
        self.map.replace_layers(layers);
    }
}
