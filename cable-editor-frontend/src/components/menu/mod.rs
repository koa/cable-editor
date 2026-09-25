use crate::pages::router::AppRoute;
use patternfly_yew::prelude::{Icon, MenuToggle, MenuToggleVariant};
use std::borrow::Cow;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, Node};
use yew::events::{FocusEvent, KeyboardEvent, MouseEvent};
use yew::{Component, Context, ContextHandle, Html, NodeRef, Properties, classes, html};
use yew_nested_router::prelude::RouterContext;

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

/// Dropdown for a breadcrumb item: a PatternFly `MenuToggle` with a `Menu` of router links.
///
/// patternfly-yew's `Dropdown` is not used on purpose: it positions the menu with popper.js in a
/// portal and recreates its popper modifier closure on every render; in the breadcrumb, where a
/// click in the menu navigates and rebuilds the breadcrumb, that produced console errors. A
/// breadcrumb menu always opens downwards, so plain CSS positioning is enough (`.breadcrumb-menu`
/// in `style.scss`).
pub struct MenuDropdown {
    is_open: bool,
    /// Set when the menu was opened, so `rendered` focuses an entry once it exists.
    focus_on_render: bool,
    router: Option<RouterContext<AppRoute>>,
    _router_handle: Option<ContextHandle<RouterContext<AppRoute>>>,
    root_ref: NodeRef,
    toggle_ref: NodeRef,
    menu_ref: NodeRef,
}

pub enum MenuDropdownMsg {
    Toggle,
    FocusOut(FocusEvent),
    KeyDown(KeyboardEvent),
    Navigate(AppRoute),
    RouterChanged(RouterContext<AppRoute>),
}

impl Component for MenuDropdown {
    type Message = MenuDropdownMsg;
    type Properties = MenuDropdownProps;

