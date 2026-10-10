//! The work order of a plan: what is to be done in the field to implement it, per Schacht and
//! root panel. Built from `Plan.changedPorts` (each changed port with its fibers now and in the
//! plan): the loops to cut, the splices to make or cut (splice ports and the pigtail spliced to
//! the back of a connector) and, less important, the plugs to change on the front of connectors.
//! A loop only means a fiber is not cut: a fiber cut once can't be looped again, only spliced, so
//! a loop the plan adds is nothing to do.

use crate::components::load::Load;
use crate::components::select::Select;
use crate::graphql::authenticated::SchachtRef;
use crate::{
    components::{
        fiber::FiberNumber,
        links::{CableLink, PanelLink, SchachtLink},
        page_layout::{PageLayout, object_title},
        print_page::PrintPageButton,
        table::ListTable,
    },
    error::FrontendError,
    graphql::authenticated::{
        PortType, local_time, port_label,
        work_order::{PortChange, WorkOrderFiber, WorkOrderPlan},
        write_panel_path, write_port_label,
    },
    util::get_credentials,
};
use cable_editor_common::ObjectKind;
use patternfly_yew::prelude::{
    Alert, AlertType, Cell, CellContext, ColumnWidth, FormGroup, Level, TableColumn,
    TableEntryRenderer, TableHeader, Title,
};
use std::{collections::BTreeMap, rc::Rc};
use yew::{
    Component, Context, Html, Properties, html, html::IntoPropValue, html_nested,
    platform::spawn_local,
};

#[derive(Properties, PartialEq)]
pub struct WorkOrderProps {
    pub plan_id: i32,
}

pub enum Msg {
    Fetch,
    Loaded(Result<WorkOrderPlan, FrontendError>),
    Filter(Option<i32>),
}

pub struct WorkOrder {
    /// `None` while loading
    plan: Load<WorkOrderPlan>,
    /// The Schacht shown alone, e.g. to print its order, `None` for all
    schacht: Option<i32>,
    /// When the shown data was loaded, printed with every Schacht
    loaded_at: Option<js_sys::Date>,
}

impl Component for WorkOrder {
    type Message = Msg;
    type Properties = WorkOrderProps;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Fetch);
        Self {
            plan: Load::Pending,
            schacht: None,
            loaded_at: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Fetch => {
                let scope = ctx.link().clone();
                let credentials = get_credentials(&scope);
                let plan_id = ctx.props().plan_id;
                spawn_local(async move {
                    let plan = WorkOrderPlan::fetch(credentials.as_ref(), plan_id)
                        .await
                        .and_then(|plan| {
                            plan.ok_or_else(|| FrontendError::not_found(ObjectKind::Plan, plan_id))
                        });
                    scope.send_message(Msg::Loaded(plan));
                });
                false
            }
            Msg::Loaded(plan) => {
                self.plan = Load::from(plan);
                self.loaded_at = Some(js_sys::Date::new_0());
                true
            }
            Msg::Filter(schacht) => {
                self.schacht = schacht;
                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        if ctx.props().plan_id != old_props.plan_id {
            self.plan = Load::Pending;
            self.schacht = None;
            ctx.link().send_message(Msg::Fetch);
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let title = object_title("Arbeitsauftrag", self.plan.loaded().map(|plan| &plan.name));
        html! {
            <PageLayout {title}>{self.plan.view(|plan| self.view_plan(ctx, plan))}</PageLayout>
        }
    }
}

impl WorkOrder {
    fn view_plan(&self, ctx: &Context<Self>, plan: &WorkOrderPlan) -> Html {
        if plan.is_baseline {
            return html! {
                <Alert inline=true r#type={AlertType::Info}
                    title="Der Ist-Zustand hat keinen Arbeitsauftrag: er plant keine Änderungen."/>
            };
        }
        let orders = schacht_orders(&plan.changed_ports);
        if orders.is_empty() {
            return html! {
                <Alert inline=true r#type={AlertType::Info}
                    title="Diese Planung ändert keine Spleisse, Stecker oder Loops."/>
            };
        }
        let onchange = ctx.link().callback(Msg::Filter);
        let stand = self.loaded_at.as_ref().map(local_time).unwrap_or_default();
        html! {
            <div class="work-order">
                <div class="work-order__toolbar no-print">
                    <FormGroup label="Schacht">
                        <Select<i32> value={self.schacht} {onchange} placeholder="Alle Schächte"
                            options={orders.iter().map(|order| (order.schacht.id, order.schacht.name.clone())).collect::<Box<[_]>>()}/>
                    </FormGroup>
                    <PrintPageButton/>
                </div>
                { for orders
                    .iter()
                    .filter(|order| self.schacht.is_none_or(|id| id == order.schacht.id))
                    .map(|order| view_schacht(order, &plan.name, &stand)) }
            </div>
        }
    }
}

/// What is to be done in a Schacht
struct SchachtOrder<'a> {
    schacht: &'a SchachtRef,
    panels: Vec<PanelOrder<'a>>,
}

