use crate::{
    components::fiber::FiberLabel,
    error::FrontendError,
    graphql::authenticated::{
        PortType,
        panel_overview::{
            FiberOwnEndOverview, PlannedChildPanelOverview, PlannedPanelOverview,
            PlannedPortOverview, PortUsageOverview, SchachtOverview, UsedEndPortOverview,
        },
    },
    icons::IconLink,
    pages::router::{AppRoute, PanelView, PlanView},
    util::get_credentials,
};
use gloo_utils::window;
use patternfly_yew::prelude::{
    Alert, AlertType, Button, ButtonVariant, Card, CardBody, CardTitle, Divider, Icon, Spinner,
    Title,
};
use yew::{
    Component, Context, Html, Properties, classes, html, platform::spawn_local,
};
use yew_nested_router::components::Link;

#[derive(Properties, PartialEq, Clone)]
pub struct ShowPanelProps {
    pub plan_id: i32,
    pub panel_id: i32,
}

pub enum Msg {
    FetchData,
    DataFetched(Option<PlannedPanelOverview>),
    Error(FrontendError),
    Print,
}

pub struct ShowPanel {
    plan_id: i32,
    panel_id: i32,
    data: Option<PlannedPanelOverview>,
    loading: bool,
    error: Option<FrontendError>,
}

