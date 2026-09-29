use super::*;

fn in_memory_store() -> SqliteStore {
    register_vec_extension();
    let conn = Connection::open_in_memory()
        .map_err(|e| Error::StorageInitError(e.to_string()))
        .expect("in-memory connection");
    SqliteStore {
        conn: Arc::new(Mutex::new(conn)),
        db_path: ":memory:".to_string(),
    }
}

fn make_kb(id: &str) -> Kb {
    Kb {
        id: id.to_string(),
        value: "test value".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        reference: String::new(),
        namespace: "default".to_string(),
        tags: vec!["rust".to_string(), "memory".to_string()],
        metadata: std::collections::BTreeMap::new(),
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
    let kb = make_kb("id-1");
    store.save_kb(&kb).unwrap();
    let fetched = store.get_kb_by_id("id-1").unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().id, "id-1");
}

#[test]
fn get_by_id_returns_none_for_missing() {
    let store = initialized_store();
    assert_eq!(store.get_kb_by_id("no-such").unwrap(), None);
}

#[test]
fn get_kbs_returns_all_saved_entries() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1")).unwrap();
    store.save_kb(&make_kb("id-2")).unwrap();
    let filter = KbFilter::default();
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn get_kbs_filters_by_category() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.category = "bookmark".to_string();
    let kb2 = make_kb("id-2"); // category = "concept"
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        category: Some("bookmark".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "id-1");
}

#[test]
fn get_kbs_with_limit_and_offset() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1")).unwrap();
    store.save_kb(&make_kb("id-2")).unwrap();
    store.save_kb(&make_kb("id-3")).unwrap();

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
    let mut kb1 = make_kb("id-1");
    kb1.reference = "The Rust Book".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.reference = "other source".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        reference: Some("rust book".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "id-1");
}

#[test]
fn update_kb_changes_value() {
    let store = initialized_store();
    let mut kb = make_kb("id-1");
    store.save_kb(&kb).unwrap();
    kb.value = "updated value".to_string();
    assert!(store.update_kb(&kb).unwrap());
    let fetched = store.get_kb_by_id("id-1").unwrap().unwrap();
    assert_eq!(fetched.value, "updated value");
}

#[test]
fn delete_kb_removes_entry() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1")).unwrap();
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
    let mut kb = make_kb("id-1");
    kb.tags = vec!["memory".to_string(), "rust".to_string()];
    store.save_kb(&kb).unwrap();

    let filter = KbFilter {
        keyword: Some("memo".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs(&filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "id-1");
}

#[test]
fn get_kbs_with_keyword_and_reference_filter() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.tags = vec!["rust".to_string()];
    kb1.reference = "The Rust Book".to_string();
    let mut kb2 = make_kb("id-2");
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
    assert_eq!(results[0].id, "id-1");
}

#[test]
fn get_kbs_with_keyword_reference_filter_returns_empty_when_no_match() {
    let store = initialized_store();
    let mut kb = make_kb("id-1");
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
    let mut kb1 = make_kb("id-1");
    kb1.tags = vec!["rust".to_string()];
    let mut kb2 = make_kb("id-2");
    kb2.tags = vec!["rust".to_string()];
    let mut kb3 = make_kb("id-3");
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
fn random_by_category_returns_matching_entry() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-q1");
    kb1.category = "quote".to_string();
    let mut kb2 = make_kb("id-q2");
    kb2.category = "quote".to_string();
    let kb3 = make_kb("id-c1"); // category = "concept"
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let result = store.random_by_category("quote", None).unwrap();
    assert_eq!(result.category.to_lowercase(), "quote");
}

#[test]
fn random_by_category_is_case_insensitive() {
    let store = initialized_store();
    let mut kb = make_kb("id-q1");
    kb.category = "QUOTE".to_string();
    store.save_kb(&kb).unwrap();

    let result = store.random_by_category("quote", None).unwrap();
    assert_eq!(result.id, "id-q1");
}

