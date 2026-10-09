use crate::{
    components::port_usage_issues::BrokenPortUsageList,
    error::FrontendError,
    graphql::authenticated::current_user::Role,
    pages::router::{AppRoute, PlanView},
};
use patternfly_yew::prelude::{AlertType, Backdropper, Toast, Toaster};
use std::time::Duration;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};
use yew::html::Scope;
use cable_editor_common::UserError;
use std::rc::Rc;
use yew::{BaseComponent, Callback, Html, html};
use yew_nested_router::prelude::RouterContext;
use yew_oauth2::context::OAuth2Context;

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

/// What a toast tells about an error: the message for users, with the technical cause where
/// there is one (for a `FrontendError` its `title`, not the English `Display`; port usages a
/// change would break listed below it).
pub trait ToastText {
    fn toast_text(&self) -> Html;
}

impl ToastText for FrontendError {
    fn toast_text(&self) -> Html {
        match self {
            FrontendError::User(UserError::PortUsagesBroken { usages }) => html! {
                <>
                    <p>{self.title()}</p>
                    <BrokenPortUsageList usages={Rc::from(usages.as_ref())}/>
                </>
            },
            _ => html!(self.title()),
        }
    }
}

impl ToastText for String {
    fn toast_text(&self) -> Html {
        html!(self.clone())
    }
}

impl<T: ToastText + ?Sized> ToastText for &T {
    fn toast_text(&self) -> Html {
        (**self).toast_text()
    }
}

/// Shows an error that shouldn't replace the page, e.g. of a failed request behind a button
/// (the page would lose unsaved input).
pub fn toast_error(
    scope: &Scope<impl BaseComponent>,
    title: impl Into<String>,
    error: impl ToastText,
) {
    if let Some(toaster) = get_toaster(scope) {
        toaster.toast(Toast {
            title: title.into(),
            r#type: AlertType::Danger,
            body: error.toast_text(),
            ..Toast::default()
        });
    }
}

/// Confirms a change that stays on the page, e.g. "Schacht gespeichert".
pub fn toast_success(scope: &Scope<impl BaseComponent>, title: impl Into<String>) {
    if let Some(toaster) = get_toaster(scope) {
        toaster.toast(Toast {
            title: title.into(),
            r#type: AlertType::Success,
            timeout: Some(Duration::from_secs(3)),
            ..Toast::default()
        });
    }
}

/// Opens a view of the plan, e.g. after creating or deleting an object.
pub fn navigate(scope: &Scope<impl BaseComponent>, plan_id: i32, view: PlanView) {
    if let Some((router, _)) = scope.context::<RouterContext<AppRoute>>(Callback::noop()) {
        router.push(AppRoute::Plan { plan_id, view });
    }
}

/// Whether the viewport is at least PatternFly's md breakpoint (48rem), i.e. not a phone.
pub fn is_wide_screen() -> bool {
    gloo_utils::window()
        .inner_width()
        .ok()
        .and_then(|width| width.as_f64())
        .is_some_and(|width| width >= 768.0)
}

/// Saves a file the backend sent in base64, as the browser saves downloads.
pub fn save_file(file_name: &str, base64: &str, mime_type: &str) -> Result<(), FrontendError> {
    save_blob(file_name, base64, mime_type).map_err(FrontendError::SaveFile)
}

fn save_blob(file_name: &str, base64: &str, mime_type: &str) -> Result<(), JsValue> {
    let window = gloo_utils::window();
    // atob gives one character per byte
    let bytes: Vec<u8> = window.atob(base64)?.chars().map(|c| c as u8).collect();
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes.as_slice()));
    let options = BlobPropertyBag::new();
    options.set_type(mime_type);
    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &options)?;
    let url = Url::create_object_url_with_blob(&blob)?;
    let link: HtmlAnchorElement = gloo_utils::document().create_element("a")?.dyn_into()?;
    link.set_href(&url);
    link.set_download(file_name);
    link.click();
    // Firefox may still read it after the click returned
    gloo_timers::callback::Timeout::new(60_000, move || {
        let _ = Url::revoke_object_url(&url);
    })
    .forget();
    Ok(())
}
