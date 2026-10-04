use crate::components::load::Load;
use crate::components::page_layout::PageLayout;
use crate::{
    components::{plan_link::PlanNameLink, table::ListModel},
    error::FrontendError,
    graphql::authenticated::{current_user::Role, list_plans::PlanListEntry},
    util::{get_backdrop, get_credentials, get_role, toast_error, toast_success},
};
use patternfly_yew::prelude::{
    ActionGroup, Backdrop, Bullseye, Button, ButtonType, ButtonVariant, Cell, CellContext, Color,
    ExpansionState, Form, FormGroup, Label, LabelIcon, MemoizedTableModel, Modal, PopoverBody,
    Table, TableColumn, TableEntryRenderer, TableGridMode, TableHeader, TableMode, TextInput,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use web_sys::SubmitEvent;
use yew::{
    Callback, Component, Context, Html, Properties, html, html_nested, platform::spawn_local,
};

pub struct ListOfPlannings {
    data: Load<Rc<Vec<PlanListEntry>>>,
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
            data: Load::Pending,
            table_state: Rc::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Data(data) => {
                self.data = Load::Loaded(Rc::new(data.into_vec()));
                true
            }
            Msg::Error(e) => {
                self.data = Load::Failed(e);
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
        self.data.view(|data| {
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
                    let scope = ctx.link().clone();
                    let onclick = Callback::from(move |_| {
                        let oncancel = {
                            let bd = bd.clone();
                            Callback::from(move |()| bd.close())
                        };
                        let oncreated = {
                            let bd = bd.clone();
                            let scope = scope.clone();
                            Callback::from(move |()| {
                                toast_success(&scope, "Planung angelegt");
                                bd.close();
                                scope.send_message(Msg::Refresh);
                            })
                        };
                        // The dialog is rendered by the backdrop viewer, whose context has no toaster
                        let onfailed = {
                            let scope = scope.clone();
                            Callback::from(move |error: FrontendError| {
                                toast_error(&scope, "Planung konnte nicht angelegt werden", error);
                            })
                        };
                        bd.open(Backdrop::new(html! {
                            <NewPlanDialog {oncancel} {oncreated} {onfailed}/>
                        }));
                    });
                    html!(<Button label="Neue Planung" variant={ButtonVariant::Primary} {onclick}/>)
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
        })
    }
}

impl TableEntryRenderer<Columns> for PlanListEntry {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        match &context.column {
            Columns::Name => {
                Cell::new(html!(<PlanNameLink id={self.id} text={self.name.clone()}/>))
            }
            Columns::Kind => {
                let kind = if self.is_baseline {
                    "Ist-Zustand"
                } else {
                    "Planung"
                };
                Cell::new(html! {
                    <>
                        {kind}
                        if self.netbox_active {
                            {" "}<Label label="In Netbox aktiv" compact=true color={Color::Blue}/>
                        }
                    </>
                })
            }
        }
    }
}

#[derive(Properties, PartialEq)]
struct NewPlanDialogProps {
    oncancel: Callback<()>,
    oncreated: Callback<()>,
    onfailed: Callback<FrontendError>,
}

/// Asks for the name of a new plan and creates it; stays open with the name if that fails.
struct NewPlanDialog {
    name: String,
    saving: bool,
}

enum NewPlanMsg {
    Name(String),
    Create,
    Created,
    Failed(FrontendError),
}

impl NewPlanDialog {
    fn can_create(&self) -> bool {
        !self.saving && !self.name.trim().is_empty()
    }
}

impl Component for NewPlanDialog {
    type Message = NewPlanMsg;
    type Properties = NewPlanDialogProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            name: String::new(),
            saving: false,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            NewPlanMsg::Name(name) => self.name = name,
            NewPlanMsg::Create => {
                if self.can_create() {
                    self.saving = true;
                    let scope = ctx.link().clone();
                    let credentials = get_credentials(&scope);
                    let name = self.name.trim().to_string();
                    spawn_local(async move {
                        scope.send_message(
                            PlanListEntry::create(credentials.as_ref(), name)
                                .await
                                .map_or_else(NewPlanMsg::Failed, |()| NewPlanMsg::Created),
                        );
                    });
                }
            }
            NewPlanMsg::Created => {
                ctx.props().oncreated.emit(());
                return false;
            }
            NewPlanMsg::Failed(error) => {
                self.saving = false;
                ctx.props().onfailed.emit(error);
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let onsubmit = link.callback(|event: SubmitEvent| {
            event.prevent_default();
            NewPlanMsg::Create
        });
        let oncancel = ctx.props().oncancel.reform(|_| ());
        html! {
            <Bullseye>
                <Modal title="Neue Planung" onclose={ctx.props().oncancel.reform(|_| ())}>
                    <Form {onsubmit}>
                        <FormGroup
                            label="Name"
                            required=true
                            label_icon={LabelIcon::Help(html_nested!(<PopoverBody>{ "Name des Vorhabens" } </PopoverBody>))}>
                            <TextInput
                                placeholder="Vorhaben"
                                required=true
                                autofocus=true
                                value={self.name.clone()}
                                onchange={link.callback(NewPlanMsg::Name)}
                            />
                        </FormGroup>
                        <ActionGroup>
                            <Button label="Anlegen" variant={ButtonVariant::Primary} r#type={ButtonType::Submit} disabled={!self.can_create()}/>
                            <Button label="Abbrechen" variant={ButtonVariant::Link} onclick={oncancel}/>
                        </ActionGroup>
                    </Form>
                </Modal>
            </Bullseye>
        }
    }
}
