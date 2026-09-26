use crate::{
    graphql::authenticated::current_user::Role,
    pages::router::{AppRoute, PlanView},
};
use patternfly_yew::prelude::{AlertType, Backdropper, Toast, Toaster};
use std::{fmt::Display, time::Duration};
use yew::html::Scope;
use yew::{BaseComponent, Callback, html};
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

/// Shows an error that shouldn't replace the page, e.g. of a failed request behind a button
/// (the page would lose unsaved input).
pub fn toast_error(
    scope: &Scope<impl BaseComponent>,
    title: impl Into<String>,
    error: impl Display,
) {
    if let Some(toaster) = get_toaster(scope) {
        toaster.toast(Toast {
            title: title.into(),
            r#type: AlertType::Danger,
            body: html!(error.to_string()),
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
