//! Outbound ports — traits that define what the service layer needs from infrastructure.
//!
//! Concrete implementations live in [`crate::adapters`]. Depending on traits rather than
//! concrete types keeps the service layer testable without a real database or model.
//!
//! | Trait                  | Purpose                                          |
//! |------------------------|--------------------------------------------------|
//! | [`KbStore`]            | CRUD operations and FTS5 full-text search        |
//! | [`EmbeddingProvider`]  | Convert text into a float vector representation |
//! | [`VectorStore`]        | Store and query embeddings via KNN search        |

pub mod embedding;
pub mod storage;
pub mod vector_store;

pub use embedding::EmbeddingProvider;
pub use storage::KbStore;
pub use vector_store::VectorStore;
