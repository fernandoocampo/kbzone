use serde::Deserialize;

use crate::domain::{ImportKbItem, KbFilter, KbUpdate, NewKb, ScoredKbItem, SemanticQuery};
use crate::errors::Error;
use crate::ports::{EmbeddingProvider, KbStore, VectorStore};
use crate::service::KBService;

// ---------------------------------------------------------------------------
// Parameter structs (satisfy the 2-param rule)
// ---------------------------------------------------------------------------

pub struct ListParams {
    pub category: Option<String>,
    pub namespace: Option<String>,
    pub tags: Vec<String>,
    pub limit: i64,
    pub offset: i64,
}

pub struct GetParams {
    pub key: Option<String>,
    pub id: Option<String>,
}

pub struct ImportParams {
    pub file: String,
    pub failed_items_file: String,
}

// ---------------------------------------------------------------------------
// Column widths for tabular output
// ---------------------------------------------------------------------------

const COL_SCORE: usize = 8;
const COL_ID: usize = 36;
const COL_KEY: usize = 24;
const COL_CAT: usize = 12;
const COL_NS: usize = 14;
const COL_TAGS: usize = 30;

fn print_table_header() {
    println!(
        "{:<id$}  {:<key$}  {:<cat$}  {:<ns$}  {:<tags$}",
        "ID",
        "KEY",
        "CATEGORY",
        "NAMESPACE",
        "TAGS",
        id = COL_ID,
        key = COL_KEY,
        cat = COL_CAT,
        ns = COL_NS,
        tags = COL_TAGS,
    );
    println!(
        "{}",
        "-".repeat(COL_ID + COL_KEY + COL_CAT + COL_NS + COL_TAGS + 8)
    );
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub fn handle_add<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    new_kb: NewKb,
) -> Result<(), Error> {
    let kb = svc.add_kb(new_kb)?;
    println!("Created: {}", kb.id);
    Ok(())
}

pub fn handle_get<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    params: GetParams,
) -> Result<(), Error> {
    let kb = match (params.key, params.id) {
        (Some(k), _) => svc.get_kb_by_key(&k)?,
        (_, Some(i)) => svc.get_kb_by_id(&i)?,
        _ => {
            eprintln!("error: provide --key or --id");
            return Err(Error::GetKBError("no lookup key provided".to_string()));
        }
    };

    match kb {
        Some(kb) => print!("{kb}"),
        None => println!("Not found."),
    }
    Ok(())
}

pub fn handle_list<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    params: ListParams,
) -> Result<(), Error> {
    let filter = KbFilter {
        category: params.category,
        namespace: params.namespace,
        tags: if params.tags.is_empty() {
            None
        } else {
            Some(params.tags)
        },
        limit: Some(params.limit),
        offset: Some(params.offset),
        ..Default::default()
    };
    let items = svc.list_kbs(filter)?;
    if items.is_empty() {
        println!("No entries found.");
        return Ok(());
    }
    print_table_header();
    for item in items {
        println!(
            "{:<id$}  {:<key$}  {:<cat$}  {:<ns$}  {:<tags$}",
            item.id,
            item.key,
            item.category,
            item.namespace,
            item.tags.join(", "),
            id = COL_ID,
            key = COL_KEY,
            cat = COL_CAT,
            ns = COL_NS,
            tags = COL_TAGS,
        );
    }
    Ok(())
}

pub fn handle_update<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    update: KbUpdate,
) -> Result<(), Error> {
    let id = update.id.clone();
    svc.update_kb(update)?;
    println!("Updated: {}", id);
    Ok(())
}

pub fn handle_delete<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    id: String,
) -> Result<(), Error> {
    svc.delete_kb(&id)?;
    println!("Deleted: {}", id);
    Ok(())
}

