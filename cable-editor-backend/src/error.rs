use cynic::http::CynicReqwestError;
use diesel_async::pooled_connection::deadpool::{BuildError, PoolError};
use reqwest::header::InvalidHeaderValue;
use std::borrow::Cow;
use std::{env::VarError, error::Error};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("Cannot read database url {0}")]
    CannotReadDatabaseUrl(VarError),
    #[error("Cannot connect to to database {0}")]
    CannotConnectToDatabase(BuildError),
    #[error("Cannot get db connection from pool{0}")]
    CannotGetDbConnection(PoolError),
    #[error("Missing DB Connection Pool {0:?}")]
    MissingDbConnectionPool(async_graphql::Error),
    #[error("Error executing migrations {0}")]
    ErrorExecutingMigrations(Box<dyn Error + Send + Sync>),
    #[error("Error from diesel {0}")]
    DieselError(#[from] diesel::result::Error),
    #[error("Cannot create client for accessing netbox by graphql {0}")]
    CreateNetboxCynicClientError(reqwest::Error),
    #[error("Cannot create header for accessing netbox by graphql {0}")]
    CreateNetboxCynicClientHeaderError(InvalidHeaderValue),
    #[error("Error calling netbox by graphql({}): {error}", query.as_deref().unwrap_or_default())]
    ErrorCallingNetboxBackend {
        query: Option<Cow<'static, str>>,
        error: CynicReqwestError,
    },
    #[error("No response from netbox by graphql from {}",query.as_deref().unwrap_or_default())]
    NetboxCynicEmptyResponseError { query: Option<Cow<'static, str>> },
    #[error("Error from Netbox graphql({}): {errors:?}",query.as_deref().unwrap_or_default())]
    NetboxGraphqlError {
        query: Option<Cow<'static, str>>,
        errors: Vec<cynic::GraphQlError>,
    },
}
