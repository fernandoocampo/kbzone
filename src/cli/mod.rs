//! CLI layer — Clap command definitions and handler functions.
//!
//! - [`commands`] — defines the subcommand enum and all argument structs parsed by Clap
//! - [`handlers`] — one handler function per subcommand; receives a [`handlers::Services`]
//!   wrapper that groups [`crate::service::Service`] and [`crate::service::SemanticService`]
//!   to satisfy the two-parameter rule

pub mod commands;
pub mod handlers;
