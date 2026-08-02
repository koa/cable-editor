use log::debug;
use web_sys::{Element, window};
use yew::html::Scope;
use yew::{AppHandle, BaseComponent, Callback, Context};
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
            debug!("Destroy handle");
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
