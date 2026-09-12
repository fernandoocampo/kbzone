use std::collections::{HashMap, HashSet};

use crate::domain::{
    EdgeDirection, GraphExport, GraphExportEdge, GraphExportNode, GraphNode, GraphViewParams, Kb,
    KbEdge, LinkParams, NewKbEdge, RelatedQuery, RelatedResult, RemoveEdgeParams, TreeQuery,
    TreeResult, TreeRoot, TreeWalkParams,
};
use crate::errors::Error;
use crate::ports::{KbGraph, KbStore};

/// Graph-relationship service — resolves key-or-id CLI input to internal
/// `Kb.id` values and orchestrates edge add/remove/traversal. Kept separate
/// from `KBService` since edges have no coupling to the CRUD/embedding flow
/// (unlike semantic search, nothing needs to run on every `add`/`update`).
#[derive(Debug, Clone)]
pub(crate) struct GraphService<S: KbStore, G: KbGraph> {
    store: S,
    graph: G,
}

impl<S: KbStore, G: KbGraph> GraphService<S, G> {
    pub fn new(store: S, graph: G) -> Self {
        Self { store, graph }
    }

    /// Resolves both ends, rejects a self-loop, and persists a new edge.
    pub fn link(&self, params: LinkParams) -> Result<KbEdge, Error> {
        let from = self.resolve(&params.from_key_or_id)?;
        let to = self.resolve(&params.to_key_or_id)?;
        if from.id == to.id {
            return Err(Error::SelfLoopNotAllowed);
        }
        let edge = KbEdge::from(NewKbEdge {
            from_id: from.id,
            to_id: to.id,
            note: params.note,
        });
        self.graph.add_edge(&edge)?;
        Ok(edge)
    }

    /// Resolves both ends and removes the edge in that exact direction.
    /// Errors with `Error::EdgeNotFound` if no such edge exists.
    pub fn unlink(&self, from: &str, to: &str) -> Result<(), Error> {
        let from_kb = self.resolve(from)?;
        let to_kb = self.resolve(to)?;
        let removed = self.graph.remove_edge(&RemoveEdgeParams {
            from_id: from_kb.id,
            to_id: to_kb.id,
        })?;
        if !removed {
            return Err(Error::EdgeNotFound);
        }
        Ok(())
    }

    /// One-hop outgoing/incoming edges for the resolved entry.
    pub fn related(
        &self,
        key_or_id: &str,
        direction: EdgeDirection,
    ) -> Result<RelatedResult, Error> {
        let node = self.resolve(key_or_id)?;
        let edges = self.graph.get_related(&RelatedQuery {
            kb_id: node.id.clone(),
            direction,
        })?;
        Ok(RelatedResult {
            node: GraphNode::from(&node),
            outgoing: edges.outgoing,
            incoming: edges.incoming,
        })
    }

    /// Transitive traversal from the resolved entry. `Both` is rejected —
    /// tree traversal only makes sense in one direction at a time.
    pub fn tree(&self, params: TreeWalkParams) -> Result<TreeResult, Error> {
        if params.direction == EdgeDirection::Both {
            return Err(Error::GraphQueryError(
                "direction must be 'out' or 'in' for tree traversal".to_string(),
            ));
        }
        let root = self.resolve(&params.key_or_id)?;
        let nodes = self.graph.get_tree(&TreeQuery {
            kb_id: root.id.clone(),
            direction: params.direction,
            depth: params.depth,
        })?;
        Ok(TreeResult {
            root: TreeRoot {
                id: root.id,
                key: root.key,
            },
            direction: params.direction.as_str().to_string(),
            nodes,
        })
    }

    /// Breadth-first traversal from the resolved entry, in the requested
    /// direction (`Both` is valid here, unlike `tree`), bounded by
    /// `params.depth`, hydrating every visited node into a full `Kb` record
    /// — the caller (an offline HTML view) has no way to query the database
    /// live, so every field has to be embedded up front.
    pub fn export_graph(&self, params: GraphViewParams) -> Result<GraphExport, Error> {
        let root = self.resolve(&params.key_or_id)?;

        let mut visited_ids: HashSet<String> = HashSet::new();
        let mut edges_by_id: HashMap<String, GraphExportEdge> = HashMap::new();
        visited_ids.insert(root.id.clone());

        let mut frontier = vec![root.id.clone()];
        for _ in 0..params.depth {
            if frontier.is_empty() {
                break;
            }
            let mut next_frontier = Vec::new();
            for kb_id in &frontier {
                let related = self.graph.get_related(&RelatedQuery {
                    kb_id: kb_id.clone(),
                    direction: params.direction,
                })?;
                for out in related.outgoing {
                    edges_by_id
                        .entry(out.edge_id.clone())
                        .or_insert(GraphExportEdge {
                            id: out.edge_id,
                            from_id: kb_id.clone(),
                            to_id: out.to.id.clone(),
                            note: out.note,
                            created_on: out.created_on,
                        });
                    if visited_ids.insert(out.to.id.clone()) {
                        next_frontier.push(out.to.id);
                    }
                }
                for inc in related.incoming {
                    edges_by_id
                        .entry(inc.edge_id.clone())
                        .or_insert(GraphExportEdge {
                            id: inc.edge_id,
                            from_id: inc.from.id.clone(),
                            to_id: kb_id.clone(),
                            note: inc.note,
                            created_on: inc.created_on,
                        });
                    if visited_ids.insert(inc.from.id.clone()) {
                        next_frontier.push(inc.from.id);
                    }
                }
            }
            frontier = next_frontier;
        }

        let mut nodes = Vec::with_capacity(visited_ids.len());
        for id in &visited_ids {
            let kb = self.store.get_kb_by_id(id)?.ok_or(Error::KBNotFound)?;
            nodes.push(GraphExportNode::from(&kb));
        }

        Ok(GraphExport {
            root_id: root.id,
            root_key: root.key,
            nodes,
            edges: edges_by_id.into_values().collect(),
        })
    }

    /// Resolves user input to a full `Kb`: tries `key` first (the common CLI
    /// case), falls back to `id`. Errors with `Error::KBNotFound` if neither matches.
    fn resolve(&self, key_or_id: &str) -> Result<Kb, Error> {
        if let Some(kb) = self.store.get_kb_by_key(key_or_id)? {
            return Ok(kb);
        }
        self.store.get_kb_by_id(key_or_id)?.ok_or(Error::KBNotFound)
    }
}

#[cfg(test)]
#[path = "graph_service_tests.rs"]
mod tests;
