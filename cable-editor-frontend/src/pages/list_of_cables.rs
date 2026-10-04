use crate::components::load::Load;
use crate::{
    components::{
        links::{CableLink, SchachtLink},
        page_layout::PageLayout,
        plan_link::PlanLink,
        table::ListModel,
    },
    error::FrontendError,
    graphql::authenticated::{
        current_user::Role,
        list_cables::{CableListEntry, fetch_cables_list},
    },
    pages::router::PlanView,
    util::{get_credentials, get_role},
};
use patternfly_yew::prelude::{
    Cell, CellContext, ExpansionState, MemoizedTableModel, Order, Table, TableColumn,
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
    cables: Load<Rc<Vec<CableListEntry>>>,
    sort: Option<TableHeaderSortBy<Columns>>,
    /// Required by `ListModel`; the rows don't expand
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<Columns>>>>,
}

pub enum Msg {
    Fetch,
    Loaded(Result<Box<[CableListEntry]>, FrontendError>),
    Sort(TableHeaderSortBy<Columns>),
}

impl Component for ListOfCables {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Fetch);
        Self {
            cables: Load::Pending,
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
                self.cables = Load::from(cables.map(|cables| Rc::new(cables.into_vec())));
                self.sort_cables();
                true
            }
            Msg::Sort(sort) => {
                self.sort = Some(sort);
                self.sort_cables();
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let content = self.cables.view(|cables| self.view_cables(ctx, cables));
        html!(<PageLayout title="Kabel">{content}</PageLayout>)
    }
}

impl ListOfCables {
    fn sort_cables(&mut self) {
        let (Some(sort), Load::Loaded(cables)) = (&self.sort, &mut self.cables) else {
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
                if get_role(ctx.link()) >= Role::Planner {
                    // Not stretched to the width of the page's content
                    <div>
                        <PlanLink to={PlanView::NewCable { id: Uuid::new_v4() }} class="pf-v6-c-button pf-m-primary">
                            {"Neues Kabel"}
                        </PlanLink>
                    </div>
                }
            </>
        }
    }
}
