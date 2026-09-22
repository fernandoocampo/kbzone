use super::*;
use crate::domain::{IncomingEdge, KbFilter, KbItem, OutgoingEdge, RelatedEdges, TreeNode};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// ---- MockKbStore (duplicated locally — see kb_service_tests.rs's own copy;
// test modules don't share mocks in this codebase) ----

#[derive(Debug, Clone)]
struct MockKbStore {
    data: RefCell<HashMap<String, Kb>>,
}

impl MockKbStore {
    fn new() -> Self {
        Self {
            data: RefCell::new(HashMap::new()),
        }
    }

    fn with(entries: Vec<Kb>) -> Self {
        let store = Self::new();
        for kb in entries {
            store.data.borrow_mut().insert(kb.id.clone(), kb);
        }
        store
    }
}

impl KbStore for MockKbStore {
    fn initialize(&self) -> Result<(), Error> {
        Ok(())
    }

    fn get_kb_by_id(&self, id: &str) -> Result<Option<Kb>, Error> {
        Ok(self.data.borrow().get(id).cloned())
    }

    fn get_kb_by_key(&self, key: &str) -> Result<Option<Kb>, Error> {
        Ok(self
            .data
            .borrow()
            .values()
            .find(|kb| kb.key == key)
            .cloned())
    }

    fn get_kbs(&self, _filter: &KbFilter) -> Result<Vec<KbItem>, Error> {
        Ok(vec![])
    }

    fn save_kb(&self, kb: &Kb) -> Result<(), Error> {
        self.data.borrow_mut().insert(kb.id.clone(), kb.clone());
        Ok(())
    }

    fn update_kb(&self, _kb: &Kb) -> Result<bool, Error> {
        Ok(false)
    }

    fn delete_kb(&self, id: &str) -> Result<bool, Error> {
        Ok(self.data.borrow_mut().remove(id).is_some())
    }

    fn random_by_category(&self, category: &str, _namespace: Option<&str>) -> Result<Kb, Error> {
        Err(Error::RandomNotFound(category.to_string()))
    }

    fn get_children_ids(&self, _parent_id: &str) -> Result<Vec<String>, Error> {
        Ok(vec![])
    }

    fn get_kbs_full(&self, _filter: &KbFilter) -> Result<Vec<Kb>, Error> {
        Ok(self.data.borrow().values().cloned().collect())
    }

    fn get_categories(&self, _namespace: Option<&str>) -> Result<Vec<String>, Error> {
        Ok(vec![])
    }

    fn get_namespaces(&self) -> Result<Vec<String>, Error> {
        Ok(vec![])
    }
}

// ---- MockKbGraph ----

#[derive(Debug, Clone, Default)]
struct MockKbGraph {
    edges: Rc<RefCell<Vec<KbEdge>>>,
    nodes: Rc<RefCell<HashMap<String, GraphNode>>>,
    tree_nodes: Rc<RefCell<Vec<TreeNode>>>,
}

impl MockKbGraph {
    fn new() -> Self {
        Self::default()
    }

    /// A graph pre-populated with node metadata (used to build `GraphNode`
    /// projections in `get_related`, mirroring the join `SqliteStore` does
    /// against the `kbs` table).
    fn with_nodes(nodes: Vec<GraphNode>) -> Self {
        let graph = Self::new();
        for node in nodes {
            graph.nodes.borrow_mut().insert(node.id.clone(), node);
        }
        graph
    }

    fn set_tree_nodes(&self, nodes: Vec<TreeNode>) {
        *self.tree_nodes.borrow_mut() = nodes;
    }

    fn node_for(&self, id: &str) -> GraphNode {
        self.nodes.borrow().get(id).cloned().unwrap_or(GraphNode {
            id: id.to_string(),
            key: String::new(),
            category: String::new(),
            namespace: String::new(),
        })
    }
}

impl KbGraph for MockKbGraph {
    fn initialize_graph(&self) -> Result<(), Error> {
        Ok(())
    }

    fn add_edge(&self, edge: &KbEdge) -> Result<(), Error> {
        let mut edges = self.edges.borrow_mut();
        if edges
            .iter()
            .any(|e| e.from_id == edge.from_id && e.to_id == edge.to_id)
        {
            return Err(Error::DuplicateEdgeError);
        }
        edges.push(edge.clone());
        Ok(())
    }

