use crate::{
    components::{fiber::FiberLabel, table::ListModel},
    error::FrontendError,
    graphql::authenticated::{
        PortSide, PortType,
        connections::{
            Cable, CableEnd, CableId, Fiber, FiberKeyInput, Panel, PlannedPanel, PortUsageInput,
            Schacht, UpdatePortUsage,
        },
    },
    icons::{IconFiberConnected, IconFiberCut, IconLink, IconUnlink},
    util::get_credentials,
};

use crate::graphql::authenticated::connections::{FiberOwnEnd, PortUsageUpdateAction, UsedEndPort};
use itertools::Itertools;
use patternfly_yew::prelude::{
    ActionGroup, Alert, AlertType, Button, ButtonVariant, Cell, CellContext, ExpansionState,
    FormGroup, Grid, GridItem, Icon, MemoizedTableModel, SelectItemRenderer, SimpleSelect, Spinner,
    Table, TableColumn, TableEntryRenderer, TableGridMode, TableHeader, TableMode, Title,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    rc::Rc,
};
use yew::{
    Callback, Component, Context, Html, Properties, html, html::IntoPropValue, html_nested,
    platform::spawn_local,
};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum LoopColumn {
    Fiber,
    Status,
    Actions,
    TerminationA,
    TerminationB,
}

impl SelectItemRenderer for CableEnd {
    type Item = i32;

    fn label(&self) -> String {
        format!(
            "{} ({}x{}) -> {}",
            self.cable.name,
            self.cable.bundle_count,
            self.cable.fiber_count,
            self.path.far_schacht.name
        )
    }
}

#[derive(Clone, PartialEq, Debug, Copy)]
pub enum FiberStatus {
    Free,
    Looped,        // Ist aktuell als Loop durchgeschaltet
    UsedElsewhere, // Z.B. "Gepatcht auf Splice-Port 12"
}
#[derive(Clone, PartialEq, Debug)]
struct FiberData {
    status: FiberStatus,
    modified_in_plan: bool,
    reset: bool,
    end_port_a: Option<UsedEndPort>,
    end_port_b: Option<UsedEndPort>,
}

// Repräsentiert eine Zeile (eine Faser) in der Matrix
#[derive(Clone, PartialEq)]
struct FiberLoopEntry {
    pub bundle: i32,
    pub fiber: i32,
    pub data: FiberData,
    pub on_toggle: Callback<(i32, i32, bool)>,
    pub reset: Callback<(i32, i32)>,
}

