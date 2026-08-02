use crate::components::table::ListModel;
use crate::error::FrontendError;
use crate::graphql::authenticated::list_plans::{PlanListEntry, PlanStatus};
use crate::pages::router::{AppRoute, PlanView};
use crate::util::get_credentials;
use log::info;
use patternfly_yew::prelude::{
    Action, ActionGroup, Backdrop, Backdropper, Bullseye, Button, ButtonVariant, Cell, CellContext,
    ExpansionState, Form, FormGroup, LabelIcon, MemoizedTableModel, MenuAction, MenuChildVariant,
    Modal, PopoverBody, Spinner, Table, TableColumn, TableEntryRenderer, TableGridMode,
    TableHeader, TableMode, TextInput,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use yew::html::IntoPropValue;
use yew::platform::spawn_local;
use yew::{Callback, Component, Context, Html, Properties, html, html_nested};
use yew_nested_router::components::Link;
use yew_nested_router::prelude::RouterContext;

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
    Status,
}

impl Component for ListOfPlannings {
    type Message = Msg;
    type Properties = ListOfPlanningProps;

    fn create(ctx: &Context<Self>) -> Self {
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
                    <TableColumn<Columns> label="Status" index={Columns::Status}/>
                </TableHeader<Columns>>
            };

            let create_button = ctx.link()
                .context::<Backdropper>(Callback::noop())
                .map(|(bd, _)| {
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
                                if name.len() > 3 {
                                    let credentials = get_credentials(&scope);
                                    let name = name.clone();
                                    let bd=bd.clone();
                                    let scope=scope.clone();
                                    spawn_local(async move{
                                        if let Ok(_)=PlanListEntry::create(credentials.as_ref(), name).await{
                                            bd.close();
                                            scope.send_message(Msg::Refresh);
                                        }
                                    });
                                }
                            })
                        };
                        bd.open(Backdrop::new(html! {
                            <Bullseye>
                                <Modal title="Schacht setzen" onclose={onclose}>
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
                    caption="Planungen"
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

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::Refresh);
        }
    }
}

impl TableEntryRenderer<Columns> for PlanListEntry {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        match &context.column {
            Columns::Name => Cell::new(
                html!(<Link<AppRoute> to={AppRoute::Plan {plan_id: self.id,view: PlanView::Edit}}>{self.name.as_str()}</Link<AppRoute>>),
            ),
            Columns::Status => Cell::new(self.status.name().into_prop_value()),
        }
    }
    fn actions(&self) -> Vec<MenuChildVariant> {
        match self.status {
            PlanStatus::IMPLEMENTED => vec![],
            PlanStatus::OPEN => vec![html_nested!(<MenuAction>{"Akzeptieren"}</MenuAction>).into(),html_nested!(<MenuAction>{"Abbrechen"}</MenuAction>).into()],
            PlanStatus::REJECTED => vec![],
        }
    }
}