    fn remove_edge(&self, params: &RemoveEdgeParams) -> Result<bool, Error> {
        let mut edges = self.edges.borrow_mut();
        let before = edges.len();
        edges.retain(|e| !(e.from_id == params.from_id && e.to_id == params.to_id));
        Ok(edges.len() < before)
    }

    fn get_related(&self, query: &RelatedQuery) -> Result<RelatedEdges, Error> {
        let edges = self.edges.borrow();
        let mut result = RelatedEdges::default();
        if query.direction != EdgeDirection::In {
            result.outgoing = edges
                .iter()
                .filter(|e| e.from_id == query.kb_id)
                .map(|e| OutgoingEdge {
                    edge_id: e.id.clone(),
                    note: e.note.clone(),
                    created_on: e.created_on.clone(),
                    to: self.node_for(&e.to_id),
                })
                .collect();
        }
        if query.direction != EdgeDirection::Out {
            result.incoming = edges
                .iter()
                .filter(|e| e.to_id == query.kb_id)
                .map(|e| IncomingEdge {
                    edge_id: e.id.clone(),
                    note: e.note.clone(),
                    created_on: e.created_on.clone(),
                    from: self.node_for(&e.from_id),
                })
                .collect();
        }
        Ok(result)
    }

    fn get_tree(&self, _query: &TreeQuery) -> Result<Vec<TreeNode>, Error> {
        Ok(self.tree_nodes.borrow().clone())
    }

    fn get_edges_among_ids(&self, ids: &[String]) -> Result<Vec<KbEdge>, Error> {
        let set: std::collections::HashSet<&str> = ids.iter().map(String::as_str).collect();
        Ok(self
            .edges
            .borrow()
            .iter()
            .filter(|e| set.contains(e.from_id.as_str()) && set.contains(e.to_id.as_str()))
            .cloned()
            .collect())
    }
}

// ---- fixtures ----

fn make_kb(id: &str, key: &str) -> Kb {
    Kb {
        id: id.to_string(),
        key: key.to_string(),
        value: "value".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        reference: String::new(),
        namespace: "default".to_string(),
        tags: vec![],
        metadata: std::collections::BTreeMap::new(),
        created_on: "2026-01-01T00:00:00+0000".to_string(),
        parent: None,
        path: None,
        media_extension: None,
    }
}

fn make_kb_edge(id: &str, from_id: &str, to_id: &str, note: &str) -> KbEdge {
    KbEdge {
        id: id.to_string(),
        from_id: from_id.to_string(),
        to_id: to_id.to_string(),
        note: note.to_string(),
        created_on: "2026-01-01T00:00:00+0000".to_string(),
    }
}

// ---- link tests ----

#[test]
fn link_resolves_key_and_creates_edge() {
    let store = MockKbStore::with(vec![
        make_kb("car-id", "car"),
        make_kb("engine-id", "engine"),
    ]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let edge = svc
        .link(LinkParams {
            from_key_or_id: "car".to_string(),
            to_key_or_id: "engine".to_string(),
            note: "has an engine".to_string(),
            out: None,
        })
        .unwrap();
    assert_eq!(edge.from_id, "car-id");
    assert_eq!(edge.to_id, "engine-id");
    assert_eq!(edge.note, "has an engine");
}

#[test]
fn link_resolves_id_when_key_lookup_fails() {
    let store = MockKbStore::with(vec![
        make_kb("car-id", "car"),
        make_kb("engine-id", "engine"),
    ]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let edge = svc
        .link(LinkParams {
            from_key_or_id: "car-id".to_string(),
            to_key_or_id: "engine-id".to_string(),
            note: String::new(),
            out: None,
        })
        .unwrap();
    assert_eq!(edge.from_id, "car-id");
    assert_eq!(edge.to_id, "engine-id");
}

#[test]
fn link_rejects_self_loop() {
    let store = MockKbStore::with(vec![make_kb("car-id", "car")]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let result = svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "car".to_string(),
        note: String::new(),
        out: None,
    });
    assert!(matches!(result, Err(Error::SelfLoopNotAllowed)));
}

#[test]
fn link_errors_when_from_not_found() {
    let store = MockKbStore::with(vec![make_kb("engine-id", "engine")]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let result = svc.link(LinkParams {
        from_key_or_id: "no-such".to_string(),
        to_key_or_id: "engine".to_string(),
        note: String::new(),
        out: None,
    });
    assert!(matches!(result, Err(Error::KBNotFound)));
}

#[test]
fn link_errors_when_to_not_found() {
    let store = MockKbStore::with(vec![make_kb("car-id", "car")]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let result = svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "no-such".to_string(),
        note: String::new(),
        out: None,
    });
    assert!(matches!(result, Err(Error::KBNotFound)));
}

#[test]
fn link_propagates_duplicate_edge_error_from_store() {
    let store = MockKbStore::with(vec![
        make_kb("car-id", "car"),
        make_kb("engine-id", "engine"),
    ]);
    let svc = GraphService::new(store, MockKbGraph::new());

    svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "engine".to_string(),
        note: String::new(),
        out: None,
    })
    .unwrap();

    let result = svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "engine".to_string(),
        note: "again".to_string(),
        out: None,
    });
    assert!(matches!(result, Err(Error::DuplicateEdgeError)));
}

