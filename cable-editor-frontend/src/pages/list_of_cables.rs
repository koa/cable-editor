use crate::{
    components::{
        links::{CableLink, SchachtLink},
        page_layout::PageLayout,
        table::ListModel,
    },
    error::FrontendError,
    graphql::authenticated::list_cables::{CableListEntry, create_cable, fetch_cables_list},
    util::{get_backdrop, get_credentials},
};
use patternfly_yew::prelude::{
    ActionGroup, Backdrop, Bullseye, Button, ButtonVariant, Cell, CellContext, ExpansionState,
    Form, FormGroup, MemoizedTableModel, Modal, ModalVariant, Order, Spinner, Table, TableColumn,
    TableEntryRenderer, TableGridMode, TableHeader, TableHeaderSortBy, TableMode, TextInput,
};
use std::{cell::RefCell, cmp::Ordering, collections::HashMap, rc::Rc};
use web_sys::SubmitEvent;
use yew::{
    Callback, Component, Context, Html, Properties, html, html::IntoPropValue, html_nested,
    platform::spawn_local,
};
use yew_oauth2::prelude::OAuth2Context;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Columns {
    Name,
    Fibers,
    Length,
    SchachtA,
    SchachtZ,
}
impl TableEntryRenderer<Columns> for CableListEntry {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        match &context.column {
            Columns::Name => Cell::new(html!(<CableLink id={self.id} text={self.name.clone()}/>)),
            Columns::Fibers => Cell::new(
                format!(
                    "{} ({}x{})",
                    self.bundle_count * self.fiber_count,
                    self.bundle_count,
                    self.fiber_count
                )
                .into_prop_value(),
            ),
            Columns::Length => {
                Cell::new(self.length.map(|l| format!("{l:.1} m")).into_prop_value())
            }
            Columns::SchachtA => self
                .path
                .as_ref()
                .map(|sch| {
                    let schacht = &sch.near_schacht;
                    Cell::new(html!(<SchachtLink id={schacht.id} text={schacht.name.clone()}/>))
                })
                .unwrap_or_default(),
            Columns::SchachtZ => self
                .path
                .as_ref()
                .map(|sch| {
                    let schacht = &sch.far_schacht;
                    Cell::new(html!(<SchachtLink id={schacht.id} text={schacht.name.clone()}/>))
                })
                .unwrap_or_default(),
        }
    }
}

pub struct ListOfCables {
    /// `None` while loading
    cables: Option<Result<Rc<Vec<CableListEntry>>, FrontendError>>,
    sort: Option<TableHeaderSortBy<Columns>>,
    /// Required by `ListModel`; the rows don't expand
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<Columns>>>>,
}

pub enum Msg {
    Fetch,
    Loaded(Result<Box<[CableListEntry]>, FrontendError>),
    Sort(TableHeaderSortBy<Columns>),
    AddCable,
}

impl Component for ListOfCables {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Fetch);
        Self {
            cables: None,
            sort: None,
            table_state: Rc::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Fetch => {
                let scope = ctx.link().clone();
                let credentials = get_credentials(&scope);
                spawn_local(async move {
                    let cables = fetch_cables_list(credentials.as_ref()).await;
                    scope.send_message(Msg::Loaded(cables));
                });
                false
            }
            Msg::Loaded(cables) => {
                self.cables = Some(cables.map(|cables| Rc::new(cables.into_vec())));
                self.sort_cables();
                true
            }
            Msg::Sort(sort) => {
                self.sort = Some(sort);
                self.sort_cables();
                true
            }
            Msg::AddCable => {
                if let Some(backdrop) = get_backdrop(ctx.link()) {
                    // Reload afterwards, so a new cable shows up
                    let on_close = {
                        let backdrop = backdrop.clone();
                        ctx.link().callback(move |()| {
                            backdrop.close();
                            Msg::Fetch
                        })
                    };
                    backdrop.open(Backdrop::new(html! {
                        <Bullseye>
                            <Modal title="Neues Kabel" variant={ModalVariant::Small}>
                                <AddCable {on_close}/>
                            </Modal>
                        </Bullseye>
                    }));
                }
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let content = match &self.cables {
            None => html!(<Spinner/>),
            Some(Err(error)) => {
                let error: Html = error.into_prop_value();
                html!(<div class="error">{"Fehler beim Laden: "}{error}</div>)
            }
            Some(Ok(cables)) => self.view_cables(ctx, cables),
        };
        html!(<PageLayout title="Kabel">{content}</PageLayout>)
    }
}