pub fn handle_search<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    keyword: String,
) -> Result<(), Error> {
    let items = svc.search_kbs(&keyword)?;
    if items.is_empty() {
        println!("No results for '{}'.", keyword);
        return Ok(());
    }
    print_table_header();
    for item in items {
        println!(
            "{:<id$}  {:<key$}  {:<cat$}  {:<ns$}  {:<tags$}",
            item.id,
            item.key,
            item.category,
            item.namespace,
            item.tags.join(", "),
            id = COL_ID,
            key = COL_KEY,
            cat = COL_CAT,
            ns = COL_NS,
            tags = COL_TAGS,
        );
    }
    Ok(())
}

pub fn handle_ask<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    query: SemanticQuery,
) -> Result<(), Error> {
    let results = svc.ask(&query)?;
    if results.is_empty() {
        println!("No semantic matches found.");
        return Ok(());
    }
    print_scored_table_header();
    for scored in results {
        print_scored_row(&scored);
    }
    Ok(())
}

pub fn handle_reindex<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
) -> Result<(), Error> {
    let result = svc.reindex()?;
    println!("Reindexing {} entries...", result.total());
    for (key, _id) in &result.succeeded {
        println!("  [OK] {}", key);
    }
    for (key_or_id, err) in &result.failed {
        eprintln!("  [FAIL] {}: {}", key_or_id, err);
    }
    println!(
        "Done: {} indexed, {} failed.",
        result.succeeded.len(),
        result.failed.len()
    );
    Ok(())
}

