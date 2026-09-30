// The only place allowed to build a router `Link`: everywhere else `PlanLink` and `PlanNameLink`
// take care of the plan in the path (see docs/frontend-konventionen.md, section 3).
#![allow(clippy::disallowed_types)]
use crate::pages::router::{AppRoute, PlanView};
use yew::{AttrValue, Callback, Component, Context, Html, Properties, function_component, html};
use yew_nested_router::{
    components::{Link, LinkProperties},
    prelude::RouterContext,
};

/// Link to a view within the plan of the current page.
pub struct PlanLink {}
impl Component for PlanLink {
    type Message = ();
    type Properties = LinkProperties<PlanView>;

    fn create(_ctx: &Context<Self>) -> Self {
        PlanLink {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        // The router provides the route as RouterContext; nothing provides a bare AppRoute
        let plan_id = ctx
            .link()
            .context::<RouterContext<AppRoute>>(Callback::noop())
            .and_then(|(router, _)| match router.active_target {
                Some(AppRoute::Plan { plan_id, .. }) => Some(plan_id),
                _ => None,
            })
            .unwrap_or_default();
        let props = ctx.props();
        let to = AppRoute::Plan {
            plan_id,
            view: props.to.clone(),
        };
        let props = LinkProperties {
            children: props.children.clone(),
            id: props.id.clone(),
            to,
            state: props.state.clone(),
            any: props.any,
            predicate: props.predicate.clone().map(|p| {
                Callback::<AppRoute, bool>::from(move |view| {
                    if let AppRoute::Plan { plan_id: pid, view } = view {
                        pid == plan_id && p.emit(view)
                    } else {
                        false
                    }
                })
            }),
            element: props.element.clone(),
            suppress_href: props.suppress_href,
            suppress_hash: props.suppress_hash,
            class: props.class.clone(),
            active: props.active.clone(),
            inactive: props.inactive.clone(),
        };
        html!(<Link<AppRoute> ..props/>)
    }
}

#[derive(Properties, PartialEq)]
pub struct PlanNameLinkProps {
    pub id: i32,
    /// Shown text, the plan's name
    pub text: AttrValue,
}

/// Link to a plan, wherever a page names one: to its Schächte, where its list leads to as well.
#[function_component]
pub fn PlanNameLink(props: &PlanNameLinkProps) -> Html {
    let to = AppRoute::Plan {
        plan_id: props.id,
        view: PlanView::ListOfCabinets,
    };
    html!(<Link<AppRoute> {to}>{props.text.clone()}</Link<AppRoute>>)
}
