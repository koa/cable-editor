//! The types of Schächte (see docs/stammdaten.md), each linked to its page.

use crate::components::load::Load;
use crate::{
    components::{page_layout::PageLayout, plan_link::PlanLink, table::ListModel},
    error::FrontendError,
    graphql::authenticated::{
        current_user::Role,
        schacht_types::{SchachtTypEntry, fetch_schacht_typ_list, icon_src},
    },
    pages::router::PlanView,
    util::{get_credentials, get_role},
};
use patternfly_yew::prelude::{
    Cell, CellContext, ExpansionState, MemoizedTableModel, Table, TableColumn, TableEntryRenderer,
    TableGridMode, TableHeader, TableMode,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use uuid::Uuid;
use yew::{
    Component, Context, Html, html, html::IntoPropValue, html_nested, platform::spawn_local,
};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Columns {
    Icon,
    Name,
    Objektart,
    Dimensions,
    Schaechte,
}

impl TableEntryRenderer<Columns> for SchachtTypEntry {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        match context.column {
            Columns::Icon => Cell::new(html! {
                <img class="cabinet-type__icon" src={icon_src(&self.icon)} alt=""/>
            }),
            Columns::Name => Cell::new(html! {
                <PlanLink to={PlanView::CabinetType { id: self.id }}>{self.title()}</PlanLink>
            }),
            Columns::Objektart => Cell::new(self.lkmap_objektart.title().into_prop_value()),
            Columns::Dimensions => Cell::new(match self.dimensions() {
                Some(dimensions) => html!(dimensions),
                None => html!("–"),
            }),
            Columns::Schaechte => Cell::new(self.schacht_count.into_prop_value()),
        }
    }
}

/// All types of Schächte; admins create new ones.
pub struct ListOfCabinetTypes {
    /// `None` while loading
    types: Load<Rc<Vec<SchachtTypEntry>>>,
    /// Required by `ListModel`; the rows don't expand
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<Columns>>>>,
}

pub enum Msg {
    Loaded(Result<Vec<SchachtTypEntry>, FrontendError>),
}

impl Component for ListOfCabinetTypes {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let scope = ctx.link().clone();
        let credentials = get_credentials(&scope);
        spawn_local(async move {
            scope.send_message(Msg::Loaded(
                fetch_schacht_typ_list(credentials.as_ref()).await,
            ));
        });
        Self {
            types: Load::Pending,
            table_state: Rc::default(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Loaded(types) => self.types = Load::from(types.map(Rc::new)),
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let content = self.types.view(|types| {
                let header = html_nested! {
                    <TableHeader<Columns>>
                        <TableColumn<Columns> index={Columns::Icon}/>
                        <TableColumn<Columns> label="Name" index={Columns::Name}/>
                        <TableColumn<Columns> label="Objektart" index={Columns::Objektart}/>
                        <TableColumn<Columns> label="Innenmasse" index={Columns::Dimensions}/>
                        <TableColumn<Columns> label="Schächte" index={Columns::Schaechte}/>
                    </TableHeader<Columns>>
                };
                let entries = ListModel::new(
                    MemoizedTableModel::new(types.clone()),
                    self.table_state.clone(),
                );
                html! {
                    <>
                        <Table<Columns, ListModel<Columns, MemoizedTableModel<SchachtTypEntry>>>
                            mode={TableMode::Compact}
                            grid={TableGridMode::Medium}
                            {header}
                            {entries}
                        />
                        if get_role(ctx.link()) >= Role::Admin {
                            <div>
                                <PlanLink to={PlanView::NewCabinetType { id: Uuid::new_v4() }} class="pf-v6-c-button pf-m-primary">
                                    {"Neuer Schachttyp"}
                                </PlanLink>
                            </div>
                        }
                    </>
                }
        });
        html!(<PageLayout title="Schachttypen">{content}</PageLayout>)
    }
}
