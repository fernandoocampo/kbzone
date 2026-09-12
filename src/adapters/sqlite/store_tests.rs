use super::*;

fn in_memory_store() -> SqliteStore {
    register_vec_extension();
    let conn = Connection::open_in_memory()
        .map_err(|e| Error::StorageInitError(e.to_string()))
        .expect("in-memory connection");
    SqliteStore {
        conn: Arc::new(Mutex::new(conn)),
    }
}

fn make_kb(id: &str, key: &str) -> Kb {
    Kb {
        id: id.to_string(),
        key: key.to_string(),
        value: "test value".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        reference: String::new(),
        namespace: "default".to_string(),
        tags: vec!["rust".to_string(), "memory".to_string()],
        created_on: "2026-01-01T00:00:00+0000".to_string(),
        parent: None,
        path: None,
        media_extension: None,
    }
}

fn initialized_store() -> SqliteStore {
    let store = in_memory_store();
    store.initialize().expect("initialize");
    store
}

#[test]
fn initialize_is_idempotent() {
    let store = in_memory_store();
    assert!(store.initialize().is_ok());
    assert!(store.initialize().is_ok());
}

#[test]
fn save_and_get_by_id() {
    let store = initialized_store();
    let kb = make_kb("id-1", "rust-ownership");
    store.save_kb(&kb).unwrap();
    let fetched = store.get_kb_by_id("id-1").unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().key, "rust-ownership");
}

#[test]
fn save_and_get_by_key() {
    let store = initialized_store();
    let kb = make_kb("id-1", "rust-ownership");
    store.save_kb(&kb).unwrap();
    let fetched = store.get_kb_by_key("rust-ownership").unwrap();
    assert!(fetched.is_some());
}

#[test]
fn get_by_id_returns_none_for_missing() {
    let store = initialized_store();
    assert_eq!(store.get_kb_by_id("no-such").unwrap(), None);
}

#[test]
fn get_kbs_returns_all_saved_entries() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1", "key-a")).unwrap();
    store.save_kb(&make_kb("id-2", "key-b")).unwrap();
    let filter = KbFilter::default();
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn get_kbs_filters_by_category() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.category = "bookmark".to_string();
    let kb2 = make_kb("id-2", "key-b"); // category = "concept"
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        category: Some("bookmark".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "key-a");
}

#[test]
fn get_kbs_with_limit_and_offset() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1", "key-a")).unwrap();
    store.save_kb(&make_kb("id-2", "key-b")).unwrap();
    store.save_kb(&make_kb("id-3", "key-c")).unwrap();

    let filter = KbFilter {
        limit: Some(2),
        offset: Some(0),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn get_kbs_filters_by_reference_without_keyword() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.reference = "The Rust Book".to_string();
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.reference = "other source".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        reference: Some("rust book".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "key-a");
}

#[test]
fn update_kb_changes_value() {
    let store = initialized_store();
    let mut kb = make_kb("id-1", "rust-ownership");
    store.save_kb(&kb).unwrap();
    kb.value = "updated value".to_string();
    assert!(store.update_kb(&kb).unwrap());
    let fetched = store.get_kb_by_id("id-1").unwrap().unwrap();
    assert_eq!(fetched.value, "updated value");
}

#[test]
fn delete_kb_removes_entry() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1", "rust-ownership")).unwrap();
    assert!(store.delete_kb("id-1").unwrap());
    assert_eq!(store.get_kb_by_id("id-1").unwrap(), None);
}

#[test]
fn delete_kb_returns_false_for_missing() {
    let store = initialized_store();
    assert!(!store.delete_kb("no-such").unwrap());
}

