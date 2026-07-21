use crate::{
    graphql::authenticated::list_cables::{CableListEntry, fetch_cables_list},
    pages::router::{AppRoute, CableView},
};
use patternfly_yew::prelude::{
    Cell, CellContext, MemoizedTableModel, Order, Spinner, Table, TableColumn, TableEntryRenderer,
    TableGridMode, TableHeader, TableHeaderSortBy, TableMode, UseTableData, use_table_data,
};
use std::cmp::Ordering;
use yew::{
    Callback, Html, HtmlResult, Suspense, function_component, html, html::IntoPropValue,
    html_nested, suspense::use_future_with, use_memo, use_state,
};
use yew_nested_router::components::Link;
use yew_oauth2::hook::use_auth_state;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Columns {
    Name,
    Fibers,
    Length,
}
impl TableEntryRenderer<Columns> for CableListEntry {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        match &context.column {
            Columns::Name => {
                let to = AppRoute::Cable {
                    id: self.id,
                    view: CableView::Edit,
                };
                Cell::new(html! {<Link<AppRoute> {to}>{self.name.as_str()}</Link<AppRoute>>})
            }
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
        }
    }
}

#[function_component]
fn CablesTable() -> HtmlResult {
    let auth_state = use_auth_state();
    let sort_state = use_state(|| None::<TableHeaderSortBy<Columns>>);
    let cable_data = use_future_with(auth_state, |state| async move {
        fetch_cables_list((*state).as_ref()).await
    })?;

    let (mut cables, error) = match &*cable_data {
        Ok(data) => (data.clone().into_vec(), None),
        Err(e) => (vec![], Some(IntoPropValue::<Html>::into_prop_value(e))),
    };

    if let Some(sort) = &*sort_state {
        cables.sort_by(|a, b| {
            let ordering = match sort.index {
                Columns::Name => a.name.cmp(&b.name),
                Columns::Fibers => {
                    (a.fiber_count * a.bundle_count).cmp(&(b.fiber_count * b.bundle_count))
                }
                Columns::Length => a.length.partial_cmp(&b.length).unwrap_or(Ordering::Equal),
            };

            if sort.order == Order::Descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
    }

    let memoized_entries = use_memo((cables, *sort_state), |(c, _)| c.clone());

    let (entries, _) = use_table_data(MemoizedTableModel::new(memoized_entries));

    if let Some(err) = error {
        return Ok(html! { <div class="error">{ "Fehler beim Laden: " }{ err }</div> });
    }

    let onsort = {
        let sort_state = sort_state.clone();
        Callback::from(move |option: TableHeaderSortBy<Columns>| {
            sort_state.set(Some(option));
        })
    };
    let header = html_nested! {
        <TableHeader<Columns>>
            <TableColumn<Columns> label="Name" index={Columns::Name} onsort={onsort.clone()} sortby={(*sort_state)}/>
            <TableColumn<Columns> label="Fasern" index={Columns::Fibers} onsort={onsort.clone()} sortby={(*sort_state)}/>
            <TableColumn<Columns> label="Streckenlänge" index={Columns::Length} onsort={onsort.clone()} sortby={(*sort_state)}/>
        </TableHeader<Columns>>
    };

    Ok(html! {
        <Table<Columns, UseTableData<Columns, MemoizedTableModel<CableListEntry>>>
            mode={TableMode::Compact}
            grid={TableGridMode::Medium}
            caption="Kabel"
            {header}
            {entries}
        />
    })
}
#[function_component]
pub fn ListOfCables() -> Html {
    let fallback = html!(<Spinner/>);
    html! {
        <Suspense {fallback}>
            <CablesTable />
        </Suspense>
    }
}