impl Component for ShowPanel {
    type Message = Msg;
    type Properties = ShowPanelProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            plan_id: ctx.props().plan_id,
            panel_id: ctx.props().panel_id,
            data: None,
            loading: true,
            error: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::FetchData => {
                self.loading = true;
                self.error = None;
                let scope = ctx.link().clone();
                let credentials = get_credentials(ctx.link());
                let plan_id = self.plan_id;
                let panel_id = self.panel_id;

                spawn_local(async move {
                    match PlannedPanelOverview::fetch(credentials.as_ref(), plan_id, panel_id).await
                    {
                        Ok(data) => scope.send_message(Msg::DataFetched(data.map(|(_plan, p)| p))),
                        Err(err) => scope.send_message(Msg::Error(err)),
                    }
                });
                true
            }
            Msg::DataFetched(data) => {
                self.loading = false;
                self.data = data;
                self.error = None;
                true
            }
            Msg::Error(err) => {
                self.loading = false;
                self.error = Some(err);
                true
            }
            Msg::Print => {
                window().print().ok();
                false
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        if self.plan_id != ctx.props().plan_id || self.panel_id != ctx.props().panel_id {
            self.plan_id = ctx.props().plan_id;
            self.panel_id = ctx.props().panel_id;
            ctx.link().send_message(Msg::FetchData);
            true
        } else {
            false
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_message(Msg::FetchData);
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if self.loading {
            return html! {
                <div class="pf-v6-u-p-xl pf-v6-u-text-align-center">
                    <Spinner />
                    <div class="pf-v6-u-mt-md">{"Lade Verbindungsübersicht..."}</div>
                </div>
            };
        }

        if let Some(error) = &self.error {
            return html! {
                <div class="pf-v6-u-p-lg">
                    <Alert title={error.to_string()} r#type={AlertType::Danger} inline=true />
                    <div class="pf-v6-u-mt-md">
                        <Button
                            variant={ButtonVariant::Primary}
                            label="Erneut versuchen"
                            onclick={ctx.link().callback(|_| Msg::FetchData)}
                        />
                    </div>
                </div>
            };
        }

        let Some(root_planned) = &self.data else {
            return html! {
                <div class="pf-v6-u-p-lg">
                    <Alert title="Panel nicht gefunden" r#type={AlertType::Warning} inline=true />
                </div>
            };
        };

        let schacht = &root_planned.panel.schacht;
        let root_panel = &root_planned.panel;
        let child_panels_with_ports: Vec<&PlannedChildPanelOverview> = root_planned
            .all_children_recursive
            .iter()
            .filter(|c| !c.ports.is_empty())
            .collect();
        let has_root_ports = !root_planned.ports.is_empty();

        // Calculate summary metrics
        let total_panels = (if has_root_ports { 1 } else { 0 }) + child_panels_with_ports.len();
        let mut total_ports = if has_root_ports { root_planned.ports.len() } else { 0 };
        let mut used_ports = if has_root_ports { count_used_ports(&root_planned.ports) } else { 0 };
        let mut modified_ports = if has_root_ports { count_modified_ports(&root_planned.ports) } else { 0 };

        for child in &child_panels_with_ports {
            total_ports += child.ports.len();
            used_ports += count_used_ports(&child.ports);
            modified_ports += count_modified_ports(&child.ports);
        }

        let has_any_loop = root_planned
            .ports
            .iter()
            .chain(child_panels_with_ports.iter().flat_map(|c| c.ports.iter()))
            .any(|p| p.port_type == PortType::Loop);

        html! {
            <div class="panel-overview-container pf-v6-u-p-md pf-v6-u-p-lg-on-lg">
                // --- Print Only Header ---
                <div class="print-only pf-v6-u-mb-lg">
                    <div style="border-bottom: 2px solid #000; padding-bottom: 8px; margin-bottom: 12px;">
                        <h1 style="font-size: 20pt; font-weight: bold; margin: 0;">
                            {"Kabel- & Faser-Dokumentation: Verbindungsübersicht"}
                        </h1>
                        <div style="font-size: 11pt; color: #444; margin-top: 4px;">
                            {format!("Schacht: {} | Panel: {}", schacht.name, root_panel.name.as_deref().unwrap_or("-"))}
                        </div>
                    </div>
                </div>

                // --- Screen Navigation & Toolbar ---
                <div class="no-print pf-v6-u-mb-md">
                    <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 12px;">
                        <div>
                            <nav class="pf-v6-c-breadcrumb" aria-label="breadcrumb">
                                <ol class="pf-v6-c-breadcrumb__list">
                                    <li class="pf-v6-c-breadcrumb__item">
                                        <Link<AppRoute> to={AppRoute::Plan { plan_id: self.plan_id, view: PlanView::ListOfCabinets }} class="pf-v6-c-breadcrumb__link">
                                            {"Schächte"}
                                        </Link<AppRoute>>
                                    </li>
                                    <li class="pf-v6-c-breadcrumb__item">
                                        <span class="pf-v6-c-breadcrumb__link pf-m-current">
                                            {format!("{}: {}", schacht.name, root_panel.name.as_deref().unwrap_or("Panel"))}
                                        </span>
                                    </li>
                                </ol>
                            </nav>
                        </div>
                        <div>
                            <div class="overview-action-buttons">
                                <Button
                                    variant={ButtonVariant::Secondary}
                                    icon={Icon::Print}
                                    label="Drucken / PDF"
                                    onclick={ctx.link().callback(|_| Msg::Print)}
                                />
                                if self.plan_id > 0 {
                                    <Link<AppRoute>
                                        to={AppRoute::Plan {
                                            plan_id: self.plan_id,
                                            view: PlanView::Panel {
                                                id: self.panel_id,
                                                view: PanelView::Attach,
                                            },
                                        }}
                                        class="pf-v6-c-button pf-m-secondary"
                                    >
                                        <IconLink/>
                                        <span class="pf-v6-u-ml-xs">{"Fasern auflegen"}</span>
                                    </Link<AppRoute>>
                                    if has_any_loop {
                                        <Link<AppRoute>
                                            to={AppRoute::Plan {
                                                plan_id: self.plan_id,
                                                view: PlanView::Panel {
                                                    id: self.panel_id,
                                                    view: PanelView::Loop,
                                                },
                                            }}
                                            class="pf-v6-c-button pf-m-secondary"
                                        >
                                            {Icon::Redo}
                                            <span class="pf-v6-u-ml-xs">{"Loops verbinden"}</span>
                                        </Link<AppRoute>>
                                    }
                                }
                                <Link<AppRoute>
                                    to={AppRoute::Plan {
                                        plan_id: self.plan_id,
                                        view: PlanView::Panel {
                                            id: self.panel_id,
                                            view: PanelView::Edit,
                                        },
                                    }}
                                    class="pf-v6-c-button pf-m-secondary"
                                >
                                    {Icon::FolderOpen}
                                    <span class="pf-v6-u-ml-xs">{"Ports ändern"}</span>
                                </Link<AppRoute>>
                            </div>
                        </div>
                    </div>
                </div>

                // --- Header Summary Card ---
                <Card class="pf-v6-u-mb-lg panel-summary-card">
                    <CardTitle>
                        <div class="overview-header-title">
                            <div>
                                <Title size={patternfly_yew::prelude::Size::XXLarge}>
                                    {format!("Verbindungsübersicht: {}", root_panel)}
                                </Title>
                                <div class="pf-v6-u-font-size-sm pf-v6-u-color-200 pf-v6-u-mt-xs">
                                    {format!("Standort: Schacht \"{}\"", schacht.name)}
                                </div>
                            </div>
                            <div class="overview-header-badges">
                                if modified_ports > 0 {
                                    <span class="pf-v6-c-label pf-m-orange">
                                        <span class="pf-v6-c-label__content">
                                            <span class="pf-v6-c-label__icon">{Icon::InProgress}</span>
                                            {format!("{modified_ports} Ports geändert")}
                                        </span>
                                    </span>
                                }
                            </div>
                        </div>
                    </CardTitle>
                    <Divider />
                    <CardBody>
                        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px;" class="overview-stats-grid">
                            <div class="stat-box">
                                <div class="stat-value">{total_panels}</div>
                                <div class="stat-label">
                                    {if total_panels == 1 {
                                        "1 Panel".to_string()
                                    } else {
                                        format!("{total_panels} Panels")
                                    }}
                                </div>
                            </div>
                            <div class="stat-box">
                                <div class="stat-value">{total_ports}</div>
                                <div class="stat-label">{"Ports gesamt"}</div>
                            </div>
                            <div class="stat-box">
                                <div class="stat-value">{used_ports}</div>
                                <div class="stat-label">{format!("Belegt ({:.0}%)", if total_ports > 0 { (used_ports as f64 / total_ports as f64) * 100.0 } else { 0.0 })}</div>
                            </div>
                            <div class="stat-box">
                                <div class="stat-value">{total_ports.saturating_sub(used_ports)}</div>
                                <div class="stat-label">{"Freie Ports"}</div>
                            </div>
                        </div>
                    </CardBody>
                </Card>

                // --- Hierarchy Navigation Links (if multiple panels with ports) ---
                if total_panels > 1 {
                    <div class="no-print pf-v6-u-mb-lg panel-toc-card">
                        <Card>
                            <CardTitle>
                                <Title size={patternfly_yew::prelude::Size::Medium}>
                                    {Icon::Folder}
                                    <span class="pf-v6-u-ml-sm">{"Schnellnavigation"}</span>
                                </Title>
                            </CardTitle>
                            <CardBody>
                                <div class="panel-hierarchy-tree">
                                    if has_root_ports {
                                        <a href="#panel-root" class="panel-tree-node root-node">
                                            {Icon::FolderOpen}
                                            <span class="pf-v6-u-ml-xs pf-v6-u-font-weight-bold">
                                                {root_panel.name.as_deref().unwrap_or("Panel")}
                                            </span>
                                            <span class="pf-v6-u-ml-sm port-count-tag">
                                                {format!("({} Ports)", root_planned.ports.len())}
                                            </span>
                                        </a>
                                    }
                                    { for child_panels_with_ports.iter().map(|child| {
                                        let anchor_id = format!("panel-child-{}", child.panel.id);
                                        let level = child.panel.parent_chain.len().max(1);
                                        let indent_px = level * 20;
                                        let child_name = child.panel.name.as_deref().unwrap_or("Kassette");
                                        html! {
                                            <a
                                                href={format!("#{anchor_id}")}
                                                class="panel-tree-node child-node"
                                                style={format!("margin-left: {indent_px}px;")}
                                            >
                                                {Icon::AngleRight}
                                                <span class="pf-v6-u-ml-xs">{child_name}</span>
                                                <span class="pf-v6-u-ml-sm port-count-tag">
                                                    {format!("({} Ports)", child.ports.len())}
                                                </span>
                                            </a>
                                        }
                                    })}
                                </div>
                            </CardBody>
                        </Card>
                    </div>
                }

                // --- Root Panel Connections (only if it has ports) ---
                if has_root_ports {
                    <div id="panel-root" class="panel-overview-section pf-v6-u-mb-xl">
                        <div class="panel-section-header">
                            <div class="panel-section-title-wrapper">
                                <Title size={patternfly_yew::prelude::Size::Large}>
                                    {Icon::FolderOpen}
                                    <span class="pf-v6-u-ml-sm">
                                        {root_panel.name.as_deref().unwrap_or("Panel")}
                                    </span>
                                </Title>
                            </div>
                            <div class="panel-section-actions">
                                <span class="pf-v6-c-label pf-m-grey">
                                    <span class="pf-v6-c-label__content">{format!("{} Ports", root_planned.ports.len())}</span>
                                </span>
                                if self.plan_id > 0 {
                                    <Link<AppRoute>
                                        to={AppRoute::Plan {
                                            plan_id: self.plan_id,
                                            view: PlanView::Panel {
                                                id: root_panel.id,
                                                view: PanelView::Attach,
                                            },
                                        }}
                                        class="no-print pf-v6-c-button pf-m-plain pf-v6-u-ml-sm"
                                    >
                                        <IconLink/>
                                    </Link<AppRoute>>
                                }
                            </div>
                        </div>

                        { self.render_ports_table(&root_planned.ports, schacht, root_panel.id) }
                    </div>
                }

                // --- Recursive Child Panels Connections (only panels with ports) ---
                { for child_panels_with_ports.iter().map(|child| {
                    let anchor_id = format!("panel-child-{}", child.panel.id);
                    let level = child.panel.parent_chain.len().max(1);
                    let child_id = child.panel.id;

                    html! {
                        <div id={anchor_id} class={classes!("panel-overview-section", "child-panel-section", format!("hierarchy-level-{}", level), "pf-v6-u-mb-xl")}>
                            <div class="panel-section-header child-header">
                                <div class="panel-section-title-wrapper">
                                    <Title size={patternfly_yew::prelude::Size::Large}>
                                        {Icon::AngleDoubleRight}
                                        <span class="pf-v6-u-ml-sm">
                                            {format!("{}", child.panel)}
                                        </span>
                                    </Title>
                                </div>
                                <div class="panel-section-actions">
                                    <span class="pf-v6-c-label pf-m-cyan">
                                        <span class="pf-v6-c-label__content">{format!("{} Ports", child.ports.len())}</span>
                                    </span>
                                    if self.plan_id > 0 {
                                        <Link<AppRoute>
                                            to={AppRoute::Plan {
                                                plan_id: self.plan_id,
                                                view: PlanView::Panel {
                                                    id: child_id,
                                                    view: PanelView::Attach,
                                                },
                                            }}
                                            class="no-print pf-v6-c-button pf-m-plain pf-v6-u-ml-sm"
                                        >
                                            <IconLink/>
                                        </Link<AppRoute>>
                                    }
                                </div>
                            </div>

                            { self.render_ports_table(&child.ports, schacht, child_id) }
                        </div>
                    }
                })}

                if total_ports == 0 {
                    <Card class="empty-panel-card pf-v6-u-mb-xl">
                        <CardBody>
                            <div class="pf-v6-u-text-align-center pf-v6-u-color-200 pf-v6-u-p-lg">
                                {Icon::ExclamationTriangle}
                                <span class="pf-v6-u-ml-sm">{"Keine Ports auf diesen Panels vorhanden."}</span>
                            </div>
                        </CardBody>
                    </Card>
                }

                // --- Print Footer ---
                <div class="print-only pf-v6-u-mt-xl" style="border-top: 1px solid #ccc; padding-top: 8px; font-size: 9pt; color: #666; text-align: right;">
                    {format!("Ausgedruckt am {} | Kabel-Editor Dokumentation", chrono_stub_date())}
                </div>
            </div>
        }
    }
}

impl ShowPanel {
    fn render_ports_table(
        &self,
        ports: &[PlannedPortOverview],
        schacht: &SchachtOverview,
        _panel_id: i32,
    ) -> Html {
        if ports.is_empty() {
            return html! {
                <div class="pf-v6-u-p-md pf-v6-u-color-200 pf-v6-u-text-align-center empty-state-box">
                    {"Keine Ports auf diesem Panel vorhanden."}
                </div>
            };
        }

        let mut sorted_ports = ports.to_vec();
        sorted_ports.sort_by_key(|p| p.order_number);

        html! {
            <div class="panel-connections-wrapper">
                // Desktop Table View
                <table class="panel-connections-table pf-v6-c-table pf-m-grid-md pf-m-compact" role="grid">
                    <thead>
                        <tr role="row">
                            <th class="col-port" scope="col" style="width: 140px;">{"Port / Faser"}</th>
                            <th class="col-front" scope="col" style="width: 38%;">{"Front-Belegung"}</th>
                            <th class="col-flow" scope="col" style="width: 60px; text-align: center;">{"Status"}</th>
                            <th class="col-back" scope="col" style="width: 38%;">{"Back-Belegung"}</th>
                        </tr>
                    </thead>
                    <tbody role="rowgroup">
                        { for sorted_ports.iter().map(|port| self.render_port_row(port, schacht)) }
                    </tbody>
                </table>

                // Mobile Card View (shown via CSS on small viewports)
                <div class="panel-connections-mobile">
                    { for sorted_ports.iter().map(|port| self.render_port_mobile_card(port, schacht)) }
                </div>
            </div>
        }
    }

