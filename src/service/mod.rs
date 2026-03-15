//! Business logic layer. Depends only on [`crate::domain`] and [`crate::ports`] traits —
//! never on concrete adapters.
//!
//! - [`KBService`] — unified service combining CRUD and semantic operations, owning a
//!   [`crate::ports::KbStore`], a [`crate::ports::VectorStore`], and an
//!   [`crate::ports::EmbeddingProvider`] directly.

pub(crate) mod kb_service;

pub(crate) use kb_service::KBService;
pub(crate) use kb_service::SemanticDeps;
