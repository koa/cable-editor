use cynic::http::CynicReqwestError;
use patternfly_yew::prelude::{Alert, AlertType, Popover, PopoverBody};
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
    #[error("Plan not found: {0}")]
    PlanNotFound(i32),
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
            FrontendError::PlanNotFound(id) => {
                html!(<Alert inline=true title={format!("Plan {id} existiert nicht")} r#type={AlertType::Danger} />)
            }
        }
    }
}
