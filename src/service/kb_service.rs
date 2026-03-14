use crate::domain::{Kb, KbFilter, KbItem, NewKb};
use crate::errors::Error;
use crate::ports::KbStore;

/// Application service generic over any `KbStore` implementation.
#[derive(Debug, Clone)]
pub struct Service<T: KbStore> {
    store: T,
}

impl<T: KbStore> Service<T> {
    pub fn new(store: T) -> Self {
        Self { store }
    }

    /// Creates a new KB entry after checking for duplicate keys.
    pub fn add_kb(&self, new_kb: NewKb) -> Result<Kb, Error> {
        let key = new_kb.key.to_lowercase();
        if self.store.get_kb_by_key(&key)?.is_some() {
            return Err(Error::DuplicateKBError);
        }
        let kb = new_kb.to_kb();
        self.store.save_kb(&kb)?;
        Ok(kb)
    }

    pub fn get_kb_by_id(&self, id: &str) -> Result<Option<Kb>, Error> {
        self.store.get_kb_by_id(id)
    }

    pub fn get_kb_by_key(&self, key: &str) -> Result<Option<Kb>, Error> {
        self.store.get_kb_by_key(key)
    }

    /// Updates an existing KB entry.
    /// Prevents stealing another entry's key: if the new key already exists on a
    /// *different* entry, returns `DuplicateKBError`.
    pub fn update_kb(&self, kb: Kb) -> Result<(), Error> {
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

    /// Deletes a KB entry by ID. Returns `KBNotFound` if no rows were affected.
    pub fn delete_kb(&self, id: &str) -> Result<(), Error> {
        let deleted = self.store.delete_kb(id)?;
        if !deleted {
            return Err(Error::KBNotFound);
        }
        Ok(())
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
}

// ---------------------------------------------------------------------------
// Unit tests using MockKbStore
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

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
            let items = self
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
                .collect();
            Ok(items)
        }

        fn search_kbs(&self, filter: &KbFilter) -> Result<Vec<KbItem>, Error> {
            let keyword = filter.keyword.as_deref().unwrap_or("");
            let items = self
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
                .collect();
            Ok(items)
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

    #[test]
    fn add_kb_succeeds_for_new_key() {
        let svc = Service::new(MockKbStore::new());
        let result = svc.add_kb(make_new_kb("rust-ownership"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().key, "rust-ownership");
    }

    #[test]
    fn add_kb_fails_on_duplicate_key() {
        let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
        let svc = Service::new(store);
        let result = svc.add_kb(make_new_kb("rust-ownership"));
        assert_eq!(result.unwrap_err(), Error::DuplicateKBError);
    }

    #[test]
    fn get_kb_by_id_returns_none_for_unknown_id() {
        let svc = Service::new(MockKbStore::new());
        assert_eq!(svc.get_kb_by_id("no-such-id").unwrap(), None);
    }

    #[test]
    fn delete_kb_returns_not_found_for_missing_id() {
        let svc = Service::new(MockKbStore::new());
        assert_eq!(svc.delete_kb("no-such-id").unwrap_err(), Error::KBNotFound);
    }

    #[test]
    fn delete_kb_succeeds_for_existing_entry() {
        let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
        let svc = Service::new(store);
        assert!(svc.delete_kb("id-1").is_ok());
    }

    #[test]
    fn update_kb_rejects_stolen_key() {
        let existing = make_kb("id-2", "already-taken");
        let store = MockKbStore::with(vec![make_kb("id-1", "my-key"), existing]);
        let svc = Service::new(store);
        let mut updated = make_kb("id-1", "already-taken");
        updated.value = "new value".to_string();
        assert_eq!(svc.update_kb(updated).unwrap_err(), Error::DuplicateKBError);
    }

    #[test]
    fn update_kb_same_key_same_entry_is_ok() {
        let store = MockKbStore::with(vec![make_kb("id-1", "rust-ownership")]);
        let svc = Service::new(store);
        let mut updated = make_kb("id-1", "rust-ownership");
        updated.value = "new value".to_string();
        assert!(svc.update_kb(updated).is_ok());
    }

    #[test]
    fn search_kbs_returns_matching_entries() {
        let mut kb = make_kb("id-1", "rust-ownership");
        kb.tags = vec!["memory".to_string(), "rust".to_string()];
        let store = MockKbStore::with(vec![kb]);
        let svc = Service::new(store);
        let results = svc.search_kbs("memo").unwrap();
        assert_eq!(results.len(), 1);
    }
}
