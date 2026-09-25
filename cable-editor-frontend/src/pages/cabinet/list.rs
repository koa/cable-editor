use crate::components::page_layout::PageLayout;
use crate::{
    components::{links::SchachtLink, table::ListModel},
    error::FrontendError,
    graphql::authenticated::list_schacht::{SchachtListEntry, fetch_schacht_list},
    util::get_credentials,
};
use patternfly_yew::prelude::{
    Cell, CellContext, ExpansionState, MemoizedTableModel, Spinner, Table, TableColumn,
    TableEntryRenderer, TableGridMode, TableHeader, TableHeaderSortBy, TableMode,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use yew::{
    Component, Context, Html, Properties, html,
    html::{IntoPropValue, Scope},
    html_nested,
    platform::spawn_local,
};

pub struct ListOfCabinets {
    data: Option<Rc<Vec<SchachtListEntry>>>,
    error: Option<FrontendError>,
    sort: Option<TableHeaderSortBy<Columns>>,
    /// Required by `ListModel`; the rows don't expand
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<Columns>>>>,
}

pub enum Msg {
    Data(Box<[SchachtListEntry]>),
    Error(FrontendError),
    OnSort(TableHeaderSortBy<Columns>),
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
            table_state: Rc::default(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
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
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <PageLayout title="Schächte">{self.view_content(ctx)}</PageLayout>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            fetch_data(ctx.link().clone());
        }
    }
}

impl ListOfCabinets {
    fn view_content(&self, ctx: &Context<Self>) -> Html {
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
                    <TableColumn<Columns> label="Name" index={Columns::Name} onsort={onsort.clone()} sortby={self.sort}/>
                    <TableColumn<Columns> label="Panels" index={Columns::Cabinets} onsort={onsort.clone()} sortby={self.sort}/>
                </TableHeader<Columns>>
            };
            html! {
                <Table<Columns, ListModel<Columns, MemoizedTableModel<SchachtListEntry>>>
                    mode={TableMode::Compact}
                    grid={TableGridMode::Medium}
                    {header}
                    {entries}
                />
            }
        } else {
            html!(<Spinner/>)
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
            Columns::Name => Cell::new(html!(<SchachtLink id={self.id} text={self.name.clone()}/>)),
            Columns::Cabinets => Cell::new(self.root_panels.len().into_prop_value()),
        }
    }
}
