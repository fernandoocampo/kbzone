use crate::domain::{
    EmbeddingInput, FailedImportItem, ImportBatchResult, ImportKbItem, Kb, KbFilter, KbItem,
    KbUpdate, NewKb, ReindexResult, ScoredKbItem, SemanticQuery,
};
use crate::errors::Error;
use crate::ports::{EmbeddingProvider, KbStore, VectorStore};

/// Constructor parameter struct that groups the two semantic dependencies,
/// satisfying the 2-param rule for [`KBService::new`].
pub(crate) struct SemanticDeps<V: VectorStore, E: EmbeddingProvider> {
    pub vector_store: V,
    pub embedder: E,
}

/// Unified application service — owns all three outbound ports directly and
/// orchestrates both CRUD and semantic (embedding) operations.
#[derive(Debug, Clone)]
pub(crate) struct KBService<S: KbStore, V: VectorStore, E: EmbeddingProvider> {
    store: S,
    vector_store: V,
    embedder: E,
}

impl<S: KbStore, V: VectorStore, E: EmbeddingProvider> KBService<S, V, E> {
    pub fn new(store: S, deps: SemanticDeps<V, E>) -> Self {
        Self {
            store,
            vector_store: deps.vector_store,
            embedder: deps.embedder,
        }
    }

    // ---------------------------------------------------------------------------
    // Public API
    // ---------------------------------------------------------------------------

    /// Creates a new KB entry and indexes it for semantic search.
    /// Embedding failures are non-fatal: a warning is printed and `Ok(kb)` is returned.
    pub fn add_kb(&self, new_kb: NewKb) -> Result<Kb, Error> {
        let kb = self.add_kb_crud(new_kb)?;
        let input = EmbeddingInput {
            kb_id: kb.id.clone(),
            text: kb.embedding_text(),
        };
        if let Err(e) = self.index_kb(&input) {
            eprintln!("Warning: could not index embedding for '{}': {}", kb.key, e);
        }
        Ok(kb)
    }

    pub fn get_kb_by_id(&self, id: &str) -> Result<Option<Kb>, Error> {
        self.store.get_kb_by_id(id)
    }

    pub fn get_kb_by_key(&self, key: &str) -> Result<Option<Kb>, Error> {
        self.store.get_kb_by_key(key)
    }

    pub fn list_kbs(&self, filter: KbFilter) -> Result<Vec<KbItem>, Error> {
        self.store.list_kbs(&filter)
    }

    pub fn search_kbs(&self, keyword: &str) -> Result<Vec<KbItem>, Error> {
        let filter = KbFilter {
            keyword: Some(keyword.to_string()),
            ..Default::default()
        };
        self.store.search_kbs(&filter)
    }

    /// Merges `update` into the existing entry, persists, and re-indexes only if the
    /// embedding text changed. Embedding failures are non-fatal.
    pub fn update_kb(&self, update: KbUpdate) -> Result<(), Error> {
        let existing = self
            .store
            .get_kb_by_id(&update.id)?
            .ok_or(Error::KBNotFound)?;

        let old_embed_text = existing.embedding_text();

        let updated = Kb {
            id: existing.id,
            key: update.key.unwrap_or(existing.key),
            value: update.value.unwrap_or(existing.value),
            notes: update.notes.unwrap_or(existing.notes),
            category: update.category.unwrap_or(existing.category),
            namespace: update.namespace.unwrap_or(existing.namespace),
            reference: update.reference.unwrap_or(existing.reference),
            tags: update.tags.unwrap_or(existing.tags),
            created_on: existing.created_on,
        };

        let new_embed_text = updated.embedding_text();

        if old_embed_text != new_embed_text {
            let input = EmbeddingInput {
                kb_id: updated.id.clone(),
                text: new_embed_text,
            };
            if let Err(e) = self.index_kb(&input) {
                eprintln!(
                    "Warning: could not update embedding for '{}': {}",
                    updated.id, e
                );
            }
        }

        self.update_kb_crud(updated)
    }

    /// Deletes an entry and removes its embedding. Embedding removal failure is non-fatal.
    pub fn delete_kb(&self, id: &str) -> Result<(), Error> {
        let deleted = self.store.delete_kb(id)?;
        if !deleted {
            return Err(Error::KBNotFound);
        }
        if let Err(e) = self.remove_index(id) {
            eprintln!("Warning: could not remove embedding for '{}': {}", id, e);
        }
        Ok(())
    }

