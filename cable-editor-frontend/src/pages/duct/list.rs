use crate::{
    components::{
        links::{DuctLink, SchachtLink},
        page_layout::PageLayout,
        plan_link::PlanLink,
        table::ListModel,
    },
    error::FrontendError,
    graphql::authenticated::{
        current_user::Role,
        list_ducts::{DuctListEntry, fetch_duct_list},
    },
    pages::router::PlanView,
    util::{get_credentials, get_role},
};
use patternfly_yew::prelude::{
    Cell, CellContext, ExpansionState, MemoizedTableModel, Order, Spinner, Table, TableColumn,
    TableEntryRenderer, TableGridMode, TableHeader, TableHeaderSortBy, TableMode,
};
use std::{cell::RefCell, cmp::Ordering, collections::HashMap, rc::Rc};
use uuid::Uuid;
use yew::{
    Component, Context, Html, html, html::IntoPropValue, html_nested, platform::spawn_local,
};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Columns {
    Name,
    SchachtA,
    SchachtZ,
    Length,
    Cables,
}

impl TableEntryRenderer<Columns> for DuctListEntry {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        match context.column {
            Columns::Name => Cell::new(html!(<DuctLink id={self.id} text={self.title()}/>)),
            Columns::SchachtA => Cell::new(
                html!(<SchachtLink id={self.schacht_a.id} text={self.schacht_a.name.clone()}/>),
            ),
            Columns::SchachtZ => Cell::new(
                html!(<SchachtLink id={self.schacht_z.id} text={self.schacht_z.name.clone()}/>),
            ),
            Columns::Length => {
                Cell::new(self.length.map(|l| format!("{l:.1} m")).into_prop_value())
            }
            Columns::Cables => Cell::new(self.cables.len().into_prop_value()),
        }
    }
}

/// All ducts, each linked to its page.
pub struct ListOfDucts {
    /// `None` while loading
    ducts: Option<Result<Rc<Vec<DuctListEntry>>, FrontendError>>,
    sort: Option<TableHeaderSortBy<Columns>>,
    /// Required by `ListModel`; the rows don't expand
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<Columns>>>>,
}

pub enum Msg {
    Loaded(Result<Box<[DuctListEntry]>, FrontendError>),
    Sort(TableHeaderSortBy<Columns>),
}

impl Component for ListOfDucts {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        spawn_local(async move {
            scope.send_message(Msg::Loaded(fetch_duct_list(credentials.as_ref()).await));
        });
        Self {
            ducts: None,
            sort: None,
            table_state: Rc::default(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Loaded(ducts) => {
                self.ducts = Some(ducts.map(|ducts| Rc::new(ducts.into_vec())));
                self.sort_ducts();
            }
            Msg::Sort(sort) => {
                self.sort = Some(sort);
                self.sort_ducts();
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let content = match &self.ducts {
            None => html!(<Spinner/>),
            Some(Err(error)) => error.into_prop_value(),
            Some(Ok(ducts)) => self.view_ducts(ctx, ducts),
        };
        html!(<PageLayout title="Trassen">{content}</PageLayout>)
    }
}

impl ListOfDucts {
    fn sort_ducts(&mut self) {
        let (Some(sort), Some(Ok(ducts))) = (&self.sort, &mut self.ducts) else {
            return;
        };
        Rc::make_mut(ducts).sort_by(|a, b| {
            let ordering = match sort.index {
                Columns::Name => a.title().cmp(&b.title()),
                Columns::SchachtA => a.schacht_a.name.cmp(&b.schacht_a.name),
                Columns::SchachtZ => a.schacht_z.name.cmp(&b.schacht_z.name),
                Columns::Length => a.length.partial_cmp(&b.length).unwrap_or(Ordering::Equal),
                Columns::Cables => a.cables.len().cmp(&b.cables.len()),
            };
            if sort.order == Order::Descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
    }

    fn view_ducts(&self, ctx: &Context<Self>, ducts: &Rc<Vec<DuctListEntry>>) -> Html {
        let onsort = ctx.link().callback(Msg::Sort);
        let sortby = self.sort;
        let header = html_nested! {
            <TableHeader<Columns>>
                <TableColumn<Columns> label="Name" index={Columns::Name} onsort={onsort.clone()} {sortby}/>
                <TableColumn<Columns> label="Von" index={Columns::SchachtA} onsort={onsort.clone()} {sortby}/>
                <TableColumn<Columns> label="Bis" index={Columns::SchachtZ} onsort={onsort.clone()} {sortby}/>
                <TableColumn<Columns> label="Länge" index={Columns::Length} onsort={onsort.clone()} {sortby}/>
                <TableColumn<Columns> label="Kabel" index={Columns::Cables} {onsort} {sortby}/>
            </TableHeader<Columns>>
        };
        let entries = ListModel::new(
            MemoizedTableModel::new(ducts.clone()),
            self.table_state.clone(),
        );
        html! {
            <>
                <Table<Columns, ListModel<Columns, MemoizedTableModel<DuctListEntry>>>
                    mode={TableMode::Compact}
                    grid={TableGridMode::Medium}
                    {header}
                    {entries}
                />
                if get_role(ctx.link()) >= Role::Planner {
                    <PlanLink to={PlanView::NewDuct { id: Uuid::new_v4() }} class="pf-v6-c-button pf-m-primary">
                        {"Neue Trasse"}
                    </PlanLink>
                }
            </>
        }
    }
}
