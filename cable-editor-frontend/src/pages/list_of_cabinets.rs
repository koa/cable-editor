use crate::components::table::ListModel;
use crate::error::FrontendError;
use crate::graphql::authenticated::list_cables::CableListEntry;
use crate::graphql::authenticated::list_schacht::{SchachtListEntry, fetch_schacht_list};
use crate::util::get_credentials;
use log::info;
use patternfly_yew::prelude::{
    Cell, CellContext, ExpansionState, MemoizedTableModel, Span, Spinner, Table, TableColumn,
    TableEntryRenderer, TableGridMode, TableHeader, TableHeaderSortBy, TableMode,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::rc::Rc;
use yew::html::{IntoPropValue, Scope};
use yew::platform::spawn_local;
use yew::{Callback, Component, Context, Html, Properties, html, html_nested};

pub struct ListOfCabinets {
    data: Option<Rc<Vec<SchachtListEntry>>>,
    error: Option<FrontendError>,
    sort: Option<TableHeaderSortBy<Columns>>,
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<Columns>>>>,
}
pub enum Msg {
    Data(Box<[SchachtListEntry]>),
    Error(FrontendError),
    OnSort(TableHeaderSortBy<Columns>),
    SetExpandState {
        row: usize,
        state: ExpansionState<Columns>,
    },
}

#[derive(Clone, PartialEq, Properties)]
pub struct ListOfCabinetProps {
    #[prop_or_default]
    pub plan_id: i32,
}

impl Component for ListOfCabinets {
    type Message = Msg;
    type Properties = ListOfCabinetProps;

    fn create(ctx: &Context<Self>) -> Self {
        ListOfCabinets {
            data: None,
            error: None,
            sort: None,
            table_state: Rc::new(RefCell::new(Default::default())),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Data(data) => {
                self.error = None;
                self.data = Some(Rc::new(data.into_vec()));
                true
            }
            Msg::Error(error) => {
                self.error = Some(error);
                true
            }
            Msg::OnSort(sort) => {
                self.sort = Some(sort);
                true
            }
            Msg::SetExpandState { row, state } => {
                let mut states = self.table_state.as_ref().borrow_mut();
                match states.entry(row) {
                    Entry::Occupied(mut e) => {
                        if e.get() == &state {
                            e.remove();
                        } else {
                            e.insert(state);
                        }
                    }
                    Entry::Vacant(e) => {
                        e.insert(state);
                    }
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if let Some(error) = &self.error {
            error.into_prop_value()
        } else if let Some(data) = &self.data {
            let onsort = ctx.link().callback(Msg::OnSort);
            let entries = ListModel::new(
                MemoizedTableModel::new(data.clone()),
                self.table_state.clone(),
            );

            let header = html_nested! {
                <TableHeader<Columns>>
                    <TableColumn<Columns> label="Name" index={Columns::Name} onsort={onsort.clone()} sortby={(self.sort.clone())}/>
                    <TableColumn<Columns> label="Panels" expandable=true index={Columns::Cabinets} onsort={onsort.clone()} sortby={(self.sort.clone())}/>
                </TableHeader<Columns>>
            };
            let onexpand = ctx
                .link()
                .callback(|(row, state)| Msg::SetExpandState { row, state });
            html! {
                <Table<Columns, ListModel<Columns, MemoizedTableModel<SchachtListEntry>>>
                    mode={TableMode::Expandable}
                    grid={TableGridMode::Medium}
                    caption="Schächte"
                    {header}
                    {entries}
                    {onexpand}
                />
            }
        } else {
            html!(<Spinner/>)
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            fetch_data(ctx.link().clone());
        }
    }
}

fn fetch_data(scope: Scope<ListOfCabinets>) {
    let credentials = get_credentials(&scope);
    spawn_local(async move {
        scope.send_message(match fetch_schacht_list(credentials.as_ref()).await {
            Ok(data) => Msg::Data(data),
            Err(error) => Msg::Error(error),
        });
    })
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Columns {
    Name,
    Cabinets,
}
impl TableEntryRenderer<Columns> for SchachtListEntry {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        match context.column {
            Columns::Name => Cell::new(self.name.as_str().into_prop_value()),
            Columns::Cabinets => Cell::new(self.root_panels.len().into_prop_value()),
        }
    }

    fn render_column_details(&self, column: &Columns) -> Vec<Span> {
        match column {
            Columns::Name => {
                vec![Span::max(html!("Can't expand"))]
            }
            Columns::Cabinets => {
                vec![Span::max(html!("Can expand"))]
            }
        }
    }
}
