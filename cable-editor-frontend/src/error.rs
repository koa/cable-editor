use cynic::http::CynicReqwestError;
use reqwest::header::InvalidHeaderValue;
use thiserror::Error;
use yew::{Html, html::IntoPropValue};

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
}

impl IntoPropValue<Html> for &FrontendError {
    fn into_prop_value(self) -> Html {
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
            FrontendError::InvalidHeader(e) => {
                format!("Ungültiger Heder: {e}")
            }
        }
        .into_prop_value()
    }
}
