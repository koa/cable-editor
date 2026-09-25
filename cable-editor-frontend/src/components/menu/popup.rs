use crate::pages::router::AppRoute;
use patternfly_yew::prelude::{Icon, MenuToggle, MenuToggleVariant};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, Node};
use yew::events::{FocusEvent, KeyboardEvent, MouseEvent};
use yew::{
    AttrValue, Callback, Component, Context, ContextProvider, Html, NodeRef, Properties, classes,
    function_component, html, use_context,
};
use yew_nested_router::prelude::use_router;

#[derive(Properties, PartialEq)]
pub struct PopupMenuProps {
    #[prop_or_default]
    pub text: Option<Html>,
    #[prop_or_default]
    pub icon: Option<Html>,
    #[prop_or_default]
    pub aria_label: AttrValue,
    #[prop_or_default]
    pub variant: MenuToggleVariant,
    #[prop_or_default]
    pub disabled: bool,
    /// Align the menu with the end of the toggle, e.g. for a kebab at the end of a table row
    #[prop_or_default]
    pub align_end: bool,
    /// `MenuGroup`s of `MenuLinkItem`s and `MenuActionItem`s
    #[prop_or_default]
    pub children: Html,
}

/// Closes the surrounding `PopupMenu`, provided to its items.
#[derive(Clone, PartialEq)]
struct CloseMenu(Callback<()>);

/// A PatternFly `MenuToggle` with a `Menu` below it, used instead of patternfly-yew's `Dropdown`.
///
/// That one positions the menu with popper.js in a portal and recreates its popper modifier
/// closure on every render, which logged errors to the console when the menu was opened or a
/// click in it rebuilt the surrounding page (breadcrumb, "Loops verbinden"). A dropdown menu
/// opens downwards, so plain CSS positioning is enough (`.popup-menu` in `style.scss`).
pub struct PopupMenu {
    is_open: bool,
    /// Set when the menu was opened, so `rendered` focuses an entry once it exists.
    focus_on_render: bool,
    root_ref: NodeRef,
    toggle_ref: NodeRef,
    menu_ref: NodeRef,
}

pub enum PopupMenuMsg {
    Toggle,
    Close,
    FocusOut(FocusEvent),
    KeyDown(KeyboardEvent),
}

impl Component for PopupMenu {
    type Message = PopupMenuMsg;
    type Properties = PopupMenuProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            is_open: false,
            focus_on_render: false,
            root_ref: NodeRef::default(),
            toggle_ref: NodeRef::default(),
            menu_ref: NodeRef::default(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            PopupMenuMsg::Toggle => {
                self.is_open = !self.is_open;
                self.focus_on_render = self.is_open;
                true
            }
            PopupMenuMsg::Close => self.close(),
            PopupMenuMsg::FocusOut(e) => {
                let focus_inside = e
                    .related_target()
                    .and_then(|target| target.dyn_into::<Node>().ok())
                    .zip(self.root_ref.cast::<Node>())
                    .is_some_and(|(target, root)| root.contains(Some(&target)));
                !focus_inside && self.close()
            }
            PopupMenuMsg::KeyDown(e) => self.on_key_down(&e),
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let props = ctx.props();
        let mut class = classes!("popup-menu");
        if props.align_end {
            class.push("popup-menu--end");
        }
        let close = CloseMenu(link.callback(|()| PopupMenuMsg::Close));
        html! {
            <div
                ref={self.root_ref.clone()}
                {class}
                onfocusout={link.callback(PopupMenuMsg::FocusOut)}
                onkeydown={link.callback(PopupMenuMsg::KeyDown)}
            >
                <MenuToggle
                    r#ref={self.toggle_ref.clone()}
                    variant={props.variant}
                    text={props.text.clone()}
                    icon={props.icon.clone()}
                    aria_label={props.aria_label.clone()}
                    disabled={props.disabled}
                    expanded={self.is_open}
                    ontoggle={link.callback(|()| PopupMenuMsg::Toggle)}
                />
                if self.is_open {
                    <div ref={self.menu_ref.clone()} class="pf-v6-c-menu pf-m-scrollable">
                        <div class="pf-v6-c-menu__content">
                            <ContextProvider<CloseMenu> context={close}>
                                {props.children.clone()}
                            </ContextProvider<CloseMenu>>
                        </div>
                    </div>
                }
            </div>
        }
    }

