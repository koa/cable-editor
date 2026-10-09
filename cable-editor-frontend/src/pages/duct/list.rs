use crate::components::load::Load;
use crate::{
    components::{
        links::{DuctLink, OwnerLink, SchachtLink},
        page_layout::PageLayout,
        plan_link::PlanLink,
        table::ListTable,
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
    Cell, CellContext, Order, TableColumn, TableEntryRenderer, TableHeader, TableHeaderSortBy,
};
use std::{cmp::Ordering, rc::Rc};
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
    Owner,
    /// Delivered to the Leitungskataster
    Lk,
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
            Columns::Owner => Cell::new(html!(<OwnerLink text={self.owner.name.clone()}/>)),
            Columns::Lk => Cell::new(if self.leitungskataster {
                html!("ja")
            } else {
                html!("–")
            }),
        }
    }
}

/// All ducts, each linked to its page.
pub struct ListOfDucts {
    /// `None` while loading
    ducts: Load<Rc<Vec<DuctListEntry>>>,
    sort: Option<TableHeaderSortBy<Columns>>,
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
            ducts: Load::Pending,
            sort: None,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Loaded(ducts) => {
                self.ducts = Load::from(ducts.map(|ducts| Rc::new(ducts.into_vec())));
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
        let content = self.ducts.view(|ducts| self.view_ducts(ctx, ducts));
        html!(<PageLayout title="Trassen">{content}</PageLayout>)
    }
}

impl ListOfDucts {
    fn sort_ducts(&mut self) {
        let (Some(sort), Load::Loaded(ducts)) = (&self.sort, &mut self.ducts) else {
            return;
        };
        Rc::make_mut(ducts).sort_by(|a, b| {
            let ordering = match sort.index {
                Columns::Name => a.title().cmp(&b.title()),
                Columns::SchachtA => a.schacht_a.name.cmp(&b.schacht_a.name),
                Columns::SchachtZ => a.schacht_z.name.cmp(&b.schacht_z.name),
                Columns::Length => a.length.partial_cmp(&b.length).unwrap_or(Ordering::Equal),
                Columns::Cables => a.cables.len().cmp(&b.cables.len()),
                Columns::Owner => a.owner.name.cmp(&b.owner.name),
                Columns::Lk => a.leitungskataster.cmp(&b.leitungskataster),
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
                <TableColumn<Columns> label="Kabel" index={Columns::Cables} onsort={onsort.clone()} {sortby}/>
                <TableColumn<Columns> label="Eigentümer" index={Columns::Owner} onsort={onsort.clone()} {sortby}/>
                <TableColumn<Columns> label="LK" index={Columns::Lk} {onsort} {sortby}/>
            </TableHeader<Columns>>
        };
        html! {
            <>
                <ListTable<Columns, DuctListEntry>
                    {header}
                    rows={ducts.clone()}
                    empty="Keine Trassen."
                />
                if get_role(ctx.link()) >= Role::Planner {
                    // Not stretched to the width of the page's content
                    <div>
                        <PlanLink to={PlanView::NewDuct { id: Uuid::new_v4() }} class="pf-v6-c-button pf-m-primary">
                            {"Neue Trasse"}
                        </PlanLink>
                    </div>
                }
            </>
        }
    }
}
