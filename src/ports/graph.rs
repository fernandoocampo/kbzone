use std::fmt::Debug;

use crate::domain::{KbEdge, RelatedEdges, RelatedQuery, RemoveEdgeParams, TreeNode, TreeQuery};
use crate::errors::Error;

/// Outbound port for graph-relationship (edge) storage. Implemented by the
/// same adapter that implements `KbStore` (same connection, same DB file —
/// see `SqliteStore`), but kept as a distinct trait/port since edges are a
/// separate concern (semantic relationships) from `KbStore`'s CRUD rows.
pub trait KbGraph: Debug + Clone {
    /// Runs DDL to ensure the `kb_edges` schema exists (idempotent). Mirrors
    /// `VectorStore::initialize_vectors` — called once at startup, separately
    /// from `KbStore::initialize`.
    fn initialize_graph(&self) -> Result<(), Error>;

    /// Persists a fully-built edge (id/created_on already set by the domain
    /// layer). Returns `Error::DuplicateEdgeError` if `(from_id, to_id)`
    /// already exists.
    fn add_edge(&self, edge: &KbEdge) -> Result<(), Error>;

    /// Removes the edge in that exact direction. Returns `true` if a row was deleted.
    fn remove_edge(&self, params: &RemoveEdgeParams) -> Result<bool, Error>;

    /// One-hop outgoing/incoming edges for `query.kb_id`, filtered by
    /// `query.direction` (an unrequested direction is returned as an empty `Vec`).
    fn get_related(&self, query: &RelatedQuery) -> Result<RelatedEdges, Error>;

    /// Transitive traversal from `query.kb_id` in one direction, bounded by `query.depth`.
    fn get_tree(&self, query: &TreeQuery) -> Result<Vec<TreeNode>, Error>;

    /// Returns every persisted edge where **both** `from_id` and `to_id` are
    /// present in `ids`. Used by `kb export` to keep the exported `graph`
    /// section consistent with whatever `KbFilter` produced the exported
    /// `kbs` set: an edge whose other endpoint fell outside the filter is
    /// silently dropped, never emitted as a dangling reference. Returns
    /// `Ok(vec![])` immediately for an empty `ids` slice without touching
    /// storage.
    fn get_edges_among_ids(&self, ids: &[String]) -> Result<Vec<KbEdge>, Error>;
}
