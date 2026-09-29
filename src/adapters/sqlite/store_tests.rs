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
        metadata: std::collections::BTreeMap::new(),
        created_on: "2026-01-01T00:00:00+0000".to_string(),
        path: None,
        media_extension: None,
        label: None,
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
fn get_kbs_filters_by_namespace_prefix_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.namespace = "company.domain.api".to_string();
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.namespace = "company.domain.web".to_string();
    let mut kb3 = make_kb("id-3", "key-c");
    kb3.namespace = "company.other".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let filter = KbFilter {
        namespace: Some("company.domain.*".to_string()),
        ..Default::default()
    };
    let mut keys: Vec<String> = store
        .get_kbs(&filter)
        .unwrap()
        .into_iter()
        .map(|i| i.key)
        .collect();
    keys.sort();
    assert_eq!(keys, vec!["key-a".to_string(), "key-b".to_string()]);
}

#[test]
fn get_kbs_filters_by_namespace_suffix_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.namespace = "company.domain.subdomain".to_string();
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.namespace = "company.domain.subdomain.extra".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        namespace: Some("*.domain.subdomain".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "key-a");
}

#[test]
fn get_kbs_filters_by_namespace_contains_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.namespace = "company.domain.subdomain.api".to_string();
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.namespace = "company.other".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        namespace: Some("*.domain.subdomain.*".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "key-a");
}

#[test]
fn get_kbs_filters_by_namespace_glob_with_internal_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.namespace = "a.b.c".to_string();
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.namespace = "a.x.c".to_string();
    let mut kb3 = make_kb("id-3", "key-c");
    kb3.namespace = "a.b.d".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let filter = KbFilter {
        namespace: Some("a.*.c".to_string()),
        ..Default::default()
    };
    let mut keys: Vec<String> = store
        .get_kbs(&filter)
        .unwrap()
        .into_iter()
        .map(|i| i.key)
        .collect();
    keys.sort();
    assert_eq!(keys, vec!["key-a".to_string(), "key-b".to_string()]);
}

#[test]
fn get_kbs_filters_by_namespace_exact_no_wildcard_behavior_preserved() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.namespace = "rust".to_string();
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.namespace = "rust-extra".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        namespace: Some("rust".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "key-a");
}

#[test]
fn get_kbs_filters_by_namespace_wildcard_is_case_insensitive() {
    let store = initialized_store();
    let mut kb = make_kb("id-1", "key-a");
    kb.namespace = "company.domain.api".to_string();
    store.save_kb(&kb).unwrap();

    let filter = KbFilter {
        namespace: Some("COMPANY.DOMAIN.*".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "key-a");
}

#[test]
fn get_kbs_filters_by_namespace_wildcard_escapes_like_special_chars() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.namespace = "co_mp.domain".to_string();
    let mut kb2 = make_kb("id-2", "key-b");
    // Would incorrectly match `co_mp.*` too if `_` weren't escaped to a literal
    // in the translated LIKE pattern, since `_` is a SQL single-char wildcard.
    kb2.namespace = "coxmp.domain".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        namespace: Some("co_mp.*".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "key-a");
}

#[test]
fn get_kbs_via_fts5_keyword_filters_by_namespace_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "rust-ownership");
    kb1.tags = vec!["rust".to_string()];
    kb1.namespace = "company.domain.api".to_string();
    let mut kb2 = make_kb("id-2", "rust-lifetimes");
    kb2.tags = vec!["rust".to_string()];
    kb2.namespace = "company.other".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        keyword: Some("rust".to_string()),
        namespace: Some("company.domain.*".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs(&filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "rust-ownership");
}

#[test]
fn get_kbs_filters_by_path_prefix_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.path = Some("/company/domain/api".to_string());
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.path = Some("/company/domain/web".to_string());
    let mut kb3 = make_kb("id-3", "key-c");
    kb3.path = Some("/company/other".to_string());
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let filter = KbFilter {
        path: Some("/company/domain/*".to_string()),
        ..Default::default()
    };
    let mut keys: Vec<String> = store
        .get_kbs(&filter)
        .unwrap()
        .into_iter()
        .map(|i| i.key)
        .collect();
    keys.sort();
    assert_eq!(keys, vec!["key-a".to_string(), "key-b".to_string()]);
}