#[test]
fn random_by_category_returns_not_found_when_category_absent() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-c1")).unwrap();

    let result = store.random_by_category("nonexistent", None);
    assert!(matches!(result, Err(Error::RandomNotFound(_))));
}

#[test]
fn random_by_category_respects_namespace_filter() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-q1");
    kb1.category = "quote".to_string();
    kb1.namespace = "ns1".to_string();
    let mut kb2 = make_kb("id-q2");
    kb2.category = "quote".to_string();
    kb2.namespace = "ns2".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let result = store.random_by_category("quote", Some("ns1")).unwrap();
    assert_eq!(result.id, "id-q1");
    assert_eq!(result.namespace, "ns1");
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
    let kb = make_kb("id-1");
    store.save_kb(&kb).unwrap();

    let input = EmbeddingInput {
        kb_id: "id-1".to_string(),
        text: "rust ownership concept".to_string(),
        namespace: "default".to_string(),
    };
    let embedding = vec![0.1f32, 0.2, 0.3, 0.4];
    store.save_embedding(&input, &embedding).unwrap();
    assert!(store.delete_embedding("id-1").is_ok());
}

#[test]
fn search_similar_returns_closest_entry() {
    let store = initialized_store_with_vectors(4);
    let kb = make_kb("id-1");
    store.save_kb(&kb).unwrap();

    let input = EmbeddingInput {
        kb_id: "id-1".to_string(),
        text: "rust ownership".to_string(),
        namespace: "default".to_string(),
    };
    let embedding = vec![1.0f32, 0.0, 0.0, 0.0];
    store.save_embedding(&input, &embedding).unwrap();

    let query_embedding = vec![1.0f32, 0.0, 0.0, 0.0];
    let query = SemanticQuery {
        text: "rust ownership".to_string(),
        limit: Some(5),
        threshold: None,
        category: None,
        namespace: None,
    };
    let results = store.search_similar(&query, &query_embedding).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].item.id, "id-1");
}

#[test]
fn get_children_ids_returns_correct_children() {
    let store = initialized_store();
    let parent = make_kb("parent-id");
    let mut child1 = make_kb("child-id-1");
    child1.parent = Some("parent-id".to_string());
    let mut child2 = make_kb("child-id-2");
    child2.parent = Some("parent-id".to_string());
    let unrelated = make_kb("other-id");
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
    store.save_kb(&make_kb("id-1")).unwrap();
    let children = store.get_children_ids("id-1").unwrap();
    assert!(children.is_empty());
}

#[test]
fn save_and_retrieve_kb_with_parent() {
    let store = initialized_store();
    let parent = make_kb("parent-id");
    let mut child = make_kb("child-id");
    child.parent = Some("parent-id".to_string());
    store.save_kb(&parent).unwrap();
    store.save_kb(&child).unwrap();

    let fetched = store.get_kb_by_id("child-id").unwrap().unwrap();
    assert_eq!(fetched.parent, Some("parent-id".to_string()));
}

#[test]
fn search_similar_threshold_excludes_distant_entries() {
    let store = initialized_store_with_vectors(4);
    let kb1 = make_kb("id-1");
    let kb2 = make_kb("id-2");
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    // id-1: identical to query vector (distance = 0)
    store
        .save_embedding(
            &EmbeddingInput {
                kb_id: "id-1".to_string(),
                text: "rust ownership".to_string(),
                namespace: "default".to_string(),
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
                namespace: "default".to_string(),
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
        category: None,
        namespace: None,
    };
    let results = store.search_similar(&query, &query_embedding).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].item.id, "id-1");
}