    fn render_port_row(&self, port: &PlannedPortOverview, schacht: &SchachtOverview) -> Html {
        let port_label = port
            .label
            .clone()
            .unwrap_or_else(|| format!("Port {}", port.order_number));

        let (type_text, type_class) = match port.port_type {
            PortType::Splice => ("Spleiss", "pf-m-green"),
            PortType::Connector => ("Stecker", "pf-m-cyan"),
            PortType::Loop => ("Loop", "pf-m-purple"),
        };
        let is_loop = port.port_type == PortType::Loop;

        let loop_fiber = if is_loop {
            port.front_usage
                .as_ref()
                .and_then(|u| u.fiber.as_ref())
                .or_else(|| port.back_usage.as_ref().and_then(|u| u.fiber.as_ref()))
        } else {
            None
        };

        let is_modified = port
            .front_usage
            .as_ref()
            .is_some_and(|u| u.modified_in_plan)
            || port
                .back_usage
                .as_ref()
                .is_some_and(|u| u.modified_in_plan);

        let row_class = if is_modified {
            "port-row modified-row"
        } else {
            "port-row"
        };

        let flow_icon = match port.port_type {
            PortType::Loop => {
                let title = if let Some(fiber_info) = loop_fiber {
                    format!("Schleife / Loop: Bündel {}, Faser {}", fiber_info.bundle, fiber_info.fiber)
                } else {
                    "Schleife / Loop".to_string()
                };
                html! {
                    <span class="flow-badge loop-flow" {title}>
                        {Icon::Redo}
                    </span>
                }
            }
            _ => {
                let has_front = port.front_usage.as_ref().and_then(|u| u.fiber).is_some();
                let has_back = port.back_usage.as_ref().and_then(|u| u.fiber).is_some();
                if has_front && has_back {
                    html! {
                        <span class="flow-badge connected-flow" title="Beidseitig durchverbunden">
                            {Icon::ArrowRight}
                        </span>
                    }
                } else if has_front || has_back {
                    html! {
                        <span class="flow-badge partial-flow" title="Einseitig belegt">
                            {Icon::AngleRight}
                        </span>
                    }
                } else {
                    html! {
                        <span class="flow-badge empty-flow" title="Unbelegt">
                            {"—"}
                        </span>
                    }
                }
            }
        };

        let port_identity = if is_loop {
            if let Some(fiber_info) = loop_fiber {
                html! {
                    <div class="port-identity">
                        <div class="port-name-wrapper">
                            <FiberLabel fiber={fiber_info.fiber as u8}>
                                {format!("{}-{}", fiber_info.bundle, fiber_info.fiber)}
                            </FiberLabel>
                            if is_modified {
                                <span class="modified-dot" title="In dieser Planung geändert" />
                            }
                        </div>
                        <div class="pf-v6-u-mt-xs pf-v6-u-display-flex pf-v6-u-align-items-center">
                            <span class={classes!("pf-v6-c-label", type_class, "port-type-label")}>
                                <span class="pf-v6-c-label__content">{type_text}</span>
                            </span>
                            <span class="pf-v6-u-font-size-xs pf-v6-u-color-200 pf-v6-u-ml-xs">
                                {format!("(#{})", port.order_number)}
                            </span>
                        </div>
                    </div>
                }
            } else {
                html! {
                    <div class="port-identity">
                        <div class="port-name-wrapper">
                            <strong class="port-label-text">{"Loop"}</strong>
                            if is_modified {
                                <span class="modified-dot" title="In dieser Planung geändert" />
                            }
                        </div>
                        <div class="pf-v6-u-mt-xs pf-v6-u-display-flex pf-v6-u-align-items-center">
                            <span class={classes!("pf-v6-c-label", type_class, "port-type-label")}>
                                <span class="pf-v6-c-label__content">{type_text}</span>
                            </span>
                            <span class="pf-v6-u-font-size-xs pf-v6-u-color-200 pf-v6-u-ml-xs">
                                {format!("(#{})", port.order_number)}
                            </span>
                        </div>
                    </div>
                }
            }
        } else {
            html! {
                <div class="port-identity">
                    <div class="port-name-wrapper">
                        <span class="port-number-badge">{format!("#{}", port.order_number)}</span>
                        <strong class="port-label-text">{port_label}</strong>
                        if is_modified {
                            <span class="modified-dot" title="In dieser Planung geändert" />
                        }
                    </div>
                    <div class="pf-v6-u-mt-xs">
                        <span class={classes!("pf-v6-c-label", type_class, "port-type-label")}>
                            <span class="pf-v6-c-label__content">{type_text}</span>
                        </span>
                    </div>
                </div>
            }
        };

        html! {
            <tr class={row_class} role="row">
                <td class="col-port" role="cell">
                    {port_identity}
                </td>
                <td class="col-front" role="cell">
                    { self.render_connection_cell(port.front_usage.as_ref(), schacht, "Front") }
                </td>
                <td class="col-flow" role="cell" style="text-align: center; vertical-align: middle;">
                    {flow_icon}
                </td>
                <td class="col-back" role="cell">
                    { self.render_connection_cell(port.back_usage.as_ref(), schacht, "Back") }
                </td>
            </tr>
        }
    }

