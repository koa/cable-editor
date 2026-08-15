use crate::components::table::ListModel;
use crate::error::FrontendError;
use crate::graphql::authenticated::connections::{CableEndInfo, CableInfo, CablePathInfo};
use crate::icons::{IconFiberConnected, IconFiberCut, IconLink};
use crate::icons::IconUnlink;
use crate::util::get_credentials;
use patternfly_yew::prelude::{
    ActionGroup, Alert, AlertType, Button, ButtonVariant, Cell, CellContext, ExpansionState,
    FormGroup, Grid, GridItem, Icon, MemoizedTableModel, SelectItemRenderer, SimpleSelect, Spinner,
    Table, TableColumn, TableEntryRenderer, TableGridMode, TableHeader, TableMode, Title,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::thread::scope;
use yew::platform::spawn_local;
use yew::{Callback, Component, Context, Html, Properties, html, html_nested};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum LoopColumn {
    Fiber,
    Status,
    Actions,
}

impl SelectItemRenderer for CableEndInfo {
    type Item = i32;

    fn label(&self) -> String {
        format!(
            "{} ({}x{}) -> {}",
            self.cable.name, self.cable.bundle_count, self.cable.fiber_count, self.path.far_schacht.name
        )
    }
}

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
                        html!(<IconFiberCut/>),
                        "Frei".to_string(),
                        "var(--pf-v6-global--success-color--100)",
                    ),
                    FiberStatus::Looped => (
                        html!(<IconFiberConnected/>),
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
    available_cables: Rc<Vec<CableEndInfo>>,
    cable_a: Option<CableEndInfo>,
    cable_b: Option<CableEndInfo>,

    // Status der Fasern (Key: (Bundle, Fiber))
    fiber_states: HashMap<(i32, i32), FiberStatus>,

    table_state: Rc<RefCell<HashMap<usize, ExpansionState<LoopColumn>>>>,
    loading: bool,
    error: Option<FrontendError>,
}

pub enum Msg {
    FetchData,
    CablesFetched(Vec<CableEndInfo>),
    SelectCableA(Option<i32>),
    SelectCableB(Option<i32>),
    ToggleFiber(i32, i32, bool),
    Save,
    Saved,
    Error(FrontendError),
    FetchLoopStates,
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
                let panel_id = ctx.props().panel_id;
                let scope = ctx.link().clone();

                spawn_local(async move {
                    scope.send_message(
                        CableEndInfo::list_candidate_by_panel(
                            get_credentials(&scope).as_ref(),
                            panel_id,
                        )
                        .await
                        .map_or_else(Msg::Error, Msg::CablesFetched),
                    );
                });
                true
            }
            Msg::SelectCableA(cable_id) => {
                self.cable_a = cable_id.and_then(|id| {
                    self.available_cables
                        .iter()
                        .find(|c| c.cable.id == id)
                        .cloned()
                });
                self.cable_b = None; // Reset B, falls A sich ändert
                true
            }
            Msg::SelectCableB(cable_id) => {
                self.cable_b = cable_id.and_then(|id| {
                    self.available_cables
                        .iter()
                        .find(|c| c.cable.id == id)
                        .cloned()
                });
                // Wenn beide gewählt wurden, initialisieren wir die leeren States
                if self.cable_a.is_some() && self.cable_b.is_some() {
                    self.loading=true;
                    ctx.link().send_message(Msg::FetchLoopStates);
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
            Msg::CablesFetched(cables) => {
                self.available_cables = Rc::new(cables);
                self.loading = false;
                self.error = None;
                true
            }
            Msg::FetchLoopStates => {
                todo!();
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
        let select_cable_a = {
            let entries = (*self.available_cables).clone();
            let onselect = ctx
                .link()
                .callback(|c: CableEndInfo| Msg::SelectCableA(Some(c.cable.id)));

            html! {
                <FormGroup label="Zulauf-Kabel (A)">
                    <SimpleSelect<CableEndInfo>
                        {entries}
                        selected={self.cable_a.clone()}
                        {onselect}
                        placeholder="- Kabel A wählen -"
                    />
                </FormGroup>
            }
        };
        let select_cable_b = if let Some(cable_a) = &self.cable_a {
            let entries: Vec<CableEndInfo> = self
                .available_cables
                .iter()
                .filter(|c| {
                    c.cable.id != cable_a.cable.id
                        && c.cable.bundle_count == cable_a.cable.bundle_count
                        && c.cable.fiber_count == cable_a.cable.fiber_count
                })
                .cloned()
                .collect();

            let onselect = ctx
                .link()
                .callback(|c: CableEndInfo| Msg::SelectCableB(Some(c.cable.id)));

            html! {
                <FormGroup label="Ablauf-Kabel (B)">
                    <SimpleSelect<CableEndInfo>
                        entries={entries}
                        selected={self.cable_b.clone()}
                        {onselect}
                        placeholder="- Zugehöriges Kabel B wählen -"
                    />
                </FormGroup>
            }
        } else {
            Html::default()
        };

        html! {
            <Grid gutter=true>
                <GridItem cols={[6]}>
                    {select_cable_a}
                </GridItem>
                <GridItem cols={[6]}>
                    {select_cable_b}
                </GridItem>
            </Grid>
        }
    }
    fn render_active_pair(&self, _ctx: &Context<Self>) -> Html {
        let a = self.cable_a.as_ref().unwrap();
        let b = self.cable_b.as_ref().unwrap();
        html! {
            <Alert title="Loop-Paar aktiv" r#type={AlertType::Info} inline=true>
                <p>{ format!("Verbinde Fasern von '{}' direkt mit '{}' ({} Bündel à {} Fasern).", a.cable.name, b.cable.name, a.cable.bundle_count, a.cable.fiber_count) }</p>
            </Alert>
        }
    }

    fn render_fiber_table(&self, ctx: &Context<Self>) -> Html {
        let a = self.cable_a.as_ref().unwrap();
        let mut entries = Vec::new();
        let scope = ctx.link().clone();

        for bundle in 1..=a.cable.bundle_count {
            for fiber in 1..=a.cable.fiber_count {
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
