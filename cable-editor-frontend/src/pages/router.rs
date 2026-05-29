use patternfly_yew::prelude::{Nav, NavList, NavRouterItem};
use yew::{function_component, html, Callback, Html, Properties};
use yew_nested_router::prelude::{use_router, Target};

#[function_component(Sidebar)]
pub fn sidebar() -> Html {
    html! {
        <Nav>
            <NavList>
                <NavRouterItem<AppRoute> to={AppRoute::ListOfCables}>{"Cables"}</NavRouterItem<AppRoute>>
            </NavList>
        </Nav>
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
    #[default]
    ListOfCables,
}

impl AppRoute {
    pub fn content(self) -> Html {
        match self {
            AppRoute::NotFound => html! {<h1>{"Not Found"}</h1>},
            AppRoute::ListOfCables => html! {<h1>{"List of cables"}</h1>},
        }
    }
}