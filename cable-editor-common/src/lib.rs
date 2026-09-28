//! What backend and frontend share about refused requests (see docs/fehlermeldungen.md): the
//! reasons as data, without texts (the frontend words them), and the rules they check.

pub mod error;
pub mod limits;

pub use error::{ErrorOrigin, ObjectKind, UserError};
