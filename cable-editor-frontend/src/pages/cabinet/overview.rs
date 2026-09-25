use crate::components::page_layout::{PageLayout, object_title};
use crate::{
    components::{
        label_printer::{LabelText, PanelLabelButton, PrintLabelButton, use_printer_supported},
        plan_link::PlanLink,
    },
    graphql::authenticated::schacht_cables::{SchachtCableEnd, SchachtCables, SchachtPanelEntry},
    pages::router::{CabinetView, CableView, PanelView, PlanView},
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

    let cabinet_id = props.cabinet_id;
    let header = html_nested! {
        <TableHeader<Columns>>
            <TableColumn<Columns> label="Kabel" index={Columns::Cable}/>
            <TableColumn<Columns> label="Ziel" index={Columns::Destination}/>
            <TableColumn<Columns> label="Etikett" index={Columns::Label}/>
            { for printing.then(|| html_nested!(<TableColumn<Columns> index={Columns::Print}/>)) }
        </TableHeader<Columns>>
    };
    let panels = schacht.panels();
    let edit_panels = PlanView::Cabinet {
        id: cabinet_id,
        view: CabinetView::Edit,
    };
    Ok(html! {
        <PageLayout title={object_title("Schacht", Some(&schacht.name))}>
            <Title level={Level::H2}>{"Panels"}</Title>
            if panels.is_empty() {
                <p class="pf-v6-u-color-200">{"Keine Panels im Schacht."}</p>
            } else {
                <ul class="pf-v6-c-data-list pf-m-compact schacht-panels" role="list" aria-label="Panels">
                    {for panels.iter().map(view_panel)}
                </ul>
            }
            <div class="pf-v6-u-mb-xl">
                <PlanLink to={edit_panels} class="pf-v6-c-button pf-m-secondary">
                    {"Panels bearbeiten"}
                </PlanLink>
            </div>
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

/// Panel of the Schacht linking to its connection overview, with its label.
fn view_panel(panel: &SchachtPanelEntry) -> Html {
    let to = PlanView::Panel {
        id: panel.id,
        view: PanelView::Show,
    };
    let name = panel
        .name
        .clone()
        .unwrap_or_else(|| format!("Panel {}", panel.id));
    let depth = format!("--panel-depth: {}", panel.parents.len());
    html! {
        <li class="pf-v6-c-data-list__item" key={panel.id}>
            <div class="pf-v6-c-data-list__item-row">
                <div class="pf-v6-c-data-list__item-content">
                    <div class="pf-v6-c-data-list__cell schacht-panels__name" style={depth}>
                        <PlanLink {to}>{name}</PlanLink>
                        if panel.port_count > 0 {
                            <span class="pf-v6-u-ml-sm pf-v6-u-color-200">
                                {format!("{} Ports", panel.port_count)}
                            </span>
                        }
                    </div>
                </div>
                <div class="pf-v6-c-data-list__item-action">
                    <PanelLabelButton
                        id={panel.id}
                        name={panel.name.clone()}
                        parents={panel.parents.clone()}
                    />
                </div>
            </div>
        </li>
    }
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