// ---- unlink tests ----

#[test]
fn unlink_removes_existing_edge() {
    let store = MockKbStore::with(vec![
        make_kb("car-id", "car"),
        make_kb("engine-id", "engine"),
    ]);
    let svc = GraphService::new(store, MockKbGraph::new());

    svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "engine".to_string(),
        note: String::new(),
        out: None,
    })
    .unwrap();

    assert!(svc.unlink("car", "engine").is_ok());
}

#[test]
fn unlink_errors_when_edge_missing() {
    let store = MockKbStore::with(vec![
        make_kb("car-id", "car"),
        make_kb("engine-id", "engine"),
    ]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let result = svc.unlink("car", "engine");
    assert!(matches!(result, Err(Error::EdgeNotFound)));
}

// ---- related tests ----

#[test]
fn related_returns_node_plus_filtered_edges_for_direction_out() {
    let car = make_kb("car-id", "car");
    let engine = make_kb("engine-id", "engine");
    let store = MockKbStore::with(vec![car.clone(), engine.clone()]);
    let graph = MockKbGraph::with_nodes(vec![GraphNode::from(&car), GraphNode::from(&engine)]);
    let svc = GraphService::new(store, graph);

    svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "engine".to_string(),
        note: "has an engine".to_string(),
        out: None,
    })
    .unwrap();

    let result = svc.related("car", EdgeDirection::Out).unwrap();
    assert_eq!(result.node.key, "car");
    assert_eq!(result.outgoing.len(), 1);
    assert_eq!(result.outgoing[0].to.key, "engine");
    assert!(result.incoming.is_empty());
}

#[test]
fn related_returns_node_plus_filtered_edges_for_direction_in() {
    let car = make_kb("car-id", "car");
    let engine = make_kb("engine-id", "engine");
    let store = MockKbStore::with(vec![car.clone(), engine.clone()]);
    let graph = MockKbGraph::with_nodes(vec![GraphNode::from(&car), GraphNode::from(&engine)]);
    let svc = GraphService::new(store, graph);

    svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "engine".to_string(),
        note: "has an engine".to_string(),
        out: None,
    })
    .unwrap();

    let result = svc.related("engine", EdgeDirection::In).unwrap();
    assert_eq!(result.incoming.len(), 1);
    assert_eq!(result.incoming[0].from.key, "car");
    assert!(result.outgoing.is_empty());
}

#[test]
fn related_returns_node_plus_filtered_edges_for_direction_both() {
    let car = make_kb("car-id", "car");
    let engine = make_kb("engine-id", "engine");
    let kit = make_kb("kit-id", "spare-parts-kit");
    let store = MockKbStore::with(vec![car.clone(), engine.clone(), kit.clone()]);
    let graph = MockKbGraph::with_nodes(vec![
        GraphNode::from(&car),
        GraphNode::from(&engine),
        GraphNode::from(&kit),
    ]);
    let svc = GraphService::new(store, graph);

    svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "engine".to_string(),
        note: String::new(),
        out: None,
    })
    .unwrap();
    svc.link(LinkParams {
        from_key_or_id: "spare-parts-kit".to_string(),
        to_key_or_id: "car".to_string(),
        note: String::new(),
        out: None,
    })
    .unwrap();

    let result = svc.related("car", EdgeDirection::Both).unwrap();
    assert_eq!(result.outgoing.len(), 1);
    assert_eq!(result.incoming.len(), 1);
}

// ---- tree tests ----