pub fn handle_import<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    params: ImportParams,
) -> Result<(), Error> {
    let start = std::time::Instant::now();

    let content =
        std::fs::read_to_string(&params.file).map_err(|e| Error::ImportFileError(e.to_string()))?;

    let items: Vec<ImportKbItem> = serde_yaml::Deserializer::from_str(&content)
        .map(|doc| {
            <ImportKbItem as Deserialize>::deserialize(doc)
                .map_err(|e: serde_yaml::Error| Error::ParseImportFileError(e.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let result = svc.import_kbs(items);
    let saved_count = result.saved.len();
    let failed_count = result.failed.len();

    if !result.failed.is_empty() {
        let failed_items: Vec<ImportKbItem> = result.failed.into_iter().map(|f| f.item).collect();
        match serialize_failed_items(&failed_items) {
            Ok(content) => {
                if let Err(e) = write_failed_items(&params.failed_items_file, &content) {
                    eprintln!("Warning: could not write failed items file: {}", e);
                }
            }
            Err(e) => eprintln!("Warning: could not serialize failed items: {}", e),
        }
    }

    let elapsed = start.elapsed();
    println!("Import complete.");
    println!("  Imported : {}", saved_count);
    println!("  Failed   : {}", failed_count);
    println!("  Duration : {:.2?}", elapsed);
    if failed_count > 0 {
        println!("  Failed items: {}", params.failed_items_file);
    }

    Ok(())
}

fn serialize_failed_items(items: &[ImportKbItem]) -> Result<String, Error> {
    items
        .iter()
        .map(|item| {
            serde_yaml::to_string(item).map_err(|e| Error::WriteFailedItemsError(e.to_string()))
        })
        .collect::<Result<Vec<String>, _>>()
        .map(|docs| docs.join("---\n"))
}

fn write_failed_items(path: &str, content: &str) -> Result<(), Error> {
    std::fs::write(path, content).map_err(|e| Error::WriteFailedItemsError(e.to_string()))
}

// ---------------------------------------------------------------------------
// Scored-result table helpers
// ---------------------------------------------------------------------------

fn print_scored_table_header() {
    println!(
        "{:<score$}  {:<id$}  {:<key$}  {:<cat$}  {:<ns$}  {:<tags$}",
        "SCORE",
        "ID",
        "KEY",
        "CATEGORY",
        "NAMESPACE",
        "TAGS",
        score = COL_SCORE,
        id = COL_ID,
        key = COL_KEY,
        cat = COL_CAT,
        ns = COL_NS,
        tags = COL_TAGS,
    );
    println!(
        "{}",
        "-".repeat(COL_SCORE + COL_ID + COL_KEY + COL_CAT + COL_NS + COL_TAGS + 10)
    );
}

fn print_scored_row(scored: &ScoredKbItem) {
    println!(
        "{:<score$}  {:<id$}  {:<key$}  {:<cat$}  {:<ns$}  {:<tags$}",
        format!("{:.4}", scored.score),
        scored.item.id,
        scored.item.key,
        scored.item.category,
        scored.item.namespace,
        scored.item.tags.join(", "),
        score = COL_SCORE,
        id = COL_ID,
        key = COL_KEY,
        cat = COL_CAT,
        ns = COL_NS,
        tags = COL_TAGS,
    );
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::SemanticDeps;
    use std::cell::RefCell;
    use std::collections::HashMap;

    // ---- Mock implementations ----

    #[derive(Debug, Clone)]
    struct MockKbStore {
        data: RefCell<HashMap<String, crate::domain::Kb>>,
    }

    impl MockKbStore {
        fn new() -> Self {
            Self {
                data: RefCell::new(HashMap::new()),
            }
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

        fn list_kbs(
            &self,
            _filter: &crate::domain::KbFilter,
        ) -> Result<Vec<crate::domain::KbItem>, Error> {
            Ok(Vec::new())
        }

        fn search_kbs(
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

    fn make_svc() -> KBService<MockKbStore, MockVectorStore, MockEmbeddingProvider> {
        KBService::new(
            MockKbStore::new(),
            SemanticDeps {
                vector_store: MockVectorStore,
                embedder: MockEmbeddingProvider,
            },
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

    #[test]
    fn handle_import_returns_file_error_on_missing_file() {
        let svc = make_svc();
        let params = ImportParams {
            file: "/no/such/file.yaml".to_string(),
            failed_items_file: "/tmp/failed.yaml".to_string(),
        };
        let result = handle_import(&svc, params);
        assert!(matches!(result, Err(Error::ImportFileError(_))));
    }

    #[test]
    fn handle_import_returns_parse_error_on_malformed_yaml() {
        let path = write_temp_yaml("malformed_test.yaml", "Key: [\nbad yaml{{{");
        let svc = make_svc();
        let params = ImportParams {
            file: path.clone(),
            failed_items_file: "/tmp/failed_malformed.yaml".to_string(),
        };
        let result = handle_import(&svc, params);
        let _ = std::fs::remove_file(&path);
        assert!(matches!(result, Err(Error::ParseImportFileError(_))));
    }

    #[test]
    fn handle_import_succeeds_for_valid_file() {
        let yaml = "Key: rust-ownership\nValue: memory management\n";
        let path = write_temp_yaml("valid_import_test.yaml", yaml);
        let svc = make_svc();
        let params = ImportParams {
            file: path.clone(),
            failed_items_file: "/tmp/failed_valid.yaml".to_string(),
        };
        let result = handle_import(&svc, params);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn handle_import_writes_failed_items_in_yaml_format() {
        let yaml = "Key: \nValue: memory management\n";
        let path = write_temp_yaml("failed_import_test.yaml", yaml);
        let failed_path = std::env::temp_dir()
            .join("failed_items_test.yaml")
            .to_string_lossy()
            .to_string();
        let svc = make_svc();
        let params = ImportParams {
            file: path.clone(),
            failed_items_file: failed_path.clone(),
        };
        let result = handle_import(&svc, params);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_ok());
        let failed_content = std::fs::read_to_string(&failed_path).unwrap_or_default();
        let _ = std::fs::remove_file(&failed_path);
        assert!(failed_content.contains("Value") || failed_content.contains("memory"));
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
            },
            ImportKbItem {
                key: "key-two".to_string(),
                value: "value two".to_string(),
                notes: String::new(),
                category: String::new(),
                reference: String::new(),
                namespace: String::new(),
                tags: Vec::new(),
            },
        ];
        let result = serialize_failed_items(&items).unwrap();
        assert!(result.contains("key-one"));
        assert!(result.contains("key-two"));
        assert!(result.contains("---"));
    }
}