/// What is to be done at a root panel (with the panels in it)
struct PanelOrder<'a> {
    id: i32,
    name: Option<&'a str>,
    loops: Vec<&'a PortChange>,
    splices: Vec<Splice<'a>>,
    plugs: Vec<&'a PortChange>,
}

/// A splice to make, cut or change: of a splice port (front with back) or the pigtail spliced to
/// the cable on the back of a connector
struct Splice<'a> {
    change: &'a PortChange,
    pigtail: bool,
}

/// The work per Schacht (by name) and root panel (by name), each list by panel and port; only
/// what changes in the field.
fn schacht_orders(changes: &[PortChange]) -> Vec<SchachtOrder<'_>> {
    type Panels<'a> = BTreeMap<(Option<&'a str>, i32), PanelOrder<'a>>;
    let mut by_schacht = BTreeMap::<(&str, i32), (&SchachtRef, Panels)>::new();
    for change in changes {
        let panel = &change.port.panel;
        let schacht = &panel.schacht;
        let (root_id, root_name) = match panel.parent_chain.first() {
            Some(root) => (root.id, root.name.as_deref()),
            None => (panel.id, panel.name.as_deref()),
        };
        let order = by_schacht
            .entry((schacht.name.as_str(), schacht.id))
            .or_insert_with(|| (schacht, BTreeMap::new()))
            .1
            .entry((root_name, root_id))
            .or_insert_with(|| PanelOrder {
                id: root_id,
                name: root_name,
                loops: Vec::new(),
                splices: Vec::new(),
                plugs: Vec::new(),
            });
        match change.port.port_type {
            PortType::Loop => {
                if is_cut_loop(change, changes) {
                    order.loops.push(change);
                }
            }
            PortType::Splice => order.splices.push(Splice {
                change,
                pigtail: false,
            }),
            PortType::Connector => {
                if change.current_back != change.planned_back {
                    order.splices.push(Splice {
                        change,
                        pigtail: true,
                    });
                }
                if change.current_front != change.planned_front {
                    order.plugs.push(change);
                }
            }
        }
    }
    by_schacht
        .into_values()
        .map(|(schacht, panels)| SchachtOrder {
            schacht,
            panels: panels
                .into_values()
                .filter(|p| !p.loops.is_empty() || !p.splices.is_empty() || !p.plugs.is_empty())
                .collect(),
        })
        .filter(|order| !order.panels.is_empty())
        .collect()
}

/// Whether the fiber looped through this port now isn't looped any more in the plan, in no port
/// of the panel (the loop editor may move a loop to another port)
fn is_cut_loop(change: &PortChange, changes: &[PortChange]) -> bool {
    if change.current_front.is_none() && change.current_back.is_none() {
        return false;
    }
    let looped = (&change.current_front, &change.current_back);
    !changes.iter().any(|other| {
        other.port.panel.id == change.port.panel.id
            && other.port.port_type == PortType::Loop
            && (&other.planned_front, &other.planned_back) == looped
    })
}

