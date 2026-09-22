use std::collections::HashSet;

use crate::domain::{
    EmbeddingInput, ExportKbItem, ExportMediaParams, FailedImportItem, ImportBatchResult,
    ImportKbItem, Kb, KbFilter, KbItem, KbUpdate, MediaPathParams, NewKb, ReindexResult,
    ScoredKbItem, SemanticQuery, StoreMediaParams, build_metadata, file_extension,
    is_media_category, media_file_path, normalize_path, parse_metadata_input,
};
use crate::errors::Error;
use crate::ports::{EmbeddingProvider, KbStore, MediaFetcher, MediaStore, VectorStore};

/// Constructor parameter struct that groups all outbound dependencies
/// beyond the primary store, satisfying the 2-param rule for [`KBService::new`].
pub(crate) struct ServiceDeps<V: VectorStore, E: EmbeddingProvider, M: MediaStore, F: MediaFetcher>
{
    pub vector_store: V,
    pub embedder: E,
    pub media_store: M,
    pub media_fetcher: F,
    pub base_dir: String,
}

/// Unified application service — owns all outbound ports and orchestrates
/// CRUD, semantic (embedding), and media file operations.
#[derive(Debug, Clone)]
pub(crate) struct KBService<
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
> {
    store: S,
    vector_store: V,
    embedder: E,
    media_store: M,
    media_fetcher: F,
    base_dir: String,
}

