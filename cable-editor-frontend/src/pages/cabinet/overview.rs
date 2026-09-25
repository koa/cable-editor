use crate::{
    components::{
        label_printer::{LabelText, PanelLabelButton, PrintLabelButton, check_printer_supported},
        links::{CableLink, PanelLink, SchachtLink},
        page_layout::{PageLayout, object_title},
        plan_link::PlanLink,
        table::ListModel,
    },
    error::FrontendError,
    graphql::authenticated::schacht_cables::{SchachtCableEnd, SchachtCables, SchachtPanelEntry},
    pages::router::{CabinetView, PlanView},
    util::get_credentials,
};
use patternfly_yew::prelude::{
    Cell, CellContext, ExpansionState, Level, MemoizedTableModel, Spinner, Table, TableColumn,
    TableEntryRenderer, TableGridMode, TableHeader, TableMode, Title,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use yew::{
    Component, Context, Html, Properties, html, html::IntoPropValue, html_nested,
    platform::spawn_local,
};

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
                Cell::new(html!(<CableLink id={self.cable.id} text={self.cable.name.clone()}/>))
            }
            Columns::Destination => {
                let far = &self.path.far_schacht;
                Cell::new(html!(<SchachtLink id={far.id} text={far.name.clone()}/>))
            }
            Columns::Label => Cell::new(self.label_text().into_prop_value()),
            Columns::Print => {
                Cell::new(html!(<PrintLabelButton texts={LabelText::single(self.label_text())}/>))
            }
        }
    }
}

/// Panel of the Schacht linking to its connection overview, with its label.
fn view_panel(panel: &SchachtPanelEntry) -> Html {
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
                        <PanelLink id={panel.id} text={name}/>
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
pub struct CabinetOverview {
    /// `None` while loading
    schacht: Option<Result<SchachtCables, FrontendError>>,
    /// The cables of `schacht`, sorted by name
    cables: Rc<Vec<SchachtCableEnd>>,
    /// Required by `ListModel`; the rows don't expand
    table_state: Rc<RefCell<HashMap<usize, ExpansionState<Columns>>>>,
    printing: bool,
}

pub enum Msg {
    Fetch,
    Loaded(Result<SchachtCables, FrontendError>),
    PrinterSupported(bool),
}

impl Component for CabinetOverview {
    type Message = Msg;
    type Properties = CabinetOverviewProps;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Fetch);
        check_printer_supported(ctx.link(), Msg::PrinterSupported);
        Self {
            schacht: None,
            cables: Rc::default(),
            table_state: Rc::default(),
            printing: false,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Fetch => {
                let scope = ctx.link().clone();
                let credentials = get_credentials(&scope);
                let cabinet_id = ctx.props().cabinet_id;
                spawn_local(async move {
                    let schacht = SchachtCables::fetch(credentials.as_ref(), cabinet_id).await;
                    scope.send_message(Msg::Loaded(schacht));
                });
                false
            }
            Msg::Loaded(schacht) => {
                let mut cables = schacht
                    .as_ref()
                    .map(|s| s.cables.clone())
                    .unwrap_or_default();
                cables.sort_by(|a, b| a.cable.name.cmp(&b.cable.name));
                self.cables = Rc::new(cables);
                self.schacht = Some(schacht);
                true
            }
            Msg::PrinterSupported(printing) => {
                self.printing = printing;
                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        if ctx.props().cabinet_id != old_props.cabinet_id {
            self.schacht = None;
            ctx.link().send_message(Msg::Fetch);
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        match &self.schacht {
            None => html!(<PageLayout title="Schacht"><Spinner/></PageLayout>),
            Some(Err(error)) => {
                let error: Html = error.into_prop_value();
                html!(<PageLayout title="Schacht">{error}</PageLayout>)
            }
            Some(Ok(schacht)) => self.view_schacht(ctx, schacht),
        }
    }
}

impl CabinetOverview {
    fn view_schacht(&self, ctx: &Context<Self>, schacht: &SchachtCables) -> Html {
        let header = html_nested! {
            <TableHeader<Columns>>
                <TableColumn<Columns> label="Kabel" index={Columns::Cable}/>
                <TableColumn<Columns> label="Ziel" index={Columns::Destination}/>
                <TableColumn<Columns> label="Etikett" index={Columns::Label}/>
                { for self.printing.then(|| html_nested!(<TableColumn<Columns> index={Columns::Print}/>)) }
            </TableHeader<Columns>>
        };
        let entries = ListModel::new(
            MemoizedTableModel::new(self.cables.clone()),
            self.table_state.clone(),
        );
        let panels = schacht.panels();
        let edit_panels = PlanView::Cabinet {
            id: ctx.props().cabinet_id,
            view: CabinetView::Edit,
        };
        html! {
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
                <Table<Columns, ListModel<Columns, MemoizedTableModel<SchachtCableEnd>>>
                    mode={TableMode::Compact}
                    grid={TableGridMode::Medium}
                    {header}
                    {entries}
                />
            </PageLayout>
        }
    }
}