#[test]
fn search_similar_filters_by_namespace_partition() {
    let store = initialized_store_with_vectors(4);
    let mut kb1 = make_kb("id-1");
    kb1.namespace = "work".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.namespace = "personal".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    // Both entries embed identically close to the query vector.
    store
        .save_embedding(
            &EmbeddingInput {
                kb_id: "id-1".to_string(),
                text: "note".to_string(),
                namespace: "work".to_string(),
            },
            &[1.0f32, 0.0, 0.0, 0.0],
        )
        .unwrap();
    store
        .save_embedding(
            &EmbeddingInput {
                kb_id: "id-2".to_string(),
                text: "note".to_string(),
                namespace: "personal".to_string(),
            },
            &[1.0f32, 0.0, 0.0, 0.0],
        )
        .unwrap();

    let query = SemanticQuery {
        text: "note".to_string(),
        limit: Some(10),
        threshold: None,
        category: None,
        namespace: Some("work".to_string()),
    };
    let results = store
        .search_similar(&query, &[1.0f32, 0.0, 0.0, 0.0])
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].item.id, "id-1");
}

#[test]
fn search_similar_filters_by_category_post_filter() {
    let store = initialized_store_with_vectors(4);
    let mut kb1 = make_kb("id-1");
    kb1.category = "concept".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.category = "quote".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    for id in ["id-1", "id-2"] {
        store
            .save_embedding(
                &EmbeddingInput {
                    kb_id: id.to_string(),
                    text: "note".to_string(),
                    namespace: "default".to_string(),
                },
                &[1.0f32, 0.0, 0.0, 0.0],
            )
            .unwrap();
    }

    let query = SemanticQuery {
        text: "note".to_string(),
        limit: Some(10),
        threshold: None,
        category: Some("quote".to_string()),
        namespace: None,
    };
    let results = store
        .search_similar(&query, &[1.0f32, 0.0, 0.0, 0.0])
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].item.id, "id-2");
}

#[test]
fn search_similar_combines_namespace_and_category_with_and_semantics() {
    let store = initialized_store_with_vectors(4);
    let mut work_concept = make_kb("id-1");
    work_concept.namespace = "work".to_string();
    work_concept.category = "concept".to_string();
    let mut work_quote = make_kb("id-2");
    work_quote.namespace = "work".to_string();
    work_quote.category = "quote".to_string();
    let mut personal_concept = make_kb("id-3");
    personal_concept.namespace = "personal".to_string();
    personal_concept.category = "concept".to_string();
    store.save_kb(&work_concept).unwrap();
    store.save_kb(&work_quote).unwrap();
    store.save_kb(&personal_concept).unwrap();

    for (id, ns) in [("id-1", "work"), ("id-2", "work"), ("id-3", "personal")] {
        store
            .save_embedding(
                &EmbeddingInput {
                    kb_id: id.to_string(),
                    text: "note".to_string(),
                    namespace: ns.to_string(),
                },
                &[1.0f32, 0.0, 0.0, 0.0],
            )
            .unwrap();
    }

    let query = SemanticQuery {
        text: "note".to_string(),
        limit: Some(10),
        threshold: None,
        category: Some("concept".to_string()),
        namespace: Some("work".to_string()),
    };
    let results = store
        .search_similar(&query, &[1.0f32, 0.0, 0.0, 0.0])
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].item.id, "id-1");
}