#[test]
fn tree_rejects_direction_both() {
    let store = MockKbStore::with(vec![make_kb("car-id", "car")]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let result = svc.tree(TreeWalkParams {
        key_or_id: "car".to_string(),
        direction: EdgeDirection::Both,
        depth: 10,
    });
    assert!(matches!(result, Err(Error::GraphQueryError(_))));
}

#[test]
fn tree_returns_root_and_flat_node_list() {
    let store = MockKbStore::with(vec![make_kb("car-id", "car")]);
    let graph = MockKbGraph::new();
    graph.set_tree_nodes(vec![TreeNode {
        id: "engine-id".to_string(),
        key: "engine".to_string(),
        depth: 1,
        parent_id: "car-id".to_string(),
        note: "has an engine".to_string(),
    }]);
    let svc = GraphService::new(store, graph);

    let result = svc
        .tree(TreeWalkParams {
            key_or_id: "car".to_string(),
            direction: EdgeDirection::Out,
            depth: 10,
        })
        .unwrap();
    assert_eq!(result.root.key, "car");
    assert_eq!(result.direction, "out");
    assert_eq!(result.nodes.len(), 1);
    assert_eq!(result.nodes[0].key, "engine");
}

// ---- export_graph tests ----

#[test]
fn export_graph_single_hop_out_direction_includes_root_and_target() {
    let car = make_kb("car-id", "car");
    let engine = make_kb("engine-id", "engine");
    let store = MockKbStore::with(vec![car.clone(), engine.clone()]);
    let graph = MockKbGraph::with_nodes(vec![GraphNode::from(&car), GraphNode::from(&engine)]);
    let svc = GraphService::new(store, graph);

    svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "engine".to_string(),
        note: "has an engine".to_string(),
        out: None,
    })
    .unwrap();

    let result = svc
        .export_graph(GraphViewParams {
            key_or_id: "car".to_string(),
            direction: EdgeDirection::Out,
            depth: 1,
        })
        .unwrap();

    assert_eq!(result.root_id, "car-id");
    assert_eq!(result.root_key, "car");
    assert_eq!(result.nodes.len(), 2);
    assert!(result.nodes.iter().any(|n| n.key == "car"));
    assert!(result.nodes.iter().any(|n| n.key == "engine"));
    assert_eq!(result.edges.len(), 1);
    assert_eq!(result.edges[0].from_id, "car-id");
    assert_eq!(result.edges[0].to_id, "engine-id");
    assert_eq!(result.edges[0].note, "has an engine");
}

#[test]
fn export_graph_respects_depth_limit() {
    let car = make_kb("car-id", "car");
    let engine = make_kb("engine-id", "engine");
    let piston = make_kb("piston-id", "piston");
    let store = MockKbStore::with(vec![car.clone(), engine.clone(), piston.clone()]);
    let graph = MockKbGraph::with_nodes(vec![
        GraphNode::from(&car),
        GraphNode::from(&engine),
        GraphNode::from(&piston),
    ]);
    let svc = GraphService::new(store, graph);

    svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "engine".to_string(),
        note: String::new(),
        out: None,
    })
    .unwrap();
    svc.link(LinkParams {
        from_key_or_id: "engine".to_string(),
        to_key_or_id: "piston".to_string(),
        note: String::new(),
        out: None,
    })
    .unwrap();

    let result = svc
        .export_graph(GraphViewParams {
            key_or_id: "car".to_string(),
            direction: EdgeDirection::Out,
            depth: 1,
        })
        .unwrap();

    assert_eq!(result.nodes.len(), 2);
    assert!(!result.nodes.iter().any(|n| n.key == "piston"));
    assert_eq!(result.edges.len(), 1);
}

#[test]
fn export_graph_direction_both_merges_outgoing_and_incoming() {
    let car = make_kb("car-id", "car");
    let engine = make_kb("engine-id", "engine");
    let kit = make_kb("kit-id", "spare-parts-kit");
    let store = MockKbStore::with(vec![car.clone(), engine.clone(), kit.clone()]);
    let graph = MockKbGraph::with_nodes(vec![
        GraphNode::from(&car),
        GraphNode::from(&engine),
        GraphNode::from(&kit),
    ]);
    let svc = GraphService::new(store, graph);

    svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "engine".to_string(),
        note: String::new(),
        out: None,
    })
    .unwrap();
    svc.link(LinkParams {
        from_key_or_id: "spare-parts-kit".to_string(),
        to_key_or_id: "car".to_string(),
        note: String::new(),
        out: None,
    })
    .unwrap();

    let result = svc
        .export_graph(GraphViewParams {
            key_or_id: "car".to_string(),
            direction: EdgeDirection::Both,
            depth: 1,
        })
        .unwrap();

    assert_eq!(result.nodes.len(), 3);
    assert_eq!(result.edges.len(), 2);
}

