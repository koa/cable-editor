use crate::components::page_layout::{PageLayout, object_title};
use crate::components::unsaved::Unsaved;
use crate::{
    components::{
        links::{CableLink, PanelLink, SchachtLink},
        table::ListModel,
    },
    error::FrontendError,
    graphql::authenticated::{
        PortSide,
        current_user::Role,
        plan_details::{PlanDetails, PortUsage},
    },
    icons::{IconLink, IconUnlink},
    pages::router::AppRoute,
    util::{get_backdrop, get_credentials, get_role, toast_error, toast_success},
};
use cable_editor_common::ObjectKind;
use patternfly_yew::prelude::{
    ActionGroup, Backdrop, Bullseye, Button, ButtonVariant, Cell, CellContext, Color,
    ExpansionState, Form, FormGroup, Label, Level, MemoizedTableModel, Modal, ModalVariant,
    Spinner, Table, TableColumn, TableEntryRenderer, TableGridMode, TableHeader, TableMode,
    TextInput, Title,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use yew::{
    Callback, Component, Context, Html, Properties, html, html::IntoPropValue, html_nested,
    platform::spawn_local,
};
use yew_nested_router::prelude::RouterContext;

#[derive(Properties, PartialEq, Clone)]
pub struct EditPlanProps {
    pub plan_id: i32,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum UsageColumn {
    Location,
    Port,
    Front,
    Back,
}

#[derive(Clone, PartialEq, Debug)]
struct PortUsageRow {
    pub port_id: i32,
    pub schacht_id: i32,
    pub schacht_name: String,
    /// From the root panel down to the port's own panel
    panel: Box<[PanelChain]>,
    pub port_label: String,
    pub front: Option<PortUsage>,
    pub back: Option<PortUsage>,
}

#[derive(Clone, PartialEq, Debug, Ord, PartialOrd, Eq)]
struct PanelChain {
    panel_id: i32,
    panel_name: String,
}

impl TableEntryRenderer<UsageColumn> for PortUsageRow {
    fn render_cell(&self, context: CellContext<'_, UsageColumn>) -> Cell {
        match context.column {
            UsageColumn::Location => Cell::new(html! {
                <>
                    <SchachtLink id={self.schacht_id} text={self.schacht_name.clone()}/>
                    {for self.panel.iter().map(|panel| html! {
                        <>
                            {" - "}
                            <PanelLink id={panel.panel_id} text={panel.panel_name.clone()}/>
                        </>
                    })}
                </>
            }),
            // Ports have no page of their own, their panel's connection overview shows them
            UsageColumn::Port => Cell::new(match self.panel.last() {
                Some(panel) => {
                    html!(<PanelLink id={panel.panel_id} text={self.port_label.clone()}/>)
                }
                None => self.port_label.clone().into_prop_value(),
            }),
            UsageColumn::Front => Cell::new(render_action(&self.front)),
            UsageColumn::Back => Cell::new(render_action(&self.back)),
        }
    }
}

fn render_action(usage: &Option<PortUsage>) -> Html {
    match usage {
        None => html! { <span class="pf-v6-u-color-200">{"Unverändert"}</span> },
        Some(u) => {
            if let Some(fiber) = &u.fiber {
                html! {
                    <>
                        <IconLink/>
                        <span class="pf-v6-u-ml-sm">
                            <CableLink id={fiber.cable.id} text={fiber.cable.name.clone()}/>
                            {format!(" ({}-{})", fiber.bundle, fiber.fiber)}
                        </span>
                    </>
                }
            } else {
                html! { <><IconUnlink/> <span class="pf-v6-u-ml-sm">{"Entfernt"}</span></> }
            }
        }
    }
}

pub struct EditPlan {
    details: Option<PlanDetails>,
    edit_name: String,
    loading: bool,
    saving: bool,
    error: Option<FrontendError>,
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<UsageColumn>>>>,
    unsaved: Unsaved,
}

/// What a plan's page does with a request, to word the toast when it fails
#[derive(Clone, Copy)]
pub enum Action {
    SaveName,
    Implement,
    ActivateInNetbox,
}

impl Action {
    fn failed_title(self) -> &'static str {
        match self {
            Action::SaveName => "Planung konnte nicht gespeichert werden",
            Action::Implement => "Planung konnte nicht abgeschlossen werden",
            Action::ActivateInNetbox => "Planung konnte nicht aktiviert werden",
        }
    }
}

pub enum Msg {
    FetchData,
    DataFetched(Option<PlanDetails>),
    UpdateNameInput(String),
    SaveName,
    Saved(PlanDetails),
    AskImplement,
    ImplementPlan,
    /// The plan is merged into the baseline and deleted
    Implemented,
    Error(FrontendError),
    /// An action failed: the page and the input stay, the error is a toast titled by the action
    ActionFailed(Action, FrontendError),
    AskNetboxActive,
    SetNetboxActive,
    NetboxActivated(PlanDetails),
}

impl Component for EditPlan {
    type Message = Msg;
    type Properties = EditPlanProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            unsaved: Unsaved::new(ctx.link()),
            details: None,
            edit_name: String::new(),
            loading: true,
            saving: false,
            error: None,
            table_state: Rc::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchData => {
                self.loading = true;
                let plan_id = ctx.props().plan_id;
                let scope = ctx.link().clone();
                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope.send_message(
                        PlanDetails::fetch(credentials.as_ref(), plan_id)
                            .await
                            .map_or_else(Msg::Error, Msg::DataFetched),
                    );
                });
                true
            }
            Msg::DataFetched(Some(data)) => {
                self.edit_name = data.name.clone();
                self.details = Some(data);
                self.loading = false;
                self.error = None;
                true
            }
            Msg::DataFetched(None) => {
                self.loading = false;
                self.error = Some(FrontendError::not_found(
                    ObjectKind::Plan,
                    ctx.props().plan_id,
                ));
                true
            }
            Msg::UpdateNameInput(name) => {
                self.edit_name = name;
                true
            }
            Msg::SaveName => {
                self.saving = true;
                let plan_id = ctx.props().plan_id;
                let name = self.edit_name.clone();
                let scope = ctx.link().clone();
                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope.send_message(
                        PlanDetails::update_name(credentials.as_ref(), plan_id, name)
                            .await
                            .map_or_else(
                                |error| Msg::ActionFailed(Action::SaveName, error),
                                Msg::Saved,
                            ),
                    );
                });
                true
            }
            Msg::AskImplement => {
                if let Some(backdrop) = get_backdrop(ctx.link()) {
                    let scope = ctx.link().clone();
                    let on_confirm = {
                        let bd = backdrop.clone();
                        let scope = scope.clone();
                        Callback::from(move |_| {
                            bd.close();
                            scope.send_message(Msg::ImplementPlan);
                        })
                    };
                    let on_cancel = {
                        let backdrop = backdrop.clone();
                        Callback::from(move |_| backdrop.close())
                    };

                    backdrop.open(Backdrop::new(html! {
                        <Bullseye>
                            <Modal
                                title="Planung abschliessen" 
                                variant={ModalVariant::Small}
                                footer={html!{
                                    <>
                                        <Button label="Abschliessen" variant={ButtonVariant::Danger} onclick={on_confirm}/>
                                        <Button label="Abbrechen" variant={ButtonVariant::Link} onclick={on_cancel}/>
                                    </>
                                }}>
                                <p>{"Möchten Sie diese Planung wirklich abschliessen? Die Änderungen werden aktiv und die Planung schreibgeschützt."}</p>
                            </Modal>
                        </Bullseye>
                    }));
                }
                false
            }
            Msg::ImplementPlan => {
                self.saving = true;
                let plan_id = ctx.props().plan_id;
                let scope = ctx.link().clone();
                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope.send_message(
                        PlanDetails::implement(credentials.as_ref(), plan_id)
                            .await
                            .map_or_else(
                                |error| Msg::ActionFailed(Action::Implement, error),
                                |_| Msg::Implemented,
                            ),
                    );
                });
                true
            }
            Msg::Saved(data) => {
                self.saving = false;
                self.error = None;
                toast_success(ctx.link(), "Planung umbenannt");
                self.edit_name = data.name.clone();
                self.details = Some(data);
                true
            }
            Msg::Implemented => {
                self.saving = false;
                toast_success(ctx.link(), "Planung abgeschlossen");
                // Replaced, so going back doesn't lead to the deleted plan
                if let Some((router, _)) = ctx
                    .link()
                    .context::<RouterContext<AppRoute>>(Callback::noop())
                {
                    router.replace(AppRoute::ListOfPlans);
                }
                false
            }
            Msg::Error(error) => {
                self.error = Some(error);
                self.loading = false;
                self.saving = false;
                true
            }
            Msg::ActionFailed(action, error) => {
                self.saving = false;
                toast_error(ctx.link(), action.failed_title(), error);
                true
            }
            Msg::AskNetboxActive => {
                if let Some(backdrop) = get_backdrop(ctx.link()) {
                    let on_confirm = {
                        let backdrop = backdrop.clone();
                        let scope = ctx.link().clone();
                        Callback::from(move |_| {
                            backdrop.close();
                            scope.send_message(Msg::SetNetboxActive);
                        })
                    };
                    let on_cancel = {
                        let backdrop = backdrop.clone();
                        Callback::from(move |_| backdrop.close())
                    };
                    backdrop.open(Backdrop::new(html! {
                        <Bullseye>
                            <Modal
                                title="In Netbox aktivieren"
                                variant={ModalVariant::Small}
                                footer={html!{
                                    <>
                                        <Button label="In Netbox aktivieren" variant={ButtonVariant::Danger} onclick={on_confirm}/>
                                        <Button label="Abbrechen" variant={ButtonVariant::Link} onclick={on_cancel}/>
                                    </>
                                }}>
                                <p>{"Netbox zeigt danach die Circuits dieser Planung, statt die des bisher aktiven. Sie werden automatisch synchronisiert; findet der Sync Probleme, bleibt Netbox unverändert."}</p>
                            </Modal>
                        </Bullseye>
                    }));
                }
                false
            }
            Msg::SetNetboxActive => {
                self.saving = true;
                let plan_id = ctx.props().plan_id;
                let scope = ctx.link().clone();
                spawn_local(async move {
                    let credentials = get_credentials(&scope);
                    scope.send_message(
                        PlanDetails::set_netbox_active(credentials.as_ref(), plan_id)
                            .await
                            .map_or_else(
                                |error| Msg::ActionFailed(Action::ActivateInNetbox, error),
                                Msg::NetboxActivated,
                            ),
                    );
                });
                true
            }
            Msg::NetboxActivated(data) => {
                self.saving = false;
                self.details = Some(data);
                toast_success(ctx.link(), "In Netbox aktiviert");
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <PageLayout title={object_title("Planung bearbeiten", self.details.as_ref().map(|details| &details.name))}>{self.view_content(ctx)}</PageLayout>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        self.unsaved.set(
            self.details
                .as_ref()
                .is_some_and(|details| self.edit_name != details.name),
        );
        if first_render {
            ctx.link().send_message(Msg::FetchData);
        }
    }
}