#[test]
fn initialize_vectors_migrates_pre_partition_key_schema() {
    let store = in_memory_store();
    store.initialize().unwrap();
    {
        // Simulate a pre-upgrade DB: the old 2-column kb_embeddings shape.
        let conn = store.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS kb_embeddings \
             USING vec0(kb_id TEXT PRIMARY KEY, embedding float[4])",
        )
        .unwrap();
        let bytes: Vec<u8> = [0.0f32; 4].iter().flat_map(|f| f.to_le_bytes()).collect();
        conn.execute(
            "INSERT INTO kb_embeddings(kb_id, embedding) VALUES (?1, ?2)",
            params!["old-id", bytes],
        )
        .unwrap();
    }

    store
        .initialize_vectors(4)
        .expect("should migrate the old schema instead of erroring");

    let kb = make_kb("id-1");
    store.save_kb(&kb).unwrap();
    let input = EmbeddingInput {
        kb_id: "id-1".to_string(),
        text: "rust ownership".to_string(),
        namespace: "default".to_string(),
    };
    store
        .save_embedding(&input, &[1.0f32, 0.0, 0.0, 0.0])
        .expect("save_embedding should succeed against the migrated (partitioned) schema");

    // The migration recreates the table (can't ALTER a vec0 table), so old rows are gone.
    let conn = store.conn.lock().unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kb_embeddings WHERE kb_id = 'old-id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn get_kbs_full_returns_all_fields() {
    let store = initialized_store();
    let mut kb = make_kb("id-1");
    kb.notes = "some notes".to_string();
    kb.reference = "The Rust Book".to_string();
    store.save_kb(&kb).unwrap();

    let results = store.get_kbs_full(&KbFilter::default()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "id-1");
    assert_eq!(results[0].value, "test value");
    assert_eq!(results[0].notes, "some notes");
    assert_eq!(results[0].reference, "The Rust Book");
    assert_eq!(results[0].category, "concept");
    assert_eq!(results[0].namespace, "default");
}

#[test]
fn get_kbs_full_filters_by_category() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1")).unwrap(); // category = "concept"
    let mut kb2 = make_kb("id-2");
    kb2.category = "quote".to_string();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        category: Some("quote".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs_full(&filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "id-2");
}

#[test]
fn get_kbs_full_respects_limit() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1")).unwrap();
    store.save_kb(&make_kb("id-2")).unwrap();
    store.save_kb(&make_kb("id-3")).unwrap();

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
    let mut kb1 = make_kb("id-1");
    kb1.category = "zebra".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.category = "apple".to_string();
    let mut kb3 = make_kb("id-3");
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
    let mut kb = make_kb("id-1");
    kb.category = String::new();
    store.save_kb(&kb).unwrap();

    let categories = store.get_categories(None).unwrap();
    assert!(categories.is_empty());
}

#[test]
fn get_categories_filters_by_namespace_when_given() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.category = "concept".to_string();
    kb1.namespace = "rust".to_string();
    let mut kb2 = make_kb("id-2");
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

#[test]
fn get_namespaces_returns_distinct_sorted_namespaces() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.namespace = "zebra".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.namespace = "apple".to_string();
    let mut kb3 = make_kb("id-3");
    kb3.namespace = "apple".to_string(); // duplicate
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let namespaces = store.get_namespaces(None).unwrap();
    assert_eq!(namespaces, vec!["apple".to_string(), "zebra".to_string()]);
}

#[test]
fn get_namespaces_excludes_empty_namespace() {
    let store = initialized_store();
    let mut kb = make_kb("id-1");
    kb.namespace = String::new();
    store.save_kb(&kb).unwrap();

    let namespaces = store.get_namespaces(None).unwrap();
    assert!(namespaces.is_empty());
}

#[test]
fn get_namespaces_returns_empty_vec_when_no_entries() {
    let store = initialized_store();
    let namespaces = store.get_namespaces(None).unwrap();
    assert!(namespaces.is_empty());
}

#[test]
fn get_namespaces_filters_by_substring_when_given() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.namespace = "com.cubita.com".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.namespace = "cubita.subdomain.service".to_string();
    let mut kb3 = make_kb("id-3");
    kb3.namespace = "com.sura.cubita".to_string();
    let mut kb4 = make_kb("id-4");
    kb4.namespace = "unrelated.namespace".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();
    store.save_kb(&kb4).unwrap();

    let namespaces = store.get_namespaces(Some("cubita")).unwrap();
    assert_eq!(
        namespaces,
        vec![
            "com.cubita.com".to_string(),
            "com.sura.cubita".to_string(),
            "cubita.subdomain.service".to_string()
        ]
    );
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
    store.save_kb(&make_kb("car-id")).unwrap();
    store.save_kb(&make_kb("engine-id")).unwrap();
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
    assert_eq!(related.outgoing[0].to.id, "engine-id");
    assert_eq!(related.outgoing[0].note, "has an engine");
    assert!(related.incoming.is_empty());
}