    fn render_port_mobile_card(
        &self,
        port: &PlannedPortOverview,
        schacht: &SchachtOverview,
    ) -> Html {
        let port_label = port
            .label
            .clone()
            .unwrap_or_else(|| format!("Port {}", port.order_number));

        let (type_text, type_class) = match port.port_type {
            PortType::Splice => ("Spleiss", "pf-m-green"),
            PortType::Connector => ("Stecker", "pf-m-cyan"),
            PortType::Loop => ("Loop", "pf-m-purple"),
        };
        let is_loop = port.port_type == PortType::Loop;

        let loop_fiber = if is_loop {
            port.front_usage
                .as_ref()
                .and_then(|u| u.fiber.as_ref())
                .or_else(|| port.back_usage.as_ref().and_then(|u| u.fiber.as_ref()))
        } else {
            None
        };

        let is_modified = port
            .front_usage
            .as_ref()
            .is_some_and(|u| u.modified_in_plan)
            || port
                .back_usage
                .as_ref()
                .is_some_and(|u| u.modified_in_plan);

        let mobile_port_title = if is_loop {
            if let Some(fiber_info) = loop_fiber {
                html! {
                    <div class="mobile-port-title">
                        <FiberLabel fiber={fiber_info.fiber as u8}>
                            {format!("{}-{}", fiber_info.bundle, fiber_info.fiber)}
                        </FiberLabel>
                        <span class="pf-v6-u-font-size-xs pf-v6-u-color-200 pf-v6-u-ml-xs">
                            {format!("(#{})", port.order_number)}
                        </span>
                    </div>
                }
            } else {
                html! {
                    <div class="mobile-port-title">
                        <strong>{"Loop"}</strong>
                        <span class="pf-v6-u-font-size-xs pf-v6-u-color-200 pf-v6-u-ml-xs">
                            {format!("(#{})", port.order_number)}
                        </span>
                    </div>
                }
            }
        } else {
            html! {
                <div class="mobile-port-title">
                    <span class="port-number-badge">{format!("#{}", port.order_number)}</span>
                    <strong>{port_label}</strong>
                </div>
            }
        };

        html! {
            <div class={classes!("port-mobile-card", is_modified.then_some("modified-card"))}>
                <div class="mobile-card-header">
                    {mobile_port_title}
                    <div class="mobile-card-badges">
                        <span class={classes!("pf-v6-c-label", type_class)}>
                            <span class="pf-v6-c-label__content">{type_text}</span>
                        </span>
                        if is_modified {
                            <span class="pf-v6-c-label pf-m-orange">
                                <span class="pf-v6-c-label__content">
                                    <span class="pf-v6-c-label__icon">{Icon::InProgress}</span>
                                    {"Planung"}
                                </span>
                            </span>
                        }
                    </div>
                </div>
                <div class="mobile-card-body">
                    <div class="mobile-slot front-slot">
                        <div class="slot-side-label">{"Front"}</div>
                        { self.render_connection_cell(port.front_usage.as_ref(), schacht, "Front") }
                    </div>
                    <div class="mobile-slot-divider">
                        {if port.port_type == PortType::Loop { Icon::Redo } else { Icon::ArrowRight }}
                    </div>
                    <div class="mobile-slot back-slot">
                        <div class="slot-side-label">{"Back"}</div>
                        { self.render_connection_cell(port.back_usage.as_ref(), schacht, "Back") }
                    </div>
                </div>
            </div>
        }
    }