#[test]
fn get_kbs_via_fts5_keyword() {
    let store = initialized_store();
    let mut kb = make_kb("id-1", "rust-ownership");
    kb.tags = vec!["memory".to_string(), "rust".to_string()];
    store.save_kb(&kb).unwrap();

    let filter = KbFilter {
        keyword: Some("memo".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs(&filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "rust-ownership");
}

#[test]
fn get_kbs_with_keyword_and_reference_filter() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "rust-ownership");
    kb1.tags = vec!["rust".to_string()];
    kb1.reference = "The Rust Book".to_string();
    let mut kb2 = make_kb("id-2", "rust-lifetimes");
    kb2.tags = vec!["rust".to_string()];
    kb2.reference = "other source".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        keyword: Some("rust".to_string()),
        reference: Some("The Rust Book".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs(&filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "rust-ownership");
}

#[test]
fn get_kbs_with_keyword_reference_filter_returns_empty_when_no_match() {
    let store = initialized_store();
    let mut kb = make_kb("id-1", "rust-ownership");
    kb.tags = vec!["rust".to_string()];
    kb.reference = "The Rust Book".to_string();
    store.save_kb(&kb).unwrap();

    let filter = KbFilter {
        keyword: Some("rust".to_string()),
        reference: Some("nonexistent".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs(&filter).unwrap();
    assert!(results.is_empty());
}

#[test]
fn get_kbs_with_keyword_and_limit() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "rust-ownership");
    kb1.tags = vec!["rust".to_string()];
    let mut kb2 = make_kb("id-2", "rust-lifetimes");
    kb2.tags = vec!["rust".to_string()];
    let mut kb3 = make_kb("id-3", "rust-borrowing");
    kb3.tags = vec!["rust".to_string()];
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let filter = KbFilter {
        keyword: Some("rust".to_string()),
        limit: Some(2),
        ..Default::default()
    };
    let results = store.get_kbs(&filter).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn random_quote_returns_a_quote_category_entry() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-q1", "stoic-quote");
    kb1.category = "quote".to_string();
    let mut kb2 = make_kb("id-q2", "zen-quote");
    kb2.category = "quote".to_string();
    let kb3 = make_kb("id-c1", "rust-concept"); // category = "concept"
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let result = store.random_quote().unwrap();
    assert_eq!(result.category.to_lowercase(), "quote");
}

#[test]
fn random_quote_returns_not_found_when_no_quotes_exist() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-c1", "rust-concept")).unwrap();

    let result = store.random_quote();
    assert!(matches!(result, Err(Error::QuoteNotFound)));
}

fn initialized_store_with_vectors(dims: usize) -> SqliteStore {
    let store = in_memory_store();
    store.initialize().expect("initialize");
    store.initialize_vectors(dims).expect("initialize_vectors");
    store
}

#[test]
fn initialize_vectors_is_idempotent() {
    let store = in_memory_store();
    store.initialize().unwrap();
    assert!(store.initialize_vectors(4).is_ok());
    assert!(store.initialize_vectors(4).is_ok());
}

#[test]
fn save_and_delete_embedding() {
    let store = initialized_store_with_vectors(4);
    let kb = make_kb("id-1", "rust-ownership");
    store.save_kb(&kb).unwrap();

    let input = EmbeddingInput {
        kb_id: "id-1".to_string(),
        text: "rust ownership concept".to_string(),
    };
    let embedding = vec![0.1f32, 0.2, 0.3, 0.4];
    store.save_embedding(&input, &embedding).unwrap();
    assert!(store.delete_embedding("id-1").is_ok());
}

#[test]
fn search_similar_returns_closest_entry() {
    let store = initialized_store_with_vectors(4);
    let kb = make_kb("id-1", "rust-ownership");
    store.save_kb(&kb).unwrap();

    let input = EmbeddingInput {
        kb_id: "id-1".to_string(),
        text: "rust ownership".to_string(),
    };
    let embedding = vec![1.0f32, 0.0, 0.0, 0.0];
    store.save_embedding(&input, &embedding).unwrap();

    let query_embedding = vec![1.0f32, 0.0, 0.0, 0.0];
    let query = SemanticQuery {
        text: "rust ownership".to_string(),
        limit: Some(5),
        threshold: None,
    };
    let results = store.search_similar(&query, &query_embedding).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].item.key, "rust-ownership");
}