#[test]
fn export_graph_dedups_node_reached_via_multiple_paths() {
    let car = make_kb("car-id", "car");
    let engine = make_kb("engine-id", "engine");
    let wheel = make_kb("wheel-id", "wheel");
    let store = MockKbStore::with(vec![car.clone(), engine.clone(), wheel.clone()]);
    let graph = MockKbGraph::with_nodes(vec![
        GraphNode::from(&car),
        GraphNode::from(&engine),
        GraphNode::from(&wheel),
    ]);
    let svc = GraphService::new(store, graph);

    svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "engine".to_string(),
        note: String::new(),
        out: None,
    })
    .unwrap();
    svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "wheel".to_string(),
        note: String::new(),
        out: None,
    })
    .unwrap();
    svc.link(LinkParams {
        from_key_or_id: "engine".to_string(),
        to_key_or_id: "wheel".to_string(),
        note: String::new(),
        out: None,
    })
    .unwrap();

    let result = svc
        .export_graph(GraphViewParams {
            key_or_id: "car".to_string(),
            direction: EdgeDirection::Out,
            depth: 2,
        })
        .unwrap();

    assert_eq!(result.nodes.len(), 3);
    assert_eq!(result.edges.len(), 3);
}

#[test]
fn export_graph_depth_zero_returns_only_root() {
    let car = make_kb("car-id", "car");
    let engine = make_kb("engine-id", "engine");
    let store = MockKbStore::with(vec![car.clone(), engine.clone()]);
    let graph = MockKbGraph::with_nodes(vec![GraphNode::from(&car), GraphNode::from(&engine)]);
    let svc = GraphService::new(store, graph);

    svc.link(LinkParams {
        from_key_or_id: "car".to_string(),
        to_key_or_id: "engine".to_string(),
        note: String::new(),
        out: None,
    })
    .unwrap();

    let result = svc
        .export_graph(GraphViewParams {
            key_or_id: "car".to_string(),
            direction: EdgeDirection::Out,
            depth: 0,
        })
        .unwrap();

    assert_eq!(result.nodes.len(), 1);
    assert!(result.edges.is_empty());
}

#[test]
fn export_graph_unknown_root_returns_kb_not_found() {
    let svc = GraphService::new(MockKbStore::new(), MockKbGraph::new());

    let result = svc.export_graph(GraphViewParams {
        key_or_id: "does-not-exist".to_string(),
        direction: EdgeDirection::Out,
        depth: 1,
    });

    assert!(matches!(result, Err(Error::KBNotFound)));
}

// ---- export_edges tests ----

#[test]
fn export_edges_returns_empty_when_no_kbs_match_filter() {
    let svc = GraphService::new(MockKbStore::new(), MockKbGraph::new());
    let edges = svc.export_edges(&KbFilter::default()).unwrap();
    assert!(edges.is_empty());
}

#[test]
fn export_edges_returns_edge_with_keys_when_both_endpoints_in_filtered_set() {
    let store = MockKbStore::with(vec![make_kb("a-id", "car"), make_kb("b-id", "engine")]);
    let graph = MockKbGraph::new();
    graph
        .add_edge(&make_kb_edge("e1", "a-id", "b-id", "has an engine"))
        .unwrap();
    let svc = GraphService::new(store, graph);

    let edges = svc.export_edges(&KbFilter::default()).unwrap();

    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].from_key, "car");
    assert_eq!(edges[0].to_key, "engine");
    assert_eq!(edges[0].note, "has an engine");
}

#[test]
fn export_edges_omits_edge_when_target_kb_outside_filtered_set() {
    // Simulates the "split pair" scenario: only "car" survived the filter
    // that produced the exported `kbs` set.
    let store = MockKbStore::with(vec![make_kb("a-id", "car")]);
    let graph = MockKbGraph::new();
    graph
        .add_edge(&make_kb_edge("e1", "a-id", "b-id", "has an engine"))
        .unwrap();
    let svc = GraphService::new(store, graph);

    let edges = svc.export_edges(&KbFilter::default()).unwrap();

    assert!(edges.is_empty());
}

