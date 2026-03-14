//! FastEmbed adapter — implements [`crate::ports::EmbeddingProvider`].
//!
//! [`FastEmbedProvider`] uses the BAAI/bge-small-en-v1.5 model to produce 384-dimensional
//! float vectors from text. Embedding failures during `add`/`update` are non-fatal: the
//! entry is saved and a warning is printed. Run `kb reindex` to regenerate missing embeddings.

pub mod provider;

pub use provider::FastEmbedProvider;
