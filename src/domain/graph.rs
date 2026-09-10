//! Graph-relationship domain types: a directed edge between two [`Kb`] entries,
//! plus the queries/results used to add, remove, and traverse those edges.
//!
//! This is additive to the existing `parent`/`path` hierarchy — edges express a
//! separate, many-to-many, directional relationship ("why do these two records
//! relate"), not filing/organisation.

use chrono::Local;
use uuid::Uuid;

use crate::domain::kb::Kb;

/// A directed relationship between two `Kb` entries, persisted in `kb_edges`.
#[derive(Debug, Clone, PartialEq)]
pub struct KbEdge {
    /// Internal UUID, auto-generated on creation.
    pub id: String,
    /// `Kb.id` this edge points from.
    pub from_id: String,
    /// `Kb.id` this edge points to.
    pub to_id: String,
    /// Free text describing why these two entries are connected.
    pub note: String,
    /// ISO-8601 creation timestamp.
    pub created_on: String,
}

/// DTO for creating a new edge. `from_id`/`to_id` must already be resolved
/// internal `Kb.id` values — resolving user-supplied key-or-id input happens
/// one layer up, in `GraphService`.
#[derive(Debug, Clone)]
pub struct NewKbEdge {
    pub from_id: String,
    pub to_id: String,
    pub note: String,
}

impl From<NewKbEdge> for KbEdge {
    /// Converts into a full `KbEdge`, generating UUID and timestamp.
    fn from(new: NewKbEdge) -> Self {
        KbEdge {
            id: Uuid::new_v4().to_string(),
            from_id: new.from_id,
            to_id: new.to_id,
            note: new.note,
            created_on: Local::now().format("%Y-%m-%dT%H:%M:%S%z").to_string(),
        }
    }
}

/// Direction of traversal/display for `related` and `tree`.
/// `Both` is only meaningful for `related`; `tree` rejects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    Out,
    In,
    Both,
}

impl EdgeDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeDirection::Out => "out",
            EdgeDirection::In => "in",
            EdgeDirection::Both => "both",
        }
    }
}

impl std::str::FromStr for EdgeDirection {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "out" => Ok(EdgeDirection::Out),
            "in" => Ok(EdgeDirection::In),
            "both" => Ok(EdgeDirection::Both),
            other => Err(format!("invalid direction: {other} (expected out|in|both)")),
        }
    }
}

// ---------------------------------------------------------------------------
// Service-layer input DTOs (raw user input, not yet resolved to a KB_ID)
// ---------------------------------------------------------------------------

/// Input for `GraphService::link`. `from_key_or_id`/`to_key_or_id` are raw
/// CLI input (either a `key` or an internal `id`), resolved in the service.
pub struct LinkParams {
    pub from_key_or_id: String,
    pub to_key_or_id: String,
    pub note: String,
}

/// Input for `GraphService::tree`. `key_or_id` is raw CLI input.
pub struct TreeWalkParams {
    pub key_or_id: String,
    pub direction: EdgeDirection,
    pub depth: i64,
}

// ---------------------------------------------------------------------------
// Port-layer input DTOs (already resolved to internal KB_IDs)
// ---------------------------------------------------------------------------

pub struct RemoveEdgeParams {
    pub from_id: String,
    pub to_id: String,
}

pub struct RelatedQuery {
    pub kb_id: String,
    pub direction: EdgeDirection,
}

/// `direction` must be `Out` or `In` — `Both` is rejected by `GraphService`
/// before this reaches the store.
pub struct TreeQuery {
    pub kb_id: String,
    pub direction: EdgeDirection,
    pub depth: i64,
}

// ---------------------------------------------------------------------------
// Output DTOs — serde `Serialize` for `--json`; field order mirrors the
// shapes documented for `kb related --json` / `kb tree --json`.
// ---------------------------------------------------------------------------

/// Lightweight node projection embedded in edge/tree output.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GraphNode {
    pub id: String,
    pub key: String,
    pub category: String,
    pub namespace: String,
}

impl From<&Kb> for GraphNode {
    fn from(kb: &Kb) -> Self {
        GraphNode {
            id: kb.id.clone(),
            key: kb.key.clone(),
            category: kb.category.clone(),
            namespace: kb.namespace.clone(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OutgoingEdge {
    pub edge_id: String,
    pub note: String,
    pub created_on: String,
    pub to: GraphNode,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IncomingEdge {
    pub edge_id: String,
    pub note: String,
    pub created_on: String,
    pub from: GraphNode,
}

/// Port-level return value of `KbGraph::get_related` — no root node info,
/// since the caller (`GraphService`) already resolved it from the input.
#[derive(Debug, Clone, Default)]
pub struct RelatedEdges {
    pub outgoing: Vec<OutgoingEdge>,
    pub incoming: Vec<IncomingEdge>,
}

/// Service/CLI-level output for `kb related`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RelatedResult {
    pub node: GraphNode,
    pub outgoing: Vec<OutgoingEdge>,
    pub incoming: Vec<IncomingEdge>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TreeNode {
    pub id: String,
    pub key: String,
    pub depth: i64,
    pub parent_id: String,
    pub note: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TreeRoot {
    pub id: String,
    pub key: String,
}

/// Service/CLI-level output for `kb tree`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TreeResult {
    pub root: TreeRoot,
    pub direction: String,
    pub nodes: Vec<TreeNode>,
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