#[test]
fn export_edges_sorts_output_deterministically() {
    let store = MockKbStore::with(vec![
        make_kb("a-id", "zebra"),
        make_kb("b-id", "apple"),
        make_kb("c-id", "mango"),
    ]);
    let graph = MockKbGraph::new();
    graph
        .add_edge(&make_kb_edge("e1", "a-id", "c-id", "n1"))
        .unwrap();
    graph
        .add_edge(&make_kb_edge("e2", "b-id", "c-id", "n2"))
        .unwrap();
    let svc = GraphService::new(store, graph);

    let edges = svc.export_edges(&KbFilter::default()).unwrap();

    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].from_key, "apple");
    assert_eq!(edges[1].from_key, "zebra");
}

// ---------------------------------------------------------------------------
// import_edges tests
// ---------------------------------------------------------------------------

fn make_import_edge_item(from_key: &str, to_key: &str, note: &str) -> ImportEdgeItem {
    ImportEdgeItem {
        from_key: from_key.to_string(),
        to_key: to_key.to_string(),
        note: note.to_string(),
    }
}

#[test]
fn import_edges_resolves_keys_and_creates_edge() {
    let store = MockKbStore::with(vec![
        make_kb("car-id", "car"),
        make_kb("engine-id", "engine"),
    ]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let result = svc.import_edges(vec![make_import_edge_item(
        "car",
        "engine",
        "has an engine",
    )]);

    assert_eq!(result.saved.len(), 1);
    assert!(result.failed.is_empty());
    assert_eq!(result.saved[0].from_id, "car-id");
    assert_eq!(result.saved[0].to_id, "engine-id");
    assert_eq!(result.saved[0].note, "has an engine");
}

#[test]
fn import_edges_resolves_id_when_key_lookup_fails() {
    let store = MockKbStore::with(vec![
        make_kb("car-id", "car"),
        make_kb("engine-id", "engine"),
    ]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let result = svc.import_edges(vec![make_import_edge_item("car-id", "engine-id", "")]);

    assert_eq!(result.saved.len(), 1);
    assert_eq!(result.saved[0].from_id, "car-id");
    assert_eq!(result.saved[0].to_id, "engine-id");
}

#[test]
fn import_edges_reports_failure_when_from_key_not_found() {
    let store = MockKbStore::with(vec![make_kb("engine-id", "engine")]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let result = svc.import_edges(vec![make_import_edge_item("no-such", "engine", "")]);

    assert!(result.saved.is_empty());
    assert_eq!(result.failed.len(), 1);
    assert!(result.failed[0].reason.contains("from"));
}

#[test]
fn import_edges_reports_failure_when_to_key_not_found() {
    let store = MockKbStore::with(vec![make_kb("car-id", "car")]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let result = svc.import_edges(vec![make_import_edge_item("car", "no-such", "")]);

    assert!(result.saved.is_empty());
    assert_eq!(result.failed.len(), 1);
    assert!(result.failed[0].reason.contains("to"));
}

#[test]
fn import_edges_rejects_self_loop() {
    let store = MockKbStore::with(vec![make_kb("car-id", "car")]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let result = svc.import_edges(vec![make_import_edge_item("car", "car", "")]);

    assert!(result.saved.is_empty());
    assert_eq!(result.failed.len(), 1);
}

#[test]
fn import_edges_reports_duplicate_edge_as_failure() {
    let store = MockKbStore::with(vec![
        make_kb("car-id", "car"),
        make_kb("engine-id", "engine"),
    ]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let result = svc.import_edges(vec![
        make_import_edge_item("car", "engine", "first"),
        make_import_edge_item("car", "engine", "second"),
    ]);

    assert_eq!(result.saved.len(), 1);
    assert_eq!(result.failed.len(), 1);
}

#[test]
fn import_edges_processes_mixed_batch_independently() {
    let store = MockKbStore::with(vec![
        make_kb("car-id", "car"),
        make_kb("engine-id", "engine"),
    ]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let result = svc.import_edges(vec![
        make_import_edge_item("car", "engine", "valid"),
        make_import_edge_item("car", "no-such", "bad to"),
        make_import_edge_item("car", "engine", "duplicate"),
    ]);

    assert_eq!(result.saved.len(), 1);
    assert_eq!(result.failed.len(), 2);
}

#[test]
fn import_edges_returns_empty_result_for_empty_input() {
    let store = MockKbStore::with(vec![]);
    let svc = GraphService::new(store, MockKbGraph::new());

    let result = svc.import_edges(vec![]);

    assert!(result.saved.is_empty());
    assert!(result.failed.is_empty());
}
