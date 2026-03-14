//! Application wiring — configuration loading and startup orchestration.
//!
//! - [`config`] — loads `~/kbzona/config.yaml` (or `$KBZONA_HOME/config.yaml`); writes a
//!   default file if absent
//! - [`app`] — `App::build()` wires all adapters and services together; `App::run()` parses
//!   CLI arguments and dispatches to the appropriate handler

pub mod app;
pub mod config;