impl ListOfCables {
    fn sort_cables(&mut self) {
        let (Some(sort), Some(Ok(cables))) = (&self.sort, &mut self.cables) else {
            return;
        };
        Rc::make_mut(cables).sort_by(|a, b| {
            let ordering = match sort.index {
                Columns::Name => a.name.cmp(&b.name),
                Columns::Fibers => {
                    (a.fiber_count * a.bundle_count).cmp(&(b.fiber_count * b.bundle_count))
                }
                Columns::Length => a.length.partial_cmp(&b.length).unwrap_or(Ordering::Equal),
                Columns::SchachtA => a
                    .path
                    .as_ref()
                    .map(|sch| sch.near_schacht.id)
                    .cmp(&b.path.as_ref().map(|sch| sch.near_schacht.id)),
                Columns::SchachtZ => a
                    .path
                    .as_ref()
                    .map(|sch| sch.far_schacht.id)
                    .cmp(&b.path.as_ref().map(|sch| sch.far_schacht.id)),
            };
            if sort.order == Order::Descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
    }

    fn view_cables(&self, ctx: &Context<Self>, cables: &Rc<Vec<CableListEntry>>) -> Html {
        let onsort = ctx.link().callback(Msg::Sort);
        let sortby = self.sort;
        let header = html_nested! {
            <TableHeader<Columns>>
                <TableColumn<Columns> label="Name" index={Columns::Name} onsort={onsort.clone()} {sortby}/>
                <TableColumn<Columns> label="Fasern" index={Columns::Fibers} onsort={onsort.clone()} {sortby}/>
                <TableColumn<Columns> label="Streckenlänge" index={Columns::Length} onsort={onsort.clone()} {sortby}/>
                <TableColumn<Columns> label="Von" index={Columns::SchachtA} onsort={onsort.clone()} {sortby}/>
                <TableColumn<Columns> label="Bis" index={Columns::SchachtZ} {onsort} {sortby}/>
            </TableHeader<Columns>>
        };
        let entries = ListModel::new(
            MemoizedTableModel::new(cables.clone()),
            self.table_state.clone(),
        );
        html! {
            <>
                <Table<Columns, ListModel<Columns, MemoizedTableModel<CableListEntry>>>
                    mode={TableMode::Compact}
                    grid={TableGridMode::Medium}
                    {header}
                    {entries}
                />
                <Button variant={ButtonVariant::Primary} label="Neues Kabel" onclick={ctx.link().callback(|_| Msg::AddCable)}/>
            </>
        }
    }
}

struct AddCable {
    cable_name: String,
    error: Option<FrontendError>,
}
enum AddCableMsg {
    Save,
    Cancel,
    UpdateText(String),
    Error(FrontendError),
}
#[derive(Properties, PartialEq)]
struct AddCableProps {
    on_close: Callback<()>,
}
impl Component for AddCable {
    type Message = AddCableMsg;
    type Properties = AddCableProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            cable_name: "".to_string(),
            error: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AddCableMsg::Save => {
                let name = self.cable_name.clone();
                let scope = ctx.link().clone();
                let on_close = ctx.props().on_close.clone();
                if let Some((credentials, _)) = scope.context::<OAuth2Context>(Callback::noop()) {
                    spawn_local(async move {
                        match create_cable(Some(&credentials), name).await {
                            Ok(_) => {
                                on_close.emit(());
                            }
                            Err(error) => {
                                scope.send_message(AddCableMsg::Error(error));
                            }
                        }
                    });
                }
                false
            }
            AddCableMsg::Cancel => {
                ctx.props().on_close.emit(());
                true
            }
            AddCableMsg::UpdateText(text) => {
                self.cable_name = text;
                true
            }
            AddCableMsg::Error(error) => {
                self.error = Some(error);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let value = self.cable_name.clone();
        let disabled = value.is_empty();
        html! {
            <Form onsubmit={ctx.link().callback(|event: SubmitEvent|{
                event.prevent_default();
                AddCableMsg::Save
            })}>
                <FormGroup label="name" required=true>
                    <TextInput required=true {value} onchange={ctx.link().callback(|text|{AddCableMsg::UpdateText(text)})}/>
                </FormGroup>
                <ActionGroup>
                    <Button variant={ButtonVariant::Primary} label="Speichern" onclick={ctx.link().callback(|_|{AddCableMsg::Save})} {disabled}/>
                    <Button variant={ButtonVariant::Secondary} label="Abbrechen" onclick={ctx.link().callback(|_|{AddCableMsg::Cancel})}/>
                </ActionGroup>
            </Form>
        }
    }
}
