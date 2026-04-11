use super::*;
use crate::ports::{MediaFetcher, MediaStore};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

// ---- MockKbStore ----

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

    fn get_kbs(&self, filter: &KbFilter) -> Result<Vec<KbItem>, Error> {
        let keyword = filter.keyword.as_deref().unwrap_or("");
        let ref_filter = filter.reference.as_deref().unwrap_or("");
        if keyword.is_empty() {
            return Ok(self
                .data
                .borrow()
                .values()
                .filter(|kb| {
                    filter
                        .category
                        .as_deref()
                        .map_or(true, |c| kb.category == c)
                })
                .filter(|kb| {
                    filter
                        .namespace
                        .as_deref()
                        .map_or(true, |n| kb.namespace == n)
                })
                .filter(|kb| {
                    ref_filter.is_empty()
                        || kb
                            .reference
                            .to_lowercase()
                            .contains(&ref_filter.to_lowercase())
                })
                .map(|kb| KbItem {
                    id: kb.id.clone(),
                    key: kb.key.clone(),
                    category: kb.category.clone(),
                    namespace: kb.namespace.clone(),
                    tags: kb.tags.clone(),
                })
                .collect());
        }
        Ok(self
            .data
            .borrow()
            .values()
            .filter(|kb| kb.tags.iter().any(|t| t.contains(keyword)))
            .filter(|kb| {
                ref_filter.is_empty()
                    || kb
                        .reference
                        .to_lowercase()
                        .contains(&ref_filter.to_lowercase())
            })
            .map(|kb| KbItem {
                id: kb.id.clone(),
                key: kb.key.clone(),
                category: kb.category.clone(),
                namespace: kb.namespace.clone(),
                tags: kb.tags.clone(),
            })
            .collect())
    }

    fn save_kb(&self, kb: &Kb) -> Result<(), Error> {
        self.data.borrow_mut().insert(kb.id.clone(), kb.clone());
        Ok(())
    }

    fn update_kb(&self, kb: &Kb) -> Result<bool, Error> {
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

    fn random_quote(&self) -> Result<Kb, Error> {
        self.data
            .borrow()
            .values()
            .find(|kb| kb.category.to_lowercase() == "quote")
            .cloned()
            .ok_or(Error::QuoteNotFound)
    }

    fn get_children_ids(&self, parent_id: &str) -> Result<Vec<String>, Error> {
        Ok(self
            .data
            .borrow()
            .values()
            .filter(|kb| kb.parent.as_deref() == Some(parent_id))
            .map(|kb| kb.id.clone())
            .collect())
    }

    fn get_kbs_full(&self, filter: &KbFilter) -> Result<Vec<Kb>, Error> {
        Ok(self
            .data
            .borrow()
            .values()
            .filter(|kb| {
                filter
                    .category
                    .as_deref()
                    .map_or(true, |c| kb.category == c)
            })
            .filter(|kb| {
                filter
                    .namespace
                    .as_deref()
                    .map_or(true, |n| kb.namespace == n)
            })
            .cloned()
            .collect())
    }
}

// ---- GhostItemKbStore: list_kbs returns items but get_kb_by_id returns None ----

#[derive(Debug, Clone)]
struct GhostItemKbStore {
    ghost_id: String,
    ghost_key: String,
}

impl KbStore for GhostItemKbStore {
    fn initialize(&self) -> Result<(), Error> {
        Ok(())
    }

    fn get_kb_by_id(&self, _id: &str) -> Result<Option<Kb>, Error> {
        Ok(None)
    }

    fn get_kb_by_key(&self, _key: &str) -> Result<Option<Kb>, Error> {
        Ok(None)
    }

    fn get_kbs(&self, filter: &KbFilter) -> Result<Vec<KbItem>, Error> {
        if filter.keyword.as_deref().map_or(true, |k| k.is_empty()) {
            return Ok(vec![KbItem {
                id: self.ghost_id.clone(),
                key: self.ghost_key.clone(),
                category: "concept".to_string(),
                namespace: "default".to_string(),
                tags: vec![],
            }]);
        }
        Ok(vec![])
    }

    fn save_kb(&self, _kb: &Kb) -> Result<(), Error> {
        Ok(())
    }

    fn update_kb(&self, _kb: &Kb) -> Result<bool, Error> {
        Ok(false)
    }

    fn delete_kb(&self, _id: &str) -> Result<bool, Error> {
        Ok(false)
    }

    fn random_quote(&self) -> Result<Kb, Error> {
        Err(Error::QuoteNotFound)
    }

    fn get_children_ids(&self, _parent_id: &str) -> Result<Vec<String>, Error> {
        Ok(vec![])
    }

    fn get_kbs_full(&self, _filter: &KbFilter) -> Result<Vec<Kb>, Error> {
        Ok(vec![])
    }
}

// ---- MockVectorStore (Arc-backed so clones share state) ----

#[derive(Debug, Clone)]
struct MockVectorStore {
    indexed: Arc<RefCell<Vec<String>>>,
    deleted: Arc<RefCell<Vec<String>>>,
}

impl Default for MockVectorStore {
    fn default() -> Self {
        Self {
            indexed: Arc::new(RefCell::new(Vec::new())),
            deleted: Arc::new(RefCell::new(Vec::new())),
        }
    }
}

impl VectorStore for MockVectorStore {
    fn initialize_vectors(&self, _dimensions: usize) -> Result<(), Error> {
        Ok(())
    }

    fn save_embedding(&self, input: &EmbeddingInput, _embedding: &[f32]) -> Result<(), Error> {
        self.indexed.borrow_mut().push(input.kb_id.clone());
        Ok(())
    }

    fn delete_embedding(&self, kb_id: &str) -> Result<(), Error> {
        self.deleted.borrow_mut().push(kb_id.to_string());
        Ok(())
    }

    fn search_similar(
        &self,
        _query: &SemanticQuery,
        _embedding: &[f32],
    ) -> Result<Vec<ScoredKbItem>, Error> {
        Ok(vec![])
    }
}

