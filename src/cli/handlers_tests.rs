use super::*;
use crate::service::ServiceDeps;
use std::cell::RefCell;
use std::collections::HashMap;

// ---- Mock implementations ----

// `data` is `Rc`-shared (not a bare `RefCell`) so that cloning a store
// yields a handle to the *same* underlying data — mirroring how the real
// `SqliteStore` shares one DB connection across `KBService`/`GraphService`.
// This lets a test wire one store into both services and observe writes
// made through either side (needed for the import round-trip test below).
#[derive(Debug, Clone)]
struct MockKbStore {
    data: std::rc::Rc<RefCell<HashMap<String, crate::domain::Kb>>>,
}

impl MockKbStore {
    fn new() -> Self {
        Self {
            data: std::rc::Rc::new(RefCell::new(HashMap::new())),
        }
    }

    fn with(entries: Vec<crate::domain::Kb>) -> Self {
        let store = Self::new();
        for kb in entries {
            store.data.borrow_mut().insert(kb.id.clone(), kb);
        }
        store
    }
}

impl crate::ports::KbStore for MockKbStore {
    fn initialize(&self) -> Result<(), Error> {
        Ok(())
    }

    fn get_kb_by_id(&self, id: &str) -> Result<Option<crate::domain::Kb>, Error> {
        Ok(self.data.borrow().get(id).cloned())
    }

    fn get_kb_by_key(&self, key: &str) -> Result<Option<crate::domain::Kb>, Error> {
        Ok(self
            .data
            .borrow()
            .values()
            .find(|kb| kb.key == key)
            .cloned())
    }

    fn get_kbs(
        &self,
        _filter: &crate::domain::KbFilter,
    ) -> Result<Vec<crate::domain::KbItem>, Error> {
        Ok(Vec::new())
    }

    fn save_kb(&self, kb: &crate::domain::Kb) -> Result<(), Error> {
        self.data.borrow_mut().insert(kb.id.clone(), kb.clone());
        Ok(())
    }

