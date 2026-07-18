use crate::graphql::authenticated::list_cables::{CableListEntry, fetch_cables_list};
use patternfly_yew::prelude::{
    Cell, CellContext, MemoizedTableModel, Spinner, Table, TableColumn, TableEntryRenderer,
    TableHeader, UseTableData, use_table_data,
};
use yew::{
    Html, HtmlResult, Suspense, function_component, html, html::IntoPropValue, html_nested,
    suspense::use_future_with, use_memo,
};
use yew_oauth2::hook::use_auth_state;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Columns {
    Name,
    Fibers,
}
impl TableEntryRenderer<Columns> for CableListEntry {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        match &context.column {
            Columns::Name => Cell::new(self.name.as_str().into_prop_value()),
            Columns::Fibers => Cell::new(
                format!(
                    "{} ({}x{})",
                    self.bundle_count * self.fiber_count,
                    self.bundle_count,
                    self.fiber_count
                )
                .into_prop_value(),
            ),
        }
    }
}

#[function_component]
fn CablesTable() -> HtmlResult {
    let auth_state = use_auth_state();
    let cable_data = use_future_with(auth_state, |state| async move {
        fetch_cables_list((*state).as_ref()).await
    })?;

    let (cables, error) = match &*cable_data {
        Ok(data) => (data.clone().into_vec(), None), // Angenommen data ist ein Vec<CableListEntry>
        Err(e) => (vec![], Some(IntoPropValue::<Html>::into_prop_value(e))),
    };

    let memoized_entries = use_memo(cables.clone(), |c| c.clone());

    let (entries, _) = use_table_data(MemoizedTableModel::new(memoized_entries));

    if let Some(err) = error {
        return Ok(html! { <div class="error">{ "Fehler beim Laden: " }{ err }</div> });
    }

    let header = html_nested! {
        <TableHeader<Columns>>
            <TableColumn<Columns> label="Name" index={Columns::Name} />
            <TableColumn<Columns> label="Fasern" index={Columns::Fibers} />
        </TableHeader<Columns>>
    };

    Ok(html! {
        <Table<Columns, UseTableData<Columns, MemoizedTableModel<CableListEntry>>>
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
