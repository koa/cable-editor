use crate::pages::router::AppRoute;
use patternfly_yew::prelude::Icon;
use std::borrow::Cow;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, Node};
use yew::events::FocusEvent;
use yew::{
    Callback, Html, Properties, function_component, html, html_nested, use_node_ref, use_state,
};
use yew_nested_router::components::Link;

pub mod list_cabinet;
pub mod list_cable;
pub mod list_panel;
pub mod list_plan;

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

#[function_component(MenuDropdown)]
pub fn menu_dropdown(props: &MenuDropdownProps) -> Html {
    let is_open = use_state(|| false);
    let node_ref = use_node_ref();

    let on_toggle = {
        let is_open = is_open.clone();
        let node_ref = node_ref.clone();
        Callback::from(move |_| {
            let next_state = !*is_open;
            is_open.set(next_state);

            if next_state {
                if let Some(el) = node_ref.cast::<HtmlElement>() {
                    let _ = el.focus();
                }
            }
        })
    };

    let on_focusout = {
        let is_open = is_open.clone();
        let node_ref = node_ref.clone();
        Callback::from(move |e: FocusEvent| {
            let mut focus_inside = false;

            if let Some(related_target) = e.related_target() {
                if let Some(node) = related_target.dyn_ref::<Node>() {
                    if let Some(root) = node_ref.cast::<Node>() {
                        if root.contains(Some(node)) {
                            focus_inside = true;
                        }
                    }
                }
            }

            if !focus_inside {
                is_open.set(false);
            }
        })
    };

    let entries = props.entries.iter().map(|e| {
        let to = e.target.clone();
        let link_text = e.text.clone();
        let is_open = is_open.clone();

        let onclick = Callback::from(move |_| {
            is_open.set(false);
        });

        html_nested! {
            <li key={link_text.to_string()} {onclick} class="pf-v6-c-menu__list-item" role="none">
                <Link<AppRoute> {to} class="pf-v6-c-menu__item">
                    <span class="pf-v6-c-menu__item-main">
                        <span class="pf-v6-c-menu__item-text">{link_text.as_ref()}</span>
                    </span>
                </Link<AppRoute>>
            </li>
        }
    });

    // Keine dicken Inline-Styles mehr! Patternfly macht die Optik, wir nur die Positionierung.
    let menu_style = if *is_open {
        "position: absolute; top: 100%; left: 0; z-index: 9999; min-width: max-content; margin-top: 4px;"
    } else {
        "display: none;"
    };

    html! {
        <div
            ref={node_ref}
            tabindex="-1"
            style="position: relative; display: inline-block; outline: none;"
            onfocusout={on_focusout}
        >
            <button
                class="pf-v6-c-dropdown__toggle pf-m-plain"
                type="button"
                onclick={on_toggle}
                style="display: flex; align-items: center; gap: 4px; background: none; border: none; cursor: pointer; padding: 0; font-family: inherit; font-size: inherit; color: inherit;"
            >
                <span class="pf-v6-c-dropdown__toggle-text">{props.title.to_string()}</span>
                <span class="pf-v6-c-dropdown__toggle-icon" style="font-size: 0.8em; opacity: 0.7;">
                    {Icon::CaretDown}
                </span>
            </button>

            <div style={menu_style} class="pf-v6-c-menu">
                <div class="pf-v6-c-menu__content">
                    <ul class="pf-v6-c-menu__list" role="menu">
                        {for entries}
                    </ul>
                </div>
            </div>
        </div>
    }
}
