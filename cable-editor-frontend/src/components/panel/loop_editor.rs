use crate::components::table::ListModel;
use crate::error::FrontendError;
use crate::icons::IconLink;
use crate::icons::IconUnlink;
use patternfly_yew::prelude::{
    ActionGroup, Alert, AlertType, Button, ButtonVariant, Cell, CellContext, ExpansionState,
    FormGroup, Grid, GridItem, Icon, MemoizedTableModel, SelectItemRenderer, SimpleSelect, Spinner,
    Table, TableColumn, TableEntryRenderer, TableGridMode, TableHeader, TableMode, Title,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use yew::platform::spawn_local;
use yew::{Callback, Component, Context, Html, Properties, html, html_nested};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum LoopColumn {
    Fiber,
    Status,
    Actions,
}

#[derive(Clone, PartialEq, Debug, Hash, Eq)]
pub struct CableInfo {
    pub id: i32,
    pub name: String,
    pub bundle_count: i32,
    pub fiber_count: i32,
}
impl SelectItemRenderer for CableInfo {
    type Item = i32;

    fn label(&self) -> String {
        format!("{} ({}x{})", self.name, self.bundle_count, self.fiber_count)
    }
}
/*impl std::fmt::Display for CableInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}x{})",
            self.name, self.bundle_count, self.fiber_count
        )
    }
}*/

#[derive(Clone, PartialEq, Debug, Copy)]
pub enum FiberStatus {
    Free,
    Looped,        // Ist aktuell als Loop durchgeschaltet
    UsedElsewhere, // Z.B. "Gepatcht auf Splice-Port 12"
}

// Repräsentiert eine Zeile (eine Faser) in der Matrix
#[derive(Clone, PartialEq)]
struct FiberLoopEntry {
    pub bundle: i32,
    pub fiber: i32,
    pub status: FiberStatus,
    pub on_toggle: Callback<(i32, i32, bool)>, // (bundle, fiber, should_loop)
}

impl TableEntryRenderer<LoopColumn> for FiberLoopEntry {
    fn render_cell(&self, context: CellContext<'_, LoopColumn>) -> Cell {
        match context.column {
            LoopColumn::Fiber => Cell::new(format!("{}-{}", self.bundle, self.fiber).into()),
            LoopColumn::Status => {
                let (icon, text, variant) = match &self.status {
                    FiberStatus::Free => (
                        Icon::CheckCircle.as_html(),
                        "Frei".to_string(),
                        "var(--pf-v6-global--success-color--100)",
                    ),
                    FiberStatus::Looped => (
                        html!(<IconLink/>),
                        "Verbunden".to_string(),
                        "var(--pf-v6-global--info-color--100)",
                    ),
                    FiberStatus::UsedElsewhere => (
                        Icon::ExclamationTriangle.as_html(),
                        "Benutzt".to_string(),
                        "var(--pf-v6-global--warning-color--100)",
                    ),
                };
                Cell::new(html! {
                    <span style={format!("color: {}", variant)}>
                        {icon} <span class="pf-v6-u-ml-sm">{text}</span>
                    </span>
                })
            }
            LoopColumn::Actions => {
                let bundle = self.bundle;
                let fiber = self.fiber;

                match self.status {
                    FiberStatus::Free => {
                        let on_loop = {
                            let cb = self.on_toggle.clone();
                            Callback::from(move |_| cb.emit((bundle, fiber, true)))
                        };
                        Cell::new(html!(
                            <Button variant={ButtonVariant::Secondary} onclick={on_loop}>
                                <IconLink/> <span class="pf-v6-u-ml-sm">{"Verbinden"}</span>
                            </Button>
                        ))
                    }
                    FiberStatus::Looped => {
                        let on_unloop = {
                            let cb = self.on_toggle.clone();
                            Callback::from(move |_| cb.emit((bundle, fiber, false)))
                        };
                        Cell::new(html!(
                            <Button variant={ButtonVariant::DangerSecondary} onclick={on_unloop}>
                                <IconUnlink/> <span class="pf-v6-u-ml-sm">{"Auftrennen"}</span>
                            </Button>
                        ))
                    }
                    FiberStatus::UsedElsewhere => {
                        // Wenn blockiert, kann nicht geloopt werden
                        Cell::new(
                            html!(<Button variant={ButtonVariant::Plain} disabled=true icon={Icon::Ban} />),
                        )
                    }
                }
            }
        }
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct LoopPortEditorProps {
    pub plan_id: i32,
    pub panel_id: i32,
}

pub struct LoopPortEditor {
    available_cables: Rc<Vec<CableInfo>>,
    cable_a: Option<CableInfo>,
    cable_b: Option<CableInfo>,

    // Status der Fasern (Key: (Bundle, Fiber))
    fiber_states: HashMap<(i32, i32), FiberStatus>,

    table_state: Rc<RefCell<HashMap<usize, ExpansionState<LoopColumn>>>>,
    loading: bool,
    error: Option<FrontendError>,
}

pub enum Msg {
    FetchData,
    DataFetched {
        cables: Vec<CableInfo>,
        active_a: Option<i32>,
        active_b: Option<i32>,
        states: HashMap<(i32, i32), FiberStatus>,
    },
    SelectCableA(Option<i32>),
    SelectCableB(Option<i32>),
    ToggleFiber(i32, i32, bool),
    Save,
    Saved,
    Error(FrontendError),
}

impl Component for LoopPortEditor {
    type Message = Msg;
    type Properties = LoopPortEditorProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            available_cables: Rc::new(Vec::new()),
            cable_a: None,
            cable_b: None,
            fiber_states: HashMap::new(),
            table_state: Rc::default(),
            loading: true,
            error: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchData => {
                self.loading = true;
                let _plan_id = ctx.props().plan_id;
                let _panel_id = ctx.props().panel_id;
                let scope = ctx.link().clone();

                spawn_local(async move {
                    // TODO: GraphQL: Lade verfügbare Kabel im Schacht und die existierenden Loop-Belegungen des Panels für diesen Plan
                    // Dummy-Daten:
                    let dummy_cables = vec![
                        CableInfo {
                            id: 101,
                            name: "Trunk Nord".to_string(),
                            bundle_count: 6,
                            fiber_count: 12,
                        },
                        CableInfo {
                            id: 102,
                            name: "Trunk Süd".to_string(),
                            bundle_count: 6,
                            fiber_count: 12,
                        },
                    ];

                    let mut states = HashMap::new();
                    states.insert((1, 1), FiberStatus::Looped); // Faser 1 ist geloopt
                    states.insert((1, 2), FiberStatus::UsedElsewhere); // Faser 2 ist blockiert

                    scope.send_message(Msg::DataFetched {
                        cables: dummy_cables,
                        active_a: Some(101), // Simulieren: Paar ist schon definiert
                        active_b: Some(102),
                        states,
                    });
                });
                true
            }
            Msg::DataFetched {
                cables,
                active_a,
                active_b,
                states,
            } => {
                self.available_cables = Rc::new(cables.clone());
                self.cable_a = active_a.and_then(|id| cables.iter().find(|c| c.id == id).cloned());
                self.cable_b = active_b.and_then(|id| cables.iter().find(|c| c.id == id).cloned());
                self.fiber_states = states;
                self.loading = false;
                true
            }
            Msg::SelectCableA(cable_id) => {
                self.cable_a = cable_id
                    .and_then(|id| self.available_cables.iter().find(|c| c.id == id).cloned());
                self.cable_b = None; // Reset B, falls A sich ändert
                true
            }
            Msg::SelectCableB(cable_id) => {
                self.cable_b = cable_id
                    .and_then(|id| self.available_cables.iter().find(|c| c.id == id).cloned());
                // Wenn beide gewählt wurden, initialisieren wir die leeren States
                if self.cable_a.is_some() && self.cable_b.is_some() {
                    self.fiber_states.clear();
                }
                true
            }
            Msg::ToggleFiber(bundle, fiber, should_loop) => {
                if should_loop {
                    self.fiber_states
                        .insert((bundle, fiber), FiberStatus::Looped);
                } else {
                    self.fiber_states.insert((bundle, fiber), FiberStatus::Free);
                }
                true
            }
            Msg::Save => {
                self.loading = true;
                let scope = ctx.link().clone();
                let _plan_id = ctx.props().plan_id;

                // Wir filtern nur die Fasern heraus, die effektiv den Status "Looped" haben
                let _to_loop: Vec<(i32, i32)> = self
                    .fiber_states
                    .iter()
                    .filter_map(|(&(b, f), status)| {
                        if matches!(status, FiberStatus::Looped) {
                            Some((b, f))
                        } else {
                            None
                        }
                    })
                    .collect();

                spawn_local(async move {
                    // TODO: GraphQL Mutation "sync_loop_fibers(plan_id, panel_id, cable_a, cable_b, loops)"
                    // Das Backend sucht sich freie Loop-Ports und persistiert die Belegungen.
                    scope.send_message(Msg::Saved);
                });
                true
            }
            Msg::Saved => {
                ctx.link().send_message(Msg::FetchData);
                true
            }
            Msg::Error(error) => {
                self.error = Some(error);
                self.loading = false;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if self.loading {
            return html!(<Spinner />);
        }

        let is_pair_defined = self.cable_a.is_some() && self.cable_b.is_some();
        let scope = ctx.link().clone();

        html! {
            <div class="pf-v6-c-panel">
                <div class="pf-v6-c-panel__main">
                    <div class="pf-v6-c-panel__main-body">
                        <Title size={patternfly_yew::prelude::Size::XLarge}>{"Loop-Verbindungen (Durchschaltungen)"}</Title>
                        <p class="pf-v6-u-mb-md">{"Loopen Sie Fasern eines Kabels direkt auf ein anderes Kabel mit derselben Kapazität."}</p>

                        if let Some(err) = &self.error {
                            <Alert title={err.to_string()} r#type={AlertType::Danger} inline=true />
                        }

                        // 1. KABELPAAR AUSWAHL / ANZEIGE
                        if !is_pair_defined {
                            { self.render_cable_selection(ctx) }
                        } else {
                            { self.render_active_pair(ctx) }

                            // 2. FASER-MATRIX (Nur wenn Paar definiert ist)
                            <div class="pf-v6-u-mt-lg">
                                { self.render_fiber_table(ctx) }
                            </div>
                            // class="pf-v6-u-mt-md"
                            <ActionGroup>
                                <Button label="Änderungen Speichern" variant={ButtonVariant::Primary} onclick={ctx.link().callback(|_| Msg::Save)} />
                            </ActionGroup>
                        }
                    </div>
                </div>
            </div>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::FetchData);
        }
    }
}

impl LoopPortEditor {
    fn render_cable_selection(&self, ctx: &Context<Self>) -> Html {
        // SimpleSelect gibt uns direkt das ausgewählte CableInfo-Objekt zurück
        let on_a_select = ctx
            .link()
            .callback(|c: CableInfo| Msg::SelectCableA(Some(c.id)));
        let on_b_select = ctx
            .link()
            .callback(|c: CableInfo| Msg::SelectCableB(Some(c.id)));

        // PatternFly erwartet die Optionen als Vec<T>
        let options_a = (*self.available_cables).clone();

        let options_b: Vec<CableInfo> = self
            .available_cables
            .iter()
            .filter(|c| {
                if let Some(a) = &self.cable_a {
                    c.id != a.id
                        && c.bundle_count == a.bundle_count
                        && c.fiber_count == a.fiber_count
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        html! {
            <Grid gutter=true>
                <GridItem cols={[6]}>
                    <FormGroup label="Zulauf-Kabel (A)">
                        <SimpleSelect<CableInfo>
                            entries={options_a}
                            selected={self.cable_a.clone()}
                            onselect={on_a_select}
                            placeholder="- Kabel A wählen -"
                        />
                    </FormGroup>
                </GridItem>
                <GridItem cols={[6]}>
                    <FormGroup label="Ablauf-Kabel (B)">
                        <SimpleSelect<CableInfo>
                            entries={options_b}
                            selected={self.cable_b.clone()}
                            onselect={on_b_select}
                            //disabled={self.cable_a.is_none()}
                            placeholder="- Zugehöriges Kabel B wählen -"
                        />
                    </FormGroup>
                </GridItem>
            </Grid>
        }
    }
    fn render_active_pair(&self, _ctx: &Context<Self>) -> Html {
        let a = self.cable_a.as_ref().unwrap();
        let b = self.cable_b.as_ref().unwrap();
        html! {
            <Alert title="Loop-Paar aktiv" r#type={AlertType::Info} inline=true>
                <p>{ format!("Verbinde Fasern von '{}' direkt mit '{}' ({} Bündel à {} Fasern).", a.name, b.name, a.bundle_count, a.fiber_count) }</p>
            </Alert>
        }
    }

    fn render_fiber_table(&self, ctx: &Context<Self>) -> Html {
        let a = self.cable_a.as_ref().unwrap();
        let mut entries = Vec::new();
        let scope = ctx.link().clone();

        for bundle in 1..=a.bundle_count {
            for fiber in 1..=a.fiber_count {
                let status = self
                    .fiber_states
                    .get(&(bundle, fiber))
                    .cloned()
                    .unwrap_or(FiberStatus::Free);
                entries.push(FiberLoopEntry {
                    bundle,
                    fiber,
                    status,
                    on_toggle: scope
                        .callback(|(b, f, should_loop)| Msg::ToggleFiber(b, f, should_loop)),
                });
            }
        }

        let table_model = ListModel::new(
            MemoizedTableModel::new(Rc::new(entries)),
            self.table_state.clone(),
        );

        let header = html_nested! {
            <TableHeader<LoopColumn>>
                <TableColumn<LoopColumn> label="Faser" index={LoopColumn::Fiber} />
                <TableColumn<LoopColumn> label="Status" index={LoopColumn::Status} />
                <TableColumn<LoopColumn> label="Aktion" index={LoopColumn::Actions} />
            </TableHeader<LoopColumn>>
        };

        html! {
            <Table<LoopColumn, ListModel<LoopColumn, MemoizedTableModel<FiberLoopEntry>>>
                mode={TableMode::Compact}
                grid={TableGridMode::Medium}
                {header}
                entries={table_model}
            />
        }
    }
}
