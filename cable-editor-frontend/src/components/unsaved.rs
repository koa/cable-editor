//! The warning about unsaved changes (docs/frontend-konventionen.md, section 5).
//!
//! yew-nested-router can't hold back a change of page: `Link`, `push` and `replace` navigate at
//! once, and `popstate` (back in the browser) can't be prevented. So `UnsavedGuard` around the
//! router watches for the ways out of the app itself:
//! - a click on a link within the app is caught before the link sees it (capture phase on the
//!   document) and asked about first,
//! - `popstate` is caught before the router sees it, the address moves back to the page, and the
//!   question follows,
//! - closing or reloading the window gets the browser's own warning (`beforeunload`).
//!
//! A page with changes that aren't stored yet holds an `Unsaved` and says so after every render.
//! Navigation a page starts itself (`util::navigate`, after saving or deleting) isn't asked about.
use gloo_events::{EventListener, EventListenerOptions, EventListenerPhase};
use patternfly_yew::prelude::{
    Backdrop, Backdropper, Bullseye, Button, ButtonVariant, Modal, ModalVariant,
};
use std::{cell::Cell, cell::RefCell, rc::Rc};
use wasm_bindgen::JsCast;
use web_sys::{
    BeforeUnloadEvent, Element, Event, HtmlAnchorElement, HtmlElement, MouseEvent, PopStateEvent,
};
use yew::{
    BaseComponent, Callback, Children, Component, Context, ContextProvider, Html, Properties,
    function_component, html, html::Scope,
};

use crate::util::get_backdrop;

#[derive(Default)]
struct State {
    /// How many `Unsaved` hold changes
    unsaved: Cell<usize>,
    /// Set while a discarded page is left, so the navigation isn't asked about again
    leaving: Cell<bool>,
    /// The address of the page holding changes, to move back to when the browser went elsewhere
    page_url: RefCell<String>,
}

impl State {
    fn has_unsaved(&self) -> bool {
        self.unsaved.get() > 0 && !self.leaving.get()
    }
}

#[derive(Clone)]
struct UnsavedContext(Rc<State>);

impl PartialEq for UnsavedContext {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Held by a page for its unsaved changes. Dropping it, e.g. with the page, means none.
#[derive(Default)]
pub struct Unsaved {
    state: Option<Rc<State>>,
    has_changes: Cell<bool>,
}

impl Unsaved {
    pub fn new(scope: &Scope<impl BaseComponent>) -> Self {
        Self {
            state: scope
                .context::<UnsavedContext>(Callback::noop())
                .map(|(context, _)| context.0),
            has_changes: Cell::new(false),
        }
    }

    /// Tells whether the page holds changes that aren't stored; to call after every render.
    pub fn set(&self, has_changes: bool) {
        let Some(state) = &self.state else {
            return;
        };
        if has_changes {
            *state.page_url.borrow_mut() =
                gloo_utils::window().location().href().unwrap_or_default();
        }
        if has_changes != self.has_changes.replace(has_changes) {
            let count = state.unsaved.get();
            state.unsaved.set(if has_changes {
                count + 1
            } else {
                count.saturating_sub(1)
            });
        }
    }
}

impl std::fmt::Debug for Unsaved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Unsaved")
            .field(&self.has_changes.get())
            .finish()
    }
}

impl Drop for Unsaved {
    fn drop(&mut self) {
        self.set(false);
    }
}

#[derive(Properties, PartialEq)]
pub struct UnsavedGuardProps {
    pub children: Children,
}

/// Provides `Unsaved` to the pages below it and asks before they are left with changes.
pub struct UnsavedGuard {
    state: Rc<State>,
    _listeners: Vec<EventListener>,
}

impl Component for UnsavedGuard {
    type Message = ();
    type Properties = UnsavedGuardProps;

    fn create(ctx: &Context<Self>) -> Self {
        let state = Rc::new(State::default());
        let mut listeners = Vec::new();
        if let Some(backdropper) = get_backdrop(ctx.link()) {
            let capture = EventListenerOptions {
                phase: EventListenerPhase::Capture,
                passive: false,
            };
            listeners.push(EventListener::new_with_options(
                &gloo_utils::document(),
                "click",
                capture,
                {
                    let state = state.clone();
                    let backdropper = backdropper.clone();
                    move |event| on_click(&state, &backdropper, event)
                },
            ));
            listeners.push(EventListener::new_with_options(
                &gloo_utils::window(),
                "popstate",
                capture,
                {
                    let state = state.clone();
                    move |event| on_pop_state(&state, &backdropper, event)
                },
            ));
        }
        listeners.push(EventListener::new_with_options(
            &gloo_utils::window(),
            "beforeunload",
            EventListenerOptions::enable_prevent_default(),
            {
                let state = state.clone();
                move |event| {
                    if state.has_unsaved() {
                        event.prevent_default();
                        if let Some(event) = event.dyn_ref::<BeforeUnloadEvent>() {
                            // Older browsers show their question only with a return value
                            event.set_return_value("");
                        }
                    }
                }
            },
        ));
        Self {
            state,
            _listeners: listeners,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <ContextProvider<UnsavedContext> context={UnsavedContext(self.state.clone())}>
                {ctx.props().children.clone()}
            </ContextProvider<UnsavedContext>>
        }
    }
}