impl TableEntryRenderer<LoopColumn> for FiberLoopEntry {
    fn render_cell(&self, context: CellContext<'_, LoopColumn>) -> Cell {
        match context.column {
            LoopColumn::Fiber => Cell::new(
                html!(<FiberLabel fiber={self.fiber as u8}>{format!("{}-{}", self.bundle, self.fiber)}</FiberLabel>),
            ),
            LoopColumn::Status => {
                let (icon, text) = match &self.data.status {
                    FiberStatus::Free => (html!(<IconFiberCut/>), "Frei "),
                    FiberStatus::Looped => (html!(<IconFiberConnected/>), "Verbunden "),
                    FiberStatus::UsedElsewhere => (Icon::ExclamationTriangle.as_html(), "Benutzt "),
                };
                let marker = if self.data.modified_in_plan && !self.data.reset {
                    Some(Icon::InProgress)
                } else {
                    None
                };
                let node = html! {
                    <>
                        {icon} <span class="pf-v6-u-ml-sm">{text}</span> {marker}
                    </>
                };
                Cell::new(node)
            }
            LoopColumn::Actions => {
                let bundle = self.bundle;
                let fiber = self.fiber;
                let reset_button = if self.data.modified_in_plan && !self.data.reset {
                    let cb = self.reset.clone();
                    let onclick = Callback::from(move |_| cb.emit((bundle, fiber)));
                    Some(html! {
                            <Button variant={ButtonVariant::DangerSecondary} {onclick} icon={Icon::Redo}>
                                {"Planung zurücksetzen"}
                            </Button>
                    })
                } else {
                    None
                };

                match self.data.status {
                    FiberStatus::Free => {
                        let on_loop = {
                            let cb = self.on_toggle.clone();
                            Callback::from(move |_| cb.emit((bundle, fiber, true)))
                        };
                        Cell::new(html!(
                            <>
                            <Button variant={ButtonVariant::Secondary} onclick={on_loop}>
                                <IconLink/> <span class="pf-v6-u-ml-sm">{"Verbinden"}</span>
                            </Button>
                            {reset_button}
                            </>
                        ))
                    }
                    FiberStatus::Looped => {
                        let on_unloop = {
                            let cb = self.on_toggle.clone();
                            Callback::from(move |_| cb.emit((bundle, fiber, false)))
                        };
                        Cell::new(html!(
                            <>
                            <Button variant={ButtonVariant::DangerSecondary} onclick={on_unloop}>
                                <IconUnlink/> <span class="pf-v6-u-ml-sm">{"Auftrennen"}</span>
                            </Button>
                            {reset_button}
                            </>
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
            LoopColumn::TerminationA | LoopColumn::TerminationB => {
                let end_port = if let LoopColumn::TerminationA = context.column {
                    self.data.end_port_a.as_ref()
                } else {
                    self.data.end_port_b.as_ref()
                };
                let description = end_port.map(|p| {
                    format!(
                        "{} {} {}",
                        p.port.panel.schacht.name,
                        p.port.panel.name.as_deref().unwrap_or_default(),
                        p.port.label.as_deref().unwrap_or_default()
                    )
                });
                Cell::new(description.into_prop_value())
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
    current_situation: Option<PlannedPanel>,
    cable_a: Option<CableEnd>,
    cable_b: Option<CableEnd>,

    // Status der Fasern (Key: (Bundle, Fiber))
    fiber_states: BTreeMap<(i32, i32), FiberData>,

    table_state: Rc<RefCell<HashMap<usize, ExpansionState<LoopColumn>>>>,
    loading: bool,
    error: Option<FrontendError>,
    missing_port_count: usize,
}

pub enum Msg {
    FetchData,
    DataFetched(Option<PlannedPanel>),
    SelectCableA(i32),
    SelectCableB(i32),
    ToggleFiber(i32, i32, bool),
    Save,
    Saved,
    Error(FrontendError),
    PrepareLoopStates,
    ResetFiber(i32, i32),
}

impl Component for LoopPortEditor {
    type Message = Msg;
    type Properties = LoopPortEditorProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            current_situation: None,
            cable_a: None,
            cable_b: None,
            fiber_states: BTreeMap::new(),
            table_state: Rc::default(),
            loading: true,
            error: None,
            missing_port_count: 0,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchData => {
                self.loading = true;
                let plan_id = ctx.props().plan_id;
                let panel_id = ctx.props().panel_id;
                let scope = ctx.link().clone();

                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope.send_message(
                        PlannedPanel::fetch_situation(credentials.as_ref(), plan_id, panel_id)
                            .await
                            .map_or_else(Msg::Error, Msg::DataFetched),
                    );
                });
                true
            }
            Msg::SelectCableA(cable_id) => {
                if let Some(data) = self.current_situation.as_ref() {
                    let found_cable = data
                        .panel
                        .schacht
                        .cables
                        .iter()
                        .find(|c| c.cable.id == cable_id)
                        .cloned();
                    if let Some(found) = &found_cable
                        && let Some(other_cable) = data
                            .panel
                            .schacht
                            .cables
                            .iter()
                            .filter(|c| {
                                c.cable.id != cable_id
                                    && c.cable.bundle_count == found.cable.bundle_count
                                    && c.cable.fiber_count == found.cable.fiber_count
                            })
                            .exactly_one()
                            .ok()
                            .cloned()
                    {
                        self.cable_b = Some(other_cable);
                        ctx.link().send_message(Msg::PrepareLoopStates);
                    }
                    self.cable_a = found_cable;
                }

                true
            }
            Msg::SelectCableB(cable_id) => {
                if let Some(data) = self.current_situation.as_ref() {
                    self.cable_b = data
                        .panel
                        .schacht
                        .cables
                        .iter()
                        .find(|c| c.cable.id == cable_id)
                        .cloned();
                    ctx.link().send_message(Msg::PrepareLoopStates);
                }
                true
            }
            Msg::ResetFiber(bundle, fiber) => {
                if let (Some(data), Some(global_data)) = (
                    self.fiber_states.get_mut(&(bundle, fiber)),
                    &self.current_situation,
                ) {
                    let mut state = FiberStatus::Free;
                    for port in &global_data.ports {
                        for port_fiber in port
                            .current_front_usage
                            .iter()
                            .chain(&port.current_back_usage)
                            .filter_map(|p| p.fiber.as_ref())
                        {
                            if port_fiber.bundle == bundle && port_fiber.fiber == fiber {
                                state = FiberStatus::Looped;
                            }
                        }
                    }
                    data.status = state;
                    data.reset = true;
                }
                true
            }
            Msg::ToggleFiber(bundle, fiber, should_loop) => {
                if let Some(state) = self.fiber_states.get_mut(&(bundle, fiber)) {
                    state.status = if should_loop {
                        FiberStatus::Looped
                    } else {
                        FiberStatus::Free
                    };
                    state.modified_in_plan = true;
                    state.reset = false;
                }
                true
            }
            Msg::Save => {
                self.loading = true;
                let scope = ctx.link().clone();
                let _plan_id = ctx.props().plan_id;

                if let (
                    Some(PlannedPanel { ports, .. }),
                    Some(CableEnd {
                        cable: Cable { id: cable_a_id, .. },
                        ..
                    }),
                    Some(CableEnd {
                        cable: Cable { id: cable_b_id, .. },
                        ..
                    }),
                ) = (&self.current_situation, &self.cable_a, &self.cable_b)
                {
                    let mut to_loop: Box<[(i32, i32)]> = self
                        .fiber_states
                        .iter()
                        .filter_map(|(&(b, f), status)| {
                            if matches!(status.status, FiberStatus::Looped) {
                                Some((b, f))
                            } else {
                                None
                            }
                        })
                        .collect();
                    to_loop.sort();
                    let mut updates = Vec::with_capacity(to_loop.len() * 2);
                    let mut available_ports = ports.iter();
                    let mut missing_port_count = 0;
                    for (bundle, fiber) in to_loop {
                        if let Some(port) = available_ports.next() {
                            if port.front_usage.as_ref().and_then(|u| u.fiber.as_ref())
                                != Some(&Fiber {
                                    bundle,
                                    fiber,
                                    cable: CableId { id: *cable_a_id },
                                })
                            {
                                updates.push(PortUsageInput {
                                    port_id: port.id,
                                    side: PortSide::FRONT,
                                    fiber: PortUsageUpdateAction::Attach(FiberKeyInput {
                                        cable_id: *cable_a_id,
                                        bundle,
                                        fiber,
                                    }),
                                })
                            }
                            if port.back_usage.as_ref().and_then(|u| u.fiber.as_ref())
                                != Some(&Fiber {
                                    bundle,
                                    fiber,
                                    cable: CableId { id: *cable_b_id },
                                })
                            {
                                updates.push(PortUsageInput {
                                    port_id: port.id,
                                    side: PortSide::BACK,
                                    fiber: PortUsageUpdateAction::Attach(FiberKeyInput {
                                        cable_id: *cable_b_id,
                                        bundle,
                                        fiber,
                                    }),
                                })
                            }
                        } else {
                            missing_port_count += 1;
                        }
                    }
                    self.missing_port_count = missing_port_count;
                    for remaining_port in available_ports {
                        if remaining_port.front_usage.is_some() {
                            updates.push(PortUsageInput {
                                port_id: remaining_port.id,
                                side: PortSide::FRONT,
                                fiber: PortUsageUpdateAction::Remove(true),
                            });
                        }
                        if remaining_port.back_usage.is_some() {
                            updates.push(PortUsageInput {
                                port_id: remaining_port.id,
                                side: PortSide::BACK,
                                fiber: PortUsageUpdateAction::Remove(true),
                            });
                        }
                    }

                    if updates.is_empty() {
                        scope.send_message(Msg::Saved);
                    } else {
                        let update = UpdatePortUsage {
                            plan_id: ctx.props().plan_id,
                            usages: updates,
                        };
                        spawn_local(async move {
                            scope.send_message(
                                update
                                    .store(get_credentials(&scope).as_ref())
                                    .await
                                    .map_or_else(Msg::Error, |_| Msg::Saved),
                            );
                        });
                    }
                }
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

            Msg::PrepareLoopStates => {
                self.fiber_states = self.calculate_current_states(ctx.props().panel_id);
                true
            }
            Msg::DataFetched(Some(data)) => {
                self.loading = false;
                let used_cables = data
                    .ports
                    .iter()
                    .filter(|p| p.port_type == PortType::Loop)
                    .flat_map(|p| {
                        p.front_usage
                            .iter()
                            .chain(p.back_usage.iter())
                            .filter_map(|u| u.fiber.map(|f| f.cable.id))
                    })
                    .collect::<HashSet<_>>();
                let mut mapped_cables = data
                    .panel
                    .schacht
                    .cables
                    .iter()
                    .filter(|c| used_cables.contains(&c.cable.id))
                    .cloned();
                self.cable_a = mapped_cables.next();
                self.cable_b = mapped_cables.next();
                if self.cable_b.is_some() {
                    ctx.link().send_message(Msg::PrepareLoopStates);
                }

                self.current_situation = Some(data);
                true
            }
            Msg::DataFetched(None) => {
                self.loading = false;
                self.error = Some(FrontendError::NotFound);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if self.loading {
            return html!(<Spinner />);
        }

        let is_pair_defined = self.cable_a.is_some() && self.cable_b.is_some();
        let unmodified = self.calculate_current_states(ctx.props().panel_id) == self.fiber_states;
        let scope = ctx.link().clone();

        html! {
            <div class="pf-v6-c-panel">
                <div class="pf-v6-c-panel__main">
                    <div class="pf-v6-c-panel__main-body">
                        <Title size={patternfly_yew::prelude::Size::XLarge}>{"Direktverbindungen"}</Title>
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
                                <Button label="Änderungen Speichern" disabled={unmodified} variant={ButtonVariant::Primary} onclick={ctx.link().callback(|_| Msg::Save)} />
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
            let entries = self
                .current_situation
                .as_ref()
                .map(|s| s.panel.schacht.cables.clone())
                .unwrap_or_default();
            let onselect = ctx
                .link()
                .callback(|c: CableEnd| Msg::SelectCableA(c.cable.id));

            html! {
                <FormGroup label="Zulauf-Kabel (A)">
                    <SimpleSelect<CableEnd>
                        {entries}
                        selected={self.cable_a.clone()}
                        {onselect}
                        placeholder="- Kabel A wählen -"
                    />
                </FormGroup>
            }
        };
        let select_cable_b = if let Some(cable_a) = &self.cable_a {
            let entries: Vec<_> = self
                .current_situation
                .as_ref()
                .map(|s| {
                    s.panel
                        .schacht
                        .cables
                        .iter()
                        .filter(|c| {
                            c.cable.id != cable_a.cable.id
                                && c.cable.bundle_count == cable_a.cable.bundle_count
                                && c.cable.fiber_count == cable_a.cable.fiber_count
                        })
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();

            let onselect = ctx
                .link()
                .callback(|c: CableEnd| Msg::SelectCableB(c.cable.id));

            html! {
                <FormGroup label="Ablauf-Kabel (B)">
                    <SimpleSelect<CableEnd>
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
        let connection_description = format!(
            "{}({})->{}({}) ({}x{}).",
            a.path.far_schacht.name,
            a.cable.name,
            b.path.far_schacht.name,
            b.cable.name,
            a.cable.bundle_count,
            a.cable.fiber_count
        );
        html! {
            <Alert title="Verbindung" r#type={AlertType::Info} inline=true>
                <p>{connection_description}</p>
            </Alert>
        }
    }

    fn render_fiber_table(&self, ctx: &Context<Self>) -> Html {
        let a = self.cable_a.as_ref().unwrap();
        let mut entries = Vec::new();
        let scope = ctx.link().clone();

        for ((bundle, fiber), data) in self.fiber_states.iter() {
            entries.push(FiberLoopEntry {
                bundle: *bundle,
                fiber: *fiber,
                data: data.clone(),
                on_toggle: scope
                    .callback(|(b, f, should_loop)| Msg::ToggleFiber(b, f, should_loop)),
                reset: scope.callback(|(b, f)| Msg::ResetFiber(b, f)),
            })
        }

        let table_model = ListModel::new(
            MemoizedTableModel::new(Rc::new(entries)),
            self.table_state.clone(),
        );

        let header = html_nested! {
            <TableHeader<LoopColumn>>
                <TableColumn<LoopColumn> label="Ende" index={LoopColumn::TerminationA} />
                <TableColumn<LoopColumn> label="Faser" index={LoopColumn::Fiber} />
                <TableColumn<LoopColumn> label="Status" index={LoopColumn::Status} />
                <TableColumn<LoopColumn> label="Aktion" index={LoopColumn::Actions} />
                <TableColumn<LoopColumn> label="Ende" index={LoopColumn::TerminationB} />
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

    fn calculate_current_states(&self, panel_id: i32) -> BTreeMap<(i32, i32), FiberData> {
        let mut states = BTreeMap::new();
        if let (
            Some(cable_a),
            Some(cable_b),
            Some(PlannedPanel {
                ports,
                panel: Panel {
                    schacht: Schacht { cables },
                },
            }),
        ) = (&self.cable_a, &self.cable_b, &self.current_situation)
        {
            let fibers_a = cable_a
                .fibers
                .iter()
                .map(|f| ((f.bundle, f.fiber), f))
                .collect::<HashMap<_, _>>();
            let fibers_b = cable_b
                .fibers
                .iter()
                .map(|f| ((f.bundle, f.fiber), f))
                .collect::<HashMap<_, _>>();
            let all_fibers = fibers_a
                .keys()
                .chain(fibers_b.keys())
                .copied()
                .collect::<HashSet<_>>();
            for fiber_key in all_fibers {
                let fiber_a = fibers_a.get(&fiber_key).copied();
                let fiber_b = fibers_b.get(&fiber_key).copied();
                let modified_in_plan = fiber_a
                    .and_then(|f| f.used_port.as_ref())
                    .map(|p| p.modified_in_plan)
                    .unwrap_or_default()
                    || fiber_b
                        .and_then(|f| f.used_port.as_ref())
                        .map(|p| p.modified_in_plan)
                        .unwrap_or_default();
                let end_port_a = fiber_a
                    .and_then(|f| f.other_end.as_ref())
                    .and_then(|e| e.used_port.as_ref())
                    .and_then(|p| p.panel_side_end_port.as_ref())
                    .cloned();
                let end_port_b = fiber_b
                    .and_then(|f| f.other_end.as_ref())
                    .and_then(|e| e.used_port.as_ref())
                    .and_then(|p| p.panel_side_end_port.as_ref())
                    .cloned();

                states.insert(
                    fiber_key,
                    FiberData {
                        status: match (fiber_a, fiber_b) {
                            (Some(fa), Some(fb))
                                if fa
                                    .used_port
                                    .as_ref()
                                    .map(|p| p.port.panel.id == panel_id)
                                    .unwrap_or(false)
                                    && fb
                                        .used_port
                                        .as_ref()
                                        .map(|p| p.port.panel.id == panel_id)
                                        .unwrap_or(false) =>
                            {
                                FiberStatus::Looped
                            }
                            (
                                Some(FiberOwnEnd {
                                    used_port: None, ..
                                }),
                                Some(FiberOwnEnd {
                                    used_port: None, ..
                                }),
                            ) => FiberStatus::Free,
                            _ => FiberStatus::UsedElsewhere,
                        },
                        modified_in_plan,
                        reset: false,
                        end_port_a,
                        end_port_b,
                    },
                );
            }
        }
        states
    }
}
