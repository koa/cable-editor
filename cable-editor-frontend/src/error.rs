use cynic::http::CynicReqwestError;
use patternfly_yew::prelude::{Popover, PopoverBody};
use reqwest::header::InvalidHeaderValue;
use thiserror::Error;
use yew::{Html, html, html::IntoPropValue, html_nested};

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
    #[error("Errors from server: {0:?}")]
    Graphql(Vec<cynic::GraphQlError>),
}

impl IntoPropValue<Html> for &FrontendError {
    fn into_prop_value(self) -> Html {
        match self {
            FrontendError::ErrorQueryingAnonymousConnect(e) => {
                format!("Fehler beim anyonymen Verbindungsaufbau: {e}").into_prop_value()
            }
            FrontendError::ErrorQueryingAnonymousTransfer(e) => {
                format!("Fehler bei einer anonymen Abfrage: {e}").into_prop_value()
            }
            FrontendError::ErrorQueryingAuthenticatedConnect(e) => {
                format!("Fehler beim authentisierten Verbindungsaufbau: {e}").into_prop_value()
            }
            FrontendError::ErrorQueryingAuthenticatedTransfer(e) => {
                format!("Fehler bei einer authentisierten Abfrage: {e}").into_prop_value()
            }
            FrontendError::InvalidHeader(e) => format!("Ungültiger Heder: {e}").into_prop_value(),
            FrontendError::Graphql(e) => e
                .iter()
                .map(|e| {
                    let target = e.message.as_str();
                    let locations = e.locations.as_ref().map(|l| {
                        let locations = l.iter().map(|location|{
                            html!(<dt>{format!("{}:{}",location.line,location.column)}</dt>)
                        });
                        html!(<><dd>{"Position"}</dd>{for locations}</>)
                    });

                    let body = html_nested!(
                        <PopoverBody
                            header={html!("Details")}
                        >
                            <dl>
                                <dd>{"Fehler"}</dd>
                                <dt>{target}</dt>
                                {locations}
                            </dl>
                        </PopoverBody>
                    );
                    html!(<Popover {target} {body}/>)
                })
                .collect(),
        }
    }
}