#[test]
fn add_edge_then_get_related_in() {
    let store = initialized_graph_store();
    store.save_kb(&make_kb("car-id")).unwrap();
    store.save_kb(&make_kb("engine-id")).unwrap();
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
    assert_eq!(related.incoming[0].from.id, "car-id");
    assert!(related.outgoing.is_empty());
}

#[test]
fn add_edge_then_get_related_both() {
    let store = initialized_graph_store();
    store.save_kb(&make_kb("car-id")).unwrap();
    store.save_kb(&make_kb("engine-id")).unwrap();
    store.save_kb(&make_kb("kit-id")).unwrap();
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
    store.save_kb(&make_kb("car-id")).unwrap();
    store.save_kb(&make_kb("engine-id")).unwrap();
    store
        .add_edge(&make_edge("edge-1", "car-id", "engine-id", "has an engine"))
        .unwrap();

    let result = store.add_edge(&make_edge("edge-2", "car-id", "engine-id", "again"));
    assert!(matches!(result, Err(Error::DuplicateEdgeError)));
}

#[test]
fn remove_edge_returns_true_when_deleted() {
    let store = initialized_graph_store();
    store.save_kb(&make_kb("car-id")).unwrap();
    store.save_kb(&make_kb("engine-id")).unwrap();
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
    store.save_kb(&make_kb("car-id")).unwrap();
    store.save_kb(&make_kb("engine-id")).unwrap();
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
    for id in ["car-id", "engine-id", "camshaft-id", "bolt-id", "thread-id"] {
        store.save_kb(&make_kb(id)).unwrap();
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
    store.save_kb(&make_kb("car-id")).unwrap();
    store.save_kb(&make_kb("engine-id")).unwrap();
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
    assert_eq!(nodes[0].id, "car-id");
    assert_eq!(nodes[0].parent_id, "engine-id");
}

#[test]
fn get_tree_terminates_on_cycle() {
    let store = initialized_graph_store();
    store.save_kb(&make_kb("a-id")).unwrap();
    store.save_kb(&make_kb("b-id")).unwrap();
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
    store.save_kb(&make_kb("car-id")).unwrap();
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

#[test]
fn count_kbs_returns_total_matches_ignoring_limit() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1")).unwrap();
    store.save_kb(&make_kb("id-2")).unwrap();
    store.save_kb(&make_kb("id-3")).unwrap();

    let filter = KbFilter {
        limit: Some(1),
        offset: Some(0),
        ..Default::default()
    };
    let count = store.count_kbs(&filter).unwrap();
    assert_eq!(count, 3);
}

#[test]
fn count_kbs_filters_by_category() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.category = "bookmark".to_string();
    let kb2 = make_kb("id-2"); // category = "concept"
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        category: Some("bookmark".to_string()),
        ..Default::default()
    };
    let count = store.count_kbs(&filter).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn count_kbs_returns_zero_when_no_matches() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1")).unwrap();

    let filter = KbFilter {
        category: Some("nonexistent".to_string()),
        ..Default::default()
    };
    let count = store.count_kbs(&filter).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn count_kbs_via_fts5_keyword() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.tags = vec!["memory".to_string(), "rust".to_string()];
    let mut kb2 = make_kb("id-2");
    kb2.tags = vec!["rust".to_string()];
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        keyword: Some("rust".to_string()),
        limit: Some(1),
        offset: Some(0),
        ..Default::default()
    };
    let count = store.count_kbs(&filter).unwrap();
    assert_eq!(count, 2);
}

