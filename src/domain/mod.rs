//! Core domain structs with no external dependencies.
//!
//! This is the innermost layer of the architecture. Nothing here imports from
//! `ports`, `service`, or `adapters` — dependency flow is always inward toward this module.
//!
//! Key types:
//! - [`Kb`] — a fully hydrated knowledge-base entry as read from storage
//! - [`NewKb`] — data required to create a new entry
//! - [`KbUpdate`] — partial update payload
//! - [`KbFilter`] — query filter for listing entries
//! - [`KbItem`] — lightweight projection used in list results
//! - [`ScoredKbItem`] — [`KbItem`] paired with a semantic similarity score
//! - [`SemanticQuery`] — input for a vector/semantic search
//! - [`EmbeddingInput`] — text prepared for embedding generation

pub mod graph;
pub mod kb;
pub mod tag_suggestion;

pub use graph::{
    EdgeDirection, ExportDocument, ExportEdgeItem, FailedImportEdgeItem, GraphExport,
    GraphExportEdge, GraphExportNode, GraphNode, GraphViewParams, ImportDocument,
    ImportEdgeBatchResult, ImportEdgeItem, IncomingEdge, KbEdge, KbRelationships,
    KbWithRelationships, LinkConfirmation, LinkErrorResponse, LinkParams, NewKbEdge, OutgoingEdge,
    RelatedEdges, RelatedQuery, RelatedResult, RemoveEdgeParams, TreeNode, TreeQuery, TreeResult,
    TreeRoot, TreeWalkParams,
};
pub use kb::{
    AddJsonInput, CategoriesErrorResponse, DeleteConfirmation, DeleteErrorResponse, EmbeddingInput,
    ExportKbItem, ExportMediaParams, FailedImportItem, ImportBatchResult, ImportKbItem, Kb,
    KbFilter, KbItem, KbUpdate, MediaPathParams, NewKb, OutputFormat, RandomErrorResponse,
    ReindexResult, ScoredKbItem, SemanticQuery, StoreMediaParams, build_metadata, file_extension,
    format_metadata, is_media_category, media_file_path, normalize_path, parse_metadata_input,
};
pub use tag_suggestion::{TagSuggestionInput, suggest_tags};
