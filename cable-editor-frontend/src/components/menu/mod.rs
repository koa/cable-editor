use crate::pages::router::AppRoute;
use patternfly_yew::prelude::MenuToggleVariant;
use popup::{MenuLinkItem, PopupMenu};
use std::borrow::Cow;
use yew::{Html, Properties, function_component, html};

pub mod list_cabinet;
pub mod list_cable;
pub mod list_panel;
pub mod list_plan;
pub mod popup;

#[derive(Properties, PartialEq)]
pub struct MenuDropdownProps {
    pub title: Cow<'static, str>,
    pub entries: Vec<MenuEntry>,
}

#[derive(PartialEq, Clone)]
pub struct MenuEntry {
    pub text: Box<str>,
    pub target: AppRoute,
}

/// Dropdown for a breadcrumb item: a `PopupMenu` of router links, the current page marked.
#[function_component]
pub fn MenuDropdown(props: &MenuDropdownProps) -> Html {
    html! {
        <PopupMenu variant={MenuToggleVariant::Plain} text={html!(props.title.as_ref())}>
            {for props.entries.iter().map(|entry| html! {
                <MenuLinkItem key={entry.text.as_ref()} to={entry.target.clone()}>
                    {entry.text.as_ref()}
                </MenuLinkItem>
            })}
        </PopupMenu>
    }
}