// ---- FailingVectorStore: all write operations fail ----

#[derive(Debug, Clone)]
struct FailingVectorStore;

impl VectorStore for FailingVectorStore {
    fn initialize_vectors(&self, _dimensions: usize) -> Result<(), Error> {
        Ok(())
    }

    fn save_embedding(&self, _input: &EmbeddingInput, _embedding: &[f32]) -> Result<(), Error> {
        Err(Error::EmbeddingError("forced save failure".to_string()))
    }

    fn delete_embedding(&self, _kb_id: &str) -> Result<(), Error> {
        Err(Error::EmbeddingError("forced delete failure".to_string()))
    }

    fn search_similar(
        &self,
        _query: &SemanticQuery,
        _embedding: &[f32],
    ) -> Result<Vec<ScoredKbItem>, Error> {
        Ok(vec![])
    }
}

// ---- MockEmbeddingProvider ----

#[derive(Debug, Clone)]
struct MockEmbeddingProvider;

impl EmbeddingProvider for MockEmbeddingProvider {
    fn embed(&self, _text: &str) -> Result<Vec<f32>, Error> {
        Ok(vec![0.0; 4])
    }

    fn dimensions(&self) -> usize {
        4
    }
}

// ---- FailingEmbeddingProvider ----

#[derive(Debug, Clone)]
struct FailingEmbeddingProvider;

impl EmbeddingProvider for FailingEmbeddingProvider {
    fn embed(&self, _text: &str) -> Result<Vec<f32>, Error> {
        Err(Error::EmbeddingError("forced failure".to_string()))
    }

    fn dimensions(&self) -> usize {
        4
    }
}

// ---- MockMediaStore ----

#[derive(Debug, Clone, Default)]
struct MockMediaStore {
    stored: Arc<RefCell<Vec<String>>>,
    deleted: Arc<RefCell<Vec<String>>>,
    dirs_copied: Arc<RefCell<Vec<(String, String)>>>,
}

impl MediaStore for MockMediaStore {
    fn store_media(&self, params: &StoreMediaParams) -> Result<String, Error> {
        self.stored.borrow_mut().push(params.destination.clone());
        Ok(params.destination.clone())
    }

    fn delete_media(&self, path: &str) -> Result<(), Error> {
        self.deleted.borrow_mut().push(path.to_string());
        Ok(())
    }

    fn copy_dir(&self, source: &str, destination: &str) -> Result<u64, Error> {
        self.dirs_copied
            .borrow_mut()
            .push((source.to_string(), destination.to_string()));
        Ok(0)
    }
}

// ---- FailingMediaStore ----

#[derive(Debug, Clone)]
struct FailingMediaStore;

impl MediaStore for FailingMediaStore {
    fn store_media(&self, _params: &StoreMediaParams) -> Result<String, Error> {
        Err(Error::MediaCopyError("forced copy failure".to_string()))
    }

    fn delete_media(&self, _path: &str) -> Result<(), Error> {
        Err(Error::MediaDeleteError("forced delete failure".to_string()))
    }

    fn copy_dir(&self, _source: &str, _destination: &str) -> Result<u64, Error> {
        Err(Error::MediaCopyError("forced dir copy failure".to_string()))
    }
}

// ---- MockMediaFetcher ----

#[derive(Debug, Clone)]
struct MockMediaFetcher;

impl MediaFetcher for MockMediaFetcher {
    fn fetch(&self, url: &str) -> Result<String, Error> {
        // Return a fake temp path derived from the URL for testing.
        Ok(format!(
            "/tmp/mock-{}",
            url.replace("://", "-").replace('/', "-")
        ))
    }
}

// ---- CountingEmbeddingProvider: fails on and after the Nth call ----

#[derive(Debug, Clone)]
struct CountingEmbeddingProvider {
    fail_after: usize,
    count: Arc<RefCell<usize>>,
}

impl CountingEmbeddingProvider {
    fn new(fail_after: usize) -> Self {
        Self {
            fail_after,
            count: Arc::new(RefCell::new(0)),
        }
    }
}

impl EmbeddingProvider for CountingEmbeddingProvider {
    fn embed(&self, _text: &str) -> Result<Vec<f32>, Error> {
        let n = *self.count.borrow();
        *self.count.borrow_mut() += 1;
        if n >= self.fail_after {
            Err(Error::EmbeddingError("forced failure".to_string()))
        } else {
            Ok(vec![0.0; 4])
        }
    }

    fn dimensions(&self) -> usize {
        4
    }
}

// ---- Helpers ----

fn make_kb(id: &str, key: &str) -> Kb {
    Kb {
        id: id.to_string(),
        key: key.to_string(),
        value: "some value".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        reference: String::new(),
        namespace: "default".to_string(),
        tags: vec!["rust".to_string()],
        created_on: "2026-01-01T00:00:00+0000".to_string(),
        parent: None,
        path: None,
        media_extension: None,
    }
}

fn make_new_kb(key: &str) -> NewKb {
    NewKb {
        key: key.to_string(),
        value: "some value".to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        reference: String::new(),
        namespace: "default".to_string(),
        tags: vec!["rust".to_string()],
        parent: None,
        path: None,
        media_url: None,
        media_extension: None,
    }
}

fn make_kb_with_parent(id: &str, key: &str, parent_id: &str) -> Kb {
    Kb {
        parent: Some(parent_id.to_string()),
        ..make_kb(id, key)
    }
}

fn make_import_item(key: &str, value: &str) -> ImportKbItem {
    ImportKbItem {
        key: key.to_string(),
        value: value.to_string(),
        notes: String::new(),
        category: "concept".to_string(),
        reference: String::new(),
        namespace: "default".to_string(),
        tags: vec!["rust".to_string()],
        parent_key: None,
        path: None,
        media_extension: None,
    }
}