    pub fn ask(&self, query: &SemanticQuery) -> Result<Vec<ScoredKbItem>, Error> {
        let embedding = self.embedder.embed(&query.text)?;
        self.vector_store.search_similar(query, &embedding)
    }

    /// Re-indexes all entries. Outer `Err` only if `list_kbs` fails.
    /// Per-entry failures are collected in `ReindexResult::failed`.
    pub fn reindex(&self) -> Result<ReindexResult, Error> {
        let items = self.store.list_kbs(&KbFilter::default())?;
        let mut succeeded = Vec::new();
        let mut failed = Vec::new();

        for item in items {
            match self.store.get_kb_by_id(&item.id) {
                Err(e) => failed.push((format!("id={}", item.id), e.to_string())),
                Ok(None) => {
                    failed.push((format!("id={}", item.id), "not found".to_string()));
                }
                Ok(Some(kb)) => {
                    let input = EmbeddingInput {
                        kb_id: kb.id.clone(),
                        text: kb.embedding_text(),
                    };
                    match self.index_kb(&input) {
                        Ok(_) => succeeded.push((kb.key.clone(), kb.id.clone())),
                        Err(e) => failed.push((kb.key.clone(), e.to_string())),
                    }
                }
            }
        }

        Ok(ReindexResult { succeeded, failed })
    }

    /// Batch-imports items, indexing each successfully saved entry.
    /// Never returns `Err` — failures are reported inside `ImportBatchResult`.
    pub fn import_kbs(&self, items: Vec<ImportKbItem>) -> ImportBatchResult {
        let result = self.add_kbs_crud(items);
        for kb in &result.saved {
            let input = EmbeddingInput {
                kb_id: kb.id.clone(),
                text: kb.embedding_text(),
            };
            if let Err(e) = self.index_kb(&input) {
                eprintln!("Warning: could not index embedding for '{}': {}", kb.key, e);
            }
        }
        result
    }

    // ---------------------------------------------------------------------------
    // Private helpers
    // ---------------------------------------------------------------------------

    /// Low-level CRUD add: duplicate-key check, convert to `Kb`, persist.
    fn add_kb_crud(&self, new_kb: NewKb) -> Result<Kb, Error> {
        let key = new_kb.key.to_lowercase();
        if self.store.get_kb_by_key(&key)?.is_some() {
            return Err(Error::DuplicateKBError);
        }
        let kb = Kb::from(new_kb);
        self.store.save_kb(&kb)?;
        Ok(kb)
    }

    /// Low-level CRUD update: duplicate-key guard, persist.
    fn update_kb_crud(&self, kb: Kb) -> Result<(), Error> {
        if let Some(existing) = self.store.get_kb_by_key(&kb.key)? {
            if existing.id != kb.id {
                return Err(Error::DuplicateKBError);
            }
        }
        let updated = self.store.update_kb(&kb)?;
        if !updated {
            return Err(Error::KBWasNotUpdatedError);
        }
        Ok(())
    }

    /// Batch CRUD add: validates, calls `add_kb_crud`, collects failures.
    fn add_kbs_crud(&self, items: Vec<ImportKbItem>) -> ImportBatchResult {
        let mut saved = Vec::new();
        let mut failed = Vec::new();

        for item in items {
            if let Some(reason) = item.validate() {
                failed.push(FailedImportItem { item, reason });
                continue;
            }
            let new_kb = NewKb::from(item.clone());
            match self.add_kb_crud(new_kb) {
                Ok(kb) => saved.push(kb),
                Err(e) => failed.push(FailedImportItem {
                    item,
                    reason: e.to_string(),
                }),
            }
        }

        ImportBatchResult { saved, failed }
    }

    /// Embeds the text in `input` and stores the resulting vector.
    fn index_kb(&self, input: &EmbeddingInput) -> Result<(), Error> {
        let embedding = self.embedder.embed(&input.text)?;
        self.vector_store.save_embedding(input, &embedding)
    }

