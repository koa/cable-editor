use crate::components::menu::list_cabinet::ListCabinet;
use crate::components::menu::list_cable::ListCable;
use crate::components::menu::list_panel::ListPanel;
use crate::components::menu::list_plan::ListPlan;
use crate::components::menu::{MenuDropdown, MenuEntry};
use crate::{
    components::panel::{attach_fiber::AttachFiber, loop_editor::LoopPortEditor, show::ShowPanel},
    error::FrontendError,
    graphql::authenticated::plan_details::PlanDetails,
    pages::{
        cabinet::{edit::EditCabinetPanels, list::ListOfCabinets, overview::CabinetOverview},
        cable::edit::EditCable,
        list_of_cables::ListOfCables,
        panel::EditPanel,
        planning::{edit::EditPlan, list::ListOfPlannings},
    },
    util::get_credentials,
};
use patternfly_yew::prelude::{
    Breadcrumb, BreadcrumbItem, Nav, NavList, NavRouterItem, PageSection, PageSectionType, Spinner,
};
use std::borrow::Cow;
use yew::virtual_dom::VNode;
use yew::{
    Callback, Component, Context, ContextHandle, Html, Properties, function_component, html,
    html::IntoPropValue, html_nested, platform::spawn_local, use_effect_with,
};
use yew_nested_router::prelude::{RouterContext, Target, use_router};

