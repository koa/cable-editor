//! Links to elements shown on a page, each to the view that fits best: a Schacht to its overview,
//! a panel to its connection overview, a cable or a duct to its page (its only view).
use crate::{
    components::plan_link::PlanLink,
    pages::router::{CabinetView, CableView, DuctView, PanelView, PlanView},
};
use yew::{AttrValue, Html, Properties, function_component, html};

#[derive(Properties, PartialEq)]
pub struct ElementLinkProps {
    pub id: i32,
    /// Shown text, the element's name
    pub text: AttrValue,
}

#[function_component]
pub fn SchachtLink(props: &ElementLinkProps) -> Html {
    let to = PlanView::Cabinet {
        id: props.id,
        view: CabinetView::Overview,
    };
    html!(<PlanLink {to}>{props.text.clone()}</PlanLink>)
}

#[function_component]
pub fn PanelLink(props: &ElementLinkProps) -> Html {
    let to = PlanView::Panel {
        id: props.id,
        view: PanelView::Show,
    };
    html!(<PlanLink {to}>{props.text.clone()}</PlanLink>)
}

#[function_component]
pub fn CableLink(props: &ElementLinkProps) -> Html {
    let to = PlanView::Cable {
        id: props.id,
        view: CableView::Edit,
    };
    html!(<PlanLink {to}>{props.text.clone()}</PlanLink>)
}

#[function_component]
pub fn DuctLink(props: &ElementLinkProps) -> Html {
    let to = PlanView::Duct {
        id: props.id,
        view: DuctView::Show,
    };
    html!(<PlanLink {to}>{props.text.clone()}</PlanLink>)
}
