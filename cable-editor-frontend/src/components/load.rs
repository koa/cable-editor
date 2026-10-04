//! What a component loads: pending until the answer arrives, then the data or why it failed.
//! Pages show it with `view`, breadcrumb menus with `menu::view_load`; only what stands outside
//! of a page shows it its own way (`App` and `UserProvider` before the router, `NetboxHint`).
use crate::error::FrontendError;
use patternfly_yew::prelude::Spinner;
use yew::{Html, html, html::IntoPropValue};

#[derive(Debug, Default)]
pub enum Load<T> {
    #[default]
    Pending,
    Failed(FrontendError),
    Loaded(T),
}

impl<T> Load<T> {
    pub fn loaded(&self) -> Option<&T> {
        match self {
            Self::Loaded(data) => Some(data),
            Self::Pending | Self::Failed(_) => None,
        }
    }

    pub fn loaded_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Loaded(data) => Some(data),
            Self::Pending | Self::Failed(_) => None,
        }
    }

    /// A spinner while pending, the error as alert, else the content
    pub fn view(&self, content: impl FnOnce(&T) -> Html) -> Html {
        match self {
            Self::Pending => html!(<Spinner/>),
            Self::Failed(error) => error.into_prop_value(),
            Self::Loaded(data) => content(data),
        }
    }
}

impl<T> From<Result<T, FrontendError>> for Load<T> {
    fn from(result: Result<T, FrontendError>) -> Self {
        match result {
            Ok(data) => Self::Loaded(data),
            Err(error) => Self::Failed(error),
        }
    }
}