#[test]
fn count_kbs_with_keyword_and_reference_filter() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.tags = vec!["rust".to_string()];
    kb1.reference = "The Rust Book".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.tags = vec!["rust".to_string()];
    kb2.reference = "other source".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        keyword: Some("rust".to_string()),
        reference: Some("The Rust Book".to_string()),
        ..Default::default()
    };
    let count = store.count_kbs(&filter).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn get_kbs_filters_by_start_date_inclusive() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb3 = make_kb("id-3");
    kb3.created_on = "2026-01-10T00:00:00+0000".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let filter = KbFilter {
        start_date: Some("2026-01-05".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs(&filter).unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.id == "id-2"));
    assert!(results.iter().any(|r| r.id == "id-3"));
}

#[test]
fn get_kbs_filters_by_end_date_inclusive() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb3 = make_kb("id-3");
    kb3.created_on = "2026-01-10T00:00:00+0000".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let filter = KbFilter {
        end_date: Some("2026-01-05".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs(&filter).unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.id == "id-1"));
    assert!(results.iter().any(|r| r.id == "id-2"));
}

#[test]
fn get_kbs_filters_by_start_and_end_date_range() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb3 = make_kb("id-3");
    kb3.created_on = "2026-01-10T00:00:00+0000".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let filter = KbFilter {
        start_date: Some("2026-01-02".to_string()),
        end_date: Some("2026-01-09".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs(&filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "id-2");
}

#[test]
fn get_kbs_date_range_excludes_entries_outside_range() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.created_on = "2025-12-31T00:00:00+0000".to_string();
    store.save_kb(&kb1).unwrap();

    let filter = KbFilter {
        start_date: Some("2026-01-01".to_string()),
        end_date: Some("2026-01-31".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs(&filter).unwrap();
    assert!(results.is_empty());
}

#[test]
fn get_kbs_via_fts5_keyword_filters_by_date_range() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.tags = vec!["rust".to_string()];
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.tags = vec!["rust".to_string()];
    kb2.created_on = "2026-01-10T00:00:00+0000".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        keyword: Some("rust".to_string()),
        start_date: Some("2026-01-05".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs(&filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "id-2");
}

#[test]
fn count_kbs_filters_by_date_range() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb3 = make_kb("id-3");
    kb3.created_on = "2026-01-10T00:00:00+0000".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let filter = KbFilter {
        start_date: Some("2026-01-02".to_string()),
        end_date: Some("2026-01-09".to_string()),
        ..Default::default()
    };
    let count = store.count_kbs(&filter).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn count_kbs_via_fts5_keyword_filters_by_date_range() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.tags = vec!["rust".to_string()];
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.tags = vec!["rust".to_string()];
    kb2.created_on = "2026-01-10T00:00:00+0000".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        keyword: Some("rust".to_string()),
        start_date: Some("2026-01-05".to_string()),
        ..Default::default()
    };
    let count = store.count_kbs(&filter).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn get_kbs_full_filters_by_date_range() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb3 = make_kb("id-3");
    kb3.created_on = "2026-01-10T00:00:00+0000".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let filter = KbFilter {
        start_date: Some("2026-01-02".to_string()),
        end_date: Some("2026-01-09".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs_full(&filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "id-2");
}

#[test]
fn get_kbs_date_range_combined_with_category_and_reference() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1");
    kb1.category = "concept".to_string();
    kb1.reference = "Book A".to_string();
    kb1.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2");
    kb2.category = "quote".to_string();
    kb2.reference = "Book A".to_string();
    kb2.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb3 = make_kb("id-3");
    kb3.category = "concept".to_string();
    kb3.reference = "Book B".to_string();
    kb3.created_on = "2026-01-05T00:00:00+0000".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let filter = KbFilter {
        category: Some("concept".to_string()),
        reference: Some("Book A".to_string()),
        start_date: Some("2026-01-01".to_string()),
        end_date: Some("2026-01-10".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs(&filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "id-1");
}

// ---------------------------------------------------------------------------
// One-time migration: drop KB_KEY
// ---------------------------------------------------------------------------

/// Seeds a real on-disk SQLite file with the pre-key-removal schema (`KB_KEY`
/// still present, `kb_edges` already created — the graph feature shipped
/// before key removal, so a real pre-upgrade DB already has it), one plain
/// entry and one media entry with a file on disk at the old `{key}.{ext}`
/// path. Returns the db path as a string.
fn seed_pre_key_removal_db(dir: &std::path::Path) -> String {
    let db_path = dir.join("kbzona.db");
    let db_path_str = db_path.to_str().unwrap().to_string();

    let conn = Connection::open(&db_path).expect("open temp db");
    conn.execute_batch(
        "CREATE TABLE kbs (
            INTERNAL_ID     INTEGER PRIMARY KEY AUTOINCREMENT,
            KB_ID           TEXT NOT NULL UNIQUE,
            KB_KEY          TEXT NOT NULL UNIQUE,
            KB_VALUE        TEXT NOT NULL,
            NOTES           TEXT NOT NULL DEFAULT '',
            CATEGORY        TEXT NOT NULL DEFAULT '',
            NAMESPACE       TEXT NOT NULL DEFAULT '',
            REFERENCE       TEXT NOT NULL DEFAULT '',
            TAG_VALUES      TEXT NOT NULL DEFAULT '',
            CREATED_ON      TEXT NOT NULL,
            KB_PATH         TEXT DEFAULT NULL,
            PARENT_KB_ID    TEXT DEFAULT NULL,
            MEDIA_EXTENSION TEXT DEFAULT NULL,
            METADATA        TEXT DEFAULT NULL
        );
        CREATE VIRTUAL TABLE tags_idx USING fts5(TAG_VALUES, content='kbs', content_rowid='INTERNAL_ID');
        CREATE TRIGGER kbs_ai AFTER INSERT ON kbs BEGIN
            INSERT INTO tags_idx(rowid, TAG_VALUES) VALUES (new.INTERNAL_ID, new.TAG_VALUES);
        END;
        CREATE TRIGGER kbs_ad AFTER DELETE ON kbs BEGIN
            INSERT INTO tags_idx(tags_idx, rowid, TAG_VALUES) VALUES ('delete', old.INTERNAL_ID, old.TAG_VALUES);
        END;
        CREATE TRIGGER kbs_au AFTER UPDATE ON kbs BEGIN
            INSERT INTO tags_idx(tags_idx, rowid, TAG_VALUES) VALUES ('delete', old.INTERNAL_ID, old.TAG_VALUES);
            INSERT INTO tags_idx(rowid, TAG_VALUES) VALUES (new.INTERNAL_ID, new.TAG_VALUES);
        END;
        CREATE TABLE kb_edges (
            INTERNAL_ID   INTEGER PRIMARY KEY AUTOINCREMENT,
            EDGE_ID       TEXT NOT NULL UNIQUE,
            FROM_KB_ID    TEXT NOT NULL,
            TO_KB_ID      TEXT NOT NULL,
            NOTE          TEXT NOT NULL DEFAULT '',
            CREATED_ON    TEXT NOT NULL,
            UNIQUE(FROM_KB_ID, TO_KB_ID)
        );
        CREATE TRIGGER kbs_ad_edges AFTER DELETE ON kbs BEGIN
            DELETE FROM kb_edges WHERE FROM_KB_ID = old.KB_ID OR TO_KB_ID = old.KB_ID;
        END;",
    )
    .expect("create old schema");

    conn.execute(
        "INSERT INTO kbs (KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE, REFERENCE, TAG_VALUES, CREATED_ON, KB_PATH, PARENT_KB_ID, MEDIA_EXTENSION, METADATA) \
         VALUES ('note-id', 'rust-ownership', 'ownership rules', '', 'concept', 'default', '', 'rust memory', '2026-01-01T00:00:00+0000', NULL, NULL, NULL, NULL)",
        [],
    )
    .expect("insert note row");
    conn.execute(
        "INSERT INTO kbs (KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE, REFERENCE, TAG_VALUES, CREATED_ON, KB_PATH, PARENT_KB_ID, MEDIA_EXTENSION, METADATA) \
         VALUES ('photo-id', 'car-photo', 'a car', '', 'media', 'default', '', '', '2026-01-01T00:00:00+0000', NULL, NULL, 'jpg', NULL)",
        [],
    )
    .expect("insert media row");
    drop(conn);

    let old_media_path = dir.join("media").join("default").join("car-photo.jpg");
    std::fs::create_dir_all(old_media_path.parent().unwrap()).expect("create media dir");
    std::fs::write(&old_media_path, b"fake jpg bytes").expect("write old media file");

    db_path_str
}

#[test]
fn initialize_migrates_pre_key_removal_schema() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path_str = seed_pre_key_removal_db(dir.path());
    let old_media_path = dir
        .path()
        .join("media")
        .join("default")
        .join("car-photo.jpg");

    let store = SqliteStore::new(&db_path_str).expect("open store");
    store.initialize().expect("migration should succeed");

    // KB_KEY column is gone.
    {
        let conn = store.conn.lock().unwrap();
        assert!(!table_has_column(&conn, "kbs", "KB_KEY").unwrap());
    }

    // Non-key fields survive, addressable by id.
    let note = store
        .get_kb_by_id("note-id")
        .unwrap()
        .expect("note row survives migration");
    assert_eq!(note.value, "ownership rules");
    assert_eq!(note.category, "concept");
    let photo = store
        .get_kb_by_id("photo-id")
        .unwrap()
        .expect("media row survives migration");
    assert_eq!(photo.media_extension.as_deref(), Some("jpg"));

    // A backup file was written next to the original db.
    let count_backups = || {
        std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("kbzona.db.bak-")
            })
            .count()
    };
    assert_eq!(
        count_backups(),
        1,
        "expected exactly one .bak-<timestamp> file next to the db"
    );

    // Media file moved from the old key-based path to the new id-based path.
    assert!(
        !old_media_path.exists(),
        "old key-based media path should be gone"
    );
    let new_media_path = dir
        .path()
        .join("media")
        .join("default")
        .join("photo-id.jpg");
    assert!(
        new_media_path.exists(),
        "media file should exist at the new id-based path"
    );
    assert_eq!(std::fs::read(&new_media_path).unwrap(), b"fake jpg bytes");

    // FTS keyword search still finds the seeded row — regression check for
    // INTERNAL_ID being preserved (not reassigned) across the table rebuild,
    // since tags_idx is keyed on content_rowid='INTERNAL_ID'.
    let results = store
        .get_kbs(&KbFilter {
            keyword: Some("memory".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "note-id");

    // A fresh save after migration works.
    store.save_kb(&make_kb("new-id")).unwrap();
    assert!(store.get_kb_by_id("new-id").unwrap().is_some());

    // Re-running initialize() is a no-op: no second backup file, no error.
    store
        .initialize()
        .expect("second initialize should be a no-op");
    assert_eq!(
        count_backups(),
        1,
        "second initialize should not create another backup"
    );
}

#[test]
fn initialize_skips_migration_on_fresh_schema() {
    // A fresh install (schema created by initialize() itself) never had
    // KB_KEY, so the migration must be a true no-op: no backup file.
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("kbzona.db");
    let store = SqliteStore::new(db_path.to_str().unwrap()).expect("open store");
    store.initialize().expect("initialize");

    let has_backup = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("kbzona.db.bak-")
        });
    assert!(
        !has_backup,
        "a fresh install must not trigger the key-removal migration"
    );
}
