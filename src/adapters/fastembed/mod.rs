//! FastEmbed adapter — implements [`crate::ports::EmbeddingProvider`].
//!
//! [`FastEmbedProvider`] runs one of the curated [`SupportedModel`]s (default
//! BAAI/bge-small-en-v1.5, 384 dimensions) locally to produce float vectors from text.
//! Embedding failures during `add`/`update` are non-fatal: the entry is saved and a
//! warning is printed. Run `kb reindex` to regenerate missing embeddings.

pub mod model;
pub mod provider;

pub use model::SupportedModel;
pub use provider::{FastEmbedParams, FastEmbedProvider};