fn on_click(state: &Rc<State>, backdropper: &Backdropper, event: &Event) {
    if !state.has_unsaved() {
        return;
    }
    let Some(click) = event.dyn_ref::<MouseEvent>() else {
        return;
    };
    // Other buttons and modifiers open the link elsewhere, this page stays
    if click.button() != 0
        || click.ctrl_key()
        || click.meta_key()
        || click.shift_key()
        || click.alt_key()
    {
        return;
    }
    let Some(anchor) = event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())
        .and_then(|element| element.closest("a[href]").ok().flatten())
        .and_then(|element| element.dyn_into::<HtmlAnchorElement>().ok())
    else {
        return;
    };
    let location = gloo_utils::window().location();
    let leaves_page = anchor.origin() == location.origin().unwrap_or_default()
        && (anchor.pathname() != location.pathname().unwrap_or_default()
            || anchor.search() != location.search().unwrap_or_default());
    // Downloads and links to other sites don't leave the app, the latter get the browser's question
    if !leaves_page || anchor.has_attribute("download") || anchor.target() == "_blank" {
        return;
    }
    event.prevent_default();
    event.stop_propagation();
    let url = anchor.href();
    ask(state, backdropper, move || go_to(&url, false));
}

fn on_pop_state(state: &Rc<State>, backdropper: &Backdropper, event: &Event) {
    if !state.has_unsaved() || event.dyn_ref::<PopStateEvent>().is_none() {
        return;
    }
    // Before the router sees it: the page stays as it is, and so does its address
    event.stop_immediate_propagation();
    let window = gloo_utils::window();
    let url = window.location().href().unwrap_or_default();
    let page_url = state.page_url.borrow().clone();
    // The browser already went to `url`: the page's address comes back as a new entry
    let _ = gloo_utils::history().push_state_with_url(
        &wasm_bindgen::JsValue::NULL,
        "",
        Some(&page_url),
    );
    ask(state, backdropper, move || go_to(&url, true));
}

/// Opens `url` as the router does when the browser goes there: the address changes, `popstate`
/// tells the router to read it.
fn go_to(url: &str, replace: bool) {
    let history = gloo_utils::history();
    let null = wasm_bindgen::JsValue::NULL;
    let _ = if replace {
        history.replace_state_with_url(&null, "", Some(url))
    } else {
        history.push_state_with_url(&null, "", Some(url))
    };
    if let Ok(event) = PopStateEvent::new("popstate") {
        let _ = gloo_utils::window().dispatch_event(&event);
    }
}

/// Asks whether to drop the changes and calls `leave` if so.
fn ask(state: &Rc<State>, backdropper: &Backdropper, leave: impl Fn() + 'static) {
    let on_discard = {
        let state = state.clone();
        let backdropper = backdropper.clone();
        Callback::from(move |()| {
            backdropper.close();
            state.leaving.set(true);
            leave();
            state.leaving.set(false);
        })
    };
    let on_keep = {
        let backdropper = backdropper.clone();
        Callback::from(move |()| backdropper.close())
    };
    // The click never reached an open menu, losing the focus closes it
    if let Some(element) = gloo_utils::document()
        .active_element()
        .and_then(|element| element.dyn_into::<HtmlElement>().ok())
    {
        let _ = element.blur();
    }
    backdropper.open(Backdrop::new(html! {
        <UnsavedDialog {on_discard} {on_keep}/>
    }));
}

#[derive(Properties, PartialEq)]
struct UnsavedDialogProps {
    on_discard: Callback<()>,
    on_keep: Callback<()>,
}

#[function_component]
fn UnsavedDialog(props: &UnsavedDialogProps) -> Html {
    let footer = html! {
        <>
            <Button
                label="Verwerfen"
                onclick={props.on_discard.reform(|_| ())}
                variant={ButtonVariant::Danger}
            />
            <Button
                label="Weiter bearbeiten"
                onclick={props.on_keep.reform(|_| ())}
                variant={ButtonVariant::Link}
            />
        </>
    };
    html! {
        <Bullseye>
            <Modal title="Änderungen verwerfen?" variant={ModalVariant::Small} {footer}>
                <p>{"Die Änderungen auf dieser Seite sind nicht gespeichert und gehen verloren."}</p>
            </Modal>
        </Bullseye>
    }
}
