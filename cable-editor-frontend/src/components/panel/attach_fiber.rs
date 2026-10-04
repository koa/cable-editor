use crate::components::icon_button::IconButton;
use crate::components::load::Load;
use crate::components::menu::popup::{MenuActionItem, MenuGroup, PopupMenu};
use crate::components::page_layout::{PageLayout, object_title};
use crate::components::select::Select;
use crate::components::unsaved::Unsaved;
use crate::{
    components::{
        fiber::{FiberLabel, FiberNumber},
        table::ListModel,
    },
    error::FrontendError,
    graphql::authenticated::{
        PortSide, PortType,
        connections::{
            CableEnd, FiberKeyInput, FiberOwnEnd, PlannedPanel, PlannedPort, PortUsageInput,
            PortUsageUpdateAction, UpdatePortUsage, UsedEndPort,
        },
        port_label,
    },
    icons::IconLink,
    util::{get_credentials, toast_error, toast_success},
};
use cable_editor_common::ObjectKind;
use itertools::Itertools;
use patternfly_yew::prelude::{
    Alert, AlertType, Button, ButtonVariant, Cell, CellContext, ExpansionState, Icon,
    MemoizedTableModel, SelectItemRenderer, Spinner, Table, TableColumn, TableEntryRenderer,
    TableGridMode, TableHeader, TableMode,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    rc::Rc,
};
use yew::{Component, Context, Html, Properties, html, html_nested, platform::spawn_local};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum AttachColumn {
    Front,
    Port,
    Back,
}

#[derive(Clone, PartialEq)]
enum SlotState {
    Empty,
    Assigned(FiberKeyInput),
}

#[derive(Clone, PartialEq)]
struct SlotEdit {
    port_id: i32,
    side: PortSide,
    cable_bundle: Option<CableBundleSelectEntry>,
}

#[derive(Clone, PartialEq)]
struct PortRow {
    front: Html,
    port: Html,
    back: Html,
}

