//! Business logic layer. Depends only on [`crate::domain`] and [`crate::ports`] traits —
//! never on concrete adapters.
//!
//! - [`Service`] — CRUD and full-text search operations over a [`crate::ports::KbStore`]
//! - [`SemanticService`] — embedding generation and vector search, coordinating a
//!   [`crate::ports::VectorStore`] and an [`crate::ports::EmbeddingProvider`]

pub mod kb_service;
pub mod semantic_service;

pub use kb_service::Service;
pub use semantic_service::SemanticService;
