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
fn list_all_returns_saved_entries() {
    let store = initialized_store();
    store.save_kb(&make_kb("id-1", "key-a")).unwrap();
    store.save_kb(&make_kb("id-2", "key-b")).unwrap();
    let filter = KbFilter::default();
    let items = store.list_kbs(&filter).unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn list_filters_by_category() {
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
    let items = store.list_kbs(&filter).unwrap();
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
fn search_kbs_via_fts5() {
    let store = initialized_store();
    let mut kb = make_kb("id-1", "rust-ownership");
    kb.tags = vec!["memory".to_string(), "rust".to_string()];
    store.save_kb(&kb).unwrap();

    let filter = KbFilter {
        keyword: Some("memo".to_string()),
        ..Default::default()
    };
    let results = store.search_kbs(&filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "rust-ownership");
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
    };
    let results = store.search_similar(&query, &query_embedding).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].item.key, "rust-ownership");
}
