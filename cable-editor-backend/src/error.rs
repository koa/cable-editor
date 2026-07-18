use diesel_async::pooled_connection::deadpool::{BuildError, PoolError};
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
}