pub struct Sidebar {
    current_route: AppRoute,
    context_handle: Option<ContextHandle<RouterContext<AppRoute>>>,
    plan: Option<PlanDetails>,
    error: Option<FrontendError>,
}
pub enum SidebarMsg {
    AppRoute(RouterContext<AppRoute>),
    PlanDetails(PlanDetails),
    Error(FrontendError),
}
impl Component for Sidebar {
    type Message = SidebarMsg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        Sidebar {
            current_route: Default::default(),
            context_handle: None,
            plan: None,
            error: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            SidebarMsg::AppRoute(router) => {
                let id = router
                    .active_target
                    .as_ref()
                    .and_then(|r| {
                        if let AppRoute::Plan { plan_id, view } = &r {
                            Some(*plan_id)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                if Some(id) == self.plan.as_ref().map(|p| p.id) {
                    false
                } else {
                    let credentials = get_credentials(ctx.link());
                    let scope = ctx.link().clone();
                    spawn_local(async move {
                        scope.send_message(
                            match PlanDetails::fetch(credentials.as_ref(), id).await {
                                Ok(Some(details)) => SidebarMsg::PlanDetails(details),
                                Err(e) => SidebarMsg::Error(e),
                                Ok(None) => SidebarMsg::Error(FrontendError::PlanNotFound(id)),
                            },
                        );
                    });
                    self.plan = None;
                    true
                }
            }
            SidebarMsg::PlanDetails(details) => {
                self.plan = Some(details);
                self.error = None;
                true
            }
            SidebarMsg::Error(error) => {
                self.error = Some(error);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if let Some(error) = &self.error {
            error.into_prop_value()
        } else {
            match self.plan.as_ref() {
                None => {
                    html!(<Spinner/>)
                }
                Some(plan) => {
                    let id = plan.name.as_str();
                    html! {
                        <Nav>
                            <NavList>
                                //<NavRouterItem<AppRoute> to={AppRoute::Map}>{"Karte"}</NavRouterItem<AppRoute>>
                                //<NavRouterItem<AppRoute> to={AppRoute::MapTest}>{"Karte Editor Test"}</NavRouterItem<AppRoute>>
                                <NavRouterItem<AppRoute> to={AppRoute::ListOfPlans}>{format!("Planung \"{id}\"")}</NavRouterItem<AppRoute>>
                                <NavRouterItem<AppRoute> to={AppRoute::Plan {plan_id: plan.id,view: PlanView::ListOfCabinets}}>{"Schächte"}</NavRouterItem<AppRoute>>
                                <NavRouterItem<AppRoute> to={AppRoute::Plan {plan_id: plan.id,view: PlanView::ListOfCables}}>{"Kabel"}</NavRouterItem<AppRoute>>
                            </NavList>
                        </Nav>
                    }
                }
            }
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            if let Some((current_route, context_handle)) = ctx
                .link()
                .context::<RouterContext<AppRoute>>(ctx.link().callback(SidebarMsg::AppRoute))
            {
                self.context_handle = Some(context_handle);
                ctx.link().send_message(SidebarMsg::AppRoute(current_route));
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Properties)]
pub struct SwitchProps<T>
where
    T: Target,
{
    /// The function rendering based on the active target.
    pub render: Callback<T, Html>,

    /// The default, in case no route is active (not found).
    #[prop_or_default]
    pub default: Html,
}

/// A component two switch rendering between the different targets.
#[function_component(Switch)]
pub fn switch<T>(props: &SwitchProps<T>) -> Html
where
    T: Target + 'static,
{
    let router = use_router::<T>().expect("Must be a child of a Router or Nested component");

    match router.active_target {
        Some(target) => props.render.emit(target),
        None => props.default.clone(),
    }
}

/// Rendered for URLs that match no route: replaces them with the list of plans.
#[function_component(RedirectToPlans)]
pub fn redirect_to_plans() -> Html {
    let router = use_router::<AppRoute>();
    use_effect_with((), move |_| {
        if let Some(router) = router {
            router.replace(AppRoute::ListOfPlans);
        }
    });
    Html::default()
}

#[derive(Debug, Clone, PartialEq, Eq, Target, Default)]
pub enum AppRoute {
    //Map,
    //MapTest,
    #[default]
    ListOfPlans,
    Plan {
        plan_id: i32,
        #[target(nested)]
        view: PlanView,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Target)]
pub enum CableView {
    Edit,
}

#[derive(Debug, Clone, PartialEq, Eq, Target)]
pub enum CabinetView {
    Overview,
    Edit,
}
#[derive(Debug, Clone, PartialEq, Eq, Target)]
pub enum PanelView {
    Edit,
    Loop,
    Attach,
    Show,
}

impl PanelView {
    pub fn content(&self, plan_id: i32, panel_id: i32) -> Html {
        match self {
            PanelView::Edit => {
                html!(<EditPanel {plan_id} {panel_id}/>)
            }
            PanelView::Loop => {
                html!(<LoopPortEditor  {plan_id} {panel_id}/>)
            }
            PanelView::Attach => {
                html!(<AttachFiber {plan_id} {panel_id}/>)
            }
            PanelView::Show => {
                html!(<ShowPanel {plan_id} {panel_id}/>)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Target)]
pub enum PlanView {
    Edit,
    ListOfCabinets,
    Cabinet {
        id: i32,
        #[target(nested)]
        view: CabinetView,
    },
    ListOfCables,
    Cable {
        id: i32,
        #[target(nested)]
        view: CableView,
    },
    Panel {
        id: i32,
        #[target(nested)]
        view: PanelView,
    },
}

impl PlanView {
    pub fn append_breadcrumbs(&self, plan_id: i32, item_contents: &mut Vec<VNode>) {
        let title: Cow<'static, str> = match self {
            PlanView::Edit => "Ändern",
            PlanView::Cabinet { .. } | PlanView::ListOfCabinets | PlanView::Panel { .. } => {
                "Schacht"
            }
            PlanView::Cable { .. } | PlanView::ListOfCables => "Kabel",
        }
        .into();
        let mut entries = Vec::new();
        entries.push(MenuEntry {
            text: "Ändern".into(),
            target: AppRoute::Plan {
                plan_id,
                view: PlanView::Edit,
            },
        });

        entries.push(MenuEntry {
            text: "Schacht".into(),
            target: AppRoute::Plan {
                plan_id,
                view: PlanView::ListOfCabinets,
            },
        });

        entries.push(MenuEntry {
            text: "Kabel".into(),
            target: AppRoute::Plan {
                plan_id,
                view: PlanView::ListOfCables,
            },
        });

        item_contents.push(html!(<MenuDropdown {title} {entries}/>));

        match self {
            PlanView::Cabinet { id, view } => {
                item_contents.push(
                    html!(<ListCabinet plan_id={plan_id} cabinet_id={*id} view={view.clone()}/>),
                );
                view.append_breadcrumbs(plan_id, id, item_contents);
            }
            PlanView::Cable { id, view } => {
                item_contents.push(html!(<ListCable {plan_id} cable_id={id} view={view.clone()}/>))
            }
            PlanView::Panel { id, view } => {
                item_contents
                    .push(html!(<ListPanel {plan_id} panel_id={*id} view={view.clone()}/>));
            }
            _ => {}
        }
    }
}

impl AppRoute {
    /// Renders the page in PatternFly's page layout: the breadcrumb in a `PageSection` inside
    /// `<main>`, followed by the page, which adds its title and content sections with
    /// `PageLayout`. There is no masthead or sidebar, and unlike PatternFly's `Page` the document
    /// scrolls instead of the main container (`.app-page` in `style.scss`).
    pub fn content(self) -> Html {
        let breadcrumb = self.breadcrumb();
        let content = match self {
            AppRoute::ListOfPlans => html!(<ListOfPlannings/>),
            AppRoute::Plan { plan_id, view } => view.content(plan_id),
        };
        html! {
            <div class="pf-v6-c-page pf-m-no-sidebar app-page">
                <div class="pf-v6-c-page__main-container">
                    <main class="pf-v6-c-page__main" id="main-content" tabindex="-1">
                        <PageSection r#type={PageSectionType::Breadcrumbs}>{breadcrumb}</PageSection>
                        {content}
                    </main>
                </div>
            </div>
        }
    }
    fn breadcrumb(&self) -> Html {
        let mut item_contents = Vec::new();
        item_contents.push(match self {
            AppRoute::ListOfPlans => {
                html!(<ListPlan/>)
            }
            AppRoute::Plan { plan_id, view } => {
                html!(<ListPlan {plan_id} view={view.clone()}/>)
            }
        });
        if let AppRoute::Plan { plan_id, view } = self {
            view.append_breadcrumbs(*plan_id, &mut item_contents);
        }
        let items = item_contents
            .into_iter()
            .map(|item_content| html_nested!(<BreadcrumbItem>{item_content}</BreadcrumbItem>));
        html! {
            <Breadcrumb>
                {for items}
            </Breadcrumb>
        }
    }
}

impl PlanView {
    fn content(self, plan_id: i32) -> Html {
        match self {
            PlanView::Edit => html!(<EditPlan {plan_id}/>),
            PlanView::ListOfCabinets => html! {<ListOfCabinets {plan_id}/>},
            PlanView::Cabinet { id, view } => view.content(plan_id, id),
            PlanView::ListOfCables => html! {<ListOfCables/>},
            PlanView::Cable { id, view } => view.content(plan_id, id),
            PlanView::Panel { id, view } => view.content(plan_id, id),
        }
    }
}

impl CableView {
    fn content(self, plan_id: i32, cable_id: i32) -> Html {
        match self {
            CableView::Edit => {
                html!(<EditCable {plan_id} {cable_id}/>)
            }
        }
    }
}

impl CabinetView {
    fn content(self, plan_id: i32, cabinet_id: i32) -> Html {
        match self {
            CabinetView::Overview => html!(<CabinetOverview {plan_id} {cabinet_id}/>),
            CabinetView::Edit => html!(<EditCabinetPanels {plan_id} {cabinet_id}/>),
        }
    }

    fn title(&self) -> &'static str {
        match self {
            CabinetView::Overview => "Übersicht",
            CabinetView::Edit => "Panels bearbeiten",
        }
    }

    /// Menu to switch between the views of the Schacht.
    pub fn append_breadcrumbs(
        &self,
        plan_id: i32,
        cabinet_id: &i32,
        breadcrumb_items: &mut Vec<VNode>,
    ) {
        let entries = [CabinetView::Overview, CabinetView::Edit]
            .into_iter()
            .map(|view| MenuEntry {
                text: view.title().into(),
                target: AppRoute::Plan {
                    plan_id,
                    view: PlanView::Cabinet {
                        id: *cabinet_id,
                        view,
                    },
                },
            })
            .collect::<Vec<_>>();
        let title: Cow<'static, str> = self.title().into();
        breadcrumb_items.push(html!(<MenuDropdown {title} {entries}/>));
    }
}