impl EditPlan {
    fn view_content(&self, ctx: &Context<Self>) -> Html {
        if self.loading {
            return html!(<Spinner />);
        }

        let Some(details) = &self.details else {
            return match &self.error {
                Some(error) => error.into_prop_value(),
                None => (&FrontendError::not_found(ObjectKind::Plan, ctx.props().plan_id))
                    .into_prop_value(),
            };
        };

        let is_open = !details.is_baseline;
        let role = get_role(ctx.link());
        let can_rename = is_open && role >= Role::Planner;
        let name_changed = self.edit_name != details.name;

        // Tabelle aufbereiten: Usages nach Port-ID gruppieren
        let mut row_map: HashMap<i32, PortUsageRow> = HashMap::new();
        for u in &details.usage {
            let entry = row_map.entry(u.port.id).or_insert_with(|| {
                // parent_chain holds only the parents, the port's own panel comes last
                let panel = &u.port.panel;
                let panel_chain: Vec<_> = panel
                    .parent_chain
                    .iter()
                    .map(|p| (p.id, &p.name))
                    .chain([(panel.id, &panel.name)])
                    .map(|(panel_id, name)| PanelChain {
                        panel_id,
                        panel_name: name.clone().unwrap_or_else(|| format!("Panel {panel_id}")),
                    })
                    .collect();
                PortUsageRow {
                    port_id: u.port.id,
                    schacht_id: u.port.panel.schacht.id,
                    schacht_name: u.port.panel.schacht.name.clone(),

                    port_label: u.port.label.clone().unwrap_or_default(),
                    front: None,
                    back: None,
                    panel: panel_chain.into_boxed_slice(),
                }
            });
            match u.side {
                PortSide::FRONT => entry.front = Some(u.clone()),
                PortSide::BACK => entry.back = Some(u.clone()),
            }
        }

        let mut rows: Vec<PortUsageRow> = row_map.into_values().collect();
        rows.sort_by(|a, b| {
            a.schacht_name
                .cmp(&b.schacht_name)
                .then(a.panel.cmp(&b.panel))
                .then(a.port_label.cmp(&b.port_label))
        });

        let table_model = ListModel::new(
            MemoizedTableModel::new(Rc::new(rows)),
            self.table_state.clone(),
        );

        let header = html_nested! {
            <TableHeader<UsageColumn>>
                <TableColumn<UsageColumn> label="Schacht - Panel" index={UsageColumn::Location} />
                <TableColumn<UsageColumn> label="Port" index={UsageColumn::Port} />
                <TableColumn<UsageColumn> label="Vorne" index={UsageColumn::Front} />
                <TableColumn<UsageColumn> label="Hinten" index={UsageColumn::Back} />
            </TableHeader<UsageColumn>>
        };

        let kind = if details.is_baseline {
            "Ist-Zustand"
        } else {
            "Offene Planung"
        };

        html! {
            <div class="pf-v6-c-panel">
                <div class="pf-v6-c-panel__main">
                    <div class="pf-v6-c-panel__main-body">
                        if let Some(err) = &self.error {
                            { IntoPropValue::<Html>::into_prop_value(err) }
                        }

                        <Form>
                            <FormGroup label="Art">
                                <div><strong>{kind}</strong></div>
                            </FormGroup>

                            <FormGroup label="Planungs-Name">
                                <div style="display: flex; gap: 8px;">
                                    <TextInput
                                        value={self.edit_name.clone()}
                                        onchange={ctx.link().callback(Msg::UpdateNameInput)}
                                        disabled={!can_rename || self.saving}
                                    />
                                    <Button
                                        label="Umbenennen"
                                        variant={ButtonVariant::Secondary}
                                        disabled={!can_rename || !name_changed || self.saving}
                                        onclick={ctx.link().callback(|_| Msg::SaveName)}
                                    />
                                </div>
                            </FormGroup>
                            <FormGroup label="Netbox">
                                if details.netbox_active {
                                    <div>
                                        <Label label="In Netbox aktiv" compact=true color={Color::Blue}/>
                                        {" Netbox zeigt die Circuits dieser Planung."}
                                    </div>
                                } else {
                                    <div>{"Netbox zeigt die Circuits einer anderen Planung."}</div>
                                    if role >= Role::Admin {
                                        <div class="pf-v6-u-mt-sm">
                                            <Button
                                                variant={ButtonVariant::Secondary}
                                                label="In Netbox aktivieren"
                                                disabled={self.saving}
                                                onclick={ctx.link().callback(|_| Msg::AskNetboxActive)}
                                            />
                                        </div>
                                    }
                                }
                            </FormGroup>
                        </Form>

                        if is_open {
                            <div class="pf-v6-u-mt-xl">
                                <Title level={Level::H2}>{"Geplante Änderungen"}</Title>
                                <Table<UsageColumn, ListModel<UsageColumn, MemoizedTableModel<PortUsageRow>>>
                                    mode={TableMode::Compact}
                                    grid={TableGridMode::Medium}
                                    {header}
                                    entries={table_model}
                                />
                            </div>
                            if role >= Role::Admin {
                                <div class="pf-v6-u-mt-xl">
                                    <ActionGroup>
                                        <Button
                                            label="Planung abschliessen"
                                            variant={ButtonVariant::Primary}
                                            disabled={self.saving}
                                            onclick={ctx.link().callback(|_| Msg::AskImplement)}
                                        />
                                    </ActionGroup>
                                </div>
                            }
                        }
                    </div>
                </div>
            </div>
        }
    }
}