#[test]
fn get_children_ids_returns_correct_children() {
    let store = initialized_store();
    let parent = make_kb("parent-id", "parent-key");
    let mut child1 = make_kb("child-id-1", "child-key-1");
    child1.parent = Some("parent-id".to_string());
    let mut child2 = make_kb("child-id-2", "child-key-2");
    child2.parent = Some("parent-id".to_string());
    let unrelated = make_kb("other-id", "other-key");
    store.save_kb(&parent).unwrap();
    store.save_kb(&child1).unwrap();
    store.save_kb(&child2).unwrap();
    store.save_kb(&unrelated).unwrap();

    let children = store.get_children_ids("parent-id").unwrap();
    assert_eq!(children.len(), 2);
    assert!(children.contains(&"child-id-1".to_string()));
    assert!(children.contains(&"child-id-2".to_string()));
}

#[test]
fn get_children_ids_returns_empty_for_childless_entry() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1", "key-1")).unwrap();
    let children = store.get_children_ids("id-1").unwrap();
    assert!(children.is_empty());
}

#[test]
fn save_and_retrieve_kb_with_parent() {
    let store = initialized_store();
    let parent = make_kb("parent-id", "parent-key");
    let mut child = make_kb("child-id", "child-key");
    child.parent = Some("parent-id".to_string());
    store.save_kb(&parent).unwrap();
    store.save_kb(&child).unwrap();

    let fetched = store.get_kb_by_id("child-id").unwrap().unwrap();
    assert_eq!(fetched.parent, Some("parent-id".to_string()));
}

#[test]
fn search_similar_threshold_excludes_distant_entries() {
    let store = initialized_store_with_vectors(4);
    let kb1 = make_kb("id-1", "rust-ownership");
    let kb2 = make_kb("id-2", "go-channels");
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    // id-1: identical to query vector (distance = 0)
    store
        .save_embedding(
            &EmbeddingInput {
                kb_id: "id-1".to_string(),
                text: "rust ownership".to_string(),
            },
            &[1.0f32, 0.0, 0.0, 0.0],
        )
        .unwrap();
    // id-2: orthogonal (distance will be high)
    store
        .save_embedding(
            &EmbeddingInput {
                kb_id: "id-2".to_string(),
                text: "go channels".to_string(),
            },
            &[0.0f32, 1.0, 0.0, 0.0],
        )
        .unwrap();

    let query_embedding = vec![1.0f32, 0.0, 0.0, 0.0];
    // Strict threshold: only id-1 (distance ~0) should pass
    let query = SemanticQuery {
        text: "rust ownership".to_string(),
        limit: Some(10),
        threshold: Some(0.5),
    };
    let results = store.search_similar(&query, &query_embedding).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].item.key, "rust-ownership");
}

#[test]
fn get_kbs_full_returns_all_fields() {
    let store = initialized_store();
    let mut kb = make_kb("id-1", "rust-ownership");
    kb.notes = "some notes".to_string();
    kb.reference = "The Rust Book".to_string();
    store.save_kb(&kb).unwrap();

    let results = store.get_kbs_full(&KbFilter::default()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "rust-ownership");
    assert_eq!(results[0].value, "test value");
    assert_eq!(results[0].notes, "some notes");
    assert_eq!(results[0].reference, "The Rust Book");
    assert_eq!(results[0].category, "concept");
    assert_eq!(results[0].namespace, "default");
}

#[test]
fn get_kbs_full_filters_by_category() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1", "key-a")).unwrap(); // category = "concept"
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.category = "quote".to_string();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        category: Some("quote".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs_full(&filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "key-b");
}