impl<S: KbStore, V: VectorStore, E: EmbeddingProvider, M: MediaStore, F: MediaFetcher>
    KBService<S, V, E, M, F>
{
    pub fn new(store: S, deps: ServiceDeps<V, E, M, F>) -> Self {
        Self {
            store,
            vector_store: deps.vector_store,
            embedder: deps.embedder,
            media_store: deps.media_store,
            media_fetcher: deps.media_fetcher,
            base_dir: deps.base_dir,
        }
    }

    // ---------------------------------------------------------------------------
    // Public API
    // ---------------------------------------------------------------------------

    /// Creates a new KB entry and indexes it for semantic search.
    /// For `media` category entries, the media file is fetched/copied BEFORE the DB row
    /// is saved — a media failure returns an error without creating a dangling DB record.
    /// Embedding failures are non-fatal: a warning is printed and `Ok(kb)` is returned.
    pub fn add_kb(&self, new_kb: NewKb) -> Result<Kb, Error> {
        if is_media_category(&new_kb.category) {
            self.store_media_for_new_kb(&new_kb)?;
        }
        let kb = self.add_kb_crud(new_kb)?;
        let input = EmbeddingInput {
            kb_id: kb.id.clone(),
            text: kb.embedding_text(),
            namespace: kb.namespace.clone(),
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
    /// Returns `MediaPathUpdateNotAllowed` if the entry is a media item and `path` is set.
    pub fn update_kb(&self, update: KbUpdate) -> Result<(), Error> {
        let existing = self
            .store
            .get_kb_by_id(&update.id)?
            .ok_or(Error::KBNotFound)?;

        if is_media_category(&existing.category) && update.path.is_some() {
            return Err(Error::MediaPathUpdateNotAllowed(existing.key.clone()));
        }

        self.validate_parent_exists(&update.parent)?;

        let old_embed_text = existing.embedding_text();

        let path = match update.path {
            Some(ref p) if !p.is_empty() => Some(normalize_path(p)?),
            Some(_) => None,
            None => existing.path.clone(),
        };

        let metadata = match update.metadata {
            Some(ref s) if !s.trim().is_empty() => build_metadata(parse_metadata_input(s))?,
            Some(_) => std::collections::BTreeMap::new(),
            None => existing.metadata.clone(),
        };

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
            metadata,
            created_on: existing.created_on,
            parent: update.parent.or(existing.parent),
            path,
            media_extension: existing.media_extension,
        };

        let new_embed_text = updated.embedding_text();

        if old_embed_text != new_embed_text {
            let input = EmbeddingInput {
                kb_id: updated.id.clone(),
                text: new_embed_text,
                namespace: updated.namespace.clone(),
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
    /// For `media` category entries the media file is deleted FIRST; failure aborts the
    /// operation so the DB row is not left without its file.
    /// Returns `KBHasChildrenError` if the entry has children — delete them first.
    pub fn delete_kb(&self, id: &str) -> Result<(), Error> {
        let children = self.store.get_children_ids(id)?;
        if !children.is_empty() {
            return Err(Error::KBHasChildrenError(children.join(", ")));
        }
        let existing = self.store.get_kb_by_id(id)?.ok_or(Error::KBNotFound)?;
        if is_media_category(&existing.category)
            && let Some(ref ext) = existing.media_extension
        {
            let path = media_file_path(&MediaPathParams {
                base_dir: &self.base_dir,
                namespace: &existing.namespace,
                path: existing.path.as_deref(),
                key: &existing.key,
                extension: Some(ext),
            });
            self.media_store.delete_media(&path)?;
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

    pub fn random(&self, category: &str, namespace: Option<&str>) -> Result<Kb, Error> {
        self.store.random_by_category(category, namespace)
    }

    /// Returns all distinct, non-empty category values, optionally scoped to a
    /// namespace, sorted alphabetically.
    pub fn categories(&self, namespace: Option<&str>) -> Result<Vec<String>, Error> {
        self.store.get_categories(namespace)
    }

    /// Returns all distinct, non-empty namespace values, sorted alphabetically.
    pub fn namespaces(&self) -> Result<Vec<String>, Error> {
        self.store.get_namespaces()
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
                        namespace: kb.namespace.clone(),
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
                namespace: kb.namespace.clone(),
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
                path: kb.path.clone(),
                media_extension: kb.media_extension.clone(),
            })
            .collect();

        Ok(items)
    }

    /// Copies media files to `params.target_dir`, applying the smart copy strategy:
    ///
    /// - No category/namespace filter, or category=media without namespace → bulk-copy
    ///   the entire `<base_dir>/media/` directory.
    /// - category=media + namespace → bulk-copy `<base_dir>/media/<namespace>/`.
    /// - Any other filter → iterate over `params.items` and copy each media entry individually.
    ///
    /// Returns the number of files copied.
    pub fn export_media(&self, params: ExportMediaParams) -> Result<u64, Error> {
        let is_media_cat = params
            .category
            .as_deref()
            .map(is_media_category)
            .unwrap_or(false);
        let bulk_all = params.namespace.is_none() && (params.category.is_none() || is_media_cat);

        if bulk_all {
            let source = format!("{}/media", self.base_dir);
            let dest = format!("{}/media", params.target_dir);
            return self.media_store.copy_dir(&source, &dest);
        }

        if let Some(ns) = params.namespace.as_deref().filter(|_| is_media_cat) {
            let source = format!("{}/media/{}", self.base_dir, ns);
            let dest = format!("{}/media/{}", params.target_dir, ns);
            return self.media_store.copy_dir(&source, &dest);
        }

        let mut count = 0u64;
        for item in &params.items {
            if !is_media_category(&item.category) {
                continue;
            }
            if let Some(ext) = &item.media_extension {
                let source = media_file_path(&MediaPathParams {
                    base_dir: &self.base_dir,
                    namespace: &item.namespace,
                    path: item.path.as_deref(),
                    key: &item.key,
                    extension: Some(ext),
                });
                let dest = media_file_path(&MediaPathParams {
                    base_dir: &params.target_dir,
                    namespace: &item.namespace,
                    path: item.path.as_deref(),
                    key: &item.key,
                    extension: Some(ext),
                });
                self.media_store.store_media(&StoreMediaParams {
                    source,
                    destination: dest,
                })?;
                count += 1;
            }
        }
        Ok(count)
    }

    // ---------------------------------------------------------------------------
    // Private helpers
    // ---------------------------------------------------------------------------

    /// Low-level CRUD add: duplicate-key check, parent existence check, convert to `Kb`, persist.
    fn add_kb_crud(&self, mut new_kb: NewKb) -> Result<Kb, Error> {
        let key = new_kb.key.to_lowercase();
        if self.store.get_kb_by_key(&key)?.is_some() {
            return Err(Error::DuplicateKBError);
        }
        self.validate_parent_exists(&new_kb.parent)?;
        new_kb.path = match new_kb.path.take() {
            Some(p) if !p.is_empty() => Some(normalize_path(&p)?),
            _ => None,
        };
        let kb = Kb::from(new_kb);
        self.store.save_kb(&kb)?;
        Ok(kb)
    }

    /// Low-level CRUD update: duplicate-key guard, persist.
    fn update_kb_crud(&self, kb: Kb) -> Result<(), Error> {
        if let Some(existing) = self.store.get_kb_by_key(&kb.key)?
            && existing.id != kb.id
        {
            return Err(Error::DuplicateKBError);
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

    /// Fetches (if URL) or uses directly (if local path) the media file described by
    /// `new_kb.media_url` and copies it to its final destination under `base_dir`.
    /// Returns `MediaUrlRequired` when `media_url` is absent or empty.
    fn store_media_for_new_kb(&self, new_kb: &NewKb) -> Result<(), Error> {
        let url = new_kb
            .media_url
            .as_deref()
            .filter(|u| !u.is_empty())
            .ok_or_else(|| Error::MediaUrlRequired(new_kb.key.clone()))?;

        let is_remote = url.starts_with("http://") || url.starts_with("https://");
        let source = if is_remote {
            self.media_fetcher.fetch(url)?
        } else {
            url.to_string()
        };

        let destination = media_file_path(&MediaPathParams {
            base_dir: &self.base_dir,
            namespace: &new_kb.namespace,
            path: new_kb.path.as_deref(),
            key: &new_kb.key,
            extension: file_extension(url).as_deref(),
        });

        let result = self.media_store.store_media(&StoreMediaParams {
            source: source.clone(),
            destination,
        });

        // Clean up the temp file created by the fetcher; ignore cleanup errors.
        if is_remote {
            let _ = std::fs::remove_file(&source);
        }

        result.map(|_| ())
    }

    fn validate_parent_exists(&self, parent: &Option<String>) -> Result<(), Error> {
        if let Some(pid) = parent {
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