    fn update_kb(&self, kb: &crate::domain::Kb) -> Result<bool, Error> {
        let mut data = self.data.borrow_mut();
        if data.contains_key(&kb.id) {
            data.insert(kb.id.clone(), kb.clone());
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn delete_kb(&self, id: &str) -> Result<bool, Error> {
        Ok(self.data.borrow_mut().remove(id).is_some())
    }

    fn random_quote(&self) -> Result<crate::domain::Kb, Error> {
        self.data
            .borrow()
            .values()
            .find(|kb| kb.category.to_lowercase() == "quote")
            .cloned()
            .ok_or(Error::QuoteNotFound)
    }

    fn get_children_ids(&self, _parent_id: &str) -> Result<Vec<String>, Error> {
        Ok(vec![])
    }

    fn get_kbs_full(
        &self,
        _filter: &crate::domain::KbFilter,
    ) -> Result<Vec<crate::domain::Kb>, Error> {
        Ok(self.data.borrow().values().cloned().collect())
    }

    fn get_categories(&self, _namespace: Option<&str>) -> Result<Vec<String>, Error> {
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone)]
struct MockVectorStore;

impl crate::ports::VectorStore for MockVectorStore {
    fn initialize_vectors(&self, _dimensions: usize) -> Result<(), Error> {
        Ok(())
    }

    fn save_embedding(
        &self,
        _input: &crate::domain::EmbeddingInput,
        _embedding: &[f32],
    ) -> Result<(), Error> {
        Ok(())
    }

    fn delete_embedding(&self, _kb_id: &str) -> Result<(), Error> {
        Ok(())
    }

    fn search_similar(
        &self,
        _query: &crate::domain::SemanticQuery,
        _embedding: &[f32],
    ) -> Result<Vec<crate::domain::ScoredKbItem>, Error> {
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone)]
struct MockEmbeddingProvider;

impl crate::ports::EmbeddingProvider for MockEmbeddingProvider {
    fn embed(&self, _text: &str) -> Result<Vec<f32>, Error> {
        Ok(vec![0.0; 384])
    }

    fn dimensions(&self) -> usize {
        384
    }
}

#[derive(Debug, Clone)]
struct MockMediaStore;

impl crate::ports::MediaStore for MockMediaStore {
    fn store_media(&self, params: &crate::domain::StoreMediaParams) -> Result<String, Error> {
        Ok(params.destination.clone())
    }

    fn delete_media(&self, _path: &str) -> Result<(), Error> {
        Ok(())
    }

    fn copy_dir(&self, _source: &str, _destination: &str) -> Result<u64, Error> {
        Ok(0)
    }
}

#[derive(Debug, Clone)]
struct MockMediaFetcher;

impl crate::ports::MediaFetcher for MockMediaFetcher {
    fn fetch(&self, url: &str) -> Result<String, Error> {
        Ok(format!("/tmp/mock-{}", url))
    }
}

/// `KbGraph` mock shared by `handle_export`/`handle_import` tests (only
/// `get_edges_among_ids` needs real behavior there) and by `handle_get`
/// relationship tests (which need real `get_related` filtering). `nodes`
/// lets a test pre-populate the node metadata a real SQL join would attach
/// to each edge (mirrors `MockKbGraph::with_nodes` in
/// `service/graph_service_tests.rs` — test modules don't share mocks in
/// this codebase, so this is a local duplicate of that pattern).
#[derive(Debug, Clone, Default)]
struct MockKbGraph {
    edges: std::rc::Rc<RefCell<Vec<crate::domain::KbEdge>>>,
    nodes: std::rc::Rc<RefCell<HashMap<String, crate::domain::GraphNode>>>,
}

impl MockKbGraph {
    fn with_nodes(nodes: Vec<crate::domain::GraphNode>) -> Self {
        let graph = Self::default();
        for node in nodes {
            graph.nodes.borrow_mut().insert(node.id.clone(), node);
        }
        graph
    }

    fn node_for(&self, id: &str) -> crate::domain::GraphNode {
        self.nodes
            .borrow()
            .get(id)
            .cloned()
            .unwrap_or_else(|| crate::domain::GraphNode {
                id: id.to_string(),
                key: String::new(),
                category: String::new(),
                namespace: String::new(),
            })
    }
}

impl crate::ports::KbGraph for MockKbGraph {
    fn initialize_graph(&self) -> Result<(), Error> {
        Ok(())
    }

    fn add_edge(&self, edge: &crate::domain::KbEdge) -> Result<(), Error> {
        self.edges.borrow_mut().push(edge.clone());
        Ok(())
    }

    fn remove_edge(&self, _params: &crate::domain::RemoveEdgeParams) -> Result<bool, Error> {
        Ok(false)
    }

    fn get_related(
        &self,
        query: &crate::domain::RelatedQuery,
    ) -> Result<crate::domain::RelatedEdges, Error> {
        let edges = self.edges.borrow();
        let mut result = crate::domain::RelatedEdges::default();
        if query.direction != crate::domain::EdgeDirection::In {
            result.outgoing = edges
                .iter()
                .filter(|e| e.from_id == query.kb_id)
                .map(|e| crate::domain::OutgoingEdge {
                    edge_id: e.id.clone(),
                    note: e.note.clone(),
                    created_on: e.created_on.clone(),
                    to: self.node_for(&e.to_id),
                })
                .collect();
        }
        if query.direction != crate::domain::EdgeDirection::Out {
            result.incoming = edges
                .iter()
                .filter(|e| e.to_id == query.kb_id)
                .map(|e| crate::domain::IncomingEdge {
                    edge_id: e.id.clone(),
                    note: e.note.clone(),
                    created_on: e.created_on.clone(),
                    from: self.node_for(&e.from_id),
                })
                .collect();
        }
        Ok(result)
    }

    fn get_tree(
        &self,
        _query: &crate::domain::TreeQuery,
    ) -> Result<Vec<crate::domain::TreeNode>, Error> {
        Ok(vec![])
    }

    fn get_edges_among_ids(&self, ids: &[String]) -> Result<Vec<crate::domain::KbEdge>, Error> {
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

fn make_svc_with_store(
    store: MockKbStore,
) -> KBService<MockKbStore, MockVectorStore, MockEmbeddingProvider, MockMediaStore, MockMediaFetcher>
{
    KBService::new(
        store,
        ServiceDeps {
            vector_store: MockVectorStore,
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore,
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    )
}

fn make_svc()
-> KBService<MockKbStore, MockVectorStore, MockEmbeddingProvider, MockMediaStore, MockMediaFetcher>
{
    make_svc_with_store(MockKbStore::new())
}

/// Builds a `(KBService, GraphService)` pair for `handle_get` relationship
/// tests: `kbs` are seeded into both services' stores (so `GraphService`'s
/// internal key-or-id resolution can find them by id), and `graph` carries
/// any pre-seeded edges/nodes.
fn make_get_services(
    kbs: Vec<crate::domain::Kb>,
    graph: MockKbGraph,
) -> (
    KBService<
        MockKbStore,
        MockVectorStore,
        MockEmbeddingProvider,
        MockMediaStore,
        MockMediaFetcher,
    >,
    GraphService<MockKbStore, MockKbGraph>,
) {
    (
        make_svc_with_store(MockKbStore::with(kbs.clone())),
        GraphService::new(MockKbStore::with(kbs), graph),
    )
}

fn write_temp_yaml(name: &str, content: &str) -> String {
    let path = std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .to_string();
    std::fs::write(&path, content).unwrap();
    path
}

fn make_import_services() -> (
    KBService<
        MockKbStore,
        MockVectorStore,
        MockEmbeddingProvider,
        MockMediaStore,
        MockMediaFetcher,
    >,
    GraphService<MockKbStore, MockKbGraph>,
) {
    (
        make_svc(),
        GraphService::new(MockKbStore::new(), MockKbGraph::default()),
    )
}

fn make_kb(id: &str, key: &str) -> crate::domain::Kb {
    crate::domain::Kb {
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

fn make_kb_edge(id: &str, from_id: &str, to_id: &str, note: &str) -> crate::domain::KbEdge {
    crate::domain::KbEdge {
        id: id.to_string(),
        from_id: from_id.to_string(),
        to_id: to_id.to_string(),
        note: note.to_string(),
        created_on: "2026-01-01T00:00:00+0000".to_string(),
    }
}

#[test]
fn handle_import_returns_file_error_on_missing_file() {
    let (svc, graph_svc) = make_import_services();
    let params = ImportParams {
        file: "/no/such/file.yaml".to_string(),
        failed_items_file: "/tmp/failed.yaml".to_string(),
        failed_edges_file: "/tmp/failed_edges.yaml".to_string(),
    };
    let result = handle_import(
        ImportServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(matches!(result, Err(Error::ImportFileError(_))));
}

#[test]
fn handle_import_returns_parse_error_on_malformed_yaml() {
    let path = write_temp_yaml("malformed_test.yaml", "Key: [\nbad yaml{{{");
    let (svc, graph_svc) = make_import_services();
    let params = ImportParams {
        file: path.clone(),
        failed_items_file: "/tmp/failed_malformed.yaml".to_string(),
        failed_edges_file: "/tmp/failed_malformed_edges.yaml".to_string(),
    };
    let result = handle_import(
        ImportServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    let _ = std::fs::remove_file(&path);
    assert!(matches!(result, Err(Error::ParseImportFileError(_))));
}

#[test]
fn handle_import_succeeds_for_valid_file() {
    let yaml = "kbs:\n  - Key: rust-ownership\n    Value: memory management\n";
    let path = write_temp_yaml("valid_import_test.yaml", yaml);
    let (svc, graph_svc) = make_import_services();
    let params = ImportParams {
        file: path.clone(),
        failed_items_file: "/tmp/failed_valid.yaml".to_string(),
        failed_edges_file: "/tmp/failed_valid_edges.yaml".to_string(),
    };
    let result = handle_import(
        ImportServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    let _ = std::fs::remove_file(&path);
    assert!(result.is_ok());
}

#[test]
fn handle_import_writes_failed_items_in_yaml_format() {
    let yaml = "kbs:\n  - Key: \n    Value: memory management\n";
    let path = write_temp_yaml("failed_import_test.yaml", yaml);
    let failed_path = std::env::temp_dir()
        .join("failed_items_test.yaml")
        .to_string_lossy()
        .to_string();
    let (svc, graph_svc) = make_import_services();
    let params = ImportParams {
        file: path.clone(),
        failed_items_file: failed_path.clone(),
        failed_edges_file: "/tmp/failed_items_test_edges.yaml".to_string(),
    };
    let result = handle_import(
        ImportServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    let _ = std::fs::remove_file(&path);
    assert!(result.is_ok());
    let failed_content = std::fs::read_to_string(&failed_path).unwrap_or_default();
    let _ = std::fs::remove_file(&failed_path);
    assert!(failed_content.contains("Value") || failed_content.contains("memory"));
}

#[test]
fn handle_import_imports_edges_from_graph_section() {
    let svc = make_svc();
    let yaml = "kbs:\n  - Key: car\n    Value: a car\n  - Key: engine\n    Value: an engine\ngraph:\n  - From: car\n    To: engine\n    Note: has an engine\n";
    let path = write_temp_yaml("import_with_edges_test.yaml", yaml);

    let graph_store = MockKbStore::with(vec![
        make_kb("car-id", "car"),
        make_kb("engine-id", "engine"),
    ]);
    let mock_graph = MockKbGraph::default();
    let graph_svc = GraphService::new(graph_store, mock_graph.clone());

    let params = ImportParams {
        file: path.clone(),
        failed_items_file: "/tmp/import_edges_failed_items.yaml".to_string(),
        failed_edges_file: "/tmp/import_edges_failed_edges.yaml".to_string(),
    };
    let result = handle_import(
        ImportServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    let _ = std::fs::remove_file(&path);

    assert!(result.is_ok());
    assert_eq!(mock_graph.edges.borrow().len(), 1);
}

#[test]
fn handle_import_reports_failed_edges_and_writes_failed_edges_file() {
    let svc = make_svc();
    let yaml = "kbs:\n  - Key: car\n    Value: a car\ngraph:\n  - From: car\n    To: no-such-key\n    Note: bad edge\n";
    let path = write_temp_yaml("import_with_bad_edge_test.yaml", yaml);
    let failed_edges_path = std::env::temp_dir()
        .join("failed_edges_test.yaml")
        .to_string_lossy()
        .to_string();

    let graph_store = MockKbStore::with(vec![make_kb("car-id", "car")]);
    let graph_svc = GraphService::new(graph_store, MockKbGraph::default());

    let params = ImportParams {
        file: path.clone(),
        failed_items_file: "/tmp/import_bad_edge_failed_items.yaml".to_string(),
        failed_edges_file: failed_edges_path.clone(),
    };
    let result = handle_import(
        ImportServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    let _ = std::fs::remove_file(&path);

    assert!(result.is_ok());
    let failed_content = std::fs::read_to_string(&failed_edges_path).unwrap_or_default();
    let _ = std::fs::remove_file(&failed_edges_path);
    assert!(failed_content.contains("no-such-key"));
}

#[test]
fn handle_import_end_to_end_round_trip_from_handle_export_output() {
    // Export phase: two linked kbs, written by `handle_export`.
    let export_svc = make_svc();
    let kb_a = export_svc.add_kb(make_new_kb("car")).unwrap();
    let kb_b = export_svc.add_kb(make_new_kb("engine")).unwrap();
    let export_graph_store = MockKbStore::with(vec![kb_a.clone(), kb_b.clone()]);
    let export_mock_graph = MockKbGraph::default();
    export_mock_graph
        .add_edge(&crate::domain::KbEdge {
            id: "edge-1".to_string(),
            from_id: kb_a.id.clone(),
            to_id: kb_b.id.clone(),
            note: "has an engine".to_string(),
            created_on: "2026-01-01T00:00:00+0000".to_string(),
        })
        .unwrap();
    let export_graph_svc = GraphService::new(export_graph_store, export_mock_graph);

    let dir = unique_export_dir("roundtrip");
    let export_params = ExportParams {
        file_name: Some("out.yaml".to_string()),
        folder_output: dir.clone(),
        category: None,
        namespace: None,
        limit: None,
        offset: None,
    };
    handle_export(
        ExportServices {
            svc: &export_svc,
            graph_svc: &export_graph_svc,
        },
        export_params,
    )
    .unwrap();

    // Import phase: fresh services sharing one store (mirrors `App::build()`
    // wiring the same concrete `SqliteStore` into both `KBService` and
    // `GraphService`), so edges can resolve kbs created earlier in the
    // same import.
    let shared_store = MockKbStore::new();
    let import_svc = KBService::new(
        shared_store.clone(),
        ServiceDeps {
            vector_store: MockVectorStore,
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore,
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    let import_mock_graph = MockKbGraph::default();
    let import_graph_svc = GraphService::new(shared_store, import_mock_graph.clone());

    let import_params = ImportParams {
        file: format!("{}/out.yaml", dir),
        failed_items_file: "/tmp/rt_failed_items.yaml".to_string(),
        failed_edges_file: "/tmp/rt_failed_edges.yaml".to_string(),
    };
    let result = handle_import(
        ImportServices {
            svc: &import_svc,
            graph_svc: &import_graph_svc,
        },
        import_params,
    );
    let _ = std::fs::remove_dir_all(&dir);

    assert!(result.is_ok());
    assert_eq!(import_mock_graph.edges.borrow().len(), 1);
}

// ---------------------------------------------------------------------------
// handle_export tests
// ---------------------------------------------------------------------------

fn make_new_kb(key: &str) -> crate::domain::NewKb {
    crate::domain::NewKb {
        key: key.to_string(),
        value: "a value".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        namespace: "default".to_string(),
        reference: String::new(),
        tags: vec![],
        metadata: std::collections::BTreeMap::new(),
        parent: None,
        path: None,
        media_url: None,
        media_extension: None,
    }
}

fn unique_export_dir(name: &str) -> String {
    std::env::temp_dir()
        .join(format!("kb_export_test_{}", name))
        .to_string_lossy()
        .to_string()
}

#[test]
fn handle_export_writes_kbs_and_graph_sections_to_a_single_yaml_document() {
    let svc = make_svc();
    let kb_a = svc.add_kb(make_new_kb("car")).unwrap();
    let kb_b = svc.add_kb(make_new_kb("engine")).unwrap();

    let graph_store = MockKbStore::with(vec![kb_a.clone(), kb_b.clone()]);
    let mock_graph = MockKbGraph::default();
    mock_graph
        .add_edge(&crate::domain::KbEdge {
            id: "edge-1".to_string(),
            from_id: kb_a.id.clone(),
            to_id: kb_b.id.clone(),
            note: "has an engine".to_string(),
            created_on: "2026-01-01T00:00:00+0000".to_string(),
        })
        .unwrap();
    let graph_svc = GraphService::new(graph_store, mock_graph);

    let dir = unique_export_dir("full");
    let params = ExportParams {
        file_name: Some("out.yaml".to_string()),
        folder_output: dir.clone(),
        category: None,
        namespace: None,
        limit: None,
        offset: None,
    };

    let result = handle_export(
        ExportServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(result.is_ok());

    let content = std::fs::read_to_string(format!("{}/out.yaml", dir)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(content.starts_with("kbs:"));
    assert!(content.contains("graph:"));
    assert!(content.contains("From: car"));
    assert!(content.contains("To: engine"));
}

#[test]
fn handle_export_prints_no_entries_message_and_skips_write_when_filter_matches_nothing() {
    let svc = make_svc();
    let graph_svc = GraphService::new(MockKbStore::new(), MockKbGraph::default());

    let dir = unique_export_dir("empty");
    let params = ExportParams {
        file_name: Some("out.yaml".to_string()),
        folder_output: dir.clone(),
        category: None,
        namespace: None,
        limit: None,
        offset: None,
    };

    let result = handle_export(
        ExportServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(result.is_ok());
    assert!(!std::path::Path::new(&format!("{}/out.yaml", dir)).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn handle_export_omits_edge_when_filter_splits_a_linked_pair() {
    let svc = make_svc();
    let kb_a = svc.add_kb(make_new_kb("car")).unwrap();

    // Simulates the "split pair" scenario: only "car" survives the filter
    // that produced the exported `kbs` set — "engine" (and its id) never
    // appears in the graph store either.
    let graph_store = MockKbStore::with(vec![kb_a.clone()]);
    let mock_graph = MockKbGraph::default();
    mock_graph
        .add_edge(&crate::domain::KbEdge {
            id: "edge-1".to_string(),
            from_id: kb_a.id.clone(),
            to_id: "engine-id-not-in-set".to_string(),
            note: "has an engine".to_string(),
            created_on: "2026-01-01T00:00:00+0000".to_string(),
        })
        .unwrap();
    let graph_svc = GraphService::new(graph_store, mock_graph);

    let dir = unique_export_dir("split");
    let params = ExportParams {
        file_name: Some("out.yaml".to_string()),
        folder_output: dir.clone(),
        category: None,
        namespace: None,
        limit: None,
        offset: None,
    };

    let result = handle_export(
        ExportServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(result.is_ok());

    let content = std::fs::read_to_string(format!("{}/out.yaml", dir)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(content.contains("graph: []"));
    assert!(!content.contains("From:"));
}

#[test]
fn handle_version_returns_ok() {
    let result = handle_version();
    assert!(result.is_ok());
}

// handle_add always calls confirm_or_adjust which requires stdin input.
// Tests cannot easily mock stdin, so we verify the builder path directly.
// The full handle_add pipeline (including confirm) is covered by manual testing.
#[test]
fn build_new_kb_non_interactive_all_fields_provided_no_prompt() {
    let params = AddParams {
        key: Some("test-key".to_string()),
        value: Some("test-value".to_string()),
        notes: "notes".to_string(),
        category: "concept".to_string(),
        reference: "the book".to_string(),
        namespace: "rust".to_string(),
        tags: vec!["rust".to_string()],
        metadata: vec![],
        interactive: false,
        parent: None,
        path: None,
        media_url: None,
        json: None,
    };
    let result = build_new_kb_non_interactive(params);
    assert!(result.is_ok());
    let kb = result.expect("expected Ok NewKb");
    assert_eq!(kb.key, "test-key");
    assert_eq!(kb.value, "test-value");
    assert_eq!(kb.reference, "the book");
    assert_eq!(kb.tags, vec!["rust".to_string()]);
}

#[test]
fn handle_add_non_interactive_fails_without_key() {
    let svc = make_svc();
    let params = AddParams {
        key: None,
        value: Some("test-value".to_string()),
        notes: String::new(),
        category: String::new(),
        reference: String::new(),
        namespace: String::new(),
        tags: Vec::new(),
        metadata: vec![],
        interactive: false,
        parent: None,
        path: None,
        media_url: None,
        json: None,
    };
    let result = handle_add(&svc, params);
    assert!(matches!(result, Err(Error::MissingRequiredField(_))));
}

#[test]
fn handle_add_non_interactive_fails_without_value() {
    let svc = make_svc();
    let params = AddParams {
        key: Some("test-key".to_string()),
        value: None,
        notes: String::new(),
        category: String::new(),
        reference: String::new(),
        namespace: String::new(),
        tags: Vec::new(),
        metadata: vec![],
        interactive: false,
        parent: None,
        path: None,
        media_url: None,
        json: None,
    };
    let result = handle_add(&svc, params);
    assert!(matches!(result, Err(Error::MissingRequiredField(_))));
}

#[test]
fn build_new_kb_non_interactive_builds_correctly() {
    let params = AddParams {
        key: Some("my-key".to_string()),
        value: Some("my-value".to_string()),
        notes: "some notes".to_string(),
        category: "concept".to_string(),
        reference: "some-reference".to_string(),
        namespace: "default".to_string(),
        tags: vec!["rust".to_string()],
        metadata: vec![],
        interactive: false,
        parent: None,
        path: None,
        media_url: None,
        json: None,
    };
    let result = build_new_kb_non_interactive(params);
    assert!(result.is_ok());
    let new_kb = result.expect("expected Ok NewKb");
    assert_eq!(new_kb.key, "my-key");
    assert_eq!(new_kb.value, "my-value");
    assert_eq!(new_kb.tags, vec!["rust".to_string()]);
}

// ---------------------------------------------------------------------------
// parse_tags tests
// ---------------------------------------------------------------------------

#[test]
fn parse_tags_splits_by_comma() {
    let result = parse_tags("rust,memory,concepts");
    assert_eq!(result, vec!["rust", "memory", "concepts"]);
}

#[test]
fn parse_tags_trims_whitespace() {
    let result = parse_tags("rust , memory , concepts");
    assert_eq!(result, vec!["rust", "memory", "concepts"]);
}

#[test]
fn parse_tags_filters_empty_strings() {
    let result = parse_tags("rust,,memory,");
    assert_eq!(result, vec!["rust", "memory"]);
}

#[test]
fn parse_tags_returns_empty_for_empty_input() {
    let result = parse_tags("");
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// suggested_tags_for tests
// ---------------------------------------------------------------------------

#[test]
fn suggested_tags_for_derives_suggestions_from_fields() {
    let result = suggested_tags_for(&["rust ownership", "memory safety"], &[]);
    assert!(result.contains(&"ownership".to_string()));
}

#[test]
fn suggested_tags_for_excludes_existing_tags() {
    let result = suggested_tags_for(&["rust ownership memory"], &["rust".to_string()]);
    assert!(!result.contains(&"rust".to_string()));
    assert!(result.contains(&"ownership".to_string()));
}

#[test]
fn suggested_tags_for_returns_empty_for_stop_words_only() {
    let result = suggested_tags_for(&["the a an is"], &[]);
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// format_preview tests
// ---------------------------------------------------------------------------

#[test]
fn format_preview_includes_all_fields() {
    use crate::domain::NewKb;
    let kb = NewKb {
        key: "rust-ownership".to_string(),
        value: "memory management in rust".to_string(),
        notes: "important concept".to_string(),
        category: "concept".to_string(),
        namespace: "rust".to_string(),
        reference: "the book".to_string(),
        tags: vec!["rust".to_string(), "memory".to_string()],
        metadata: std::collections::BTreeMap::new(),
        parent: None,
        path: None,
        media_url: None,
        media_extension: None,
    };
    let preview = format_preview(&kb);
    assert!(preview.contains("rust-ownership"));
    assert!(preview.contains("memory management in rust"));
    assert!(preview.contains("important concept"));
    assert!(preview.contains("concept"));
    assert!(preview.contains("rust"));
    assert!(preview.contains("the book"));
}

#[test]
fn format_preview_shows_tags_joined_with_comma() {
    use crate::domain::NewKb;
    let kb = NewKb {
        key: "k".to_string(),
        value: "v".to_string(),
        notes: String::new(),
        category: String::new(),
        namespace: String::new(),
        reference: String::new(),
        tags: vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
        metadata: std::collections::BTreeMap::new(),
        parent: None,
        path: None,
        media_url: None,
        media_extension: None,
    };
    let preview = format_preview(&kb);
    assert!(preview.contains("alpha, beta, gamma"));
}

// ---------------------------------------------------------------------------
// build_new_kb_non_interactive: provided reference and tags (no prompt path)
// ---------------------------------------------------------------------------

#[test]
fn build_new_kb_non_interactive_uses_provided_reference() {
    let params = AddParams {
        key: Some("ref-key".to_string()),
        value: Some("ref-value".to_string()),
        notes: String::new(),
        category: String::new(),
        reference: "some book".to_string(),
        namespace: String::new(),
        tags: vec!["tag1".to_string()],
        metadata: vec![],
        interactive: false,
        parent: None,
        path: None,
        media_url: None,
        json: None,
    };
    let result = build_new_kb_non_interactive(params);
    assert!(result.is_ok());
    let kb = result.expect("expected Ok");
    assert_eq!(kb.reference, "some book");
}

#[test]
fn build_new_kb_non_interactive_uses_provided_tags() {
    let params = AddParams {
        key: Some("tags-key".to_string()),
        value: Some("tags-value".to_string()),
        notes: String::new(),
        category: String::new(),
        reference: "ref".to_string(),
        namespace: String::new(),
        tags: vec!["rust".to_string(), "memory".to_string()],
        metadata: vec![],
        interactive: false,
        parent: None,
        path: None,
        media_url: None,
        json: None,
    };
    let result = build_new_kb_non_interactive(params);
    assert!(result.is_ok());
    let kb = result.expect("expected Ok");
    assert_eq!(kb.tags, vec!["rust".to_string(), "memory".to_string()]);
}

// ---------------------------------------------------------------------------
// handle_add --json tests
// ---------------------------------------------------------------------------

fn add_params_with_json(json: Option<&str>) -> AddParams {
    AddParams {
        key: None,
        value: None,
        notes: String::new(),
        category: String::new(),
        reference: String::new(),
        namespace: String::new(),
        tags: Vec::new(),
        metadata: vec![],
        interactive: false,
        parent: None,
        path: None,
        media_url: None,
        json: json.map(str::to_string),
    }
}

#[test]
fn handle_add_json_creates_entry_and_returns_ok() {
    let svc = make_svc();
    let params = add_params_with_json(Some(
        r#"{"key":"complexity-views","value":"Fools ignore complexity.","category":"quote","tags":["a","b"],"reference":"Alan Perlis"}"#,
    ));
    let result = handle_add(&svc, params);
    assert!(result.is_ok());
    let kb = svc
        .get_kb_by_key("complexity-views")
        .expect("lookup should succeed")
        .expect("entry should be persisted");
    assert_eq!(kb.value, "Fools ignore complexity.");
    assert_eq!(kb.category, "quote");
    assert_eq!(kb.reference, "Alan Perlis");
    assert_eq!(kb.tags, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn handle_add_json_dedupes_tags_end_to_end() {
    let svc = make_svc();
    let params = add_params_with_json(Some(
        r#"{"key":"dedup-key","value":"v","category":"concept","tags":["a","a","b"]}"#,
    ));
    let result = handle_add(&svc, params);
    assert!(result.is_ok());
    let kb = svc
        .get_kb_by_key("dedup-key")
        .expect("lookup should succeed")
        .expect("entry should be persisted");
    assert_eq!(kb.tags, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn handle_add_json_rejects_blank_tag() {
    let svc = make_svc();
    let params = add_params_with_json(Some(
        r#"{"key":"blank-tag-key","value":"v","category":"concept","tags":["a"," "]}"#,
    ));
    let result = handle_add(&svc, params);
    assert!(matches!(result, Err(Error::InvalidTagError(_))));
    assert!(
        svc.get_kb_by_key("blank-tag-key")
            .expect("lookup should succeed")
            .is_none()
    );
}

#[test]
fn handle_add_json_rejects_missing_required_field() {
    let svc = make_svc();
    let params = add_params_with_json(Some(r#"{"key":"k","value":"v","tags":["a"]}"#));
    let result = handle_add(&svc, params);
    assert!(matches!(result, Err(Error::InvalidJsonInput(_))));
}

#[test]
fn handle_add_json_rejects_malformed_json() {
    let svc = make_svc();
    let params = add_params_with_json(Some("not json"));
    let result = handle_add(&svc, params);
    assert!(matches!(result, Err(Error::InvalidJsonInput(_))));
}

#[test]
fn handle_add_json_conflicts_with_key_flag() {
    let svc = make_svc();
    let mut params = add_params_with_json(Some(
        r#"{"key":"k","value":"v","category":"concept","tags":["a"]}"#,
    ));
    params.key = Some("some-other-key".to_string());
    let result = handle_add(&svc, params);
    assert!(matches!(result, Err(Error::ConflictingAddFlags(_))));
    assert!(
        svc.get_kb_by_key("k")
            .expect("lookup should succeed")
            .is_none()
    );
}

#[test]
fn handle_add_json_conflicts_with_tags_flag() {
    let svc = make_svc();
    let mut params = add_params_with_json(Some(
        r#"{"key":"k","value":"v","category":"concept","tags":["a"]}"#,
    ));
    params.tags = vec!["extra".to_string()];
    let result = handle_add(&svc, params);
    assert!(matches!(result, Err(Error::ConflictingAddFlags(_))));
}

#[test]
fn handle_add_json_conflicts_with_interactive_flag() {
    let svc = make_svc();
    let mut params = add_params_with_json(Some(
        r#"{"key":"k","value":"v","category":"concept","tags":["a"]}"#,
    ));
    params.interactive = true;
    let result = handle_add(&svc, params);
    assert!(matches!(result, Err(Error::ConflictingAddFlags(_))));
}

#[test]
fn serialize_failed_items_produces_multi_doc_yaml() {
    let items = vec![
        ImportKbItem {
            key: "key-one".to_string(),
            value: "value one".to_string(),
            notes: String::new(),
            category: String::new(),
            reference: String::new(),
            namespace: String::new(),
            tags: Vec::new(),
            parent_key: None,
            path: None,
            media_extension: None,
        },
        ImportKbItem {
            key: "key-two".to_string(),
            value: "value two".to_string(),
            notes: String::new(),
            category: String::new(),
            reference: String::new(),
            namespace: String::new(),
            tags: Vec::new(),
            parent_key: None,
            path: None,
            media_extension: None,
        },
    ];
    let result = serialize_failed_items(&items).unwrap();
    assert!(result.contains("key-one"));
    assert!(result.contains("key-two"));
    assert!(result.contains("---"));
}

// ---------------------------------------------------------------------------
// Graph render helpers
// ---------------------------------------------------------------------------

use crate::domain::{GraphNode, IncomingEdge, OutgoingEdge, TreeRoot};

fn make_graph_node(key: &str, category: &str) -> GraphNode {
    GraphNode {
        id: format!("{key}-id"),
        key: key.to_string(),
        category: category.to_string(),
        namespace: "default".to_string(),
    }
}

#[test]
fn truncate_note_leaves_short_notes_unchanged() {
    assert_eq!(truncate_note("short note"), "short note");
}

#[test]
fn truncate_note_truncates_long_notes_to_40_chars_with_ellipsis() {
    let long = "x".repeat(100);
    let truncated = truncate_note(&long);
    assert_eq!(truncated.chars().count(), 40);
    assert!(truncated.ends_with('…'));
}

#[test]
fn truncate_note_is_char_safe_with_multibyte_input() {
    let multibyte = "é".repeat(100);
    let truncated = truncate_note(&multibyte);
    assert_eq!(truncated.chars().count(), 40);
    assert!(truncated.ends_with('…'));
}

#[test]
fn render_related_shows_both_sections_by_default() {
    let result = RelatedResult {
        node: make_graph_node("car", "concept"),
        outgoing: vec![OutgoingEdge {
            edge_id: "e1".to_string(),
            note: "has an engine".to_string(),
            created_on: "2026-01-01T00:00:00+0000".to_string(),
            to: make_graph_node("engine", "concept"),
        }],
        incoming: vec![IncomingEdge {
            edge_id: "e2".to_string(),
            note: "compatible with car".to_string(),
            created_on: "2026-01-01T00:00:00+0000".to_string(),
            from: make_graph_node("spare-parts-kit", "bookmark"),
        }],
    };
    let output = render_related(&result, EdgeDirection::Both);
    assert!(output.contains("→ car points to (1)"));
    assert!(output.contains("engine"));
    assert!(output.contains("← pointed to by car (1)"));
    assert!(output.contains("spare-parts-kit"));
}

#[test]
fn render_related_direction_out_hides_incoming_section() {
    let result = RelatedResult {
        node: make_graph_node("car", "concept"),
        outgoing: vec![],
        incoming: vec![IncomingEdge {
            edge_id: "e2".to_string(),
            note: "n".to_string(),
            created_on: "2026-01-01T00:00:00+0000".to_string(),
            from: make_graph_node("spare-parts-kit", "bookmark"),
        }],
    };
    let output = render_related(&result, EdgeDirection::Out);
    assert!(output.contains("→ car points to (0)"));
    assert!(!output.contains("pointed to by"));
}

#[test]
fn render_related_direction_in_hides_outgoing_section() {
    let result = RelatedResult {
        node: make_graph_node("car", "concept"),
        outgoing: vec![OutgoingEdge {
            edge_id: "e1".to_string(),
            note: "n".to_string(),
            created_on: "2026-01-01T00:00:00+0000".to_string(),
            to: make_graph_node("engine", "concept"),
        }],
        incoming: vec![],
    };
    let output = render_related(&result, EdgeDirection::In);
    assert!(!output.contains("points to"));
    assert!(output.contains("← pointed to by car (0)"));
}

#[test]
fn resolve_relationship_direction_none_when_no_flags() {
    let flags = RelationshipFlags {
        with_out: false,
        with_in: false,
        with_all: false,
    };
    assert_eq!(resolve_relationship_direction(flags), None);
}

#[test]
fn resolve_relationship_direction_out_only() {
    let flags = RelationshipFlags {
        with_out: true,
        with_in: false,
        with_all: false,
    };
    assert_eq!(
        resolve_relationship_direction(flags),
        Some(EdgeDirection::Out)
    );
}

#[test]
fn resolve_relationship_direction_in_only() {
    let flags = RelationshipFlags {
        with_out: false,
        with_in: true,
        with_all: false,
    };
    assert_eq!(
        resolve_relationship_direction(flags),
        Some(EdgeDirection::In)
    );
}

#[test]
fn resolve_relationship_direction_all_flag() {
    let flags = RelationshipFlags {
        with_out: false,
        with_in: false,
        with_all: true,
    };
    assert_eq!(
        resolve_relationship_direction(flags),
        Some(EdgeDirection::Both)
    );
}

#[test]
fn resolve_relationship_direction_out_and_in_combined() {
    let flags = RelationshipFlags {
        with_out: true,
        with_in: true,
        with_all: false,
    };
    assert_eq!(
        resolve_relationship_direction(flags),
        Some(EdgeDirection::Both)
    );
}

#[test]
fn resolve_relationship_direction_all_wins_over_others() {
    let flags = RelationshipFlags {
        with_out: true,
        with_in: false,
        with_all: true,
    };
    assert_eq!(
        resolve_relationship_direction(flags),
        Some(EdgeDirection::Both)
    );
}

#[test]
fn render_tree_draws_branches_for_a_multi_level_hierarchy() {
    let result = TreeResult {
        root: TreeRoot {
            id: "car-id".to_string(),
            key: "car".to_string(),
        },
        direction: "out".to_string(),
        nodes: vec![
            TreeNode {
                id: "engine-id".to_string(),
                key: "engine".to_string(),
                depth: 1,
                parent_id: "car-id".to_string(),
                note: "car has an engine".to_string(),
            },
            TreeNode {
                id: "chassis-id".to_string(),
                key: "chassis".to_string(),
                depth: 1,
                parent_id: "car-id".to_string(),
                note: "car is built on a chassis".to_string(),
            },
            TreeNode {
                id: "camshaft-id".to_string(),
                key: "camshaft".to_string(),
                depth: 2,
                parent_id: "engine-id".to_string(),
                note: "engine contains camshaft".to_string(),
            },
            TreeNode {
                id: "piston-id".to_string(),
                key: "piston".to_string(),
                depth: 2,
                parent_id: "engine-id".to_string(),
                note: "engine contains piston".to_string(),
            },
        ],
    };
    let output = render_tree(&result);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "car");
    assert!(lines[1].starts_with("├── engine"));
    assert!(lines[1].contains("car has an engine"));
    assert!(lines[2].starts_with("│   ├── camshaft"));
    assert!(lines[2].contains("engine contains camshaft"));
    assert!(lines[3].starts_with("│   └── piston"));
    assert!(lines[4].starts_with("└── chassis"));
    assert!(lines[4].contains("car is built on a chassis"));
}

fn seed_kb(
    svc: &KBService<
        MockKbStore,
        MockVectorStore,
        MockEmbeddingProvider,
        MockMediaStore,
        MockMediaFetcher,
    >,
    key: &str,
) {
    svc.add_kb(NewKb {
        key: key.to_string(),
        value: "test-value".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        reference: String::new(),
        namespace: "rust".to_string(),
        tags: Vec::new(),
        metadata: std::collections::BTreeMap::new(),
        parent: None,
        path: None,
        media_url: None,
        media_extension: None,
    })
    .expect("seed add_kb should succeed");
}

/// A `graph_svc` with no seeded edges — fine for tests that pass no
/// relationship flags, since `direction` then resolves to `None` and
/// `graph_svc.related` is never called.
fn make_empty_graph_svc() -> GraphService<MockKbStore, MockKbGraph> {
    GraphService::new(MockKbStore::new(), MockKbGraph::default())
}

fn update_params_for(id: &str, out: Option<&str>) -> UpdateParams {
    UpdateParams {
        update: KbUpdate {
            id: id.to_string(),
            key: None,
            value: Some("updated-value".to_string()),
            notes: None,
            category: None,
            namespace: None,
            reference: None,
            tags: None,
            parent: None,
            path: None,
            metadata: None,
        },
        out: out.map(str::to_string),
    }
}

#[test]
fn handle_update_no_out_succeeds() {
    let svc = make_svc();
    seed_kb(&svc, "update-test-key");
    let id = svc.get_kb_by_key("update-test-key").unwrap().unwrap().id;
    let result = handle_update(&svc, update_params_for(&id, None));
    assert!(result.is_ok());
}

#[test]
fn handle_update_out_json_succeeds() {
    let svc = make_svc();
    seed_kb(&svc, "update-test-json");
    let id = svc.get_kb_by_key("update-test-json").unwrap().unwrap().id;
    let result = handle_update(&svc, update_params_for(&id, Some("json")));
    assert!(result.is_ok());
}

#[test]
fn handle_update_out_yaml_succeeds() {
    let svc = make_svc();
    seed_kb(&svc, "update-test-yaml");
    let id = svc.get_kb_by_key("update-test-yaml").unwrap().unwrap().id;
    let result = handle_update(&svc, update_params_for(&id, Some("yaml")));
    assert!(result.is_ok());
}

#[test]
fn handle_update_invalid_out_value_returns_error() {
    let svc = make_svc();
    seed_kb(&svc, "update-test-invalid-out");
    let id = svc
        .get_kb_by_key("update-test-invalid-out")
        .unwrap()
        .unwrap()
        .id;
    let result = handle_update(&svc, update_params_for(&id, Some("xml")));
    assert!(matches!(result, Err(Error::UpdateKBError(_))));
}

#[test]
fn handle_get_found_no_out_succeeds() {
    let svc = make_svc();
    seed_kb(&svc, "get-test-key");
    let graph_svc = make_empty_graph_svc();
    let params = GetParams {
        key: Some("get-test-key".to_string()),
        id: None,
        base_dir: String::new(),
        out: None,
        with_out_connections: false,
        with_in_connections: false,
        with_all_connections: false,
    };
    let result = handle_get(
        GetServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(result.is_ok());
}

#[test]
fn handle_get_found_out_json_succeeds() {
    let svc = make_svc();
    seed_kb(&svc, "get-test-json");
    let graph_svc = make_empty_graph_svc();
    let params = GetParams {
        key: Some("get-test-json".to_string()),
        id: None,
        base_dir: String::new(),
        out: Some("json".to_string()),
        with_out_connections: false,
        with_in_connections: false,
        with_all_connections: false,
    };
    let result = handle_get(
        GetServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(result.is_ok());
}

#[test]
fn handle_get_found_out_yaml_succeeds() {
    let svc = make_svc();
    seed_kb(&svc, "get-test-yaml");
    let graph_svc = make_empty_graph_svc();
    let params = GetParams {
        key: Some("get-test-yaml".to_string()),
        id: None,
        base_dir: String::new(),
        out: Some("yaml".to_string()),
        with_out_connections: false,
        with_in_connections: false,
        with_all_connections: false,
    };
    let result = handle_get(
        GetServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(result.is_ok());
}

#[test]
fn handle_get_not_found_returns_ok() {
    let svc = make_svc();
    let graph_svc = make_empty_graph_svc();
    let params = GetParams {
        key: Some("no-such-key".to_string()),
        id: None,
        base_dir: String::new(),
        out: None,
        with_out_connections: false,
        with_in_connections: false,
        with_all_connections: false,
    };
    let result = handle_get(
        GetServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(result.is_ok());
}

#[test]
fn handle_get_not_found_with_out_json_returns_ok() {
    let svc = make_svc();
    let graph_svc = make_empty_graph_svc();
    let params = GetParams {
        key: Some("no-such-key".to_string()),
        id: None,
        base_dir: String::new(),
        out: Some("json".to_string()),
        with_out_connections: false,
        with_in_connections: false,
        with_all_connections: false,
    };
    let result = handle_get(
        GetServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(result.is_ok());
}

#[test]
fn handle_get_invalid_out_value_returns_error() {
    let svc = make_svc();
    seed_kb(&svc, "get-test-invalid-out");
    let graph_svc = make_empty_graph_svc();
    let params = GetParams {
        key: Some("get-test-invalid-out".to_string()),
        id: None,
        base_dir: String::new(),
        out: Some("xml".to_string()),
        with_out_connections: false,
        with_in_connections: false,
        with_all_connections: false,
    };
    let result = handle_get(
        GetServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(matches!(result, Err(Error::GetKBError(_))));
}

#[test]
fn handle_get_no_key_no_id_returns_error() {
    let svc = make_svc();
    let graph_svc = make_empty_graph_svc();
    let params = GetParams {
        key: None,
        id: None,
        base_dir: String::new(),
        out: None,
        with_out_connections: false,
        with_in_connections: false,
        with_all_connections: false,
    };
    let result = handle_get(
        GetServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(matches!(result, Err(Error::GetKBError(_))));
}

fn get_params_for(key: &str, flags: RelationshipFlags) -> GetParams {
    GetParams {
        key: Some(key.to_string()),
        id: None,
        base_dir: String::new(),
        out: None,
        with_out_connections: flags.with_out,
        with_in_connections: flags.with_in,
        with_all_connections: flags.with_all,
    }
}

#[test]
fn handle_get_with_out_connections_only_succeeds() {
    let car = make_kb("car-id", "car");
    let engine = make_kb("engine-id", "engine");
    let graph = MockKbGraph::with_nodes(vec![GraphNode::from(&car), GraphNode::from(&engine)]);
    graph
        .edges
        .borrow_mut()
        .push(make_kb_edge("e1", &car.id, &engine.id, "has an engine"));
    let (svc, graph_svc) = make_get_services(vec![car, engine], graph);

    let params = get_params_for(
        "car",
        RelationshipFlags {
            with_out: true,
            with_in: false,
            with_all: false,
        },
    );
    let result = handle_get(
        GetServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(result.is_ok());
}

#[test]
fn handle_get_with_in_connections_only_succeeds() {
    let car = make_kb("car-id", "car");
    let engine = make_kb("engine-id", "engine");
    let graph = MockKbGraph::with_nodes(vec![GraphNode::from(&car), GraphNode::from(&engine)]);
    graph
        .edges
        .borrow_mut()
        .push(make_kb_edge("e1", &car.id, &engine.id, "has an engine"));
    let (svc, graph_svc) = make_get_services(vec![car, engine], graph);

    let params = get_params_for(
        "engine",
        RelationshipFlags {
            with_out: false,
            with_in: true,
            with_all: false,
        },
    );
    let result = handle_get(
        GetServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(result.is_ok());
}

#[test]
fn handle_get_with_all_connections_succeeds() {
    let car = make_kb("car-id", "car");
    let engine = make_kb("engine-id", "engine");
    let graph = MockKbGraph::with_nodes(vec![GraphNode::from(&car), GraphNode::from(&engine)]);
    graph
        .edges
        .borrow_mut()
        .push(make_kb_edge("e1", &car.id, &engine.id, "has an engine"));
    let (svc, graph_svc) = make_get_services(vec![car, engine], graph);

    let params = get_params_for(
        "car",
        RelationshipFlags {
            with_out: false,
            with_in: false,
            with_all: true,
        },
    );
    let result = handle_get(
        GetServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(result.is_ok());
}

#[test]
fn handle_get_combined_out_and_in_flags_resolves_to_both() {
    let car = make_kb("car-id", "car");
    let engine = make_kb("engine-id", "engine");
    let graph = MockKbGraph::with_nodes(vec![GraphNode::from(&car), GraphNode::from(&engine)]);
    graph
        .edges
        .borrow_mut()
        .push(make_kb_edge("e1", &car.id, &engine.id, "has an engine"));
    let (svc, graph_svc) = make_get_services(vec![car, engine], graph);

    let params = get_params_for(
        "car",
        RelationshipFlags {
            with_out: true,
            with_in: true,
            with_all: false,
        },
    );
    let result = handle_get(
        GetServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(result.is_ok());
}

#[test]
fn handle_get_not_found_with_relationship_flag_still_prints_not_found() {
    let (svc, graph_svc) = make_get_services(vec![], MockKbGraph::default());
    let params = get_params_for(
        "missing",
        RelationshipFlags {
            with_out: false,
            with_in: false,
            with_all: true,
        },
    );
    let result = handle_get(
        GetServices {
            svc: &svc,
            graph_svc: &graph_svc,
        },
        params,
    );
    assert!(result.is_ok());
}

fn search_params(out: Option<&str>) -> SearchParams {
    SearchParams {
        keyword: None,
        category: None,
        namespace: None,
        tags: Vec::new(),
        reference: None,
        limit: 20,
        offset: 0,
        out: out.map(str::to_string),
    }
}

#[test]
fn handle_search_no_out_succeeds() {
    let svc = make_svc();
    seed_kb(&svc, "search-test-plain");
    let result = handle_search(&svc, search_params(None));
    assert!(result.is_ok());
}

#[test]
fn handle_search_out_json_succeeds() {
    let svc = make_svc();
    seed_kb(&svc, "search-test-json");
    let result = handle_search(&svc, search_params(Some("json")));
    assert!(result.is_ok());
}

#[test]
fn handle_search_out_yaml_succeeds() {
    let svc = make_svc();
    seed_kb(&svc, "search-test-yaml");
    let result = handle_search(&svc, search_params(Some("yaml")));
    assert!(result.is_ok());
}

#[test]
fn handle_search_empty_results_out_json_succeeds() {
    let svc = make_svc();
    let result = handle_search(&svc, search_params(Some("json")));
    assert!(result.is_ok());
}

#[test]
fn handle_search_invalid_out_value_returns_error() {
    let svc = make_svc();
    seed_kb(&svc, "search-test-invalid-out");
    let result = handle_search(&svc, search_params(Some("xml")));
    assert!(matches!(result, Err(Error::SearchError(_))));
}

fn ask_params(out: Option<&str>) -> AskParams {
    AskParams {
        query: crate::domain::SemanticQuery {
            text: "test query".to_string(),
            limit: Some(10),
            threshold: Some(0.9),
            category: None,
            namespace: None,
        },
        out: out.map(str::to_string),
    }
}

#[test]
fn handle_ask_no_out_succeeds() {
    let svc = make_svc();
    let result = handle_ask(&svc, ask_params(None));
    assert!(result.is_ok());
}

#[test]
fn handle_ask_out_json_succeeds() {
    let svc = make_svc();
    let result = handle_ask(&svc, ask_params(Some("json")));
    assert!(result.is_ok());
}

#[test]
fn handle_ask_out_yaml_succeeds() {
    let svc = make_svc();
    let result = handle_ask(&svc, ask_params(Some("yaml")));
    assert!(result.is_ok());
}

#[test]
fn handle_ask_invalid_out_value_returns_error() {
    let svc = make_svc();
    let result = handle_ask(&svc, ask_params(Some("xml")));
    assert!(matches!(result, Err(Error::VectorSearchError(_))));
}

// ---- handle_delete tests ----

#[test]
fn handle_delete_plain_text_success() {
    let svc = make_svc();
    let kb = svc
        .add_kb(NewKb {
            key: "test-key".to_string(),
            value: "test value".to_string(),
            notes: String::new(),
            category: "concept".to_string(),
            namespace: "test".to_string(),
            reference: String::new(),
            tags: vec![],
            metadata: std::collections::BTreeMap::new(),
            path: None,
            parent: None,
            media_url: None,
            media_extension: None,
        })
        .unwrap();
    let params = DeleteParams {
        id: kb.id.clone(),
        out: None,
    };
    let result = handle_delete(&svc, params);
    assert!(result.is_ok());
}

#[test]
fn handle_delete_json_success() {
    let svc = make_svc();
    let kb = svc
        .add_kb(NewKb {
            key: "test-key-json".to_string(),
            value: "test value".to_string(),
            notes: String::new(),
            category: "concept".to_string(),
            namespace: "test".to_string(),
            reference: String::new(),
            tags: vec![],
            metadata: std::collections::BTreeMap::new(),
            path: None,
            parent: None,
            media_url: None,
            media_extension: None,
        })
        .unwrap();
    let params = DeleteParams {
        id: kb.id.clone(),
        out: Some("json".to_string()),
    };
    let result = handle_delete(&svc, params);
    assert!(result.is_ok());
}

#[test]
fn handle_delete_invalid_out_value() {
    let svc = make_svc();
    let kb = svc
        .add_kb(NewKb {
            key: "test-key-invalid".to_string(),
            value: "test value".to_string(),
            notes: String::new(),
            category: "concept".to_string(),
            namespace: "test".to_string(),
            reference: String::new(),
            tags: vec![],
            metadata: std::collections::BTreeMap::new(),
            path: None,
            parent: None,
            media_url: None,
            media_extension: None,
        })
        .unwrap();
    let params = DeleteParams {
        id: kb.id.clone(),
        out: Some("xml".to_string()),
    };
    let result = handle_delete(&svc, params);
    assert!(matches!(result, Err(Error::DeleteKBError(_))));
}

#[test]
fn handle_delete_not_found_returns_error() {
    let svc = make_svc();
    let params = DeleteParams {
        id: "nonexistent-id".to_string(),
        out: None,
    };
    let result = handle_delete(&svc, params);
    assert!(matches!(result, Err(Error::KBNotFound)));
}

#[test]
fn handle_delete_not_found_json_returns_error() {
    let svc = make_svc();
    let params = DeleteParams {
        id: "nonexistent-id-json".to_string(),
        out: Some("json".to_string()),
    };
    let result = handle_delete(&svc, params);
    assert!(matches!(result, Err(Error::KBNotFound)));
}

// ---- handle_categories tests ----

#[test]
fn handle_categories_plain_text_success() {
    let svc = make_svc();
    svc.add_kb(NewKb {
        key: "kb-1".to_string(),
        value: "value 1".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        namespace: "test".to_string(),
        reference: String::new(),
        tags: vec![],
        metadata: std::collections::BTreeMap::new(),
        path: None,
        parent: None,
        media_url: None,
        media_extension: None,
    })
    .unwrap();
    svc.add_kb(NewKb {
        key: "kb-2".to_string(),
        value: "value 2".to_string(),
        notes: String::new(),
        category: "bookmark".to_string(),
        namespace: "test".to_string(),
        reference: String::new(),
        tags: vec![],
        metadata: std::collections::BTreeMap::new(),
        path: None,
        parent: None,
        media_url: None,
        media_extension: None,
    })
    .unwrap();
    let params = CategoriesParams {
        namespace: None,
        out: None,
    };
    let result = handle_categories(&svc, params);
    assert!(result.is_ok());
}

#[test]
fn handle_categories_json_success() {
    let svc = make_svc();
    svc.add_kb(NewKb {
        key: "kb-1".to_string(),
        value: "value 1".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        namespace: "test".to_string(),
        reference: String::new(),
        tags: vec![],
        metadata: std::collections::BTreeMap::new(),
        path: None,
        parent: None,
        media_url: None,
        media_extension: None,
    })
    .unwrap();
    let params = CategoriesParams {
        namespace: None,
        out: Some("json".to_string()),
    };
    let result = handle_categories(&svc, params);
    assert!(result.is_ok());
}

#[test]
fn handle_categories_json_empty_success() {
    let svc = make_svc();
    let params = CategoriesParams {
        namespace: None,
        out: Some("json".to_string()),
    };
    let result = handle_categories(&svc, params);
    assert!(result.is_ok());
}

#[test]
fn handle_categories_namespace_filter_json() {
    let svc = make_svc();
    svc.add_kb(NewKb {
        key: "kb-rust".to_string(),
        value: "value".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        namespace: "rust".to_string(),
        reference: String::new(),
        tags: vec![],
        metadata: std::collections::BTreeMap::new(),
        path: None,
        parent: None,
        media_url: None,
        media_extension: None,
    })
    .unwrap();
    svc.add_kb(NewKb {
        key: "kb-k8s".to_string(),
        value: "value".to_string(),
        notes: String::new(),
        category: "command".to_string(),
        namespace: "k8s".to_string(),
        reference: String::new(),
        tags: vec![],
        metadata: std::collections::BTreeMap::new(),
        path: None,
        parent: None,
        media_url: None,
        media_extension: None,
    })
    .unwrap();
    let params = CategoriesParams {
        namespace: Some("rust".to_string()),
        out: Some("json".to_string()),
    };
    let result = handle_categories(&svc, params);
    assert!(result.is_ok());
}

#[test]
fn handle_categories_invalid_out_value() {
    let svc = make_svc();
    let params = CategoriesParams {
        namespace: None,
        out: Some("xml".to_string()),
    };
    let result = handle_categories(&svc, params);
    assert!(matches!(result, Err(Error::ListError(_))));
}

// ---- handle_link tests ----

#[test]
fn handle_link_plain_text_success() {
    let (_svc, graph_svc) = make_get_services(
        vec![make_kb("id-from", "from-key"), make_kb("id-to", "to-key")],
        MockKbGraph::default(),
    );
    let params = LinkParams {
        from_key_or_id: "from-key".to_string(),
        to_key_or_id: "to-key".to_string(),
        note: "related".to_string(),
        out: None,
    };
    let result = handle_link(&graph_svc, params);
    assert!(result.is_ok());
}

#[test]
fn handle_link_json_success() {
    let (_svc, graph_svc) = make_get_services(
        vec![make_kb("id-from", "from-key"), make_kb("id-to", "to-key")],
        MockKbGraph::default(),
    );
    let params = LinkParams {
        from_key_or_id: "from-key".to_string(),
        to_key_or_id: "to-key".to_string(),
        note: "related".to_string(),
        out: Some("json".to_string()),
    };
    let result = handle_link(&graph_svc, params);
    assert!(result.is_ok());
}

#[test]
fn handle_link_invalid_out_value_returns_error() {
    let (_svc, graph_svc) = make_get_services(
        vec![make_kb("id-from", "from-key"), make_kb("id-to", "to-key")],
        MockKbGraph::default(),
    );
    let params = LinkParams {
        from_key_or_id: "from-key".to_string(),
        to_key_or_id: "to-key".to_string(),
        note: String::new(),
        out: Some("xml".to_string()),
    };
    let result = handle_link(&graph_svc, params);
    assert!(matches!(result, Err(Error::AddEdgeError(_))));
}

#[test]
fn handle_link_not_found_returns_error() {
    let (_svc, graph_svc) = make_get_services(
        vec![make_kb("id-from", "from-key"), make_kb("id-to", "to-key")],
        MockKbGraph::default(),
    );
    let params = LinkParams {
        from_key_or_id: "unknown-key".to_string(),
        to_key_or_id: "to-key".to_string(),
        note: String::new(),
        out: None,
    };
    let result = handle_link(&graph_svc, params);
    assert!(matches!(result, Err(Error::KBNotFound)));
}

#[test]
fn handle_link_self_loop_returns_error() {
    let (_svc, graph_svc) = make_get_services(
        vec![make_kb("id-from", "from-key"), make_kb("id-to", "to-key")],
        MockKbGraph::default(),
    );
    let params = LinkParams {
        from_key_or_id: "from-key".to_string(),
        to_key_or_id: "from-key".to_string(),
        note: String::new(),
        out: None,
    };
    let result = handle_link(&graph_svc, params);
    assert!(matches!(result, Err(Error::SelfLoopNotAllowed)));
}

#[test]
fn handle_link_self_loop_json_returns_error() {
    let (_svc, graph_svc) = make_get_services(
        vec![make_kb("id-from", "from-key"), make_kb("id-to", "to-key")],
        MockKbGraph::default(),
    );
    let params = LinkParams {
        from_key_or_id: "from-key".to_string(),
        to_key_or_id: "from-key".to_string(),
        note: String::new(),
        out: Some("json".to_string()),
    };
    let result = handle_link(&graph_svc, params);
    assert!(matches!(result, Err(Error::SelfLoopNotAllowed)));
}