#[test]
fn get_kbs_full_respects_limit() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1", "key-a")).unwrap();
    store.save_kb(&make_kb("id-2", "key-b")).unwrap();
    store.save_kb(&make_kb("id-3", "key-c")).unwrap();

    let filter = KbFilter {
        limit: Some(2),
        ..Default::default()
    };
    let results = store.get_kbs_full(&filter).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn get_categories_returns_distinct_sorted_categories() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.category = "zebra".to_string();
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.category = "apple".to_string();
    let mut kb3 = make_kb("id-3", "key-c");
    kb3.category = "apple".to_string(); // duplicate
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let categories = store.get_categories(None).unwrap();
    assert_eq!(categories, vec!["apple".to_string(), "zebra".to_string()]);
}

#[test]
fn get_categories_excludes_empty_category() {
    let store = initialized_store();
    let mut kb = make_kb("id-1", "key-a");
    kb.category = String::new();
    store.save_kb(&kb).unwrap();

    let categories = store.get_categories(None).unwrap();
    assert!(categories.is_empty());
}

#[test]
fn get_categories_filters_by_namespace_when_given() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.category = "concept".to_string();
    kb1.namespace = "rust".to_string();
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.category = "quote".to_string();
    kb2.namespace = "personal".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let categories = store.get_categories(Some("rust")).unwrap();
    assert_eq!(categories, vec!["concept".to_string()]);
}

#[test]
fn get_categories_returns_empty_vec_when_no_entries() {
    let store = initialized_store();
    let categories = store.get_categories(None).unwrap();
    assert!(categories.is_empty());
}

// ---------------------------------------------------------------------------
// KbGraph tests
// ---------------------------------------------------------------------------

fn initialized_graph_store() -> SqliteStore {
    let store = initialized_store();
    store.initialize_graph().expect("initialize_graph");
    store
}

fn make_edge(id: &str, from_id: &str, to_id: &str, note: &str) -> KbEdge {
    KbEdge {
        id: id.to_string(),
        from_id: from_id.to_string(),
        to_id: to_id.to_string(),
        note: note.to_string(),
        created_on: "2026-01-01T00:00:00+0000".to_string(),
    }
}

#[test]
fn initialize_graph_is_idempotent() {
    let store = initialized_store();
    assert!(store.initialize_graph().is_ok());
    assert!(store.initialize_graph().is_ok());
}

#[test]
fn add_edge_then_get_related_out() {
    let store = initialized_graph_store();
    store.save_kb(&make_kb("car-id", "car")).unwrap();
    store.save_kb(&make_kb("engine-id", "engine")).unwrap();
    store
        .add_edge(&make_edge("edge-1", "car-id", "engine-id", "has an engine"))
        .unwrap();

    let related = store
        .get_related(&RelatedQuery {
            kb_id: "car-id".to_string(),
            direction: EdgeDirection::Out,
        })
        .unwrap();
    assert_eq!(related.outgoing.len(), 1);
    assert_eq!(related.outgoing[0].to.key, "engine");
    assert_eq!(related.outgoing[0].note, "has an engine");
    assert!(related.incoming.is_empty());
}

#[test]
fn add_edge_then_get_related_in() {
    let store = initialized_graph_store();
    store.save_kb(&make_kb("car-id", "car")).unwrap();
    store.save_kb(&make_kb("engine-id", "engine")).unwrap();
    store
        .add_edge(&make_edge("edge-1", "car-id", "engine-id", "has an engine"))
        .unwrap();

    let related = store
        .get_related(&RelatedQuery {
            kb_id: "engine-id".to_string(),
            direction: EdgeDirection::In,
        })
        .unwrap();
    assert_eq!(related.incoming.len(), 1);
    assert_eq!(related.incoming[0].from.key, "car");
    assert!(related.outgoing.is_empty());
}

#[test]
fn add_edge_then_get_related_both() {
    let store = initialized_graph_store();
    store.save_kb(&make_kb("car-id", "car")).unwrap();
    store.save_kb(&make_kb("engine-id", "engine")).unwrap();
    store
        .save_kb(&make_kb("kit-id", "spare-parts-kit"))
        .unwrap();
    store
        .add_edge(&make_edge("edge-1", "car-id", "engine-id", "has an engine"))
        .unwrap();
    store
        .add_edge(&make_edge(
            "edge-2",
            "kit-id",
            "car-id",
            "compatible with car",
        ))
        .unwrap();

    let related = store
        .get_related(&RelatedQuery {
            kb_id: "car-id".to_string(),
            direction: EdgeDirection::Both,
        })
        .unwrap();
    assert_eq!(related.outgoing.len(), 1);
    assert_eq!(related.incoming.len(), 1);
}

