use crate::{
    components::recovery::Recovery, graphql::authenticated::current_user::Role,
    pages::router::AppRoute,
};
use patternfly_yew::prelude::{Icon, MenuToggleVariant};
use popup::{MenuActionItem, MenuGroup, MenuLinkItem, PopupMenu};
use std::borrow::Cow;
use yew::{AttrValue, Html, Properties, function_component, html, use_context};

pub mod list_cabinet;
pub mod list_cabinet_type;
pub mod list_cable;
pub mod list_duct;
pub mod list_panel;
pub mod list_plan;
pub mod popup;

#[derive(Properties, PartialEq)]
pub struct MenuDropdownProps {
    pub title: Cow<'static, str>,
    /// Entries without a group title, shown first
    #[prop_or_default]
    pub entries: Box<[MenuEntry]>,
    /// Titled groups after the entries; empty groups are left out
    #[prop_or_default]
    pub groups: Box<[MenuEntryGroup]>,
}

#[derive(PartialEq, Clone)]
pub struct MenuEntry {
    /// Marked as selected also when `target` is not the current page: the entry the dropdown
    /// shows as its title (e.g. the area or the panel the current page lies in)
    pub selected: bool,
    pub text: Box<str>,
    pub target: AppRoute,
}

#[derive(PartialEq, Clone)]
pub struct MenuEntryGroup {
    pub title: &'static str,
    pub entries: Box<[MenuEntry]>,
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
/// Leaves out the pages the user may not open (`AppRoute::required_role`).
#[function_component]
pub fn MenuDropdown(props: &MenuDropdownProps) -> Html {
    let role = use_context::<Role>().unwrap_or(Role::Reader);
    let allowed = |entry: &&MenuEntry| entry.target.required_role() <= role;
    let links = |entries: &[MenuEntry]| {
        entries
            .iter()
            .filter(allowed)
            .map(|entry| {
                html! {
                    <MenuLinkItem
                        key={entry.text.as_ref()}
                        to={entry.target.clone()}
                        selected={entry.selected}
                    >
                        {entry.text.as_ref()}
                    </MenuLinkItem>
                }
            })
            .collect::<Html>()
    };
    let has_entries = props.entries.iter().any(|entry| allowed(&entry));
    let groups = props
        .groups
        .iter()
        .filter(|group| group.entries.iter().any(|entry| allowed(&entry)))
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

#[derive(Properties, PartialEq)]
pub struct MenuErrorProps {
    /// `FrontendError::title`
    pub title: AttrValue,
    /// `FrontendError::details`
    #[prop_or_default]
    pub details: Box<[String]>,
}

/// Breadcrumb item whose menu couldn't be loaded: instead of an alert, which doesn't fit into
/// the breadcrumb, a menu with the error and an entry to load breadcrumb and page anew, so the
/// way on isn't lost.
#[function_component]
pub fn MenuError(props: &MenuErrorProps) -> Html {
    let retry = match use_context::<Recovery>() {
        Some(Recovery::Retry(retry)) => Some(retry),
        _ => None,
    };
    html! {
        <PopupMenu
            variant={MenuToggleVariant::Plain}
            icon={html!(<span class="menu-error__icon">{Icon::ExclamationCircle}</span>)}
            text={html!("Nicht geladen")}
        >
            <div class="menu-error__message">
                <p>{props.title.clone()}</p>
                if !props.details.is_empty() {
                    <ul>{for props.details.iter().map(|detail| html!(<li>{detail}</li>))}</ul>
                }
            </div>
            if let Some(retry) = retry {
                <MenuGroup divider=true>
                    <MenuActionItem onclick={retry}>{"Erneut laden"}</MenuActionItem>
                </MenuGroup>
            }
        </PopupMenu>
    }
}

impl MenuErrorProps {
    pub fn from_error(error: &crate::error::FrontendError) -> Self {
        Self {
            title: error.title().into(),
            details: error.details(),
        }
    }
}