#[test]
fn get_kbs_filters_by_path_suffix_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.path = Some("/company/domain/subdomain".to_string());
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.path = Some("/company/domain/subdomain/extra".to_string());
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        path: Some("*/domain/subdomain".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "key-a");
}

#[test]
fn get_kbs_filters_by_path_contains_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.path = Some("/company/domain/subdomain/api".to_string());
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.path = Some("/company/other".to_string());
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        path: Some("*/domain/subdomain/*".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "key-a");
}

#[test]
fn get_kbs_filters_by_path_glob_with_internal_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.path = Some("/a/b/c".to_string());
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.path = Some("/a/x/c".to_string());
    let mut kb3 = make_kb("id-3", "key-c");
    kb3.path = Some("/a/b/d".to_string());
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let filter = KbFilter {
        path: Some("/a/*/c".to_string()),
        ..Default::default()
    };
    let mut keys: Vec<String> = store
        .get_kbs(&filter)
        .unwrap()
        .into_iter()
        .map(|i| i.key)
        .collect();
    keys.sort();
    assert_eq!(keys, vec!["key-a".to_string(), "key-b".to_string()]);
}

#[test]
fn get_kbs_filters_by_path_exact_no_wildcard_behavior_preserved() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.path = Some("/rust".to_string());
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.path = Some("/rust-extra".to_string());
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        path: Some("/rust".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "key-a");
}

#[test]
fn get_kbs_filters_by_path_wildcard_is_case_insensitive() {
    let store = initialized_store();
    let mut kb = make_kb("id-1", "key-a");
    kb.path = Some("/company/domain/api".to_string());
    store.save_kb(&kb).unwrap();

    let filter = KbFilter {
        path: Some("/COMPANY/DOMAIN/*".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "key-a");
}

#[test]
fn get_kbs_filters_by_path_wildcard_escapes_like_special_chars() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.path = Some("/co_mp/domain".to_string());
    let mut kb2 = make_kb("id-2", "key-b");
    // Would incorrectly match `/co_mp/*` too if `_` weren't escaped to a literal
    // in the translated LIKE pattern, since `_` is a SQL single-char wildcard.
    kb2.path = Some("/coxmp/domain".to_string());
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        path: Some("/co_mp/*".to_string()),
        ..Default::default()
    };
    let items = store.get_kbs(&filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "key-a");
}

#[test]
fn get_kbs_via_fts5_keyword_filters_by_path_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "rust-ownership");
    kb1.tags = vec!["rust".to_string()];
    kb1.path = Some("/company/domain/api".to_string());
    let mut kb2 = make_kb("id-2", "rust-lifetimes");
    kb2.tags = vec!["rust".to_string()];
    kb2.path = Some("/company/other".to_string());
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        keyword: Some("rust".to_string()),
        path: Some("/company/domain/*".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs(&filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "rust-ownership");
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
fn random_by_category_returns_matching_entry() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-q1", "stoic-quote");
    kb1.category = "quote".to_string();
    let mut kb2 = make_kb("id-q2", "zen-quote");
    kb2.category = "quote".to_string();
    let kb3 = make_kb("id-c1", "rust-concept"); // category = "concept"
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let result = store.random_by_category("quote", None).unwrap();
    assert_eq!(result.category.to_lowercase(), "quote");
}

#[test]
fn random_by_category_is_case_insensitive() {
    let store = initialized_store();
    let mut kb = make_kb("id-q1", "stoic-quote");
    kb.category = "QUOTE".to_string();
    store.save_kb(&kb).unwrap();

    let result = store.random_by_category("quote", None).unwrap();
    assert_eq!(result.id, "id-q1");
}

#[test]
fn random_by_category_returns_not_found_when_category_absent() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-c1", "rust-concept")).unwrap();

    let result = store.random_by_category("nonexistent", None);
    assert!(matches!(result, Err(Error::RandomNotFound(_))));
}

#[test]
fn random_by_category_respects_namespace_filter() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-q1", "stoic-quote");
    kb1.category = "quote".to_string();
    kb1.namespace = "ns1".to_string();
    let mut kb2 = make_kb("id-q2", "zen-quote");
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
    let kb = make_kb("id-1", "rust-ownership");
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
    let kb = make_kb("id-1", "rust-ownership");
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
    assert_eq!(results[0].item.key, "rust-ownership");
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
    assert_eq!(results[0].item.key, "rust-ownership");
}