fn view_schacht(order: &SchachtOrder, plan: &str, stand: &str) -> Html {
    let schacht = order.schacht;
    html! {
        <section class="work-order__schacht">
            <p class="work-order__print-line">
                {format!("Arbeitsauftrag {plan} · Stand {stand}")}
            </p>
            <Title level={Level::H2}>
                {"Schacht "}<SchachtLink id={schacht.id} text={schacht.name.clone()}/>
            </Title>
            { for order.panels.iter().map(view_panel) }
        </section>
    }
}

fn view_panel(order: &PanelOrder) -> Html {
    let name = order.name.unwrap_or("Panel").to_string();
    html! {
        <div class="work-order__panel">
            <Title level={Level::H3}>{"Panel "}<PanelLink id={order.id} text={name}/></Title>
            if !order.loops.is_empty() {
                <Title level={Level::H4}>{"Loops auftrennen"}</Title>
                {view_loops(&order.loops)}
            }
            if !order.splices.is_empty() {
                <Title level={Level::H4}>{"Spleisse"}</Title>
                {view_table(
                    order.splices.iter().map(|splice| {
                        let change = splice.change;
                        let (before, after) = if splice.pigtail {
                            (
                                view_fiber(change.current_back.as_ref()),
                                view_fiber(change.planned_back.as_ref()),
                            )
                        } else {
                            (
                                view_pair(change.current_front.as_ref(), change.current_back.as_ref()),
                                view_pair(change.planned_front.as_ref(), change.planned_back.as_ref()),
                            )
                        };
                        let had = change.current_front.is_some() || change.current_back.is_some();
                        let has = change.planned_front.is_some() || change.planned_back.is_some();
                        let work = match (splice.pigtail, had, has) {
                            (false, false, _) => "Spleissen",
                            (false, true, false) => "Auftrennen",
                            (false, true, true) => "Umspleissen",
                            (true, _, _) => match (change.current_back.is_some(), change.planned_back.is_some()) {
                                (false, _) => "Pigtail anspleissen",
                                (true, false) => "Pigtail abtrennen",
                                (true, true) => "Pigtail umspleissen",
                            },
                        };
                        WorkRow { location: view_location(change, order.id), work, before, after }
                    }),
                )}
            }
            if !order.plugs.is_empty() {
                <Title level={Level::H4}>{"Steckverbindungen"}</Title>
                {view_table(
                    order.plugs.iter().map(|change| {
                        let work = match (change.current_front.is_some(), change.planned_front.is_some()) {
                            (false, _) => "Stecken",
                            (true, false) => "Ziehen",
                            (true, true) => "Umstecken",
                        };
                        WorkRow {
                            location: view_location(change, order.id),
                            work,
                            before: view_fiber(change.current_front.as_ref()),
                            after: view_fiber(change.planned_front.as_ref()),
                        }
                    }),
                )}
            }
        </div>
    }
}