    /// Removes the stored embedding for the given KB entry.
    fn remove_index(&self, kb_id: &str) -> Result<(), Error> {
        self.vector_store.delete_embedding(kb_id)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
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

        fn list_kbs(&self, _filter: &KbFilter) -> Result<Vec<KbItem>, Error> {
            Ok(self
                .data
                .borrow()
                .values()
                .map(|kb| KbItem {
                    id: kb.id.clone(),
                    key: kb.key.clone(),
                    category: kb.category.clone(),
                    namespace: kb.namespace.clone(),
                    tags: kb.tags.clone(),
                })
                .collect())
        }

        fn search_kbs(&self, filter: &KbFilter) -> Result<Vec<KbItem>, Error> {
            let keyword = filter.keyword.as_deref().unwrap_or("");
            Ok(self
                .data
                .borrow()
                .values()
                .filter(|kb| kb.tags.iter().any(|t| t.contains(keyword)))
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

        fn list_kbs(&self, _filter: &KbFilter) -> Result<Vec<KbItem>, Error> {
            Ok(vec![KbItem {
                id: self.ghost_id.clone(),
                key: self.ghost_key.clone(),
                category: "concept".to_string(),
                namespace: "default".to_string(),
                tags: vec![],
            }])
        }

        fn search_kbs(&self, _filter: &KbFilter) -> Result<Vec<KbItem>, Error> {
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
        }
    }

    fn make_svc() -> KBService<MockKbStore, MockVectorStore, MockEmbeddingProvider> {
        KBService::new(
            MockKbStore::new(),
            SemanticDeps {
                vector_store: MockVectorStore::default(),
                embedder: MockEmbeddingProvider,
            },
        )
    }

    fn make_svc_with_store(
        store: MockKbStore,
    ) -> KBService<MockKbStore, MockVectorStore, MockEmbeddingProvider> {
        KBService::new(
            store,
            SemanticDeps {
                vector_store: MockVectorStore::default(),
                embedder: MockEmbeddingProvider,
            },
        )
    }

    // ---- add_kb tests ----

    #[test]
    fn add_kb_saves_entry_and_indexes_embedding() {
        let vector = MockVectorStore::default();
        let svc = KBService::new(
            MockKbStore::new(),
            SemanticDeps {
                vector_store: vector.clone(),
                embedder: MockEmbeddingProvider,
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
            SemanticDeps {
                vector_store: FailingVectorStore,
                embedder: MockEmbeddingProvider,
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
            SemanticDeps {
                vector_store: vector.clone(),
                embedder: MockEmbeddingProvider,
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
            SemanticDeps {
                vector_store: vector.clone(),
                embedder: MockEmbeddingProvider,
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
            SemanticDeps {
                vector_store: FailingVectorStore,
                embedder: MockEmbeddingProvider,
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
        };
        assert!(svc.update_kb(update).is_ok());
    }

    // ---- delete_kb tests ----

    #[test]
    fn delete_kb_removes_entry_and_embedding() {
        let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
        let vector = MockVectorStore::default();
        let svc = KBService::new(
            store,
            SemanticDeps {
                vector_store: vector.clone(),
                embedder: MockEmbeddingProvider,
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
            SemanticDeps {
                vector_store: FailingVectorStore,
                embedder: MockEmbeddingProvider,
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

    // ---- search_kbs tests ----

    #[test]
    fn search_kbs_returns_matching_entries() {
        let mut kb = make_kb("id-1", "rust-ownership");
        kb.tags = vec!["memory".to_string(), "rust".to_string()];
        let store = MockKbStore::with(vec![kb]);
        let svc = make_svc_with_store(store);
        let results = svc.search_kbs("memo").unwrap();
        assert_eq!(results.len(), 1);
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
            SemanticDeps {
                vector_store: MockVectorStore::default(),
                embedder: CountingEmbeddingProvider::new(1),
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
            SemanticDeps {
                vector_store: MockVectorStore::default(),
                embedder: MockEmbeddingProvider,
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
            SemanticDeps {
                vector_store: vector.clone(),
                embedder: MockEmbeddingProvider,
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
            SemanticDeps {
                vector_store: vector.clone(),
                embedder: MockEmbeddingProvider,
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
            SemanticDeps {
                vector_store: FailingVectorStore,
                embedder: MockEmbeddingProvider,
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

    // ---- ask / list / search delegation smoke tests ----

    #[test]
    fn ask_returns_results_without_error() {
        let query = SemanticQuery {
            text: "rust memory".to_string(),
            limit: Some(5),
        };
        assert!(make_svc().ask(&query).is_ok());
    }

    #[test]
    fn list_kbs_returns_all_entries() {
        let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
        let result = make_svc_with_store(store)
            .list_kbs(KbFilter::default())
            .unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn ask_fails_when_embedding_provider_fails() {
        let svc = KBService::new(
            MockKbStore::new(),
            SemanticDeps {
                vector_store: MockVectorStore::default(),
                embedder: FailingEmbeddingProvider,
            },
        );
        let query = SemanticQuery {
            text: "rust".to_string(),
            limit: Some(5),
        };
        assert!(matches!(svc.ask(&query), Err(Error::EmbeddingError(_))));
    }
}
