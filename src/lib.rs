//! # kbzone
//!
//! A synchronous CLI for managing a personal knowledge base backed by a local SQLite file.
//!
//! ## Architecture
//!
//! The codebase follows a hexagonal (ports & adapters) style:
//!
//! - [`domain`] — core structs with no external dependencies
//! - [`errors`] — error types shared across all layers
//! - [`ports`] — outbound traits defining what the service layer needs from infrastructure
//! - [`service`] — business logic, depends only on domain and ports
//! - [`adapters`] — concrete implementations of the ports (SQLite, FastEmbed)
//! - [`cli`] — Clap command definitions and handler functions
//! - [`application`] — wiring: config loading, `App::build()`, and `App::run()`

pub mod adapters;
pub mod application;
pub mod cli;
pub mod domain;
pub mod errors;
pub mod ports;
pub mod service;
