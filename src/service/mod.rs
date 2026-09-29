//! Business logic layer. Depends only on [`crate::domain`] and [`crate::ports`] traits —
//! never on concrete adapters.
//!
//! - [`KBService`] — unified service combining CRUD and semantic operations, owning a
//!   [`crate::ports::KbStore`], a [`crate::ports::VectorStore`], and an
//!   [`crate::ports::EmbeddingProvider`] directly.
//! - [`GraphService`] — graph-relationship (edge) service, owning a [`crate::ports::KbStore`]
//!   (for key-or-id resolution) and a [`crate::ports::KbGraph`].

pub(crate) mod graph_service;
pub(crate) mod kb_service;

pub(crate) use graph_service::GraphService;
pub(crate) use kb_service::KBService;
pub(crate) use kb_service::ServiceDeps;
