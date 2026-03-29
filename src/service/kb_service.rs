use std::collections::HashSet;

use crate::domain::{
    EmbeddingInput, ExportKbItem, FailedImportItem, ImportBatchResult, ImportKbItem, Kb, KbFilter,
    KbItem, KbUpdate, NewKb, ReindexResult, ScoredKbItem, SemanticQuery,
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

    pub fn get_kbs(&self, filter: KbFilter) -> Result<Vec<KbItem>, Error> {
        self.store.get_kbs(&filter)
    }

    /// Merges `update` into the existing entry, persists, and re-indexes only if the
    /// embedding text changed. Embedding failures are non-fatal.
    pub fn update_kb(&self, update: KbUpdate) -> Result<(), Error> {
        let existing = self
            .store
            .get_kb_by_id(&update.id)?
            .ok_or(Error::KBNotFound)?;

        self.validate_parent_exists(&update.parent)?;

        let old_embed_text = existing.embedding_text();

        let updated = Kb {
            id: existing.id,
            key: update.key.map(|v| v.to_lowercase()).unwrap_or(existing.key),
            value: update.value.unwrap_or(existing.value),
            notes: update.notes.unwrap_or(existing.notes),
            category: update
                .category
                .map(|v| v.to_lowercase())
                .unwrap_or(existing.category),
            namespace: update
                .namespace
                .map(|v| v.to_lowercase())
                .unwrap_or(existing.namespace),
            reference: update.reference.unwrap_or(existing.reference),
            tags: update.tags.unwrap_or(existing.tags),
            created_on: existing.created_on,
            parent: update.parent.or(existing.parent),
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
    /// Returns `KBHasChildrenError` if the entry has children — delete them first.
    pub fn delete_kb(&self, id: &str) -> Result<(), Error> {
        let children = self.store.get_children_ids(id)?;
        if !children.is_empty() {
            return Err(Error::KBHasChildrenError(children.join(", ")));
        }
        let deleted = self.store.delete_kb(id)?;
        if !deleted {
            return Err(Error::KBNotFound);
        }
        if let Err(e) = self.remove_index(id) {
            eprintln!("Warning: could not remove embedding for '{}': {}", id, e);
        }
        Ok(())
    }

    pub fn quote(&self) -> Result<Kb, Error> {
        self.store.random_quote()
    }

    pub fn ask(&self, query: &SemanticQuery) -> Result<Vec<ScoredKbItem>, Error> {
        let embedding = self.embedder.embed(&query.text)?;
        self.vector_store.search_similar(query, &embedding)
    }

    /// Re-indexes all entries. Outer `Err` only if `list_kbs` fails.
    /// Per-entry failures are collected in `ReindexResult::failed`.
    pub fn reindex(&self) -> Result<ReindexResult, Error> {
        let items = self.store.get_kbs(&KbFilter::default())?;
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

    /// Exports entries matching `filter` as a list of [`ExportKbItem`], ordered so
    /// parents always appear before their children. If a parent is not in the filtered
    /// set, the child's `parent_key` field is omitted so the file can be re-imported
    /// cleanly.
    pub fn export_kbs(&self, filter: KbFilter) -> Result<Vec<ExportKbItem>, Error> {
        let kbs = self.store.get_kbs_full(&filter)?;
        if kbs.is_empty() {
            return Ok(vec![]);
        }

        let id_to_key: std::collections::HashMap<&str, &str> = kbs
            .iter()
            .map(|kb| (kb.id.as_str(), kb.key.as_str()))
            .collect();

        let mut emitted: HashSet<&str> = HashSet::new();
        let mut ordered: Vec<&Kb> = Vec::with_capacity(kbs.len());
        let mut remaining: Vec<&Kb> = kbs.iter().collect();

        while !remaining.is_empty() {
            let before = remaining.len();
            remaining.retain(|kb| {
                let parent_in_set = kb
                    .parent
                    .as_ref()
                    .is_some_and(|pid| id_to_key.contains_key(pid.as_str()));
                let parent_emitted = kb
                    .parent
                    .as_ref()
                    .is_none_or(|pid| emitted.contains(pid.as_str()));

                if !parent_in_set || parent_emitted {
                    emitted.insert(kb.id.as_str());
                    ordered.push(kb);
                    false
                } else {
                    true
                }
            });
            if remaining.len() == before {
                for kb in remaining.drain(..) {
                    ordered.push(kb);
                }
            }
        }

        let items = ordered
            .into_iter()
            .map(|kb| ExportKbItem {
                key: kb.key.clone(),
                value: kb.value.clone(),
                notes: kb.notes.clone(),
                category: kb.category.clone(),
                reference: kb.reference.clone(),
                namespace: kb.namespace.clone(),
                tags: kb.tags.clone(),
                parent_key: kb
                    .parent
                    .as_ref()
                    .and_then(|pid| id_to_key.get(pid.as_str()))
                    .map(|s| s.to_string()),
            })
            .collect();

        Ok(items)
    }

    // ---------------------------------------------------------------------------
    // Private helpers
    // ---------------------------------------------------------------------------

    /// Low-level CRUD add: duplicate-key check, parent existence check, convert to `Kb`, persist.
    fn add_kb_crud(&self, new_kb: NewKb) -> Result<Kb, Error> {
        let key = new_kb.key.to_lowercase();
        if self.store.get_kb_by_key(&key)?.is_some() {
            return Err(Error::DuplicateKBError);
        }
        self.validate_parent_exists(&new_kb.parent)?;
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

    /// Batch CRUD add: validates, resolves parent_key to parent_id, calls `add_kb_crud`, collects failures.
    fn add_kbs_crud(&self, items: Vec<ImportKbItem>) -> ImportBatchResult {
        let mut saved = Vec::new();
        let mut failed = Vec::new();

        for item in items {
            if let Some(reason) = item.validate() {
                failed.push(FailedImportItem { item, reason });
                continue;
            }
            let parent_key = item.parent_key.clone();
            let parent_id = if let Some(pk) = parent_key {
                match self.store.get_kb_by_key(&pk) {
                    Ok(Some(parent_kb)) => Some(parent_kb.id),
                    Ok(None) => {
                        failed.push(FailedImportItem {
                            item,
                            reason: format!("parent key not found: {}", pk),
                        });
                        continue;
                    }
                    Err(e) => {
                        failed.push(FailedImportItem {
                            item,
                            reason: e.to_string(),
                        });
                        continue;
                    }
                }
            } else {
                None
            };
            let mut new_kb = NewKb::from(item.clone());
            new_kb.parent = parent_id;
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

    fn validate_parent_exists(&self, parent: &Option<String>) -> Result<(), Error> {
        if let Some(ref pid) = parent {
            self.store
                .get_kb_by_id(pid)?
                .ok_or(Error::ParentKBNotFound)?;
        }
        Ok(())
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
#[path = "kb_service_tests.rs"]
mod tests;
