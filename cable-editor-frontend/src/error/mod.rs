pub mod messages;

use crate::components::recovery::ErrorRecovery;
use cable_editor_common::{ErrorOrigin, ObjectKind, UserError};
use cynic::http::CynicReqwestError;
use patternfly_yew::prelude::{Alert, AlertType};
use reqwest::header::InvalidHeaderValue;
use thiserror::Error;
use wasm_bindgen::{JsCast, JsValue};
use yew::{Html, html, html::IntoPropValue};

/// An error of a GraphQL response that isn't a refusal.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerError {
    pub message: Box<str>,
    /// Where it came from, if the backend knows (errors of its resolvers)
    pub origin: Option<ErrorOrigin>,
}

impl ServerError {
    /// The message with its origin, for those who can tell what it means.
    pub fn detail(&self) -> String {
        match &self.origin {
            Some(ErrorOrigin {
                library,
                location,
                id,
            }) => format!("{} ({library}, {location}, Fehler-ID {id})", self.message),
            None => self.message.to_string(),
        }
    }
}

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
    #[error("Unerwarteter Fehler vom Server: {}", .0.iter().map(ServerError::detail).collect::<Vec<_>>().join("; "))]
    Graphql(Box<[ServerError]>),
    /// The data of a response doesn't fit the query (e.g. a new version changed the schema)
    #[error("Invalid response of the server: {0}")]
    InvalidResponse(serde_json::Error),
    /// The response has neither data nor errors
    #[error("Empty response of the server")]
    EmptyResponse,
    /// An object the page asked for doesn't exist (the backend's refusal for the same reason
    /// is `User(UserError::NotFound)`, worded the same)
    #[error("{}", messages::not_found(*.kind, *.id))]
    NotFound { kind: ObjectKind, id: i64 },
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
    /// The browser lacks something the page needs, e.g. the document
    #[error("The browser lacks {0:?}")]
    BrowserMissing(BrowserPart),
    /// The browser refused a call, e.g. to the canvas
    #[error("Browser error: {0:?}")]
    Browser(JsValue),
    /// The browser didn't save a downloaded file
    #[error("Datei konnte nicht gespeichert werden: {}", js_message(.0))]
    SaveFile(JsValue),
}

/// What the browser has to offer for a page, worded in [`FrontendError::title`]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserPart {
    Document,
    CanvasContext,
}

impl BrowserPart {
    fn name(self) -> &'static str {
        match self {
            BrowserPart::Document => "ein Dokument",
            BrowserPart::CanvasContext => "einen 2D-Zeichenbereich",
        }
    }
}

impl FrontendError {
    /// The object `kind` with `id` doesn't exist.
    pub fn not_found(kind: ObjectKind, id: i32) -> Self {
        FrontendError::NotFound {
            kind,
            id: id.into(),
        }
    }

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
            FrontendError::InvalidResponse(e) => {
                format!("Antwort des Servers nicht lesbar: {e}")
            }
            FrontendError::EmptyResponse => "Leere Antwort des Servers".to_string(),
            FrontendError::NotFound { kind, id } => messages::not_found(*kind, *id),
            FrontendError::NoServerAddress => {
                "Adresse des Servers konnte nicht bestimmt werden".to_string()
            }
            FrontendError::InvalidPanelTree => {
                "Die Panels des Schachts bilden keinen gültigen Baum".to_string()
            }
            FrontendError::Printer(e) => format!("Druckfehler: {e}"),
            FrontendError::PrinterDisconnected => "Etikettendrucker nicht verbunden".to_string(),
            FrontendError::PrinterNoSupply => {
                "Etikettendrucker hat kein Etikett gemeldet".to_string()
            }
            FrontendError::UnsupportedTape => {
                "Etikettentyp wird nicht unterstützt (nur Endlosband)".to_string()
            }
            FrontendError::Map(e) => {
                format!("Karte konnte nicht angezeigt werden: {}", js_message(e))
            }
            FrontendError::SaveFile(e) => {
                format!("Datei konnte nicht gespeichert werden: {}", js_message(e))
            }
            FrontendError::BrowserMissing(part) => {
                format!("Der Browser bietet {} nicht an", part.name())
            }
            FrontendError::Browser(e) => {
                format!("Der Browser meldet einen Fehler: {}", js_message(e))
            }
        }
    }

    /// Further details, the messages of an unexpected server error with where they came from.
    pub fn details(&self) -> Box<[String]> {
        match self {
            FrontendError::Graphql(errors) => errors.iter().map(ServerError::detail).collect(),
            _ => Box::default(),
        }
    }
}

impl IntoPropValue<Html> for &FrontendError {
    fn into_prop_value(self) -> Html {
        let title = self.title();
        let details = self.details();
        let mut children = Vec::new();
        if !details.is_empty() {
            children.push(html! {
                <ul>
                    {for details.into_iter().map(|detail| html!(<li>{detail}</li>))}
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
                | FrontendError::InvalidResponse(_)
        );
        let reload = matches!(
            self,
            FrontendError::Graphql(_) | FrontendError::InvalidResponse(_)
        );
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