    fn rendered(&mut self, _ctx: &Context<Self>, _first_render: bool) {
        // Focus the selected entry (or the first one), so the arrow keys work right away and
        // leaving the menu with the focus closes it.
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

impl PopupMenu {
    /// Returns whether the view has to be rerendered.
    fn close(&mut self) -> bool {
        std::mem::replace(&mut self.is_open, false)
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

#[derive(Properties, PartialEq)]
pub struct MenuGroupProps {
    #[prop_or_default]
    pub title: Option<AttrValue>,
    /// Separate the group from the one before by a divider
    #[prop_or_default]
    pub divider: bool,
    /// `MenuLinkItem`s and `MenuActionItem`s
    #[prop_or_default]
    pub children: Html,
}

/// A list of entries in a `PopupMenu`, optionally with a title.
#[function_component]
pub fn MenuGroup(props: &MenuGroupProps) -> Html {
    html! {
        <>
            if props.divider {
                <hr class="pf-v6-c-divider"/>
            }
            <section class="pf-v6-c-menu__group">
                if let Some(title) = &props.title {
                    <h1 class="pf-v6-c-menu__group-title">{title.clone()}</h1>
                }
                <ul class="pf-v6-c-menu__list" role="menu">{props.children.clone()}</ul>
            </section>
        </>
    }
}

fn item_main(selected: bool, children: &Html) -> Html {
    html! {
        <span class="pf-v6-c-menu__item-main">
            <span class="pf-v6-c-menu__item-text">{children.clone()}</span>
            if selected {
                <span class="pf-v6-c-menu__item-select-icon">{Icon::Check}</span>
            }
        </span>
    }
}

fn item_class(selected: bool) -> yew::Classes {
    let mut class = classes!("pf-v6-c-menu__item");
    if selected {
        class.push("pf-m-selected");
    }
    class
}

#[derive(Properties, PartialEq)]
pub struct MenuLinkItemProps {
    pub to: AppRoute,
    #[prop_or_default]
    pub children: Html,
}

/// Entry of a `PopupMenu` navigating to `to`; marked as selected when that is the current page.
#[function_component]
pub fn MenuLinkItem(props: &MenuLinkItemProps) -> Html {
    let router = use_router::<AppRoute>();
    let close = use_context::<CloseMenu>();
    let selected = router.as_ref().is_some_and(|r| r.is_same(&props.to));
    let href = router.as_ref().map(|r| r.render_target(props.to.clone()));
    let onclick = {
        let to = props.to.clone();
        Callback::from(move |e: MouseEvent| {
            // Modified clicks keep the browser behaviour (e.g. open in a new tab)
            if e.ctrl_key() || e.meta_key() || e.shift_key() || e.button() != 0 {
                return;
            }
            e.prevent_default();
            if let Some(CloseMenu(close)) = &close {
                close.emit(());
            }
            if let Some(router) = &router {
                router.push(to.clone());
            }
        })
    };
    html! {
        <li class="pf-v6-c-menu__list-item" role="none">
            <a class={item_class(selected)} {href} role="menuitem" tabindex="-1" {onclick}>
                {item_main(selected, &props.children)}
            </a>
        </li>
    }
}

#[derive(Properties, PartialEq)]
pub struct MenuActionItemProps {
    pub onclick: Callback<()>,
    #[prop_or_default]
    pub selected: bool,
    #[prop_or_default]
    pub children: Html,
}

/// Entry of a `PopupMenu` running `onclick` and closing the menu.
#[function_component]
pub fn MenuActionItem(props: &MenuActionItemProps) -> Html {
    let close = use_context::<CloseMenu>();
    let onclick = {
        let onclick = props.onclick.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(CloseMenu(close)) = &close {
                close.emit(());
            }
            onclick.emit(());
        })
    };
    html! {
        <li class="pf-v6-c-menu__list-item" role="none">
            <button class={item_class(props.selected)} type="button" role="menuitem" tabindex="-1" {onclick}>
                {item_main(props.selected, &props.children)}
            </button>
        </li>
    }
}
