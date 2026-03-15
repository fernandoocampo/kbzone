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