#[test]
fn add_edge_duplicate_returns_duplicate_edge_error() {
    let store = initialized_graph_store();
    store.save_kb(&make_kb("car-id", "car")).unwrap();
    store.save_kb(&make_kb("engine-id", "engine")).unwrap();
    store
        .add_edge(&make_edge("edge-1", "car-id", "engine-id", "has an engine"))
        .unwrap();

    let result = store.add_edge(&make_edge("edge-2", "car-id", "engine-id", "again"));
    assert!(matches!(result, Err(Error::DuplicateEdgeError)));
}

#[test]
fn remove_edge_returns_true_when_deleted() {
    let store = initialized_graph_store();
    store.save_kb(&make_kb("car-id", "car")).unwrap();
    store.save_kb(&make_kb("engine-id", "engine")).unwrap();
    store
        .add_edge(&make_edge("edge-1", "car-id", "engine-id", "has an engine"))
        .unwrap();

    let removed = store
        .remove_edge(&RemoveEdgeParams {
            from_id: "car-id".to_string(),
            to_id: "engine-id".to_string(),
        })
        .unwrap();
    assert!(removed);
}

#[test]
fn remove_edge_returns_false_when_missing() {
    let store = initialized_graph_store();
    let removed = store
        .remove_edge(&RemoveEdgeParams {
            from_id: "no-such-1".to_string(),
            to_id: "no-such-2".to_string(),
        })
        .unwrap();
    assert!(!removed);
}

#[test]
fn deleting_a_kb_cascades_edges_via_kbs_ad_edges_trigger() {
    let store = initialized_graph_store();
    store.save_kb(&make_kb("car-id", "car")).unwrap();
    store.save_kb(&make_kb("engine-id", "engine")).unwrap();
    store
        .add_edge(&make_edge("edge-1", "car-id", "engine-id", "has an engine"))
        .unwrap();

    store.delete_kb("engine-id").unwrap();

    let related = store
        .get_related(&RelatedQuery {
            kb_id: "car-id".to_string(),
            direction: EdgeDirection::Out,
        })
        .unwrap();
    assert!(related.outgoing.is_empty());
}

#[test]
fn get_tree_respects_depth_bound() {
    let store = initialized_graph_store();
    // car -> engine -> camshaft -> bolt -> thread (chain of 5 nodes, 4 edges)
    for (id, key) in [
        ("car-id", "car"),
        ("engine-id", "engine"),
        ("camshaft-id", "camshaft"),
        ("bolt-id", "bolt"),
        ("thread-id", "thread"),
    ] {
        store.save_kb(&make_kb(id, key)).unwrap();
    }
    store
        .add_edge(&make_edge("e1", "car-id", "engine-id", "n"))
        .unwrap();
    store
        .add_edge(&make_edge("e2", "engine-id", "camshaft-id", "n"))
        .unwrap();
    store
        .add_edge(&make_edge("e3", "camshaft-id", "bolt-id", "n"))
        .unwrap();
    store
        .add_edge(&make_edge("e4", "bolt-id", "thread-id", "n"))
        .unwrap();

    let nodes = store
        .get_tree(&TreeQuery {
            kb_id: "car-id".to_string(),
            direction: EdgeDirection::Out,
            depth: 2,
        })
        .unwrap();
    assert_eq!(nodes.len(), 2);
    assert!(nodes.iter().all(|n| n.depth <= 2));
}