impl TableEntryRenderer<AttachColumn> for PortRow {
    fn render_cell(&self, context: CellContext<'_, AttachColumn>) -> Cell {
        match context.column {
            AttachColumn::Front => Cell::new(self.front.clone()),
            AttachColumn::Port => Cell::new(self.port.clone()),
            AttachColumn::Back => Cell::new(self.back.clone()),
        }
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct AttachFiberProps {
    pub plan_id: i32,
    pub panel_id: i32,
}

pub struct AttachFiber {
    current_situation: Load<PlannedPanel>,
    slot_states: BTreeMap<(i32, PortSide), SlotState>,
    edit_slot: Option<SlotEdit>,
    reset_ports: HashSet<i32>,
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<AttachColumn>>>>,
    saving: bool,
    unsaved: Unsaved,
}

pub enum Msg {
    FetchData,
    DataFetched(Option<PlannedPanel>),
    StartEdit(i32, PortSide),
    SelectCableBundle(CableBundleSelectEntry),
    SelectFiber(i32),
    CancelEdit,
    ClearSlot(i32, PortSide),
    ResetPort(i32),
    Save,
    Saved,
    Error(FrontendError),
    /// Saving failed: the input stays, the error is a toast
    SaveFailed(FrontendError),
}

impl Component for AttachFiber {
    type Message = Msg;
    type Properties = AttachFiberProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            unsaved: Unsaved::new(ctx.link()),
            current_situation: Load::Pending,
            slot_states: BTreeMap::new(),
            edit_slot: None,
            reset_ports: HashSet::new(),
            table_state: Rc::default(),
            saving: false,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchData => {
                self.current_situation = Load::Pending;
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
            Msg::DataFetched(Some(data)) => {
                self.slot_states = calculate_current_states(&data);
                self.reset_ports.clear();
                self.edit_slot = None;
                self.current_situation = Load::Loaded(data);
                true
            }
            Msg::DataFetched(None) => {
                self.current_situation = Load::Failed(FrontendError::not_found(
                    ObjectKind::Panel,
                    ctx.props().panel_id,
                ));
                true
            }
            Msg::StartEdit(port_id, side) => {
                self.edit_slot = Some(SlotEdit {
                    port_id,
                    side,
                    cable_bundle: None,
                });
                true
            }
            Msg::SelectCableBundle(cable_bundle) => {
                if let Some(edit) = &mut self.edit_slot {
                    edit.cable_bundle = Some(cable_bundle);
                }
                true
            }
            Msg::SelectFiber(fiber) => {
                if let Some(edit) = self.edit_slot.take()
                    && let Some(cable_bundle) = edit.cable_bundle
                {
                    let key = FiberKeyInput {
                        cable_id: cable_bundle.cable.cable.id,
                        bundle: cable_bundle.bundle,
                        fiber,
                    };
                    self.slot_states
                        .insert((edit.port_id, edit.side), SlotState::Assigned(key));
                    self.reset_ports.remove(&edit.port_id);
                }
                true
            }
            Msg::CancelEdit => {
                self.edit_slot = None;
                true
            }
            Msg::ClearSlot(port_id, side) => {
                self.slot_states.insert((port_id, side), SlotState::Empty);
                self.reset_ports.remove(&port_id);
                true
            }
            Msg::ResetPort(port_id) => {
                self.reset_ports.insert(port_id);

                // Back to what the port holds
                if let Load::Loaded(situation) = &self.current_situation
                    && let Some(port) = situation.ports.iter().find(|p| p.id == port_id)
                {
                    for (side, usage) in [
                        (PortSide::FRONT, &port.front_usage),
                        (PortSide::BACK, &port.back_usage),
                    ] {
                        let state = usage
                            .as_ref()
                            .and_then(|u| u.fiber)
                            .map(|f| {
                                SlotState::Assigned(FiberKeyInput {
                                    cable_id: f.cable.id,
                                    bundle: f.bundle,
                                    fiber: f.fiber,
                                })
                            })
                            .unwrap_or(SlotState::Empty);
                        self.slot_states.insert((port_id, side), state);
                    }
                }

                if self.edit_slot.as_ref().map(|e| e.port_id) == Some(port_id) {
                    self.edit_slot = None;
                }
                true
            }
            Msg::Save => {
                self.saving = true;
                let scope = ctx.link().clone();
                let plan_id = ctx.props().plan_id;

                if let Load::Loaded(situation) = &self.current_situation {
                    let mut usages = Vec::new();

                    for port in &situation.ports {
                        if port.port_type == PortType::Loop {
                            continue;
                        }
                        if self.reset_ports.contains(&port.id) {
                            usages.push(PortUsageInput {
                                port_id: port.id,
                                side: PortSide::FRONT,
                                fiber: PortUsageUpdateAction::Reset(true),
                            });
                            continue;
                        }

                        for (side, current_usage) in [
                            (PortSide::FRONT, &port.front_usage),
                            (PortSide::BACK, &port.back_usage),
                        ] {
                            let desired_state = self
                                .slot_states
                                .get(&(port.id, side))
                                .unwrap_or(&SlotState::Empty);

                            let initial_key =
                                current_usage.as_ref().and_then(|u| u.fiber).map(|f| {
                                    FiberKeyInput {
                                        cable_id: f.cable.id,
                                        bundle: f.bundle,
                                        fiber: f.fiber,
                                    }
                                });

                            match desired_state {
                                SlotState::Assigned(new_key) => {
                                    if Some(*new_key) != initial_key {
                                        usages.push(PortUsageInput {
                                            port_id: port.id,
                                            side,
                                            fiber: PortUsageUpdateAction::Attach(*new_key),
                                        });
                                    }
                                }
                                SlotState::Empty => {
                                    if initial_key.is_some() {
                                        usages.push(PortUsageInput {
                                            port_id: port.id,
                                            side,
                                            fiber: PortUsageUpdateAction::Remove(true),
                                        });
                                    }
                                }
                            }
                        }
                    }

                    if usages.is_empty() {
                        scope.send_message(Msg::Saved);
                    } else {
                        let update = UpdatePortUsage { plan_id, usages };
                        spawn_local(async move {
                            scope.send_message(
                                update
                                    .store(get_credentials(&scope).as_ref())
                                    .await
                                    .map_or_else(Msg::SaveFailed, |_| Msg::Saved),
                            );
                        });
                    }
                }
                true
            }
            Msg::Saved => {
                self.saving = false;
                toast_success(ctx.link(), "Verbindungen gespeichert");
                ctx.link().send_message(Msg::FetchData);
                true
            }
            Msg::SaveFailed(error) => {
                self.saving = false;
                toast_error(
                    ctx.link(),
                    "Verbindungen konnten nicht gespeichert werden",
                    error,
                );
                true
            }
            Msg::Error(error) => {
                self.current_situation = Load::Failed(error);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <PageLayout title={object_title("Fasern auflegen", self.current_situation.loaded().and_then(|situation| situation.panel.name.as_deref()))}>{self.view_content(ctx)}</PageLayout>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        self.unsaved.set(!self.saving && self.has_changes());
        if first_render {
            ctx.link().send_message(Msg::FetchData);
        }
    }
}

impl AttachFiber {
    fn view_content(&self, ctx: &Context<Self>) -> Html {
        if self.saving {
            return html!(<Spinner />);
        }
        self.current_situation.view(|_| self.view_form(ctx))
    }

    fn view_form(&self, ctx: &Context<Self>) -> Html {
        let validation_errors = self.validate();
        let has_changes = self.has_changes();

        let can_save = has_changes && validation_errors.is_empty();

        html! {
            <div class="pf-v6-c-panel">
                <div class="pf-v6-c-panel__main">
                    <div class="pf-v6-c-panel__main-body">
                        { for validation_errors.iter().map(|err| html! {
                            <Alert title={err.clone()} r#type={AlertType::Warning} inline=true />
                        }) }

                        <div class="pf-v6-u-mt-lg">
                            { self.render_port_table(ctx) }
                        </div>

                        <div class="pf-v6-u-mt-md">
                            <Button
                                label="Speichern"
                                variant={ButtonVariant::Primary}
                                disabled={!can_save}
                                onclick={ctx.link().callback(|_| Msg::Save)}
                            />
                        </div>
                    </div>
                </div>
            </div>
        }
    }

    fn validate(&self) -> Box<[String]> {
        let mut errors = Vec::new();

        if let Load::Loaded(situation) = &self.current_situation {
            for port in &situation.ports {
                if port.port_type == PortType::Loop {
                    continue;
                }

                let front = self
                    .slot_states
                    .get(&(port.id, PortSide::FRONT))
                    .unwrap_or(&SlotState::Empty);
                let back = self
                    .slot_states
                    .get(&(port.id, PortSide::BACK))
                    .unwrap_or(&SlotState::Empty);

                let port_label = port.label.as_deref().unwrap_or("Unbenannt");

                match port.port_type {
                    PortType::Splice => {
                        if (front == &SlotState::Empty && back != &SlotState::Empty)
                            || (front != &SlotState::Empty && back == &SlotState::Empty)
                        {
                            errors.push(format!(
                                "Port {}: Spleiss muss beidseitig belegt oder komplett leer sein.",
                                port_label
                            ));
                        }
                    }
                    PortType::Connector
                        if front != &SlotState::Empty && back == &SlotState::Empty =>
                    {
                        errors.push(format!("Port {}: Stecker benötigt zwingend eine Back-Belegung, wenn Front belegt ist.", port_label));
                    }
                    _ => {}
                }
            }
        }

        errors.into_boxed_slice()
    }

    fn has_changes(&self) -> bool {
        if !self.reset_ports.is_empty() {
            return true;
        }
        if let Load::Loaded(situation) = &self.current_situation {
            let initial_states = calculate_current_states(situation);
            return self.slot_states != initial_states;
        }
        false
    }

    fn render_port_table(&self, ctx: &Context<Self>) -> Html {
        let Load::Loaded(situation) = &self.current_situation else {
            return Html::default();
        };

        let mut ports: Vec<&PlannedPort> = situation.ports.iter().collect();
        ports.sort_by_key(|p| p.order_number);

        let mut entries = Vec::with_capacity(ports.len());

        for port in ports {
            entries.push(PortRow {
                front: self.view_slot(ctx, port, PortSide::FRONT),
                port: self.view_port_info(ctx, port),
                back: self.view_slot(ctx, port, PortSide::BACK),
            });
        }

        let table_model = ListModel::new(
            MemoizedTableModel::new(Rc::new(entries)),
            self.table_state.clone(),
        );

        let header = html_nested! {
            <TableHeader<AttachColumn>>
                <TableColumn<AttachColumn> label="Front-Belegung" index={AttachColumn::Front} />
                <TableColumn<AttachColumn> label="Port" index={AttachColumn::Port} />
                <TableColumn<AttachColumn> label="Back-Belegung" index={AttachColumn::Back} />
            </TableHeader<AttachColumn>>
        };

        html! {
            <Table<AttachColumn, ListModel<AttachColumn, MemoizedTableModel<PortRow>>>
                mode={TableMode::Compact}
                grid={TableGridMode::Medium}
                {header}
                entries={table_model}
            />
        }
    }

    fn view_port_info(&self, ctx: &Context<Self>, port: &PlannedPort) -> Html {
        let label = port_label(port.label.as_deref(), port.order_number).into_owned();

        let type_text = match port.port_type {
            PortType::Splice => "Spleiss",
            PortType::Connector => "Stecker",
            PortType::Loop => "Loop",
        };
        let is_loop = port.port_type == PortType::Loop;

        let loop_fiber = if is_loop {
            port.front_usage
                .as_ref()
                .and_then(|u| u.fiber.as_ref())
                .or_else(|| port.back_usage.as_ref().and_then(|u| u.fiber.as_ref()))
        } else {
            None
        };

        let is_modified = (port
            .front_usage
            .as_ref()
            .is_some_and(|u| u.modified_in_plan)
            || port.back_usage.as_ref().is_some_and(|u| u.modified_in_plan))
            && !self.reset_ports.contains(&port.id);

        let loop_hint = if is_loop {
            Some(
                html!(<div class="pf-v6-u-font-size-sm pf-v6-u-color-200">{"Bearbeitung im Loop-Editor"}</div>),
            )
        } else {
            None
        };

        let reset_btn = if is_modified && !is_loop {
            let port_id = port.id;
            Some(html! {
                <IconButton icon={Icon::Redo} name="Änderung zurücksetzen" onclick={ctx.link().callback(move |_| Msg::ResetPort(port_id))} />
            })
        } else {
            None
        };

        let status_icon = if is_modified {
            Some(Icon::InProgress)
        } else {
            None
        };

        let port_display = if is_loop {
            if let Some(fiber_info) = loop_fiber {
                html! {
                    <div style="display: flex; align-items: center; gap: 6px;">
                        <FiberNumber bundle={fiber_info.bundle} fiber={fiber_info.fiber}/>
                        <span class="pf-v6-u-font-size-xs pf-v6-u-color-200">
                            {format!("(#{})", port.order_number)}
                        </span>
                    </div>
                }
            } else {
                html! {
                    <div style="display: flex; align-items: center; gap: 6px;">
                        <strong>{"Loop"}</strong>
                        <span class="pf-v6-u-font-size-xs pf-v6-u-color-200">
                            {format!("(#{})", port.order_number)}
                        </span>
                    </div>
                }
            }
        } else {
            html! {
                <strong>{label}</strong>
            }
        };

        html! {
            <div style="display: flex; align-items: center; justify-content: space-between;">
                <div>
                    {port_display} {status_icon}
                    <div class="pf-v6-u-font-size-sm">{type_text}</div>
                    {loop_hint}
                </div>
                <div>{reset_btn}</div>
            </div>
        }
    }

    fn view_slot(&self, ctx: &Context<Self>, port: &PlannedPort, side: PortSide) -> Html {
        let is_loop = port.port_type == PortType::Loop;

        if let Some(edit) = &self.edit_slot
            && edit.port_id == port.id
            && edit.side == side
        {
            return self.view_inline_edit(ctx, edit);
        }

        let state = self
            .slot_states
            .get(&(port.id, side))
            .cloned()
            .unwrap_or(SlotState::Empty);

        match state {
            SlotState::Assigned(key) => {
                let port_id = port.id;
                let unlink_btn = if !is_loop {
                    Some(
                        html!(<IconButton icon={Icon::Times} name="Faser lösen" onclick={ctx.link().callback(move |_| Msg::ClearSlot(port_id, side))} />),
                    )
                } else {
                    None
                };

                html! {
                    <div style="display: flex; align-items: center; justify-content: space-between;">
                        <div>{ self.view_assigned_slot(&key) }</div>
                        <div>{ unlink_btn }</div>
                    </div>
                }
            }
            SlotState::Empty => {
                if is_loop {
                    Html::default()
                } else {
                    let port_id = port.id;
                    html! {
                        <Button variant={ButtonVariant::Secondary} onclick={ctx.link().callback(move |_| Msg::StartEdit(port_id, side))}>
                            <IconLink/> <span class="pf-v6-u-ml-sm">{"Faser auflegen"}</span>
                        </Button>
                    }
                }
            }
        }
    }

    fn view_assigned_slot(&self, key: &FiberKeyInput) -> Html {
        let cable_end = self.find_cable(key.cable_id);
        let cable_name = cable_end
            .map(|c| c.cable.name.clone())
            .unwrap_or_else(|| format!("Kabel {}", key.cable_id));

        let endpoint = cable_end_label(cable_end.and_then(|c| {
            c.fibers
                .iter()
                .find(|f| f.bundle == key.bundle && f.fiber == key.fiber)
        }))
        .map(|text| {
            html! {<div class="pf-v6-u-font-size-sm pf-v6-u-color-200">
                {Icon::ArrowRight} {" "} {text}
            </div>}
        });

        html! {
            <>
                <div style="font-weight: bold;">{cable_name}</div>
                <div style="margin-top: 4px; margin-bottom: 4px;">
                    <FiberNumber bundle={key.bundle} fiber={key.fiber}/>
                </div>
                {endpoint}
            </>
        }
    }

    fn view_inline_edit(&self, ctx: &Context<Self>, edit: &SlotEdit) -> Html {
        let Load::Loaded(situation) = &self.current_situation else {
            return Html::default();
        };

        // The bundles of the cables that still have free fibers
        let mut available_bundles = Vec::new();
        for cable in &situation.panel.schacht.cables {
            let free_fibers = self.get_free_fibers(cable);
            let mut distinct_bundles: Vec<i32> =
                free_fibers.iter().map(|f| f.bundle).unique().collect();
            distinct_bundles.sort();

            for bundle in distinct_bundles {
                let count = free_fibers.iter().filter(|f| f.bundle == bundle).count();
                available_bundles.push(CableBundleSelectEntry {
                    cable: cable.clone(),
                    bundle,
                    free_count: count,
                });
            }
        }

        // Options keyed by their index in `available_bundles`
        let selected_bundle = edit
            .cable_bundle
            .as_ref()
            .and_then(|selected| available_bundles.iter().position(|b| b == selected));
        let bundle_options = available_bundles
            .iter()
            .enumerate()
            .map(|(idx, b)| (idx, b.label()))
            .collect::<Box<[_]>>();
        let onchange = ctx.link().batch_callback(move |idx: Option<usize>| {
            idx.and_then(|idx| available_bundles.get(idx).cloned())
                .map(Msg::SelectCableBundle)
        });
        let cable_bundle_select = html! {
            <Select<usize> value={selected_bundle} {onchange} placeholder="Kabel & Bündel wählen" options={bundle_options}/>
        };

        let fiber_select = if let Some(cable_bundle) = &edit.cable_bundle {
            let free_fibers = self.get_free_fibers(&cable_bundle.cable);
            let fibers_in_bundle: Vec<_> = free_fibers
                .iter()
                .filter(|f| f.bundle == cable_bundle.bundle)
                .collect();

            let entries = fibers_in_bundle.iter().map(|f| {
                let fiber_num = f.fiber;
                let onclick = ctx.link().callback(move |()| Msg::SelectFiber(fiber_num));

                let extra_text = f
                    .other_end
                    .as_ref()
                    .and_then(|e| e.used_port.as_ref())
                    .and_then(|u| u.panel_side_end_port.as_ref())
                    .map(|end_port| format!(" ({end_port})"))
                    .unwrap_or_default();

                html! {
                    <MenuActionItem key={fiber_num} {onclick}>
                        <FiberLabel fiber={fiber_num}>
                            {fiber_num.to_string()}
                        </FiberLabel>
                        {extra_text}
                    </MenuActionItem>
                }
            });

            html! {
                <PopupMenu text={html!("Faser wählen")}>
                    <MenuGroup>{for entries}</MenuGroup>
                </PopupMenu>
            }
        } else {
            Html::default()
        };
        html! {
            <div style="display: flex; gap: 8px; align-items: center;">
                <div style="display: flex; flex-direction: column; gap: 4px; min-width: 200px;">
                    {cable_bundle_select}
                    {fiber_select}
                </div>
                <IconButton icon={Icon::Times} name="Abbrechen" onclick={ctx.link().callback(|_| Msg::CancelEdit)} />
            </div>
        }
    }

    fn find_cable(&self, cable_id: i32) -> Option<&CableEnd> {
        self.current_situation.loaded().and_then(|s| {
            s.panel
                .schacht
                .cables
                .iter()
                .find(|c| c.cable.id == cable_id)
        })
    }

    fn get_free_fibers<'a>(&self, cable: &'a CableEnd) -> Vec<&'a FiberOwnEnd> {
        let currently_assigned_keys: HashSet<FiberKeyInput> = self
            .slot_states
            .values()
            .filter_map(|s| match s {
                SlotState::Assigned(k) => Some(*k),
                _ => None,
            })
            .collect();

        cable
            .fibers
            .iter()
            .filter(|f| {
                let is_used_in_db = f.used_port.is_some();
                let is_assigned_locally = currently_assigned_keys.contains(&FiberKeyInput {
                    cable_id: cable.cable.id,
                    bundle: f.bundle,
                    fiber: f.fiber,
                });
                !is_used_in_db && !is_assigned_locally
            })
            .collect()
    }
}

fn calculate_current_states(data: &PlannedPanel) -> BTreeMap<(i32, PortSide), SlotState> {
    let mut states = BTreeMap::new();
    for port in &data.ports {
        for (side, usage) in [
            (PortSide::FRONT, &port.front_usage),
            (PortSide::BACK, &port.back_usage),
        ] {
            let state = usage
                .as_ref()
                .and_then(|u| u.fiber)
                .map(|f| {
                    SlotState::Assigned(FiberKeyInput {
                        cable_id: f.cable.id,
                        bundle: f.bundle,
                        fiber: f.fiber,
                    })
                })
                .unwrap_or(SlotState::Empty);
            states.insert((port.id, side), state);
        }
    }
    states
}

/// A cable's bundle, chosen together
#[derive(Clone, Eq, PartialEq)]
pub struct CableBundleSelectEntry {
    cable: CableEnd,
    bundle: i32,
    free_count: usize,
}

impl SelectItemRenderer for CableBundleSelectEntry {
    type Item = i32;

    fn label(&self) -> String {
        format!(
            "{} - Bündel {} ({} frei)",
            self.cable.cable.name, self.bundle, self.free_count
        )
    }
}

fn cable_end_label(option: Option<&FiberOwnEnd>) -> Option<String> {
    option
        .and_then(|f| f.other_end.as_ref())
        .and_then(|e| e.used_port.as_ref())
        .and_then(|p| p.panel_side_end_port.as_ref())
        .map(UsedEndPort::to_string)
}
