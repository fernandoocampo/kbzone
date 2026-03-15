//! Core domain structs with no external dependencies.
//!
//! This is the innermost layer of the architecture. Nothing here imports from
//! `ports`, `service`, or `adapters` — dependency flow is always inward toward this module.
//!
//! Key types:
//! - [`Kb`] — a fully hydrated knowledge-base entry as read from storage
//! - [`NewKb`] — data required to create a new entry
//! - [`KbUpdate`] — partial update payload
//! - [`KbFilter`] — query filter for listing entries
//! - [`KbItem`] — lightweight projection used in list results
//! - [`ScoredKbItem`] — [`KbItem`] paired with a semantic similarity score
//! - [`SemanticQuery`] — input for a vector/semantic search
//! - [`EmbeddingInput`] — text prepared for embedding generation

pub mod kb;

pub use kb::{
    EmbeddingInput, FailedImportItem, ImportBatchResult, ImportKbItem, Kb, KbFilter, KbItem,
    KbUpdate, NewKb, ScoredKbItem, SemanticQuery,
};
