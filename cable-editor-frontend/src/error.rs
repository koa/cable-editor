use cynic::http::CynicReqwestError;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum FrontendError {
    #[error("Error connecting to anonymous GraphQL endpoint: {0}")]
    ErrorQueryingAnonymousConnect(reqwest::Error),
    #[error("Error querying anonymous GraphQL endpoint: {0}")]
    ErrorQueryingAnonymousTransfer(CynicReqwestError),
}
