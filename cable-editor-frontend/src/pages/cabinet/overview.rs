use crate::components::page_layout::{PageLayout, object_title};
use crate::{
    components::{
        cabinet::edit::EditCabinet,
        label_printer::{LabelText, PrintLabelButton, use_printer_supported},
        plan_link::PlanLink,
    },
    graphql::authenticated::schacht_cables::{SchachtCableEnd, SchachtCables},
    pages::router::{CableView, PlanView},
};
use patternfly_yew::prelude::{
    Cell, CellContext, Level, MemoizedTableModel, Spinner, Table, TableColumn, TableEntryRenderer,
    TableGridMode, TableHeader, TableMode, Title, UseTableData, use_table_data,
};
use yew::{
    Html, HtmlResult, Properties, Suspense, function_component, html, html::IntoPropValue,
    html_nested, suspense::use_future_with, use_memo,
};
use yew_oauth2::hook::use_auth_state;

#[derive(Properties, PartialEq)]
pub struct CabinetOverviewProps {
    pub plan_id: i32,
    pub cabinet_id: i32,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Columns {
    Cable,
    Destination,
    Label,
    Print,
}

impl TableEntryRenderer<Columns> for SchachtCableEnd {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        match context.column {
            Columns::Cable => {
                let to = PlanView::Cable {
                    id: self.cable.id,
                    view: CableView::Edit,
                };
                Cell::new(html! {<PlanLink {to}>{self.cable.name.as_str()}</PlanLink>})
            }
            Columns::Destination => Cell::new(self.path.far_schacht.name.as_str().into()),
            Columns::Label => Cell::new(self.label_text().into_prop_value()),
            Columns::Print => {
                Cell::new(html!(<PrintLabelButton texts={LabelText::single(self.label_text())}/>))
            }
        }
    }
}

#[function_component]
fn CabinetContent(props: &CabinetOverviewProps) -> HtmlResult {
    let auth_state = use_auth_state();
    let schacht = use_future_with((auth_state, props.cabinet_id), |deps| async move {
        SchachtCables::fetch(deps.0.as_ref(), deps.1).await
    })?;
    let cables = use_memo((*schacht).as_ref().ok().cloned(), |schacht| {
        let mut cables = schacht
            .as_ref()
            .map(|s| s.cables.clone())
            .unwrap_or_default();
        cables.sort_by(|a, b| a.cable.name.cmp(&b.cable.name));
        cables
    });
    let (entries, _) = use_table_data(MemoizedTableModel::new(cables));
    let printing = use_printer_supported();
    let schacht = match &*schacht {
        Ok(schacht) => schacht,
        Err(e) => return Ok(e.into_prop_value()),
    };

    let CabinetOverviewProps {
        plan_id,
        cabinet_id,
    } = *props;
    let header = html_nested! {
        <TableHeader<Columns>>
            <TableColumn<Columns> label="Kabel" index={Columns::Cable}/>
            <TableColumn<Columns> label="Ziel" index={Columns::Destination}/>
            <TableColumn<Columns> label="Etikett" index={Columns::Label}/>
            { for printing.then(|| html_nested!(<TableColumn<Columns> index={Columns::Print}/>)) }
        </TableHeader<Columns>>
    };
    Ok(html! {
        <PageLayout title={object_title("Schacht", Some(&schacht.name))}>
            <EditCabinet {plan_id} {cabinet_id}/>
            <Title level={Level::H2}>{"Kabel"}</Title>
            <Table<Columns, UseTableData<Columns, MemoizedTableModel<SchachtCableEnd>>>
                mode={TableMode::Compact}
                grid={TableGridMode::Medium}
                {header}
                {entries}
            />
        </PageLayout>
    })
}

/// Schacht overview: panels and cables ending here.
#[function_component]
pub fn CabinetOverview(props: &CabinetOverviewProps) -> Html {
    let fallback = html!(<PageLayout title="Schacht"><Spinner/></PageLayout>);
    html! {
        <Suspense {fallback}>
            <CabinetContent plan_id={props.plan_id} cabinet_id={props.cabinet_id}/>
        </Suspense>
    }
}