#[test]
fn search_similar_filters_by_namespace_partition() {
    let store = initialized_store_with_vectors(4);
    let mut kb1 = make_kb("id-1", "work-note");
    kb1.namespace = "work".to_string();
    let mut kb2 = make_kb("id-2", "personal-note");
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
    assert_eq!(results[0].item.key, "work-note");
}

#[test]
fn search_similar_filters_by_category_post_filter() {
    let store = initialized_store_with_vectors(4);
    let mut kb1 = make_kb("id-1", "concept-note");
    kb1.category = "concept".to_string();
    let mut kb2 = make_kb("id-2", "quote-note");
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
    assert_eq!(results[0].item.key, "quote-note");
}

#[test]
fn search_similar_combines_namespace_and_category_with_and_semantics() {
    let store = initialized_store_with_vectors(4);
    let mut work_concept = make_kb("id-1", "work-concept");
    work_concept.namespace = "work".to_string();
    work_concept.category = "concept".to_string();
    let mut work_quote = make_kb("id-2", "work-quote");
    work_quote.namespace = "work".to_string();
    work_quote.category = "quote".to_string();
    let mut personal_concept = make_kb("id-3", "personal-concept");
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
    assert_eq!(results[0].item.key, "work-concept");
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

    let kb = make_kb("id-1", "rust-ownership");
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
fn get_kbs_full_namespace_wildcard_is_treated_as_literal() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.namespace = "company.domain.api".to_string();
    store.save_kb(&kb1).unwrap();

    // export (`get_kbs_full`) intentionally keeps exact-match namespace
    // semantics — a `*` is a literal character, not a wildcard, so this
    // matches nothing.
    let filter = KbFilter {
        namespace: Some("company.domain.*".to_string()),
        ..Default::default()
    };
    let results = store.get_kbs_full(&filter).unwrap();
    assert!(results.is_empty());
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

#[test]
fn get_namespaces_returns_distinct_sorted_namespaces() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.namespace = "zebra".to_string();
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.namespace = "apple".to_string();
    let mut kb3 = make_kb("id-3", "key-c");
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
    let mut kb = make_kb("id-1", "key-a");
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
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.namespace = "com.cubita.com".to_string();
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.namespace = "cubita.subdomain.service".to_string();
    let mut kb3 = make_kb("id-3", "key-c");
    kb3.namespace = "com.sura.cubita".to_string();
    let mut kb4 = make_kb("id-4", "key-d");
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

#[test]
fn count_kbs_returns_total_matches_ignoring_limit() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1", "key-a")).unwrap();
    store.save_kb(&make_kb("id-2", "key-b")).unwrap();
    store.save_kb(&make_kb("id-3", "key-c")).unwrap();

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
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.category = "bookmark".to_string();
    let kb2 = make_kb("id-2", "key-b"); // category = "concept"
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
    store.save_kb(&make_kb("id-1", "key-a")).unwrap();

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
    let mut kb1 = make_kb("id-1", "rust-ownership");
    kb1.tags = vec!["memory".to_string(), "rust".to_string()];
    let mut kb2 = make_kb("id-2", "rust-lifetimes");
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
    let count = store.count_kbs(&filter).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn count_kbs_filters_by_namespace_prefix_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.namespace = "company.domain.api".to_string();
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.namespace = "company.domain.web".to_string();
    let mut kb3 = make_kb("id-3", "key-c");
    kb3.namespace = "company.other".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let filter = KbFilter {
        namespace: Some("company.domain.*".to_string()),
        ..Default::default()
    };
    let count = store.count_kbs(&filter).unwrap();
    assert_eq!(count, 2);
}

#[test]
fn count_kbs_via_fts5_keyword_filters_by_namespace_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "rust-ownership");
    kb1.tags = vec!["rust".to_string()];
    kb1.namespace = "company.domain.api".to_string();
    let mut kb2 = make_kb("id-2", "rust-lifetimes");
    kb2.tags = vec!["rust".to_string()];
    kb2.namespace = "company.other".to_string();
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        keyword: Some("rust".to_string()),
        namespace: Some("company.domain.*".to_string()),
        ..Default::default()
    };
    let count = store.count_kbs(&filter).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn count_kbs_filters_by_path_prefix_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-a");
    kb1.path = Some("/company/domain/api".to_string());
    let mut kb2 = make_kb("id-2", "key-b");
    kb2.path = Some("/company/domain/web".to_string());
    let mut kb3 = make_kb("id-3", "key-c");
    kb3.path = Some("/company/other".to_string());
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();
    store.save_kb(&kb3).unwrap();

    let filter = KbFilter {
        path: Some("/company/domain/*".to_string()),
        ..Default::default()
    };
    let count = store.count_kbs(&filter).unwrap();
    assert_eq!(count, 2);
}

