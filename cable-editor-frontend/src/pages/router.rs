use crate::components::panel::loop_editor::LoopPortEditor;
use crate::error::FrontendError;
use crate::graphql::authenticated::plan_details::PlanDetails;
use crate::pages::cabinet::list::ListOfCabinets;
use crate::pages::cable::edit::EditCable;
use crate::pages::list_of_cables::ListOfCables;
use crate::pages::panel::EditPanel;
use crate::pages::planning::list::ListOfPlannings;
use crate::util::get_credentials;
use patternfly_yew::prelude::{Nav, NavList, NavRouterItem, Spinner};
use yew::html::IntoPropValue;
use yew::platform::spawn_local;
use yew::{
    Callback, Component, Context, ContextHandle, Html, Properties, function_component, html,
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

#[derive(Debug, Clone, PartialEq, Eq, Target, Default)]
pub enum AppRoute {
    NotFound,
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
    Edit,
}
#[derive(Debug, Clone, PartialEq, Eq, Target)]
pub enum PanelView {
    Edit,
    Loop,
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

impl AppRoute {
    pub fn content(self) -> Html {
        match self {
            AppRoute::NotFound => html! {<h1>{"Not Found"}</h1>},
            /*AppRoute::Map => {
                html! {<MapComponent center={Point( 47.417986,8.882440)}/>}
            }
            AppRoute::MapTest => {
                html! {<MapTestPage/>}
            }*/
            AppRoute::ListOfPlans => html! {<ListOfPlannings/>},
            AppRoute::Plan { plan_id, view } => view.content(plan_id),
        }
    }
}
impl PlanView {
    fn content(self, plan_id: i32) -> Html {
        match self {
            PlanView::Edit => format!("Edit Plan {plan_id}").into_prop_value(),
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
            CabinetView::Edit => format!("Edit Cabinet {cabinet_id}").into_prop_value(),
        }
    }
}
