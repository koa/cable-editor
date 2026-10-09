//! Links to elements shown on a page, each to the view that fits best: a Schacht to its overview,
//! a panel to its connection overview, a cable or a duct to its page (its only view), a type of
//! Schacht to its page, an owner (who has no page) to the list of owners; a plan is
//! `plan_link::PlanNameLink`.
use crate::{
    components::plan_link::{PlanLink, PlanViewLink},
    pages::router::{CabinetView, CableView, DuctView, PanelView, PlanView},
};
use yew::{AttrValue, Html, Properties, function_component, html};

#[derive(Properties, PartialEq)]
pub struct ElementLinkProps {
    pub id: i32,
    /// Shown text, the element's name
    pub text: AttrValue,
    /// The plan to show it in, if not the one of the current page
    #[prop_or_default]
    pub plan_id: Option<i32>,
}

fn element_link(to: PlanView, props: &ElementLinkProps) -> Html {
    match props.plan_id {
        Some(plan_id) => html!(<PlanViewLink {plan_id} {to}>{props.text.clone()}</PlanViewLink>),
        None => html!(<PlanLink {to}>{props.text.clone()}</PlanLink>),
    }
}

#[function_component]
pub fn SchachtLink(props: &ElementLinkProps) -> Html {
    let to = PlanView::Cabinet {
        id: props.id,
        view: CabinetView::Overview,
    };
    element_link(to, props)
}

#[function_component]
pub fn PanelLink(props: &ElementLinkProps) -> Html {
    let to = PlanView::Panel {
        id: props.id,
        view: PanelView::Show,
    };
    element_link(to, props)
}

#[function_component]
pub fn CableLink(props: &ElementLinkProps) -> Html {
    let to = PlanView::Cable {
        id: props.id,
        view: CableView::Edit,
    };
    element_link(to, props)
}

#[function_component]
pub fn DuctLink(props: &ElementLinkProps) -> Html {
    let to = PlanView::Duct {
        id: props.id,
        view: DuctView::Show,
    };
    element_link(to, props)
}

#[function_component]
pub fn SchachtTypLink(props: &ElementLinkProps) -> Html {
    let to = PlanView::CabinetType { id: props.id };
    element_link(to, props)
}

#[derive(Properties, PartialEq)]
pub struct OwnerLinkProps {
    /// Shown text, the owner's name
    pub text: AttrValue,
}

#[function_component]
pub fn OwnerLink(props: &OwnerLinkProps) -> Html {
    html!(<PlanLink to={PlanView::ListOfOwners}>{props.text.clone()}</PlanLink>)
}