#[test]
fn count_kbs_via_fts5_keyword_filters_by_path_wildcard() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "rust-ownership");
    kb1.tags = vec!["rust".to_string()];
    kb1.path = Some("/company/domain/api".to_string());
    let mut kb2 = make_kb("id-2", "rust-lifetimes");
    kb2.tags = vec!["rust".to_string()];
    kb2.path = Some("/company/other".to_string());
    store.save_kb(&kb1).unwrap();
    store.save_kb(&kb2).unwrap();

    let filter = KbFilter {
        keyword: Some("rust".to_string()),
        path: Some("/company/domain/*".to_string()),
        ..Default::default()
    };
    let count = store.count_kbs(&filter).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn get_kbs_filters_by_start_date_inclusive() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-1");
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2", "key-2");
    kb2.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb3 = make_kb("id-3", "key-3");
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
    assert!(results.iter().any(|r| r.key == "key-2"));
    assert!(results.iter().any(|r| r.key == "key-3"));
}

#[test]
fn get_kbs_filters_by_end_date_inclusive() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-1");
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2", "key-2");
    kb2.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb3 = make_kb("id-3", "key-3");
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
    assert!(results.iter().any(|r| r.key == "key-1"));
    assert!(results.iter().any(|r| r.key == "key-2"));
}

#[test]
fn get_kbs_filters_by_start_and_end_date_range() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-1");
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2", "key-2");
    kb2.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb3 = make_kb("id-3", "key-3");
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
    assert_eq!(results[0].key, "key-2");
}

#[test]
fn get_kbs_date_range_excludes_entries_outside_range() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-1");
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
    let mut kb1 = make_kb("id-1", "key-1");
    kb1.tags = vec!["rust".to_string()];
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2", "key-2");
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
    assert_eq!(results[0].key, "key-2");
}

#[test]
fn count_kbs_filters_by_date_range() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-1");
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2", "key-2");
    kb2.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb3 = make_kb("id-3", "key-3");
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
    let mut kb1 = make_kb("id-1", "rust-ownership");
    kb1.tags = vec!["rust".to_string()];
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2", "rust-lifetimes");
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
    let mut kb1 = make_kb("id-1", "key-1");
    kb1.created_on = "2026-01-01T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2", "key-2");
    kb2.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb3 = make_kb("id-3", "key-3");
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
    assert_eq!(results[0].key, "key-2");
}

#[test]
fn get_kbs_date_range_combined_with_category_and_reference() {
    let store = initialized_store();
    let mut kb1 = make_kb("id-1", "key-1");
    kb1.category = "concept".to_string();
    kb1.reference = "Book A".to_string();
    kb1.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb2 = make_kb("id-2", "key-2");
    kb2.category = "quote".to_string();
    kb2.reference = "Book A".to_string();
    kb2.created_on = "2026-01-05T00:00:00+0000".to_string();
    let mut kb3 = make_kb("id-3", "key-3");
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
    assert_eq!(results[0].key, "key-1");
}

#[test]
fn glob_to_like_pattern_translates_wildcards_and_escapes_special_chars() {
    assert_eq!(glob_to_like_pattern("company.domain.*"), "company.domain.%");
    assert_eq!(
        glob_to_like_pattern("*.domain.subdomain"),
        "%.domain.subdomain"
    );
    assert_eq!(
        glob_to_like_pattern("*.domain.subdomain.*"),
        "%.domain.subdomain.%"
    );
    assert_eq!(glob_to_like_pattern("a.*.b.*.c"), "a.%.b.%.c");
    assert_eq!(glob_to_like_pattern("plain"), "plain");
    assert_eq!(glob_to_like_pattern("100%_done"), "100\\%\\_done");
    assert_eq!(glob_to_like_pattern("a\\b"), "a\\\\b");
}
