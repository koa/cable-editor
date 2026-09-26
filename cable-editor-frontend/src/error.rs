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
    #[error("{}", .0.iter().map(|error| error.message.as_str()).collect::<Vec<_>>().join("; "))]
    Graphql(Vec<cynic::GraphQlError>),
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
}

impl IntoPropValue<Html> for &FrontendError {
    fn into_prop_value(self) -> Html {
        match self {
            FrontendError::ErrorQueryingAnonymousConnect(e) => {
                html!(<Alert inline=true title={format!("Fehler beim anyonymen Verbindungsaufbau: {e}")} r#type={AlertType::Danger} />)
            }
            FrontendError::ErrorQueryingAnonymousTransfer(e) => {
                html!(<Alert inline=true title={format!("Fehler bei einer anonymen Abfrage: {e}")} r#type={AlertType::Danger} />)
            }
            FrontendError::ErrorQueryingAuthenticatedConnect(e) => {
                html!(<Alert inline=true title={format!("Fehler beim authentisierten Verbindungsaufbau: {e}")} r#type={AlertType::Danger} />)
            }
            FrontendError::ErrorQueryingAuthenticatedTransfer(e) => {
                html!(<Alert inline=true title={format!("Fehler bei einer authentisierten Abfrage: {e}")} r#type={AlertType::Danger} />)
            }
            FrontendError::InvalidHeader(e) => {
                html!(<Alert inline=true title={format!("Ungültiger Header: {e}")} r#type={AlertType::Danger} />)
            }
            FrontendError::Graphql(errors) => {
                let title = match errors.as_slice() {
                    [error] => format!("Fehler vom Server: {}", error.message),
                    _ => "Fehler vom Server".to_string(),
                };
                html! {
                    <Alert inline=true {title} r#type={AlertType::Danger}>
                        if errors.len() > 1 {
                            <ul>
                                {for errors.iter().map(|error| html!(<li>{error.message.as_str()}</li>))}
                            </ul>
                        }
                    </Alert>
                }
            }
            FrontendError::PlanNotFound(id) => {
                html!(<Alert inline=true title={format!("Plan {id} existiert nicht")} r#type={AlertType::Danger} />)
            }
            FrontendError::NotFound => {
                html!(<Alert inline=true title={"Daten nicht gefunden".to_string()} r#type={AlertType::Danger} />)
            }
            FrontendError::NoServerAddress => {
                html!(<Alert inline=true title={"Adresse des Servers konnte nicht bestimmt werden".to_string()} r#type={AlertType::Danger} />)
            }
            FrontendError::InvalidPanelTree => {
                html!(<Alert inline=true title={"Die Panels des Schachts bilden keinen gültigen Baum".to_string()} r#type={AlertType::Danger} />)
            }
            FrontendError::Printer(e) => {
                html!(<Alert inline=true title={format!("Druckfehler: {e}")} r#type={AlertType::Danger} />)
            }
            FrontendError::PrinterDisconnected => {
                html!(<Alert inline=true title={"Drucker nicht verbunden".to_string()} r#type={AlertType::Danger} />)
            }
            FrontendError::PrinterNoSupply => {
                html!(<Alert inline=true title={"Drucker hat kein Etikett gemeldet".to_string()} r#type={AlertType::Danger} />)
            }
            FrontendError::UnsupportedTape => {
                html!(<Alert inline=true title={"Etikettentyp wird nicht unterstützt (nur Endlosband)".to_string()} r#type={AlertType::Danger} />)
            }
            FrontendError::Map(e) => {
                html!(<Alert inline=true title={format!("Karte konnte nicht angezeigt werden: {}", js_message(e))} r#type={AlertType::Danger} />)
            }
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
