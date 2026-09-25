use crate::pages::router::AppRoute;
use patternfly_yew::prelude::MenuToggleVariant;
use popup::{MenuGroup, MenuLinkItem, PopupMenu};
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
    /// Entries without a group title, shown first
    #[prop_or_default]
    pub entries: Vec<MenuEntry>,
    /// Titled groups after the entries; empty groups are left out
    #[prop_or_default]
    pub groups: Vec<MenuEntryGroup>,
}

#[derive(PartialEq, Clone)]
pub struct MenuEntry {
    pub text: Box<str>,
    pub target: AppRoute,
}

#[derive(PartialEq, Clone)]
pub struct MenuEntryGroup {
    pub title: &'static str,
    pub entries: Vec<MenuEntry>,
}

/// Divider between the menus of a path inside one breadcrumb item (`.breadcrumb-path`).
#[function_component]
pub fn BreadcrumbDivider() -> Html {
    html! {
        <span class="pf-v6-c-breadcrumb__item-divider breadcrumb-path__divider">
            {patternfly_yew::prelude::Icon::AngleRight}
        </span>
    }
}

/// Dropdown for a breadcrumb item: a `PopupMenu` of router links, the current page marked.
#[function_component]
pub fn MenuDropdown(props: &MenuDropdownProps) -> Html {
    let links = |entries: &[MenuEntry]| {
        entries
            .iter()
            .map(|entry| {
                html! {
                    <MenuLinkItem key={entry.text.as_ref()} to={entry.target.clone()}>
                        {entry.text.as_ref()}
                    </MenuLinkItem>
                }
            })
            .collect::<Html>()
    };
    let has_entries = !props.entries.is_empty();
    let groups = props
        .groups
        .iter()
        .filter(|group| !group.entries.is_empty())
        .enumerate()
        .map(|(i, group)| {
            let divider = has_entries || i > 0;
            html! {
                <MenuGroup title={group.title} {divider}>{links(&group.entries)}</MenuGroup>
            }
        });
    html! {
        <PopupMenu variant={MenuToggleVariant::Plain} text={html!(props.title.as_ref())}>
            if has_entries {
                <MenuGroup>{links(&props.entries)}</MenuGroup>
            }
            {for groups}
        </PopupMenu>
    }
}
