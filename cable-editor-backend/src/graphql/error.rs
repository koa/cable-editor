//! The errors of the resolvers. An unexpected error (the database, Netbox, a file) keeps where it
//! came from: the library and the line of the `?` that got it (`#[track_caller]`), sent to the
//! frontend in `extensions.origin` (`ErrorOrigin`) and logged with the same id. Refusals
//! (`UserError`) and errors of async-graphql itself (guards, missing context data) pass as
//! they are. See docs/fehlermeldungen.md.

use crate::error::BackendError;
use async_graphql::ErrorExtensions;
use cable_editor_common::{ErrorOrigin, UserError};
use std::{
    fmt::Display,
    panic::Location,
    sync::atomic::{AtomicU32, Ordering},
};

pub type ApiResult<T> = Result<T, ApiError>;

/// An error of a resolver. Deliberately not `Display`: that would turn it into an
/// `async_graphql::Error` through async-graphql's conversion for any `Display`, without its origin.
#[derive(Debug, Clone)]
pub enum ApiError {
    /// A library failed, at `location`
    Failed {
        message: Box<str>,
        library: &'static str,
        location: &'static Location<'static>,
    },
    /// Already a GraphQL error: a refusal, a guard, missing context data
    Graphql(async_graphql::Error),
}

/// An error whose library is worth telling. Only the types implementing it convert with their
/// origin; others don't compile with `?`, so a new kind of error is noticed.
pub trait Origin: Display {
    /// The crate whose error it is
    fn library(&self) -> &'static str;
}

macro_rules! origin {
    ($($error:ty => $library:literal),* $(,)?) => {
        $(impl Origin for $error {
            fn library(&self) -> &'static str {
                $library
            }
        })*
    };
}

origin! {
    diesel::result::Error => "diesel",
    diesel_async::pooled_connection::deadpool::PoolError => "deadpool",
    reqwest::Error => "reqwest",
    serde_json::Error => "serde_json",
    quick_xml::Error => "quick-xml",
    zip::result::ZipError => "zip",
    std::io::Error => "std::io",
    std::num::TryFromIntError => "std",
    std::string::FromUtf8Error => "std",
}

impl Origin for BackendError {
    fn library(&self) -> &'static str {
        match self {
            BackendError::CannotReadDatabaseUrl(_) => "std::env",
            BackendError::CannotConnectToDatabase(_) | BackendError::CannotGetDbConnection(_) => {
                "deadpool"
            }
            BackendError::MissingDbConnectionPool(_) => "async-graphql",
            BackendError::ErrorExecutingMigrations(_) => "diesel_migrations",
            BackendError::DieselError(_) => "diesel",
            BackendError::CreateNetboxCynicClientError(_) => "reqwest",
            BackendError::CreateNetboxCynicClientHeaderError(_) => "http",
            BackendError::ErrorCallingNetboxBackend { .. }
            | BackendError::NetboxCynicEmptyResponseError { .. }
            | BackendError::NetboxGraphqlError { .. } => "cynic (Netbox)",
            BackendError::NetboxSemaphoreError { .. } => "tokio",
        }
    }
}

impl ApiError {
    /// A failure that isn't an error of a library, e.g. an answer lacking what it must hold
    #[track_caller]
    pub fn failed(library: &'static str, message: &str) -> Self {
        ApiError::Failed {
            message: message.into(),
            library,
            location: Location::caller(),
        }
    }
}

impl<E: Origin> From<E> for ApiError {
    /// `?` passes its own location on (`FromResidual` is `#[track_caller]`)
    #[track_caller]
    fn from(error: E) -> Self {
        ApiError::Failed {
            message: error.to_string().into(),
            library: error.library(),
            location: Location::caller(),
        }
    }
}

impl From<async_graphql::Error> for ApiError {
    fn from(error: async_graphql::Error) -> Self {
        ApiError::Graphql(error)
    }
}

impl From<UserError> for ApiError {
    fn from(error: UserError) -> Self {
        ApiError::Graphql(error.into())
    }
}

impl From<ApiError> for async_graphql::Error {
    /// A failure is logged here, once per request that returns it, with the id the frontend
    /// shows.
    fn from(error: ApiError) -> Self {
        match error {
            ApiError::Failed {
                message,
                library,
                location,
            } => {
                let origin = ErrorOrigin {
                    library: library.into(),
                    location: format!("{}:{}", short_path(location.file()), location.line()).into(),
                    id: next_id(),
                };
                log::error!(
                    "Error {}: {message} ({}, {})",
                    origin.id,
                    origin.library,
                    origin.location
                );
                let error = async_graphql::Error::new(message);
                match serde_json::to_value(&origin).map(async_graphql::Value::from_json) {
                    Ok(Ok(value)) => {
                        error.extend_with(|_, extensions| extensions.set("origin", value))
                    }
                    _ => error,
                }
            }
            ApiError::Graphql(error) => error,
        }
    }
}

/// The path from the crate's directory on: `cable-editor-backend/src/…` for ours (paths are
/// relative to the workspace), `diesel-async-0.9.0/src/…` instead of the whole path into the
/// cargo registry.
fn short_path(file: &str) -> &str {
    match file.rfind("/registry/src/") {
        Some(start) => {
            let rest = &file[start + "/registry/src/".len()..];
            // Skips the directory of the registry (`index.crates.io-…/`)
            rest.split_once('/').map_or(rest, |(_, path)| path)
        }
        None => file,
    }
}

/// Unique across restarts and replicas for all practical purposes: the time in milliseconds
/// and a counter for errors within the same millisecond.
fn next_id() -> Box<str> {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed) % 256;
    format!("{:x}{count:02x}", chrono::Utc::now().timestamp_millis()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fail() -> ApiResult<()> {
        Err(diesel::result::Error::NotFound)?;
        Ok(())
    }

    #[test]
    fn keeps_origin() {
        let line = line!() - 6;
        let Err(ApiError::Failed {
            library, location, ..
        }) = fail()
        else {
            panic!("no failure");
        };
        assert_eq!(library, "diesel");
        assert_eq!(location.line(), line);
        assert!(location.file().ends_with("graphql/error.rs"));

        let error: async_graphql::Error = fail().unwrap_err().into();
        let extensions = serde_json::to_value(error.extensions).unwrap_or_default();
        assert_eq!(extensions["origin"]["library"], "diesel");
        assert_eq!(
            extensions["origin"]["location"],
            format!("cable-editor-backend/src/graphql/error.rs:{line}")
        );
    }

    #[test]
    fn shortens_registry_paths() {
        assert_eq!(
            short_path(
                "/usr/local/cargo/registry/src/index.crates.io-1949cf8c6b5b557f/diesel-async-0.9.0/src/lib.rs"
            ),
            "diesel-async-0.9.0/src/lib.rs"
        );
        assert_eq!(
            short_path("cable-editor-backend/src/x.rs"),
            "cable-editor-backend/src/x.rs"
        );
    }

    #[test]
    fn refusals_pass() {
        let error: async_graphql::Error = ApiError::from(UserError::InvalidRequest).into();
        assert!(error.extensions.is_none_or(|e| e.get("origin").is_none()));
    }
}