/// The loops to cut by the cables they join, each with only its fibers: on a phone in the
/// field the colours are what tells them apart
fn view_loops(loops: &[&PortChange]) -> Html {
    fn cable(fiber: &Option<WorkOrderFiber>) -> Option<(&str, i32)> {
        fiber.as_ref().map(|f| (f.cable.name.as_str(), f.cable.id))
    }
    let mut pairs = BTreeMap::<_, (&PortChange, Vec<&WorkOrderFiber>)>::new();
    for change in loops {
        let Some(fiber) = change
            .current_front
            .as_ref()
            .or(change.current_back.as_ref())
        else {
            continue;
        };
        pairs
            .entry((cable(&change.current_front), cable(&change.current_back)))
            .or_insert_with(|| (change, Vec::new()))
            .1
            .push(fiber);
    }
    html! {
        <ul class="work-order__loops">
            { for pairs.into_values().map(|(change, mut fibers)| {
                fibers.sort_by_key(|fiber| (fiber.bundle, fiber.fiber));
                html! {
                    <li class="work-order__loop">
                        <span class="work-order__loop-cables">
                            {view_cable(change.current_front.as_ref())}
                            {" ↔ "}
                            {view_cable(change.current_back.as_ref())}
                        </span>
                        <span class="work-order__loop-fibers">
                            { for fibers.into_iter().map(|fiber| html!(<FiberNumber bundle={fiber.bundle} fiber={fiber.fiber}/>)) }
                        </span>
                    </li>
                }
            }) }
        </ul>
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Columns {
    Location,
    Work,
    Before,
    After,
}

/// A splice or plug to change
#[derive(Clone)]
struct WorkRow {
    location: Html,
    work: &'static str,
    before: Html,
    after: Html,
}

impl TableEntryRenderer<Columns> for WorkRow {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        // One element, as on phones the cell lays out each child on its own
        Cell::new(html! {
            <div>{match context.column {
                Columns::Location => self.location.clone(),
                Columns::Work => self.work.into_prop_value(),
                Columns::Before => self.before.clone(),
                Columns::After => self.after.clone(),
            }}</div>
        })
    }
}

/// The changes of a panel, as a table on the printed page too
fn view_table(rows: impl Iterator<Item = WorkRow>) -> Html {
    let header = html_nested! {
        <TableHeader<Columns>>
            <TableColumn<Columns> label="Ort" index={Columns::Location} width={ColumnWidth::Percent(20)}/>
            <TableColumn<Columns> label="Arbeit" index={Columns::Work} width={ColumnWidth::Percent(20)}/>
            <TableColumn<Columns> label="Bisher" index={Columns::Before} width={ColumnWidth::Percent(30)}/>
            <TableColumn<Columns> label="Neu" index={Columns::After} width={ColumnWidth::Percent(30)}/>
        </TableHeader<Columns>>
    };
    html! {
        <ListTable<Columns, WorkRow>
            {header}
            rows={Rc::new(rows.collect::<Vec<_>>())}
            empty="Nichts zu tun."
            printable=true
        />
    }
}

/// The panels below the root panel and the port, e.g. "Spleisskassette 2 : S2-7"
fn view_location(change: &PortChange, root_id: i32) -> Html {
    let port = &change.port;
    let panel = &port.panel;
    let mut path = panel
        .parent_chain
        .iter()
        .filter(|parent| parent.id != root_id)
        .filter_map(|parent| parent.name.as_deref())
        .chain(
            (panel.id != root_id)
                .then_some(panel.name.as_deref())
                .flatten(),
        )
        .peekable();
    let label = port_label(port.label.as_deref(), port.order_number);
    let text = if path.peek().is_none() {
        label.into_owned()
    } else {
        // Writing into a String can't fail
        let mut text = String::new();
        let _ = write_panel_path(&mut text, None, path)
            .and_then(|()| write_port_label(&mut text, &label));
        text
    };
    html!(<PanelLink id={panel.id} {text}/>)
}

fn view_pair(front: Option<&WorkOrderFiber>, back: Option<&WorkOrderFiber>) -> Html {
    if front.is_none() && back.is_none() {
        return html!("—");
    }
    html! {
        <>{view_fiber(front)}{" ↔ "}{view_fiber(back)}</>
    }
}

fn view_fiber(fiber: Option<&WorkOrderFiber>) -> Html {
    match fiber {
        None => html!("—"),
        Some(fiber) => html! {
            <span class="work-order__fiber">
                <CableLink id={fiber.cable.id} text={fiber.cable.name.clone()}/>
                {" "}
                <FiberNumber bundle={fiber.bundle} fiber={fiber.fiber}/>
            </span>
        },
    }
}

fn view_cable(fiber: Option<&WorkOrderFiber>) -> Html {
    match fiber {
        None => html!("—"),
        Some(fiber) => html!(<CableLink id={fiber.cable.id} text={fiber.cable.name.clone()}/>),
    }
}
