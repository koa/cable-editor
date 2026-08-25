use crate::{
    components::table::ListModel,
    error::FrontendError,
    graphql::authenticated::{
        PortSide,
        list_plans::PlanStatus,
        plan_details::{PlanDetails, PortUsage},
    },
    icons::{IconLink, IconUnlink},
    util::{get_backdrop, get_credentials},
};
use patternfly_yew::prelude::{
    ActionGroup, Alert, AlertType, Backdrop, Backdropper, Bullseye, Button, ButtonVariant, Cell,
    CellContext, ExpansionState, Form, FormGroup, MemoizedTableModel, Modal, ModalVariant, Spinner,
    Table, TableColumn, TableEntryRenderer, TableGridMode, TableHeader, TableMode, TextInput,
    Title,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use yew::{
    Callback, Component, Context, Html, Properties, html, html::IntoPropValue, html_nested,
    platform::spawn_local,
};

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
    pub schacht_name: String,
    pub panel_name: String,
    pub port_label: String,
    pub front: Option<PortUsage>,
    pub back: Option<PortUsage>,
}

impl TableEntryRenderer<UsageColumn> for PortUsageRow {
    fn render_cell(&self, context: CellContext<'_, UsageColumn>) -> Cell {
        match context.column {
            UsageColumn::Location => {
                let text = format!("{} - {}", self.schacht_name, self.panel_name);
                Cell::new(text.into_prop_value())
            }
            UsageColumn::Port => Cell::new(self.port_label.clone().into_prop_value()),
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
                            {format!("{} ({}-{})", fiber.cable.name, fiber.bundle, fiber.fiber)}
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
}

pub enum Msg {
    FetchData,
    DataFetched(Option<PlanDetails>),
    UpdateNameInput(String),
    SaveName,
    Saved(PlanDetails),
    AskImplement,
    ImplementPlan,
    Error(FrontendError),
}

impl Component for EditPlan {
    type Message = Msg;
    type Properties = EditPlanProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
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
                self.error = Some(FrontendError::NotFound);
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
                            .map_or_else(Msg::Error, Msg::Saved),
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
                            .map_or_else(Msg::Error, Msg::Saved),
                    );
                });
                true
            }
            Msg::Saved(data) => {
                self.saving = false;
                self.error = None;
                self.edit_name = data.name.clone();
                self.details = Some(data);
                true
            }
            Msg::Error(error) => {
                self.error = Some(error);
                self.loading = false;
                self.saving = false;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if self.loading {
            return html!(<Spinner />);
        }

        let Some(details) = &self.details else {
            return html!(<Alert title="Nicht gefunden" r#type={AlertType::Danger} inline=true />);
        };

        let is_open = details.status == PlanStatus::OPEN;
        let name_changed = self.edit_name != details.name;

        // Tabelle aufbereiten: Usages nach Port-ID gruppieren
        let mut row_map: HashMap<i32, PortUsageRow> = HashMap::new();
        for u in &details.usage {
            let entry = row_map.entry(u.port.id).or_insert_with(|| PortUsageRow {
                port_id: u.port.id,
                schacht_name: u.port.panel.schacht.name.clone(),
                panel_name: u.port.panel.name.clone().unwrap_or_default(),
                port_label: u.port.label.clone().unwrap_or_default(),
                front: None,
                back: None,
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
                .then(a.panel_name.cmp(&b.panel_name))
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

        let status_label = match details.status {
            PlanStatus::OPEN => "Offen",
            PlanStatus::IMPLEMENTED => "Implementiert (Abgeschlossen)",
            PlanStatus::REJECTED => "Verworfen",
        };

        html! {
            <div class="pf-v6-c-panel">
                <div class="pf-v6-c-panel__main">
                    <div class="pf-v6-c-panel__main-body">
                        <Title size={patternfly_yew::prelude::Size::XLarge}>{"Planung bearbeiten"}</Title>

                        if let Some(err) = &self.error {
                            <Alert title={err.to_string()} r#type={AlertType::Danger} inline=true />
                        }

                        <Form>
                            <FormGroup label="Status">
                                <div><strong>{status_label}</strong></div>
                            </FormGroup>

                            <FormGroup label="Planungs-Name">
                                <div style="display: flex; gap: 8px;">
                                    <TextInput
                                        value={self.edit_name.clone()}
                                        onchange={ctx.link().callback(Msg::UpdateNameInput)}
                                        disabled={!is_open || self.saving}
                                    />
                                    <Button
                                        label="Umbenennen"
                                        variant={ButtonVariant::Secondary}
                                        disabled={!is_open || !name_changed || self.saving}
                                        onclick={ctx.link().callback(|_| Msg::SaveName)}
                                    />
                                </div>
                            </FormGroup>
                        </Form>

                        <div class="pf-v6-u-mt-xl">
                            <Title size={patternfly_yew::prelude::Size::Large}>{"Geplante Änderungen"}</Title>
                            <Table<UsageColumn, ListModel<UsageColumn, MemoizedTableModel<PortUsageRow>>>
                                mode={TableMode::Compact}
                                grid={TableGridMode::Medium}
                                {header}
                                entries={table_model}
                            />
                        </div>

                        if is_open {
                            <div class="pf-v6-u-mt-xl">
                                <ActionGroup>
                                    <Button
                                        label="Planung abschliessen (Implementieren)"
                                        variant={ButtonVariant::Primary}
                                        disabled={self.saving}
                                        onclick={ctx.link().callback(|_| Msg::AskImplement)}
                                    />
                                </ActionGroup>
                            </div>
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
