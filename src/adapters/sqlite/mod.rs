//! SQLite adapter — implements [`crate::ports::KbStore`] and [`crate::ports::VectorStore`].
//!
//! [`SqliteStore`] holds a single `Arc<Mutex<Connection>>` shared between both trait
//! implementations. The sqlite-vec extension (for KNN vector search) is loaded once per
//! process via `sqlite3_auto_extension` before any connection is opened.
//!
//! > **Note:** sqlite-vec KNN queries (`vec0` MATCH) do not support JOINs. Vector search
//! > is therefore split into two queries: KNN → kb_ids, then a regular `kbs` lookup by id.

pub mod store;

pub use store::SqliteStore;
