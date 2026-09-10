use super::*;
use crate::service::ServiceDeps;
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

    fn get_kbs(
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

    fn random_quote(&self) -> Result<crate::domain::Kb, Error> {
        self.data
            .borrow()
            .values()
            .find(|kb| kb.category.to_lowercase() == "quote")
            .cloned()
            .ok_or(Error::QuoteNotFound)
    }

    fn get_children_ids(&self, _parent_id: &str) -> Result<Vec<String>, Error> {
        Ok(vec![])
    }

    fn get_kbs_full(
        &self,
        _filter: &crate::domain::KbFilter,
    ) -> Result<Vec<crate::domain::Kb>, Error> {
        Ok(Vec::new())
    }

    fn get_categories(&self, _namespace: Option<&str>) -> Result<Vec<String>, Error> {
        Ok(Vec::new())
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

#[derive(Debug, Clone)]
struct MockMediaStore;

impl crate::ports::MediaStore for MockMediaStore {
    fn store_media(&self, params: &crate::domain::StoreMediaParams) -> Result<String, Error> {
        Ok(params.destination.clone())
    }

    fn delete_media(&self, _path: &str) -> Result<(), Error> {
        Ok(())
    }

    fn copy_dir(&self, _source: &str, _destination: &str) -> Result<u64, Error> {
        Ok(0)
    }
}

#[derive(Debug, Clone)]
struct MockMediaFetcher;

impl crate::ports::MediaFetcher for MockMediaFetcher {
    fn fetch(&self, url: &str) -> Result<String, Error> {
        Ok(format!("/tmp/mock-{}", url))
    }
}

fn make_svc()
-> KBService<MockKbStore, MockVectorStore, MockEmbeddingProvider, MockMediaStore, MockMediaFetcher>
{
    KBService::new(
        MockKbStore::new(),
        ServiceDeps {
            vector_store: MockVectorStore,
            embedder: MockEmbeddingProvider,
            media_store: MockMediaStore,
            media_fetcher: MockMediaFetcher,
            base_dir: String::new(),
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
fn handle_version_returns_ok() {
    let result = handle_version();
    assert!(result.is_ok());
}

// handle_add always calls confirm_or_adjust which requires stdin input.
// Tests cannot easily mock stdin, so we verify the builder path directly.
// The full handle_add pipeline (including confirm) is covered by manual testing.
#[test]
fn build_new_kb_non_interactive_all_fields_provided_no_prompt() {
    let params = AddParams {
        key: Some("test-key".to_string()),
        value: Some("test-value".to_string()),
        notes: "notes".to_string(),
        category: "concept".to_string(),
        reference: "the book".to_string(),
        namespace: "rust".to_string(),
        tags: vec!["rust".to_string()],
        interactive: false,
        parent: None,
        path: None,
        media_url: None,
    };
    let result = build_new_kb_non_interactive(params);
    assert!(result.is_ok());
    let kb = result.expect("expected Ok NewKb");
    assert_eq!(kb.key, "test-key");
    assert_eq!(kb.value, "test-value");
    assert_eq!(kb.reference, "the book");
    assert_eq!(kb.tags, vec!["rust".to_string()]);
}

#[test]
fn handle_add_non_interactive_fails_without_key() {
    let svc = make_svc();
    let params = AddParams {
        key: None,
        value: Some("test-value".to_string()),
        notes: String::new(),
        category: String::new(),
        reference: String::new(),
        namespace: String::new(),
        tags: Vec::new(),
        interactive: false,
        parent: None,
        path: None,
        media_url: None,
    };
    let result = handle_add(&svc, params);
    assert!(matches!(result, Err(Error::MissingRequiredField(_))));
}

#[test]
fn handle_add_non_interactive_fails_without_value() {
    let svc = make_svc();
    let params = AddParams {
        key: Some("test-key".to_string()),
        value: None,
        notes: String::new(),
        category: String::new(),
        reference: String::new(),
        namespace: String::new(),
        tags: Vec::new(),
        interactive: false,
        parent: None,
        path: None,
        media_url: None,
    };
    let result = handle_add(&svc, params);
    assert!(matches!(result, Err(Error::MissingRequiredField(_))));
}

#[test]
fn build_new_kb_non_interactive_builds_correctly() {
    let params = AddParams {
        key: Some("my-key".to_string()),
        value: Some("my-value".to_string()),
        notes: "some notes".to_string(),
        category: "concept".to_string(),
        reference: "some-reference".to_string(),
        namespace: "default".to_string(),
        tags: vec!["rust".to_string()],
        interactive: false,
        parent: None,
        path: None,
        media_url: None,
    };
    let result = build_new_kb_non_interactive(params);
    assert!(result.is_ok());
    let new_kb = result.expect("expected Ok NewKb");
    assert_eq!(new_kb.key, "my-key");
    assert_eq!(new_kb.value, "my-value");
    assert_eq!(new_kb.tags, vec!["rust".to_string()]);
}

// ---------------------------------------------------------------------------
// parse_tags tests
// ---------------------------------------------------------------------------

#[test]
fn parse_tags_splits_by_comma() {
    let result = parse_tags("rust,memory,concepts");
    assert_eq!(result, vec!["rust", "memory", "concepts"]);
}

#[test]
fn parse_tags_trims_whitespace() {
    let result = parse_tags("rust , memory , concepts");
    assert_eq!(result, vec!["rust", "memory", "concepts"]);
}

#[test]
fn parse_tags_filters_empty_strings() {
    let result = parse_tags("rust,,memory,");
    assert_eq!(result, vec!["rust", "memory"]);
}

#[test]
fn parse_tags_returns_empty_for_empty_input() {
    let result = parse_tags("");
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// format_preview tests
// ---------------------------------------------------------------------------

#[test]
fn format_preview_includes_all_fields() {
    use crate::domain::NewKb;
    let kb = NewKb {
        key: "rust-ownership".to_string(),
        value: "memory management in rust".to_string(),
        notes: "important concept".to_string(),
        category: "concept".to_string(),
        namespace: "rust".to_string(),
        reference: "the book".to_string(),
        tags: vec!["rust".to_string(), "memory".to_string()],
        parent: None,
        path: None,
        media_url: None,
        media_extension: None,
    };
    let preview = format_preview(&kb);
    assert!(preview.contains("rust-ownership"));
    assert!(preview.contains("memory management in rust"));
    assert!(preview.contains("important concept"));
    assert!(preview.contains("concept"));
    assert!(preview.contains("rust"));
    assert!(preview.contains("the book"));
}

#[test]
fn format_preview_shows_tags_joined_with_comma() {
    use crate::domain::NewKb;
    let kb = NewKb {
        key: "k".to_string(),
        value: "v".to_string(),
        notes: String::new(),
        category: String::new(),
        namespace: String::new(),
        reference: String::new(),
        tags: vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
        parent: None,
        path: None,
        media_url: None,
        media_extension: None,
    };
    let preview = format_preview(&kb);
    assert!(preview.contains("alpha, beta, gamma"));
}

// ---------------------------------------------------------------------------
// build_new_kb_non_interactive: provided reference and tags (no prompt path)
// ---------------------------------------------------------------------------

#[test]
fn build_new_kb_non_interactive_uses_provided_reference() {
    let params = AddParams {
        key: Some("ref-key".to_string()),
        value: Some("ref-value".to_string()),
        notes: String::new(),
        category: String::new(),
        reference: "some book".to_string(),
        namespace: String::new(),
        tags: vec!["tag1".to_string()],
        interactive: false,
        parent: None,
        path: None,
        media_url: None,
    };
    let result = build_new_kb_non_interactive(params);
    assert!(result.is_ok());
    let kb = result.expect("expected Ok");
    assert_eq!(kb.reference, "some book");
}

#[test]
fn build_new_kb_non_interactive_uses_provided_tags() {
    let params = AddParams {
        key: Some("tags-key".to_string()),
        value: Some("tags-value".to_string()),
        notes: String::new(),
        category: String::new(),
        reference: "ref".to_string(),
        namespace: String::new(),
        tags: vec!["rust".to_string(), "memory".to_string()],
        interactive: false,
        parent: None,
        path: None,
        media_url: None,
    };
    let result = build_new_kb_non_interactive(params);
    assert!(result.is_ok());
    let kb = result.expect("expected Ok");
    assert_eq!(kb.tags, vec!["rust".to_string(), "memory".to_string()]);
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
            parent_key: None,
            path: None,
            media_extension: None,
        },
        ImportKbItem {
            key: "key-two".to_string(),
            value: "value two".to_string(),
            notes: String::new(),
            category: String::new(),
            reference: String::new(),
            namespace: String::new(),
            tags: Vec::new(),
            parent_key: None,
            path: None,
            media_extension: None,
        },
    ];
    let result = serialize_failed_items(&items).unwrap();
    assert!(result.contains("key-one"));
    assert!(result.contains("key-two"));
    assert!(result.contains("---"));
}

// ---------------------------------------------------------------------------
// Graph render helpers
// ---------------------------------------------------------------------------

use crate::domain::{GraphNode, IncomingEdge, OutgoingEdge, TreeRoot};

fn make_graph_node(key: &str, category: &str) -> GraphNode {
    GraphNode {
        id: format!("{key}-id"),
        key: key.to_string(),
        category: category.to_string(),
        namespace: "default".to_string(),
    }
}

#[test]
fn truncate_note_leaves_short_notes_unchanged() {
    assert_eq!(truncate_note("short note"), "short note");
}

#[test]
fn truncate_note_truncates_long_notes_to_40_chars_with_ellipsis() {
    let long = "x".repeat(100);
    let truncated = truncate_note(&long);
    assert_eq!(truncated.chars().count(), 40);
    assert!(truncated.ends_with('…'));
}

#[test]
fn truncate_note_is_char_safe_with_multibyte_input() {
    let multibyte = "é".repeat(100);
    let truncated = truncate_note(&multibyte);
    assert_eq!(truncated.chars().count(), 40);
    assert!(truncated.ends_with('…'));
}

#[test]
fn render_related_shows_both_sections_by_default() {
    let result = RelatedResult {
        node: make_graph_node("car", "concept"),
        outgoing: vec![OutgoingEdge {
            edge_id: "e1".to_string(),
            note: "has an engine".to_string(),
            created_on: "2026-01-01T00:00:00+0000".to_string(),
            to: make_graph_node("engine", "concept"),
        }],
        incoming: vec![IncomingEdge {
            edge_id: "e2".to_string(),
            note: "compatible with car".to_string(),
            created_on: "2026-01-01T00:00:00+0000".to_string(),
            from: make_graph_node("spare-parts-kit", "bookmark"),
        }],
    };
    let output = render_related(&result, EdgeDirection::Both);
    assert!(output.contains("→ car points to (1)"));
    assert!(output.contains("engine"));
    assert!(output.contains("← pointed to by car (1)"));
    assert!(output.contains("spare-parts-kit"));
}

#[test]
fn render_related_direction_out_hides_incoming_section() {
    let result = RelatedResult {
        node: make_graph_node("car", "concept"),
        outgoing: vec![],
        incoming: vec![IncomingEdge {
            edge_id: "e2".to_string(),
            note: "n".to_string(),
            created_on: "2026-01-01T00:00:00+0000".to_string(),
            from: make_graph_node("spare-parts-kit", "bookmark"),
        }],
    };
    let output = render_related(&result, EdgeDirection::Out);
    assert!(output.contains("→ car points to (0)"));
    assert!(!output.contains("pointed to by"));
}

#[test]
fn render_related_direction_in_hides_outgoing_section() {
    let result = RelatedResult {
        node: make_graph_node("car", "concept"),
        outgoing: vec![OutgoingEdge {
            edge_id: "e1".to_string(),
            note: "n".to_string(),
            created_on: "2026-01-01T00:00:00+0000".to_string(),
            to: make_graph_node("engine", "concept"),
        }],
        incoming: vec![],
    };
    let output = render_related(&result, EdgeDirection::In);
    assert!(!output.contains("points to"));
    assert!(output.contains("← pointed to by car (0)"));
}

#[test]
fn render_tree_draws_branches_for_a_multi_level_hierarchy() {
    let result = TreeResult {
        root: TreeRoot {
            id: "car-id".to_string(),
            key: "car".to_string(),
        },
        direction: "out".to_string(),
        nodes: vec![
            TreeNode {
                id: "engine-id".to_string(),
                key: "engine".to_string(),
                depth: 1,
                parent_id: "car-id".to_string(),
                note: "car has an engine".to_string(),
            },
            TreeNode {
                id: "chassis-id".to_string(),
                key: "chassis".to_string(),
                depth: 1,
                parent_id: "car-id".to_string(),
                note: "car is built on a chassis".to_string(),
            },
            TreeNode {
                id: "camshaft-id".to_string(),
                key: "camshaft".to_string(),
                depth: 2,
                parent_id: "engine-id".to_string(),
                note: "engine contains camshaft".to_string(),
            },
            TreeNode {
                id: "piston-id".to_string(),
                key: "piston".to_string(),
                depth: 2,
                parent_id: "engine-id".to_string(),
                note: "engine contains piston".to_string(),
            },
        ],
    };
    let output = render_tree(&result);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "car");
    assert!(lines[1].starts_with("├── engine"));
    assert!(lines[1].contains("car has an engine"));
    assert!(lines[2].starts_with("│   ├── camshaft"));
    assert!(lines[2].contains("engine contains camshaft"));
    assert!(lines[3].starts_with("│   └── piston"));
    assert!(lines[4].starts_with("└── chassis"));
    assert!(lines[4].contains("car is built on a chassis"));
}
