//! Error types shared across all layers.
//!
//! Two enums cover the full error surface:
//! - [`Error`] — domain and storage failures that can occur at runtime (e.g. record not found,
//!   query failure, embedding error).
//! - [`AppError`] — startup/configuration failures that prevent the application from launching.

pub mod error;

pub use error::{AppError, Error};