fn make_svc(
) -> KBService<MockKbStore, MockVectorStore, MockEmbeddingProvider, MockMediaStore, MockMediaFetcher>
{
    KBService::new(
        MockKbStore::new(),
        ServiceDeps {
            vector_store: MockVectorStore::default(),
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    )
}

fn make_svc_with_store(
    store: MockKbStore,
) -> KBService<MockKbStore, MockVectorStore, MockEmbeddingProvider, MockMediaStore, MockMediaFetcher>
{
    KBService::new(
        store,
        ServiceDeps {
            vector_store: MockVectorStore::default(),
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    )
}

// ---- add_kb tests ----

#[test]
fn add_kb_saves_entry_and_indexes_embedding() {
    let vector = MockVectorStore::default();
    let svc = KBService::new(
        MockKbStore::new(),
        ServiceDeps {
            vector_store: vector.clone(),
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    let result = svc.add_kb(make_new_kb("rust-ownership"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().key, "rust-ownership");
    assert_eq!(vector.indexed.borrow().len(), 1);
}

#[test]
fn add_kb_returns_ok_when_embedding_fails() {
    let svc = KBService::new(
        MockKbStore::new(),
        ServiceDeps {
            vector_store: FailingVectorStore,
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    assert!(svc.add_kb(make_new_kb("rust-ownership")).is_ok());
}

#[test]
fn add_kb_fails_on_duplicate_key() {
    let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
    let svc = make_svc_with_store(store);
    assert!(matches!(
        svc.add_kb(make_new_kb("rust-ownership")),
        Err(Error::DuplicateKBError)
    ));
}

// ---- get tests ----

#[test]
fn get_kb_by_id_returns_none_for_unknown_id() {
    assert_eq!(make_svc().get_kb_by_id("no-such-id").unwrap(), None);
}

#[test]
fn get_kb_by_id_returns_entry_when_present() {
    let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
    let svc = make_svc_with_store(store);
    assert!(svc.get_kb_by_id("id-1").unwrap().is_some());
}

#[test]
fn get_kb_by_key_returns_entry_when_present() {
    let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
    let svc = make_svc_with_store(store);
    assert!(svc.get_kb_by_key("rust-ownership").unwrap().is_some());
}

// ---- update_kb tests ----

#[test]
fn update_kb_merges_partial_fields_correctly() {
    let original = make_kb("id-1", "rust-ownership");
    let store = MockKbStore::with(vec![original]);
    let svc = make_svc_with_store(store);
    let update = KbUpdate {
        id: "id-1".to_string(),
        key: None,
        value: Some("updated value".to_string()),
        notes: None,
        category: None,
        namespace: None,
        reference: None,
        tags: None,
        parent: None,
        path: None,
    };
    assert!(svc.update_kb(update).is_ok());
    let fetched = svc.get_kb_by_id("id-1").unwrap().unwrap();
    assert_eq!(fetched.value, "updated value");
    assert_eq!(fetched.key, "rust-ownership");
}

#[test]
fn update_kb_reindexes_when_embedding_text_changes() {
    let original = make_kb("id-1", "rust-ownership");
    let store = MockKbStore::with(vec![original]);
    let vector = MockVectorStore::default();
    let svc = KBService::new(
        store,
        ServiceDeps {
            vector_store: vector.clone(),
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    let update = KbUpdate {
        id: "id-1".to_string(),
        key: Some("rust-updated".to_string()),
        value: None,
        notes: None,
        category: None,
        namespace: None,
        reference: None,
        tags: None,
        parent: None,
        path: None,
    };
    assert!(svc.update_kb(update).is_ok());
    assert_eq!(vector.indexed.borrow().len(), 1);
}

#[test]
fn update_kb_skips_reindex_when_only_notes_changes() {
    let original = make_kb("id-1", "rust-ownership");
    let store = MockKbStore::with(vec![original]);
    let vector = MockVectorStore::default();
    let svc = KBService::new(
        store,
        ServiceDeps {
            vector_store: vector.clone(),
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    let update = KbUpdate {
        id: "id-1".to_string(),
        key: None,
        value: None,
        notes: Some("new notes only".to_string()),
        category: None,
        namespace: None,
        reference: None,
        tags: None,
        parent: None,
        path: None,
    };
    assert!(svc.update_kb(update).is_ok());
    assert!(vector.indexed.borrow().is_empty());
}

#[test]
fn update_kb_returns_not_found_for_unknown_id() {
    let update = KbUpdate {
        id: "no-such-id".to_string(),
        key: None,
        value: Some("x".to_string()),
        notes: None,
        category: None,
        namespace: None,
        reference: None,
        tags: None,
        parent: None,
        path: None,
    };
    assert!(matches!(
        make_svc().update_kb(update),
        Err(Error::KBNotFound)
    ));
}

#[test]
fn update_kb_returns_ok_when_embedding_update_fails() {
    let original = make_kb("id-1", "rust-ownership");
    let store = MockKbStore::with(vec![original]);
    let svc = KBService::new(
        store,
        ServiceDeps {
            vector_store: FailingVectorStore,
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    let update = KbUpdate {
        id: "id-1".to_string(),
        key: Some("rust-updated".to_string()),
        value: None,
        notes: None,
        category: None,
        namespace: None,
        reference: None,
        tags: None,
        parent: None,
        path: None,
    };
    assert!(svc.update_kb(update).is_ok());
}

#[test]
fn update_kb_rejects_stolen_key() {
    let store = MockKbStore::with(vec![
        make_kb("id-1", "my-key"),
        make_kb("id-2", "already-taken"),
    ]);
    let svc = make_svc_with_store(store);
    let update = KbUpdate {
        id: "id-1".to_string(),
        key: Some("already-taken".to_string()),
        value: None,
        notes: None,
        category: None,
        namespace: None,
        reference: None,
        tags: None,
        parent: None,
        path: None,
    };
    assert!(matches!(
        svc.update_kb(update),
        Err(Error::DuplicateKBError)
    ));
}

#[test]
fn update_kb_same_key_same_entry_is_ok() {
    let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
    let svc = make_svc_with_store(store);
    let update = KbUpdate {
        id: "id-1".to_string(),
        key: Some("rust-ownership".to_string()),
        value: Some("new value".to_string()),
        notes: None,
        category: None,
        namespace: None,
        reference: None,
        tags: None,
        parent: None,
        path: None,
    };
    assert!(svc.update_kb(update).is_ok());
}

// ---- update_kb lowercase tests ----

#[test]
fn update_kb_lowercases_key() {
    let original = make_kb("id-1", "rust-ownership");
    let store = MockKbStore::with(vec![original]);
    let svc = make_svc_with_store(store);
    let update = KbUpdate {
        id: "id-1".to_string(),
        key: Some("UpperKey".to_string()),
        value: None,
        notes: None,
        category: None,
        namespace: None,
        reference: None,
        tags: None,
        parent: None,
        path: None,
    };
    assert!(svc.update_kb(update).is_ok());
    let fetched = svc.get_kb_by_id("id-1").unwrap().unwrap();
    assert_eq!(fetched.key, "upperkey");
}

#[test]
fn update_kb_lowercases_category() {
    let original = make_kb("id-1", "rust-ownership");
    let store = MockKbStore::with(vec![original]);
    let svc = make_svc_with_store(store);
    let update = KbUpdate {
        id: "id-1".to_string(),
        key: None,
        value: None,
        notes: None,
        category: Some("UpperCat".to_string()),
        namespace: None,
        reference: None,
        tags: None,
        parent: None,
        path: None,
    };
    assert!(svc.update_kb(update).is_ok());
    let fetched = svc.get_kb_by_id("id-1").unwrap().unwrap();
    assert_eq!(fetched.category, "uppercat");
}

#[test]
fn update_kb_lowercases_namespace() {
    let original = make_kb("id-1", "rust-ownership");
    let store = MockKbStore::with(vec![original]);
    let svc = make_svc_with_store(store);
    let update = KbUpdate {
        id: "id-1".to_string(),
        key: None,
        value: None,
        notes: None,
        category: None,
        namespace: Some("UpperNS".to_string()),
        reference: None,
        tags: None,
        parent: None,
        path: None,
    };
    assert!(svc.update_kb(update).is_ok());
    let fetched = svc.get_kb_by_id("id-1").unwrap().unwrap();
    assert_eq!(fetched.namespace, "upperns");
}

// ---- delete_kb tests ----

#[test]
fn delete_kb_removes_entry_and_embedding() {
    let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
    let vector = MockVectorStore::default();
    let svc = KBService::new(
        store,
        ServiceDeps {
            vector_store: vector.clone(),
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    assert!(svc.delete_kb("id-1").is_ok());
    assert_eq!(vector.deleted.borrow().len(), 1);
    assert_eq!(vector.deleted.borrow()[0], "id-1");
}

#[test]
fn delete_kb_returns_ok_when_embedding_removal_fails() {
    let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
    let svc = KBService::new(
        store,
        ServiceDeps {
            vector_store: FailingVectorStore,
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    assert!(svc.delete_kb("id-1").is_ok());
}

#[test]
fn delete_kb_returns_not_found_for_unknown_id() {
    assert!(matches!(
        make_svc().delete_kb("no-such-id"),
        Err(Error::KBNotFound)
    ));
}

// ---- get_kbs tests ----

#[test]
fn get_kbs_returns_matching_entries_by_keyword() {
    let mut kb = make_kb("id-1", "rust-ownership");
    kb.tags = vec!["memory".to_string(), "rust".to_string()];
    let store = MockKbStore::with(vec![kb]);
    let svc = make_svc_with_store(store);
    let filter = KbFilter {
        keyword: Some("memo".to_string()),
        ..Default::default()
    };
    let results = svc.get_kbs(filter).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn get_kbs_with_keyword_and_reference_filter_returns_matching_entries() {
    let mut kb1 = make_kb("id-1", "rust-ownership");
    kb1.tags = vec!["rust".to_string()];
    kb1.reference = "The Rust Book".to_string();
    let mut kb2 = make_kb("id-2", "rust-lifetimes");
    kb2.tags = vec!["rust".to_string()];
    kb2.reference = "other source".to_string();
    let store = MockKbStore::with(vec![kb1, kb2]);
    let svc = make_svc_with_store(store);
    let filter = KbFilter {
        keyword: Some("rust".to_string()),
        reference: Some("The Rust Book".to_string()),
        ..Default::default()
    };
    let results = svc.get_kbs(filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "rust-ownership");
}

#[test]
fn get_kbs_with_keyword_only_returns_all_keyword_matches() {
    let mut kb1 = make_kb("id-1", "rust-ownership");
    kb1.tags = vec!["rust".to_string()];
    kb1.reference = "The Rust Book".to_string();
    let mut kb2 = make_kb("id-2", "rust-lifetimes");
    kb2.tags = vec!["rust".to_string()];
    kb2.reference = "other source".to_string();
    let store = MockKbStore::with(vec![kb1, kb2]);
    let svc = make_svc_with_store(store);
    let filter = KbFilter {
        keyword: Some("rust".to_string()),
        ..Default::default()
    };
    let results = svc.get_kbs(filter).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn get_kbs_without_keyword_returns_all_entries() {
    let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
    let results = make_svc_with_store(store)
        .get_kbs(KbFilter::default())
        .unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn get_kbs_without_keyword_filters_by_reference() {
    let mut kb1 = make_kb("id-1", "rust-ownership");
    kb1.reference = "The Rust Book".to_string();
    let mut kb2 = make_kb("id-2", "rust-lifetimes");
    kb2.reference = "other source".to_string();
    let store = MockKbStore::with(vec![kb1, kb2]);
    let svc = make_svc_with_store(store);
    let filter = KbFilter {
        reference: Some("The Rust Book".to_string()),
        ..Default::default()
    };
    let results = svc.get_kbs(filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "rust-ownership");
}

// ---- reindex tests ----

#[test]
fn reindex_all_succeed_full_succeeded_counts() {
    let store = MockKbStore::with(vec![
        make_kb("id-1", "rust-ownership"),
        make_kb("id-2", "rust-borrowing"),
    ]);
    let svc = make_svc_with_store(store);
    let result = svc.reindex().unwrap();
    assert_eq!(result.succeeded.len(), 2);
    assert!(result.failed.is_empty());
}

#[test]
fn reindex_partial_failures_reported_in_result() {
    let store = MockKbStore::with(vec![
        make_kb("id-1", "rust-ownership"),
        make_kb("id-2", "rust-borrowing"),
    ]);
    let svc = KBService::new(
        store,
        ServiceDeps {
            vector_store: MockVectorStore::default(),
            embedder: CountingEmbeddingProvider::new(1),
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    let result = svc.reindex().unwrap();
    assert_eq!(result.succeeded.len() + result.failed.len(), 2);
    assert_eq!(result.succeeded.len(), 1);
    assert_eq!(result.failed.len(), 1);
}

#[test]
fn reindex_ghost_entry_appears_in_failed() {
    let ghost_store = GhostItemKbStore {
        ghost_id: "ghost-id".to_string(),
        ghost_key: "ghost-key".to_string(),
    };
    let svc = KBService::new(
        ghost_store,
        ServiceDeps {
            vector_store: MockVectorStore::default(),
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    let result = svc.reindex().unwrap();
    assert!(result.succeeded.is_empty());
    assert_eq!(result.failed.len(), 1);
    assert!(result.failed[0].0.contains("ghost-id"));
}

#[test]
fn reindex_empty_store_returns_empty_result() {
    let result = make_svc().reindex().unwrap();
    assert!(result.succeeded.is_empty());
    assert!(result.failed.is_empty());
}

// ---- import_kbs tests ----

#[test]
fn import_kbs_indexes_all_saved_entries() {
    let vector = MockVectorStore::default();
    let svc = KBService::new(
        MockKbStore::new(),
        ServiceDeps {
            vector_store: vector.clone(),
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    let items = vec![
        make_import_item("rust-ownership", "memory management"),
        make_import_item("rust-borrowing", "borrow checker"),
    ];
    let result = svc.import_kbs(items);
    assert_eq!(result.saved.len(), 2);
    assert_eq!(vector.indexed.borrow().len(), 2);
}

#[test]
fn import_kbs_skips_indexing_for_failed_items() {
    let vector = MockVectorStore::default();
    let svc = KBService::new(
        MockKbStore::new(),
        ServiceDeps {
            vector_store: vector.clone(),
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    let items = vec![
        make_import_item("rust-ownership", "memory management"),
        make_import_item("", "empty key fails"),
    ];
    let result = svc.import_kbs(items);
    assert_eq!(result.saved.len(), 1);
    assert_eq!(result.failed.len(), 1);
    assert_eq!(vector.indexed.borrow().len(), 1);
}

#[test]
fn import_kbs_continues_on_embedding_failure() {
    let svc = KBService::new(
        MockKbStore::new(),
        ServiceDeps {
            vector_store: FailingVectorStore,
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    let items = vec![
        make_import_item("rust-ownership", "memory management"),
        make_import_item("rust-borrowing", "borrow checker"),
    ];
    let result = svc.import_kbs(items);
    assert_eq!(result.saved.len(), 2);
    assert!(result.failed.is_empty());
}

#[test]
fn add_kbs_imports_all_valid_items() {
    let svc = make_svc();
    let items = vec![
        make_import_item("rust-ownership", "memory management"),
        make_import_item("rust-borrowing", "borrow checker"),
    ];
    let result = svc.import_kbs(items);
    assert_eq!(result.saved.len(), 2);
    assert!(result.failed.is_empty());
}

#[test]
fn add_kbs_collects_item_with_empty_key() {
    let result = make_svc().import_kbs(vec![make_import_item("", "some value")]);
    assert_eq!(result.failed.len(), 1);
    assert!(result.failed[0].reason.contains("Key"));
}

#[test]
fn add_kbs_collects_item_with_empty_value() {
    let result = make_svc().import_kbs(vec![make_import_item("rust-ownership", "")]);
    assert_eq!(result.failed.len(), 1);
    assert!(result.failed[0].reason.contains("Value"));
}

#[test]
fn add_kbs_collects_duplicate_key() {
    let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
    let svc = make_svc_with_store(store);
    let result = svc.import_kbs(vec![make_import_item("rust-ownership", "some value")]);
    assert_eq!(result.failed.len(), 1);
}

#[test]
fn add_kbs_mixed_batch_correct_counts() {
    let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
    let svc = make_svc_with_store(store);
    let items = vec![
        make_import_item("rust-borrowing", "borrow checker"),
        make_import_item("", "some value"),
        make_import_item("rust-ownership", "duplicate"),
    ];
    let result = svc.import_kbs(items);
    assert_eq!(result.saved.len(), 1);
    assert_eq!(result.failed.len(), 2);
}

#[test]
fn add_kbs_empty_input_returns_empty_result() {
    let result = make_svc().import_kbs(vec![]);
    assert!(result.saved.is_empty());
    assert!(result.failed.is_empty());
}

// ---- quote tests ----

#[test]
fn quote_returns_quote_category_entry() {
    let mut kb = make_kb("id-1", "stoic-wisdom");
    kb.category = "quote".to_string();
    let store = MockKbStore::with(vec![kb]);
    let svc = make_svc_with_store(store);
    let result = svc.quote().unwrap();
    assert_eq!(result.category, "quote");
}

#[test]
fn quote_propagates_not_found_when_no_quotes_exist() {
    let svc = make_svc();
    assert!(matches!(svc.quote(), Err(Error::QuoteNotFound)));
}

// ---- ask / get delegation smoke tests ----

#[test]
fn ask_returns_results_without_error() {
    let query = SemanticQuery {
        text: "rust memory".to_string(),
        limit: Some(5),
        threshold: None,
    };
    assert!(make_svc().ask(&query).is_ok());
}

#[test]
fn ask_fails_when_embedding_provider_fails() {
    let svc = KBService::new(
        MockKbStore::new(),
        ServiceDeps {
            vector_store: MockVectorStore::default(),
            embedder: FailingEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
        },
    );
    let query = SemanticQuery {
        text: "rust".to_string(),
        limit: Some(5),
        threshold: None,
    };
    assert!(matches!(svc.ask(&query), Err(Error::EmbeddingError(_))));
}

// ---- parent relationship tests ----

#[test]
fn add_kb_with_valid_parent_succeeds() {
    let parent = make_kb("parent-id", "parent-key");
    let store = MockKbStore::with(vec![parent]);
    let svc = make_svc_with_store(store);
    let mut new_kb = make_new_kb("child-key");
    new_kb.parent = Some("parent-id".to_string());
    let result = svc.add_kb(new_kb);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().parent, Some("parent-id".to_string()));
}

#[test]
fn add_kb_with_missing_parent_fails() {
    let svc = make_svc();
    let mut new_kb = make_new_kb("child-key");
    new_kb.parent = Some("nonexistent-parent-id".to_string());
    assert!(matches!(svc.add_kb(new_kb), Err(Error::ParentKBNotFound)));
}

#[test]
fn update_kb_with_valid_parent_succeeds() {
    let parent = make_kb("parent-id", "parent-key");
    let child = make_kb("child-id", "child-key");
    let store = MockKbStore::with(vec![parent, child]);
    let svc = make_svc_with_store(store);
    let update = KbUpdate {
        id: "child-id".to_string(),
        key: None,
        value: None,
        notes: None,
        category: None,
        namespace: None,
        reference: None,
        tags: None,
        parent: Some("parent-id".to_string()),
        path: None,
    };
    assert!(svc.update_kb(update).is_ok());
    let fetched = svc.get_kb_by_id("child-id").unwrap().unwrap();
    assert_eq!(fetched.parent, Some("parent-id".to_string()));
}

#[test]
fn update_kb_with_missing_parent_fails() {
    let child = make_kb("child-id", "child-key");
    let store = MockKbStore::with(vec![child]);
    let svc = make_svc_with_store(store);
    let update = KbUpdate {
        id: "child-id".to_string(),
        key: None,
        value: None,
        notes: None,
        category: None,
        namespace: None,
        reference: None,
        tags: None,
        parent: Some("nonexistent-parent-id".to_string()),
        path: None,
    };
    assert!(matches!(
        svc.update_kb(update),
        Err(Error::ParentKBNotFound)
    ));
}

#[test]
fn delete_kb_with_children_fails_and_lists_them() {
    let parent = make_kb("parent-id", "parent-key");
    let mut child = make_kb("child-id", "child-key");
    child.parent = Some("parent-id".to_string());
    let store = MockKbStore::with(vec![parent, child]);
    let svc = make_svc_with_store(store);
    let result = svc.delete_kb("parent-id");
    assert!(matches!(result, Err(Error::KBHasChildrenError(_))));
    if let Err(Error::KBHasChildrenError(ids)) = result {
        assert!(ids.contains("child-id"));
    }
}

#[test]
fn delete_kb_without_children_succeeds() {
    let parent = make_kb("parent-id", "parent-key");
    let mut child = make_kb("child-id", "child-key");
    child.parent = Some("parent-id".to_string());
    let store = MockKbStore::with(vec![parent, child]);
    let svc = make_svc_with_store(store);
    // delete child first, then parent
    assert!(svc.delete_kb("child-id").is_ok());
    assert!(svc.delete_kb("parent-id").is_ok());
}

#[test]
fn import_kb_with_valid_parent_key_resolves_id() {
    let parent = make_kb("parent-id", "parent-key");
    let store = MockKbStore::with(vec![parent]);
    let svc = make_svc_with_store(store);
    let mut item = make_import_item("child-key", "child value");
    item.parent_key = Some("parent-key".to_string());
    let result = svc.import_kbs(vec![item]);
    assert_eq!(result.saved.len(), 1);
    assert_eq!(result.failed.len(), 0);
    assert_eq!(result.saved[0].parent, Some("parent-id".to_string()));
}

#[test]
fn import_kb_with_missing_parent_key_reports_failure() {
    let svc = make_svc();
    let mut item = make_import_item("child-key", "child value");
    item.parent_key = Some("nonexistent-key".to_string());
    let result = svc.import_kbs(vec![item]);
    assert_eq!(result.saved.len(), 0);
    assert_eq!(result.failed.len(), 1);
    assert!(result.failed[0].reason.contains("nonexistent-key"));
}

// ---- export_kbs tests ----

#[test]
fn export_kbs_returns_empty_when_no_items() {
    let result = make_svc().export_kbs(KbFilter::default());
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn export_kbs_returns_item_without_parent_field() {
    let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
    let svc = make_svc_with_store(store);
    let items = svc.export_kbs(KbFilter::default()).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "rust-ownership");
    assert!(items[0].parent_key.is_none());
}

#[test]
fn export_kbs_sets_parent_key_when_parent_in_set() {
    let parent = make_kb("parent-id", "parent-key");
    let child = make_kb_with_parent("child-id", "child-key", "parent-id");
    let store = MockKbStore::with(vec![parent, child]);
    let svc = make_svc_with_store(store);

    let items = svc.export_kbs(KbFilter::default()).unwrap();
    assert_eq!(items.len(), 2);

    let parent_pos = items.iter().position(|i| i.key == "parent-key").unwrap();
    let child_pos = items.iter().position(|i| i.key == "child-key").unwrap();
    assert!(parent_pos < child_pos, "parent must appear before child");

    assert_eq!(items[child_pos].parent_key, Some("parent-key".to_string()));
}

#[test]
fn export_kbs_omits_parent_field_when_parent_not_in_set() {
    let mut parent = make_kb("parent-id", "parent-key");
    parent.category = "excluded".to_string();
    let child = make_kb_with_parent("child-id", "child-key", "parent-id");
    // child category is "concept" (default from make_kb)

    let store = MockKbStore::with(vec![parent, child]);
    let svc = make_svc_with_store(store);

    let filter = KbFilter {
        category: Some("concept".to_string()),
        ..Default::default()
    };
    let items = svc.export_kbs(filter).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "child-key");
    assert!(
        items[0].parent_key.is_none(),
        "parent is excluded from set, so Parent field must be omitted"
    );
}

#[test]
fn export_kbs_orders_multi_level_hierarchy() {
    let grandparent = make_kb("gp-id", "gp-key");
    let parent = make_kb_with_parent("p-id", "p-key", "gp-id");
    let child = make_kb_with_parent("c-id", "c-key", "p-id");
    let store = MockKbStore::with(vec![child.clone(), parent.clone(), grandparent.clone()]);
    let svc = make_svc_with_store(store);

    let items = svc.export_kbs(KbFilter::default()).unwrap();
    assert_eq!(items.len(), 3);

    let gp_pos = items.iter().position(|i| i.key == "gp-key").unwrap();
    let p_pos = items.iter().position(|i| i.key == "p-key").unwrap();
    let c_pos = items.iter().position(|i| i.key == "c-key").unwrap();
    assert!(gp_pos < p_pos, "grandparent must be before parent");
    assert!(p_pos < c_pos, "parent must be before child");
}

// ---- media category tests ----

fn make_media_kb(id: &str, key: &str) -> Kb {
    Kb {
        id: id.to_string(),
        key: key.to_string(),
        value: "a media file".to_string(),
        notes: String::new(),
        category: "media".to_string(),
        reference: String::new(),
        namespace: "test".to_string(),
        tags: vec![],
        created_on: "2026-01-01T00:00:00+0000".to_string(),
        parent: None,
        path: None,
        media_extension: Some("jpg".to_string()),
    }
}

fn make_media_new_kb(key: &str, media_url: Option<&str>) -> NewKb {
    NewKb {
        key: key.to_string(),
        value: "a media file".to_string(),
        notes: String::new(),
        category: "media".to_string(),
        reference: String::new(),
        namespace: "test".to_string(),
        tags: vec![],
        parent: None,
        path: None,
        media_url: media_url.map(str::to_string),
        media_extension: None,
    }
}

#[test]
fn add_kb_media_copies_local_file_before_save() {
    let media_store = MockMediaStore::default();
    let svc = KBService::new(
        MockKbStore::new(),
        ServiceDeps {
            vector_store: MockVectorStore::default(),
            embedder: MockEmbeddingProvider,
            media_store: media_store.clone(),
            media_fetcher: MockMediaFetcher,
            base_dir: "/base".to_string(),
        },
    );
    let result = svc.add_kb(make_media_new_kb("photo", Some("/tmp/photo.jpg")));
    assert!(result.is_ok());
    assert_eq!(media_store.stored.borrow().len(), 1);
    assert!(media_store.stored.borrow()[0].contains("photo.jpg"));
}

#[test]
fn add_kb_media_requires_media_url() {
    let svc = KBService::new(
        MockKbStore::new(),
        ServiceDeps {
            vector_store: MockVectorStore::default(),
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore::default(),
            media_fetcher: MockMediaFetcher,
            base_dir: "/base".to_string(),
        },
    );
    assert!(matches!(
        svc.add_kb(make_media_new_kb("photo", None)),
        Err(Error::MediaUrlRequired(_))
    ));
}

#[test]
fn add_kb_media_fails_when_store_media_fails() {
    let svc = KBService::new(
        MockKbStore::new(),
        ServiceDeps {
            vector_store: MockVectorStore::default(),
            embedder: MockEmbeddingProvider,
            media_store: FailingMediaStore,
            media_fetcher: MockMediaFetcher,
            base_dir: "/base".to_string(),
        },
    );
    assert!(matches!(
        svc.add_kb(make_media_new_kb("photo", Some("/tmp/photo.jpg"))),
        Err(Error::MediaCopyError(_))
    ));
}

#[test]
fn add_kb_media_uses_fetcher_for_http_urls() {
    let media_store = MockMediaStore::default();
    let svc = KBService::new(
        MockKbStore::new(),
        ServiceDeps {
            vector_store: MockVectorStore::default(),
            embedder: MockEmbeddingProvider,
            media_store: media_store.clone(),
            media_fetcher: MockMediaFetcher,
            base_dir: "/base".to_string(),
        },
    );
    let result = svc.add_kb(make_media_new_kb(
        "remote-photo",
        Some("https://example.com/image.jpg"),
    ));
    assert!(result.is_ok());
    assert_eq!(media_store.stored.borrow().len(), 1);
}

#[test]
fn delete_kb_media_deletes_file_first() {
    let media_store = MockMediaStore::default();
    let store = MockKbStore::with(vec![make_media_kb("id-1", "photo")]);
    let svc = KBService::new(
        store,
        ServiceDeps {
            vector_store: MockVectorStore::default(),
            embedder: MockEmbeddingProvider,
            media_store: media_store.clone(),
            media_fetcher: MockMediaFetcher,
            base_dir: "/base".to_string(),
        },
    );
    assert!(svc.delete_kb("id-1").is_ok());
    assert_eq!(media_store.deleted.borrow().len(), 1);
    assert!(media_store.deleted.borrow()[0].contains("photo.jpg"));
}

#[test]
fn delete_kb_media_aborts_when_file_delete_fails() {
    let store = MockKbStore::with(vec![make_media_kb("id-1", "photo")]);
    let svc = KBService::new(
        store,
        ServiceDeps {
            vector_store: MockVectorStore::default(),
            embedder: MockEmbeddingProvider,
            media_store: FailingMediaStore,
            media_fetcher: MockMediaFetcher,
            base_dir: "/base".to_string(),
        },
    );
    assert!(matches!(
        svc.delete_kb("id-1"),
        Err(Error::MediaDeleteError(_))
    ));
    // Entry must still exist in the store (rollback).
    // The mock store's delete_kb was never called.
}

#[test]
fn delete_kb_non_media_skips_file_delete() {
    let media_store = MockMediaStore::default();
    let store = MockKbStore::with(vec![make_kb("id-1", "regular-entry")]);
    let svc = KBService::new(
        store,
        ServiceDeps {
            vector_store: MockVectorStore::default(),
            embedder: MockEmbeddingProvider,
            media_store: media_store.clone(),
            media_fetcher: MockMediaFetcher,
            base_dir: "/base".to_string(),
        },
    );
    assert!(svc.delete_kb("id-1").is_ok());
    assert!(media_store.deleted.borrow().is_empty());
}

#[test]
fn update_kb_media_blocks_path_change() {
    let store = MockKbStore::with(vec![make_media_kb("id-1", "photo")]);
    let svc = make_svc_with_store(store);
    let update = KbUpdate {
        id: "id-1".to_string(),
        key: None,
        value: None,
        notes: None,
        category: None,
        namespace: None,
        reference: None,
        tags: None,
        parent: None,
        path: Some("/new/path".to_string()),
    };
    assert!(matches!(
        svc.update_kb(update),
        Err(Error::MediaPathUpdateNotAllowed(_))
    ));
}

#[test]
fn update_kb_media_allows_non_path_changes() {
    let store = MockKbStore::with(vec![make_media_kb("id-1", "photo")]);
    let svc = make_svc_with_store(store);
    let update = KbUpdate {
        id: "id-1".to_string(),
        key: None,
        value: Some("updated description".to_string()),
        notes: None,
        category: None,
        namespace: None,
        reference: None,
        tags: None,
        parent: None,
        path: None,
    };
    assert!(svc.update_kb(update).is_ok());
}

// ---- export_media tests ----

fn make_svc_with_media_store(
    media_store: MockMediaStore,
) -> KBService<MockKbStore, MockVectorStore, MockEmbeddingProvider, MockMediaStore, MockMediaFetcher>
{
    KBService::new(
        MockKbStore::new(),
        ServiceDeps {
            vector_store: MockVectorStore::default(),
            embedder: MockEmbeddingProvider,
            media_store,
            media_fetcher: MockMediaFetcher,
            base_dir: "/base".to_string(),
        },
    )
}

fn make_export_media_item(key: &str, namespace: &str, ext: &str) -> ExportKbItem {
    ExportKbItem {
        key: key.to_string(),
        value: String::new(),
        notes: String::new(),
        category: "media".to_string(),
        reference: String::new(),
        namespace: namespace.to_string(),
        tags: vec![],
        parent_key: None,
        path: None,
        media_extension: Some(ext.to_string()),
    }
}

#[test]
fn export_media_bulk_copies_all_when_no_filter() {
    let media_store = MockMediaStore::default();
    let svc = make_svc_with_media_store(media_store.clone());
    let params = ExportMediaParams {
        target_dir: "/target".to_string(),
        category: None,
        namespace: None,
        items: vec![],
    };
    let result = svc.export_media(params);
    assert!(result.is_ok());
    let dirs = media_store.dirs_copied.borrow();
    assert_eq!(dirs.len(), 1);
    assert_eq!(dirs[0].0, "/base/media");
    assert_eq!(dirs[0].1, "/target/media");
}

#[test]
fn export_media_bulk_copies_all_when_media_category_only() {
    let media_store = MockMediaStore::default();
    let svc = make_svc_with_media_store(media_store.clone());
    let params = ExportMediaParams {
        target_dir: "/target".to_string(),
        category: Some("media".to_string()),
        namespace: None,
        items: vec![],
    };
    let result = svc.export_media(params);
    assert!(result.is_ok());
    let dirs = media_store.dirs_copied.borrow();
    assert_eq!(dirs.len(), 1);
    assert_eq!(dirs[0].0, "/base/media");
    assert_eq!(dirs[0].1, "/target/media");
}

#[test]
fn export_media_bulk_copies_namespace_when_media_and_namespace() {
    let media_store = MockMediaStore::default();
    let svc = make_svc_with_media_store(media_store.clone());
    let params = ExportMediaParams {
        target_dir: "/target".to_string(),
        category: Some("media".to_string()),
        namespace: Some("swe".to_string()),
        items: vec![],
    };
    let result = svc.export_media(params);
    assert!(result.is_ok());
    let dirs = media_store.dirs_copied.borrow();
    assert_eq!(dirs.len(), 1);
    assert_eq!(dirs[0].0, "/base/media/swe");
    assert_eq!(dirs[0].1, "/target/media/swe");
}

#[test]
fn export_media_copies_item_by_item_for_other_filter() {
    let media_store = MockMediaStore::default();
    let svc = make_svc_with_media_store(media_store.clone());
    let item = make_export_media_item("photo", "swe", "jpg");
    let params = ExportMediaParams {
        target_dir: "/target".to_string(),
        category: Some("concept".to_string()),
        namespace: None,
        items: vec![item],
    };
    let result = svc.export_media(params);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);
    assert!(media_store.dirs_copied.borrow().is_empty());
    let stored = media_store.stored.borrow();
    assert_eq!(stored.len(), 1);
    assert!(stored[0].contains("/target/media/swe/photo.jpg"));
}

#[test]
fn export_media_skips_non_media_items_in_item_by_item() {
    let media_store = MockMediaStore::default();
    let svc = make_svc_with_media_store(media_store.clone());
    let non_media = ExportKbItem {
        key: "concept-key".to_string(),
        value: String::new(),
        notes: String::new(),
        category: "concept".to_string(),
        reference: String::new(),
        namespace: "swe".to_string(),
        tags: vec![],
        parent_key: None,
        path: None,
        media_extension: None,
    };
    let params = ExportMediaParams {
        target_dir: "/target".to_string(),
        category: Some("concept".to_string()),
        namespace: None,
        items: vec![non_media],
    };
    let result = svc.export_media(params);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
    assert!(media_store.stored.borrow().is_empty());
}