#[test]
fn get_tree_direction_in_walks_backwards() {
    let store = initialized_graph_store();
    store.save_kb(&make_kb("car-id", "car")).unwrap();
    store.save_kb(&make_kb("engine-id", "engine")).unwrap();
    store
        .add_edge(&make_edge("e1", "car-id", "engine-id", "has an engine"))
        .unwrap();

    let nodes = store
        .get_tree(&TreeQuery {
            kb_id: "engine-id".to_string(),
            direction: EdgeDirection::In,
            depth: 10,
        })
        .unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].key, "car");
    assert_eq!(nodes[0].parent_id, "engine-id");
}

#[test]
fn get_tree_terminates_on_cycle() {
    let store = initialized_graph_store();
    store.save_kb(&make_kb("a-id", "a")).unwrap();
    store.save_kb(&make_kb("b-id", "b")).unwrap();
    store
        .add_edge(&make_edge("e1", "a-id", "b-id", "n"))
        .unwrap();
    store
        .add_edge(&make_edge("e2", "b-id", "a-id", "n"))
        .unwrap();

    let nodes = store
        .get_tree(&TreeQuery {
            kb_id: "a-id".to_string(),
            direction: EdgeDirection::Out,
            depth: 5,
        })
        .unwrap();
    // Bounded by depth, not hanging — exactly one row per depth level.
    assert_eq!(nodes.len(), 5);
    assert!(nodes.iter().all(|n| n.depth <= 5));
}

#[test]
fn get_tree_rejects_both_direction() {
    let store = initialized_graph_store();
    store.save_kb(&make_kb("car-id", "car")).unwrap();
    let result = store.get_tree(&TreeQuery {
        kb_id: "car-id".to_string(),
        direction: EdgeDirection::Both,
        depth: 10,
    });
    assert!(matches!(result, Err(Error::GraphQueryError(_))));
}

#[test]
fn get_edges_among_ids_returns_empty_for_empty_slice() {
    let store = initialized_graph_store();
    let edges = store.get_edges_among_ids(&[]).unwrap();
    assert!(edges.is_empty());
}

#[test]
fn get_edges_among_ids_returns_edge_when_both_endpoints_present() {
    let store = initialized_graph_store();
    store
        .add_edge(&make_edge("edge-1", "car-id", "engine-id", "has an engine"))
        .unwrap();
    let ids = vec!["car-id".to_string(), "engine-id".to_string()];
    let edges = store.get_edges_among_ids(&ids).unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].from_id, "car-id");
    assert_eq!(edges[0].to_id, "engine-id");
}

#[test]
fn get_edges_among_ids_omits_edge_when_one_endpoint_missing() {
    let store = initialized_graph_store();
    store
        .add_edge(&make_edge("edge-1", "car-id", "engine-id", "has an engine"))
        .unwrap();
    // "engine-id" deliberately excluded — simulates a filter split.
    let ids = vec!["car-id".to_string()];
    let edges = store.get_edges_among_ids(&ids).unwrap();
    assert!(edges.is_empty());
}

#[test]
fn get_edges_among_ids_ignores_edges_entirely_outside_the_set() {
    let store = initialized_graph_store();
    store
        .add_edge(&make_edge("edge-1", "car-id", "engine-id", "n"))
        .unwrap();
    store
        .add_edge(&make_edge("edge-2", "other-a", "other-b", "n"))
        .unwrap();
    let ids = vec!["car-id".to_string(), "engine-id".to_string()];
    let edges = store.get_edges_among_ids(&ids).unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].id, "edge-1");
}

#[test]
fn get_edges_among_ids_returns_multiple_edges_within_the_set() {
    let store = initialized_graph_store();
    store
        .add_edge(&make_edge("edge-1", "a-id", "b-id", "n1"))
        .unwrap();
    store
        .add_edge(&make_edge("edge-2", "b-id", "c-id", "n2"))
        .unwrap();
    let ids = vec!["a-id".to_string(), "b-id".to_string(), "c-id".to_string()];
    let edges = store.get_edges_among_ids(&ids).unwrap();
    assert_eq!(edges.len(), 2);
}
