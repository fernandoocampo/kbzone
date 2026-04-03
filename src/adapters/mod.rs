//! Concrete implementations of the outbound ports defined in [`crate::ports`].
//!
//! - [`sqlite`] — [`sqlite::SqliteStore`] implements both [`crate::ports::KbStore`] and
//!   [`crate::ports::VectorStore`] sharing a single `Arc<Mutex<Connection>>`. The sqlite-vec
//!   extension is registered once at startup via `sqlite3_auto_extension`.
//! - [`fastembed`] — [`fastembed::FastEmbedProvider`] implements [`crate::ports::EmbeddingProvider`]
//!   using the BAAI/bge-small-en-v1.5 model (384-dimensional vectors).

pub mod fastembed;
pub mod filesystem;
pub mod http;
pub mod sqlite;
