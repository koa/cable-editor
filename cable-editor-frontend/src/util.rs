use crate::graphql::authenticated::current_user::Role;
use patternfly_yew::prelude::{Backdropper, Toaster};
use web_sys::{Element, window};
use yew::html::Scope;
use yew::{AppHandle, BaseComponent, Callback};
use yew_oauth2::context::OAuth2Context;

#[derive(Debug)]
pub struct GuardAppHandle<C: BaseComponent + 'static>(Option<AppHandle<C>>);
impl<C: BaseComponent + 'static> From<AppHandle<C>> for GuardAppHandle<C> {
    fn from(value: AppHandle<C>) -> Self {
        GuardAppHandle(Some(value))
    }
}
impl<C: BaseComponent + 'static> Drop for GuardAppHandle<C> {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.destroy();
        }
    }
}
pub fn render_component<COMP>(props: COMP::Properties) -> (GuardAppHandle<COMP>, Element)
where
    COMP: BaseComponent + 'static,
{
    let div_container: Element = window()
        .expect("Missing Window")
        .document()
        .expect("Missing Document")
        .create_element("div")
        .expect("Can't create div");
    let guard: GuardAppHandle<_> =
        yew::Renderer::<COMP>::with_root_and_props(div_container.clone(), props)
            .render()
            .into();
    (guard, div_container)
}

pub fn get_credentials(scope: &Scope<impl BaseComponent>) -> Option<OAuth2Context> {
    scope
        .context::<OAuth2Context>(Callback::noop())
        .map(|(c, _)| c)
}
pub fn get_backdrop(scope: &Scope<impl BaseComponent>) -> Option<Backdropper> {
    scope
        .context::<Backdropper>(Callback::noop())
        .map(|(c, _)| c)
}
/// Role of the logged in user (`components::user::UserProvider`), `Reader` outside of it.
pub fn get_role(scope: &Scope<impl BaseComponent>) -> Role {
    scope
        .context::<Role>(Callback::noop())
        .map_or(Role::Reader, |(role, _)| role)
}
/// For errors that shouldn't replace the page, e.g. of a failed request behind a button.
pub fn get_toaster(scope: &Scope<impl BaseComponent>) -> Option<Toaster> {
    scope.context::<Toaster>(Callback::noop()).map(|(c, _)| c)
}

/// Whether the viewport is at least PatternFly's md breakpoint (48rem), i.e. not a phone.
pub fn is_wide_screen() -> bool {
    gloo_utils::window()
        .inner_width()
        .ok()
        .and_then(|width| width.as_f64())
        .is_some_and(|width| width >= 768.0)
}
