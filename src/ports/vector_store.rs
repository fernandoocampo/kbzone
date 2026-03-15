use std::fmt::Debug;

use crate::domain::{EmbeddingInput, ScoredKbItem, SemanticQuery};
use crate::errors::Error;

/// Outbound port for vector storage adapters.
///
/// The `Clone` bound exists for the same reason as `KbStore`: `SqliteStore`
/// implements both traits via the same `Arc<Mutex<Connection>>`, so cheap cloning
/// is the mechanism for sharing the connection between services.
pub trait VectorStore: Debug + Clone {
    /// Creates the vector table schema (idempotent). Must be called once at startup.
    fn initialize_vectors(&self, dimensions: usize) -> Result<(), Error>;

    /// Persists an embedding for the given KB entry (upsert semantics).
    fn save_embedding(&self, input: &EmbeddingInput, embedding: &[f32]) -> Result<(), Error>;

    /// Removes the embedding for the given KB entry.
    fn delete_embedding(&self, kb_id: &str) -> Result<(), Error>;

    /// Returns the `limit` nearest KB entries to the provided query embedding.
    fn search_similar(
        &self,
        query: &SemanticQuery,
        embedding: &[f32],
    ) -> Result<Vec<ScoredKbItem>, Error>;
}
