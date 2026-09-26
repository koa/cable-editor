use crate::components::page_layout::PageLayout;
use crate::{
    components::table::ListModel,
    error::FrontendError,
    graphql::authenticated::{current_user::Role, list_plans::PlanListEntry},
    pages::router::{AppRoute, PlanView},
    util::{get_backdrop, get_credentials, get_role},
};
use patternfly_yew::prelude::{
    ActionGroup, Backdrop, Bullseye, Button, ButtonVariant, Cell, CellContext, ExpansionState,
    Form, FormGroup, LabelIcon, MemoizedTableModel, Modal, PopoverBody, Spinner, Table,
    TableColumn, TableEntryRenderer, TableGridMode, TableHeader, TableMode, TextInput,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use yew::{
    Callback, Component, Context, Html, Properties, html, html::IntoPropValue, html_nested,
    platform::spawn_local,
};
use yew_nested_router::components::Link;

pub struct ListOfPlannings {
    error: Option<FrontendError>,
    data: Option<Rc<Vec<PlanListEntry>>>,
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<Columns>>>>,
}

#[derive(Debug)]
pub enum Msg {
    Data(Box<[PlanListEntry]>),
    Error(FrontendError),
    Refresh,
}

#[derive(PartialEq, Properties)]
pub struct ListOfPlanningProps {}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Columns {
    Name,
    Kind,
}

impl Component for ListOfPlannings {
    type Message = Msg;
    type Properties = ListOfPlanningProps;

    fn create(_ctx: &Context<Self>) -> Self {
        ListOfPlannings {
            error: None,
            data: None,
            table_state: Rc::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Data(data) => {
                self.error = None;
                self.data = Some(Rc::new(data.into_vec()));
                true
            }
            Msg::Error(e) => {
                self.error = Some(e);
                true
            }
            Msg::Refresh => {
                let scope = ctx.link().clone();
                spawn_local(async move {
                    scope.send_message(
                        PlanListEntry::fetch(get_credentials(&scope).as_ref())
                            .await
                            .map_or_else(Msg::Error, Msg::Data),
                    );
                });
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <PageLayout title="Planungen">{self.view_content(ctx)}</PageLayout>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::Refresh);
        }
    }
}

impl ListOfPlannings {
    fn view_content(&self, ctx: &Context<Self>) -> Html {
        if let Some(error) = &self.error {
            error.into_prop_value()
        } else if let Some(data) = &self.data {
            let entries = ListModel::new(
                MemoizedTableModel::new(data.clone()),
                self.table_state.clone(),
            );
            let header = html_nested! {
                <TableHeader<Columns>>
                    <TableColumn<Columns> label="Name" index={Columns::Name}/>
                    <TableColumn<Columns> label="Art" index={Columns::Kind}/>
                </TableHeader<Columns>>
            };

            let create_button = get_backdrop(ctx.link())
                .filter(|_| get_role(ctx.link()) >= Role::Planner)
                .map(|bd| {
                    let scope=ctx.link().clone();
                    let onclick = Callback::from(move |_|{
                        let scope=scope.clone();
                        let onclose = {
                            let bd = bd.clone();
                            let scope=scope.clone();
                            Callback::from(move |_| {
                                bd.close();
                                scope.send_message(Msg::Refresh);
                            })
                        };

                        let project_name=Rc::new(RefCell::new(String::default()));

                        let onchange={
                            let project_name=project_name.clone();
                            Callback::from(move |value|{
                            *project_name.borrow_mut()=value;
                        })};
                        let onclick= {
                            let bd = bd.clone();
                            let scope=scope.clone();
                            Callback::from(move |_| {
                                let name = project_name.borrow();
                                if !name.is_empty() {
                                    let credentials = get_credentials(&scope);
                                    let name = name.clone();
                                    let bd=bd.clone();
                                    let scope=scope.clone();
                                    spawn_local(async move{
                                        if PlanListEntry::create(credentials.as_ref(), name).await.is_ok() {
                                            bd.close();
                                            scope.send_message(Msg::Refresh);
                                        }
                                    });
                                }
                            })
                        };
                        bd.open(Backdrop::new(html! {
                            <Bullseye>
                                <Modal title="Neue Planung" onclose={onclose}>
                                    <Form>
                                        <FormGroup
                                            label="Name"
                                            required=true
                                            label_icon={LabelIcon::Help(html_nested!(<PopoverBody>{ "Name des Vorhabens" } </PopoverBody>))}>
                                            <TextInput placeholder="Vorhaben" required=true {onchange}/>
                                        </FormGroup>
                                        <ActionGroup>
                                            <Button label="Planung eröffnen" variant={ButtonVariant::Primary} {onclick} />
                                        </ActionGroup>
                                    </Form>
                                </Modal>
                            </Bullseye>
                    }))});
                    html!(<Button label="Neue Planung erstellen" variant={ButtonVariant::Primary} {onclick}/>)
                });
            html! {
                <>
                <Table<Columns, ListModel<Columns, MemoizedTableModel<PlanListEntry>>>
                    mode={TableMode::Compact}
                    grid={TableGridMode::Medium}
                    {header}
                    {entries}
                />
                {create_button}
                </>
            }
        } else {
            html!(<Spinner/>)
        }
    }
}

impl TableEntryRenderer<Columns> for PlanListEntry {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        match &context.column {
            Columns::Name => Cell::new(
                html!(<Link<AppRoute> to={AppRoute::Plan {plan_id: self.id,view: PlanView::ListOfCabinets}}>{self.name.as_str()}</Link<AppRoute>>),
            ),
            Columns::Kind => Cell::new(
                if self.is_baseline {
                    "Ist-Zustand"
                } else {
                    "Planung"
                }
                .into_prop_value(),
            ),
        }
    }
}
