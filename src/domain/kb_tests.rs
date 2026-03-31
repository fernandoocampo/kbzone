use super::*;

fn make_kb(value: &str) -> Kb {
    Kb {
        id: "id-1".to_string(),
        key: "k8s-pods".to_string(),
        value: value.to_string(),
        notes: String::new(),
        category: "command".to_string(),
        reference: String::new(),
        namespace: "k8s".to_string(),
        tags: vec!["kubernetes".to_string(), "pods".to_string()],
        created_on: "2026-01-01T00:00:00+0000".to_string(),
        parent: None,
        path: None,
    }
}

#[test]
fn embedding_text_contains_key_category_namespace_tags_and_value() {
    let kb = make_kb("kubectl get pods");
    let text = kb.embedding_text();
    assert!(text.contains("k8s-pods"));
    assert!(text.contains("command"));
    assert!(text.contains("k8s"));
    assert!(text.contains("kubernetes"));
    assert!(text.contains("kubectl get pods"));
}

#[test]
fn embedding_text_contains_reference() {
    let mut kb = make_kb("kubectl get pods");
    kb.reference = "test-ref".to_string();
    let text = kb.embedding_text();
    assert!(text.contains("test-ref"));
}

#[test]
fn embedding_text_truncates_value_at_200_chars() {
    let long_value = "x".repeat(300);
    let kb = make_kb(&long_value);
    let text = kb.embedding_text();
    let value_part: String = text
        .split_whitespace()
        .last()
        .unwrap_or("")
        .chars()
        .collect();
    assert!(value_part.len() <= 200);
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
    }
}

#[test]
fn import_kb_item_valid_passes_validation() {
    let item = make_import_item("rust-ownership", "memory management");
    assert!(item.validate().is_none());
}

#[test]
fn import_kb_item_empty_key_fails_validation() {
    let item = make_import_item("", "some value");
    let result = item.validate();
    assert!(result.is_some());
    assert!(result.unwrap().contains("Key"));
}

#[test]
fn import_kb_item_blank_key_fails_validation() {
    let item = make_import_item("   ", "some value");
    let result = item.validate();
    assert!(result.is_some());
    assert!(result.unwrap().contains("Key"));
}

#[test]
fn import_kb_item_empty_value_fails_validation() {
    let item = make_import_item("rust-ownership", "");
    let result = item.validate();
    assert!(result.is_some());
    assert!(result.unwrap().contains("Value"));
}

#[test]
fn import_kb_item_converts_to_new_kb() {
    let item = make_import_item("rust-ownership", "memory management");
    let new_kb = NewKb::from(item);
    assert_eq!(new_kb.key, "rust-ownership");
    assert_eq!(new_kb.value, "memory management");
}

#[test]
fn import_kb_item_deserializes_from_yaml() {
    let yaml = "Key: rust-ownership\nValue: memory management\nCategory: concept\n";
    let item: ImportKbItem = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(item.key, "rust-ownership");
    assert_eq!(item.value, "memory management");
}

#[test]
fn import_kb_item_serializes_to_yaml() {
    let item = make_import_item("rust-ownership", "memory management");
    let yaml = serde_yaml::to_string(&item).unwrap();
    assert!(yaml.contains("rust-ownership"));
    assert!(yaml.contains("memory management"));
}

#[test]
fn import_kb_item_missing_key_fails_deserialization() {
    let yaml = "Value: memory management\n";
    let result: Result<ImportKbItem, _> = serde_yaml::from_str(yaml);
    assert!(result.is_err());
}

#[test]
fn reindex_result_total_counts_both_outcomes() {
    let result = ReindexResult {
        succeeded: vec![
            ("key-a".to_string(), "id-1".to_string()),
            ("key-b".to_string(), "id-2".to_string()),
        ],
        failed: vec![("id-3".to_string(), "not found".to_string())],
    };
    assert_eq!(result.total(), 3);
}

#[test]
fn import_kb_item_missing_value_fails_deserialization() {
    let yaml = "Key: rust-ownership\n";
    let result: Result<ImportKbItem, _> = serde_yaml::from_str(yaml);
    assert!(result.is_err());
}

#[test]
fn import_kb_item_optional_fields_default_when_absent() {
    let yaml = "Key: rust-ownership\nValue: memory management\n";
    let item: ImportKbItem = serde_yaml::from_str(yaml).unwrap();
    assert!(item.notes.is_empty());
    assert!(item.category.is_empty());
    assert!(item.namespace.is_empty());
    assert!(item.tags.is_empty());
}

// ---------------------------------------------------------------------------
// normalize_path tests
// ---------------------------------------------------------------------------

#[test]
fn normalize_path_simple_path_is_valid() {
    let result = normalize_path("/personal");
    assert_eq!(result.unwrap(), "/personal");
}

#[test]
fn normalize_path_auto_prepends_slash() {
    let result = normalize_path("personal");
    assert_eq!(result.unwrap(), "/personal");
}

#[test]
fn normalize_path_nested_path_is_valid() {
    let result = normalize_path("/personal/cars/engines");
    assert_eq!(result.unwrap(), "/personal/cars/engines");
}

#[test]
fn normalize_path_auto_prepends_slash_for_nested() {
    let result = normalize_path("personal/rust");
    assert_eq!(result.unwrap(), "/personal/rust");
}

#[test]
fn normalize_path_rejects_dotdot() {
    let result = normalize_path("/a/../b");
    assert!(matches!(
        result,
        Err(crate::errors::Error::InvalidPathError(_))
    ));
}

#[test]
fn normalize_path_rejects_dot_component() {
    let result = normalize_path("/a/./b");
    assert!(matches!(
        result,
        Err(crate::errors::Error::InvalidPathError(_))
    ));
}

#[test]
fn normalize_path_rejects_double_slash() {
    let result = normalize_path("/personal//cars");
    assert!(matches!(
        result,
        Err(crate::errors::Error::InvalidPathError(_))
    ));
}

#[test]
fn normalize_path_rejects_null_byte() {
    let result = normalize_path("/personal/\0cars");
    assert!(matches!(
        result,
        Err(crate::errors::Error::InvalidPathError(_))
    ));
}

#[test]
fn import_kb_item_with_invalid_path_fails_validation() {
    let mut item = make_import_item("rust-ownership", "memory management");
    item.path = Some("/a/../b".to_string());
    let result = item.validate();
    assert!(result.is_some());
    assert!(result.unwrap().contains("invalid path"));
}

#[test]
fn import_kb_item_with_valid_path_passes_validation() {
    let mut item = make_import_item("rust-ownership", "memory management");
    item.path = Some("/personal/rust".to_string());
    let result = item.validate();
    assert!(result.is_none());
}

#[test]
fn import_kb_item_with_path_without_slash_passes_validation() {
    let mut item = make_import_item("rust-ownership", "memory management");
    item.path = Some("personal/rust".to_string());
    let result = item.validate();
    assert!(result.is_none());
}
