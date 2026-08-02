use crate::pages::router::{AppRoute, PlanView};
use yew::{Callback, Component, Context, Html, html};
use yew_nested_router::components::{Link, LinkProperties};

pub struct PlanLink {}
pub enum Msg {}

impl Component for PlanLink {
    type Message = ();
    type Properties = LinkProperties<PlanView>;

    fn create(ctx: &Context<Self>) -> Self {
        PlanLink {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let plan_id = ctx
            .link()
            .context::<AppRoute>(Callback::noop())
            .and_then(|(route, _)| match route {
                AppRoute::Plan { plan_id, .. } => Some(plan_id),
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
                    if let AppRoute::Plan {
                        plan_id: pid,
                        view,
                    } = view
                    {
                        pid==plan_id && p.emit(view)
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