    fn render_connection_cell(
        &self,
        usage_opt: Option<&PortUsageOverview>,
        schacht: &SchachtOverview,
        _side_name: &str,
    ) -> Html {
        let Some(usage) = usage_opt else {
            return html! {
                <div class="slot-empty">
                    <span class="empty-dash">{"—"}</span>
                    <span class="empty-text">{"Frei"}</span>
                </div>
            };
        };

        let Some(fiber_info) = &usage.fiber else {
            return html! {
                <div class="slot-empty">
                    <span class="empty-dash">{"—"}</span>
                    <span class="empty-text">{"Frei"}</span>
                </div>
            };
        };

        // Find cable in schacht.cables
        let cable_end = schacht
            .cables
            .iter()
            .find(|c| c.cable.id == fiber_info.cable.id);

        let cable_name = cable_end
            .map(|c| c.cable.name.clone())
            .unwrap_or_else(|| format!("Kabel {}", fiber_info.cable.id));

        let far_schacht_name = cable_end.map(|c| c.path.far_schacht.name.clone());

        // Find specific fiber end to get other end termination details
        let fiber_own_end = cable_end.and_then(|c| {
            c.fibers
                .iter()
                .find(|f| f.bundle == fiber_info.bundle && f.fiber == fiber_info.fiber)
        });

        let destination_label = cable_end_label(fiber_own_end);

        html! {
            <div class="slot-assigned">
                <div class="slot-cable-row">
                    <span class="cable-name">{cable_name}</span>
                    if let Some(far_schacht) = far_schacht_name {
                        <span class="cable-far-schacht pf-v6-u-font-size-xs pf-v6-u-color-200">
                            {format!(" ➔ {}", far_schacht)}
                        </span>
                    }
                    if usage.modified_in_plan {
                        <span class="modified-badge no-print" title="In dieser Planung geändert">
                            {Icon::InProgress}
                        </span>
                    }
                </div>
                <div class="slot-fiber-row pf-v6-u-my-xs">
                    <FiberLabel fiber={fiber_info.fiber as u8}>
                        {format!("{}-{}", fiber_info.bundle, fiber_info.fiber)}
                    </FiberLabel>
                </div>
                if let Some(dest) = destination_label {
                    <div class="slot-destination-row pf-v6-u-font-size-xs">
                        <span class="destination-icon">{Icon::ArrowRight}</span>
                        {" "}
                        <span class="destination-text">{dest}</span>
                    </div>
                }
            </div>
        }
    }
}

fn count_used_ports(ports: &[PlannedPortOverview]) -> usize {
    ports
        .iter()
        .filter(|p| {
            p.front_usage.as_ref().and_then(|u| u.fiber).is_some()
                || p.back_usage.as_ref().and_then(|u| u.fiber).is_some()
        })
        .count()
}

fn count_modified_ports(ports: &[PlannedPortOverview]) -> usize {
    ports
        .iter()
        .filter(|p| {
            p.front_usage.as_ref().is_some_and(|u| u.modified_in_plan)
                || p.back_usage.as_ref().is_some_and(|u| u.modified_in_plan)
        })
        .count()
}

fn cable_end_label(option: Option<&FiberOwnEndOverview>) -> Option<String> {
    option
        .and_then(|f| f.other_end.as_ref())
        .and_then(|e| e.used_port.as_ref())
        .and_then(|p| p.panel_side_end_port.as_ref())
        .map(UsedEndPortOverview::to_string)
}

fn chrono_stub_date() -> String {
    "2026".to_string()
}