    fn create(ctx: &Context<Self>) -> Self {
        let (router, router_handle) = ctx
            .link()
            .context::<RouterContext<AppRoute>>(ctx.link().callback(MenuDropdownMsg::RouterChanged))
            .unzip();
        Self {
            is_open: false,
            focus_on_render: false,
            router,
            _router_handle: router_handle,
            root_ref: NodeRef::default(),
            toggle_ref: NodeRef::default(),
            menu_ref: NodeRef::default(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            MenuDropdownMsg::Toggle => {
                self.is_open = !self.is_open;
                self.focus_on_render = self.is_open;
                true
            }
            MenuDropdownMsg::FocusOut(e) => {
                let focus_inside = e
                    .related_target()
                    .and_then(|target| target.dyn_into::<Node>().ok())
                    .zip(self.root_ref.cast::<Node>())
                    .is_some_and(|(target, root)| root.contains(Some(&target)));
                !focus_inside && self.close()
            }
            MenuDropdownMsg::KeyDown(e) => self.on_key_down(&e),
            MenuDropdownMsg::Navigate(target) => {
                if let Some(router) = &self.router {
                    router.push(target);
                }
                self.close()
            }
            MenuDropdownMsg::RouterChanged(router) => {
                self.router = Some(router);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        html! {
            <div
                ref={self.root_ref.clone()}
                class="breadcrumb-menu"
                onfocusout={link.callback(MenuDropdownMsg::FocusOut)}
                onkeydown={link.callback(MenuDropdownMsg::KeyDown)}
            >
                <MenuToggle
                    r#ref={self.toggle_ref.clone()}
                    variant={MenuToggleVariant::Plain}
                    text={html!(ctx.props().title.as_ref())}
                    expanded={self.is_open}
                    ontoggle={link.callback(|()| MenuDropdownMsg::Toggle)}
                />
                if self.is_open {
                    <div ref={self.menu_ref.clone()} class="pf-v6-c-menu pf-m-scrollable">
                        <div class="pf-v6-c-menu__content">
                            <ul class="pf-v6-c-menu__list" role="menu">
                                {for ctx.props().entries.iter().map(|entry| self.view_entry(ctx, entry))}
                            </ul>
                        </div>
                    </div>
                }
            </div>
        }
    }

    fn rendered(&mut self, _ctx: &Context<Self>, _first_render: bool) {
        // Focus the entry of the current page (or the first one), so the arrow keys work right
        // away and leaving the menu with the focus closes it.
        if std::mem::take(&mut self.focus_on_render)
            && let Some(menu) = self.menu_ref.cast::<Element>()
        {
            let item = menu
                .query_selector(".pf-v6-c-menu__item.pf-m-selected")
                .ok()
                .flatten()
                .or_else(|| menu_items(&menu).into_iter().next());
            if let Some(item) = item.and_then(|item| item.dyn_into::<HtmlElement>().ok()) {
                let _ = item.focus();
            }
        }
    }
}

impl MenuDropdown {
    /// Returns whether the view has to be rerendered.
    fn close(&mut self) -> bool {
        std::mem::replace(&mut self.is_open, false)
    }

    fn view_entry(&self, ctx: &Context<Self>, entry: &MenuEntry) -> Html {
        let selected = self
            .router
            .as_ref()
            .is_some_and(|router| router.is_same(&entry.target));
        let mut class = classes!("pf-v6-c-menu__item");
        if selected {
            class.push("pf-m-selected");
        }
        let href = self
            .router
            .as_ref()
            .map(|router| router.render_target(entry.target.clone()));
        let target = entry.target.clone();
        let onclick = ctx.link().batch_callback(move |e: MouseEvent| {
            // Modified clicks keep the browser behaviour (e.g. open in a new tab)
            if e.ctrl_key() || e.meta_key() || e.shift_key() || e.button() != 0 {
                None
            } else {
                e.prevent_default();
                Some(MenuDropdownMsg::Navigate(target.clone()))
            }
        });
        html! {
            <li key={entry.text.as_ref()} class="pf-v6-c-menu__list-item" role="none">
                <a {class} {href} role="menuitem" tabindex="-1" {onclick}>
                    <span class="pf-v6-c-menu__item-main">
                        <span class="pf-v6-c-menu__item-text">{entry.text.as_ref()}</span>
                        if selected {
                            <span class="pf-v6-c-menu__item-select-icon">{Icon::Check}</span>
                        }
                    </span>
                </a>
            </li>
        }
    }

    /// Escape closes the menu and returns the focus to the toggle, the arrow keys, Home and End
    /// move the focus between its entries. Returns whether the view has to be rerendered.
    fn on_key_down(&mut self, e: &KeyboardEvent) -> bool {
        let Some(menu) = self.menu_ref.cast::<Element>().filter(|_| self.is_open) else {
            return false;
        };
        let items = menu_items(&menu);
        let current = gloo_utils::document()
            .active_element()
            .and_then(|focused| items.iter().position(|item| *item == focused));
        let next = match e.key().as_str() {
            "Escape" => {
                e.prevent_default();
                if let Some(toggle) = self.toggle_ref.cast::<HtmlElement>() {
                    let _ = toggle.focus();
                }
                return self.close();
            }
            "ArrowDown" => current.map_or(0, |i| (i + 1) % items.len()),
            "ArrowUp" => current.map_or(items.len().wrapping_sub(1), |i| {
                (i + items.len() - 1) % items.len()
            }),
            "Home" => 0,
            "End" => items.len().wrapping_sub(1),
            _ => return false,
        };
        e.prevent_default();
        if let Some(item) = items
            .get(next)
            .and_then(|item| item.dyn_ref::<HtmlElement>())
        {
            let _ = item.focus();
        }
        false
    }
}

fn menu_items(menu: &Element) -> Vec<Element> {
    let Ok(items) = menu.query_selector_all("[role=menuitem]") else {
        return Vec::new();
    };
    (0..items.length())
        .filter_map(|i| items.item(i))
        .filter_map(|item| item.dyn_into::<Element>().ok())
        .collect()
}
