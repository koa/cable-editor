pub mod messages;

use crate::components::recovery::ErrorRecovery;
use cable_editor_common::UserError;
use cynic::http::CynicReqwestError;
use patternfly_yew::prelude::{Alert, AlertType};
use reqwest::header::InvalidHeaderValue;
use thiserror::Error;
use wasm_bindgen::{JsCast, JsValue};
use yew::{Html, html, html::IntoPropValue};

#[derive(Error, Debug)]
pub enum FrontendError {
    #[error("Error connecting to anonymous GraphQL endpoint: {0}")]
    ErrorQueryingAnonymousConnect(reqwest::Error),
    #[error("Error querying anonymous GraphQL endpoint: {0}")]
    ErrorQueryingAnonymousTransfer(CynicReqwestError),
    #[error("Error connecting to authenticated GraphQL endpoint: {0}")]
    ErrorQueryingAuthenticatedConnect(reqwest::Error),
    #[error("Error querying authenticated GraphQL endpoint: {0}")]
    ErrorQueryingAuthenticatedTransfer(CynicReqwestError),
    #[error("Invalid http header: {0}")]
    InvalidHeader(#[from] InvalidHeaderValue),
    /// The backend refused the request, worded in `messages`
    #[error("{}", messages::user_error(.0))]
    User(UserError),
    /// A technical error of the backend (database, Netbox), its messages
    #[error("Unerwarteter Fehler vom Server: {}", .0.join("; "))]
    Graphql(Vec<String>),
    #[error("Plan not found: {0}")]
    PlanNotFound(i32),
    #[error("Expected data not found")]
    NotFound,
    #[error("Cannot determine the address of the server")]
    NoServerAddress,
    #[error("The panels of the Schacht don't form a tree")]
    InvalidPanelTree,
    #[error("Printer error: {0}")]
    Printer(#[from] brady_web_sdk::Error),
    #[error("Printer not connected")]
    PrinterDisconnected,
    #[error("Printer did not report its supply")]
    PrinterNoSupply,
    #[error("Unsupported tape, only continuous tape is supported")]
    UnsupportedTape,
    #[error("Map error: {0:?}")]
    Map(JsValue),
    /// The browser didn't save a downloaded file
    #[error("Datei konnte nicht gespeichert werden: {}", js_message(.0))]
    SaveFile(JsValue),
}

impl FrontendError {
    /// The message, with the technical cause where there is one.
    pub fn title(&self) -> String {
        match self {
            FrontendError::ErrorQueryingAnonymousConnect(e) => {
                format!("Fehler beim anyonymen Verbindungsaufbau: {e}")
            }
            FrontendError::ErrorQueryingAnonymousTransfer(e) => {
                format!("Fehler bei einer anonymen Abfrage: {e}")
            }
            FrontendError::ErrorQueryingAuthenticatedConnect(e) => {
                format!("Fehler beim authentisierten Verbindungsaufbau: {e}")
            }
            FrontendError::ErrorQueryingAuthenticatedTransfer(e) => {
                format!("Fehler bei einer authentisierten Abfrage: {e}")
            }
            FrontendError::InvalidHeader(e) => format!("Ungültiger Header: {e}"),
            FrontendError::User(error) => messages::user_error(error),
            FrontendError::Graphql(_) => "Unerwarteter Fehler vom Server".to_string(),
            FrontendError::PlanNotFound(id) => format!("Plan {id} existiert nicht"),
            FrontendError::NotFound => "Daten nicht gefunden".to_string(),
            FrontendError::NoServerAddress => {
                "Adresse des Servers konnte nicht bestimmt werden".to_string()
            }
            FrontendError::InvalidPanelTree => {
                "Die Panels des Schachts bilden keinen gültigen Baum".to_string()
            }
            FrontendError::Printer(e) => format!("Druckfehler: {e}"),
            FrontendError::PrinterDisconnected => "Drucker nicht verbunden".to_string(),
            FrontendError::PrinterNoSupply => "Drucker hat kein Etikett gemeldet".to_string(),
            FrontendError::UnsupportedTape => {
                "Etikettentyp wird nicht unterstützt (nur Endlosband)".to_string()
            }
            FrontendError::Map(e) => {
                format!("Karte konnte nicht angezeigt werden: {}", js_message(e))
            }
            FrontendError::SaveFile(e) => {
                format!("Datei konnte nicht gespeichert werden: {}", js_message(e))
            }
        }
    }

    /// Further details, the messages of an unexpected server error.
    pub fn details(&self) -> &[String] {
        match self {
            FrontendError::Graphql(details) => details,
            _ => &[],
        }
    }
}

impl IntoPropValue<Html> for &FrontendError {
    fn into_prop_value(self) -> Html {
        let title = self.title();
        let mut children = Vec::new();
        if !self.details().is_empty() {
            children.push(html! {
                <ul>
                    {for self.details().iter().map(|detail| html!(<li>{detail.as_str()}</li>))}
                </ul>
            });
        }
        // The connection failed, or the server did (possibly a new version with another schema)
        let retry = matches!(
            self,
            FrontendError::ErrorQueryingAnonymousConnect(_)
                | FrontendError::ErrorQueryingAnonymousTransfer(_)
                | FrontendError::ErrorQueryingAuthenticatedConnect(_)
                | FrontendError::ErrorQueryingAuthenticatedTransfer(_)
                | FrontendError::Graphql(_)
        );
        let reload = matches!(self, FrontendError::Graphql(_));
        if retry || reload {
            children.push(html!(<ErrorRecovery {retry} {reload}/>));
        }
        // Any child, even an empty one, adds the space of a description
        if children.is_empty() {
            html!(<Alert inline=true {title} r#type={AlertType::Danger}/>)
        } else {
            html!(<Alert inline=true {title} r#type={AlertType::Danger}>{for children}</Alert>)
        }
    }
}

/// Message of an error thrown by JavaScript: an `Error`'s message, a thrown string itself.
fn js_message(value: &JsValue) -> String {
    if let Some(error) = value.dyn_ref::<js_sys::Error>() {
        String::from(error.message())
    } else if let Some(text) = value.as_string() {
        text
    } else {
        format!("{value:?}")
    }
}
