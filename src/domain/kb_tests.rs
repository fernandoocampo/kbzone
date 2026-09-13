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
        media_extension: None,
        metadata: std::collections::BTreeMap::new(),
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
        media_extension: None,
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

#[test]
fn import_kb_item_deserializes_parent_alias_from_yaml() {
    let yaml = "Key: engine\nValue: v\nParent: car\n";
    let item: ImportKbItem = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(item.parent_key, Some("car".to_string()));
}

#[test]
fn import_kb_item_still_deserializes_parent_key_field() {
    let yaml = "Key: engine\nValue: v\nParentKey: car\n";
    let item: ImportKbItem = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(item.parent_key, Some("car".to_string()));
}

#[test]
fn import_kb_item_still_serializes_as_parent_key() {
    let mut item = make_import_item("engine", "v");
    item.parent_key = Some("car".to_string());
    let yaml = serde_yaml::to_string(&item).unwrap();
    assert!(yaml.contains("ParentKey: car"));
    assert!(!yaml.contains("Parent:"));
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

// ---------------------------------------------------------------------------
// media_file_path tests
// ---------------------------------------------------------------------------

#[test]
fn media_file_path_with_namespace_and_path() {
    let params = MediaPathParams {
        base_dir: "/home/user/kbzona",
        namespace: "english",
        path: Some("/idioms/funny"),
        key: "lol-cat",
        extension: Some("jpg"),
    };
    let result = media_file_path(&params);
    assert_eq!(
        result,
        "/home/user/kbzona/media/english/idioms/funny/lol-cat.jpg"
    );
}

#[test]
fn media_file_path_without_path() {
    let params = MediaPathParams {
        base_dir: "/home/user/kbzona",
        namespace: "test",
        path: None,
        key: "my-image",
        extension: Some("png"),
    };
    let result = media_file_path(&params);
    assert_eq!(result, "/home/user/kbzona/media/test/my-image.png");
}

#[test]
fn media_file_path_with_url_source() {
    let params = MediaPathParams {
        base_dir: "/home/user/kbzona",
        namespace: "docs",
        path: Some("/tutorials"),
        key: "rust-book",
        extension: Some("pdf"),
    };
    let result = media_file_path(&params);
    assert_eq!(
        result,
        "/home/user/kbzona/media/docs/tutorials/rust-book.pdf"
    );
}

#[test]
fn media_file_path_with_no_extension() {
    let params = MediaPathParams {
        base_dir: "/home/user/kbzona",
        namespace: "misc",
        path: None,
        key: "readme",
        extension: None,
    };
    let result = media_file_path(&params);
    assert_eq!(result, "/home/user/kbzona/media/misc/readme");
}

#[test]
fn media_file_path_strips_leading_slash_from_path() {
    let params = MediaPathParams {
        base_dir: "/home/user/kbzona",
        namespace: "english",
        path: Some("/idioms"),
        key: "hello",
        extension: Some("mp3"),
    };
    let result = media_file_path(&params);
    assert_eq!(result, "/home/user/kbzona/media/english/idioms/hello.mp3");
}

#[test]
fn media_file_path_with_empty_path() {
    let params = MediaPathParams {
        base_dir: "/home/user/kbzona",
        namespace: "test",
        path: Some(""),
        key: "file",
        extension: Some("txt"),
    };
    let result = media_file_path(&params);
    assert_eq!(result, "/home/user/kbzona/media/test/file.txt");
}

#[test]
fn output_format_from_str_json() {
    assert_eq!("json".parse::<OutputFormat>(), Ok(OutputFormat::Json));
}

#[test]
fn output_format_from_str_yaml() {
    assert_eq!("yaml".parse::<OutputFormat>(), Ok(OutputFormat::Yaml));
}

#[test]
fn output_format_from_str_invalid_returns_error() {
    let result = "xml".parse::<OutputFormat>();
    assert!(result.is_err());
}

fn make_add_json_input() -> AddJsonInput {
    AddJsonInput {
        key: "complexity-views".to_string(),
        value: "Fools ignore complexity.".to_string(),
        category: "quote".to_string(),
        tags: vec!["complexity".to_string(), "perlis".to_string()],
        reference: "Alan Perlis".to_string(),
        notes: String::new(),
        namespace: String::new(),
        path: None,
        parent: None,
        media_url: None,
        metadata: std::collections::BTreeMap::new(),
    }
}

#[test]
fn add_json_input_validate_fails_when_key_blank() {
    let mut input = make_add_json_input();
    input.key = "   ".to_string();
    assert!(matches!(input.validate(), Err(Error::InvalidJsonInput(_))));
}

#[test]
fn add_json_input_validate_fails_when_value_blank() {
    let mut input = make_add_json_input();
    input.value = "   ".to_string();
    assert!(matches!(input.validate(), Err(Error::InvalidJsonInput(_))));
}

#[test]
fn add_json_input_validate_fails_when_category_blank() {
    let mut input = make_add_json_input();
    input.category = "   ".to_string();
    assert!(matches!(input.validate(), Err(Error::InvalidJsonInput(_))));
}

#[test]
fn add_json_input_validate_fails_when_tags_empty() {
    let mut input = make_add_json_input();
    input.tags = Vec::new();
    assert!(matches!(input.validate(), Err(Error::InvalidJsonInput(_))));
}

#[test]
fn add_json_input_validate_passes_with_all_required_fields() {
    let input = make_add_json_input();
    assert!(input.validate().is_ok());
}

#[test]
fn dedup_tags_rejects_blank_tag() {
    let result = dedup_tags(vec!["a".to_string(), "  ".to_string()]);
    assert!(matches!(result, Err(Error::InvalidTagError(_))));
}

#[test]
fn dedup_tags_preserves_first_occurrence_order_and_dedupes() {
    let result = dedup_tags(vec![
        "b".to_string(),
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
        "a".to_string(),
    ]);
    assert_eq!(
        result.expect("expected Ok"),
        vec!["b".to_string(), "a".to_string(), "c".to_string()]
    );
}

#[test]
fn dedup_tags_trims_before_comparing() {
    let result = dedup_tags(vec![" a".to_string(), "a ".to_string()]);
    assert_eq!(result.expect("expected Ok"), vec!["a".to_string()]);
}

#[test]
fn build_metadata_rejects_blank_key() {
    let result = build_metadata(vec![("  ".to_string(), "v".to_string())]);
    assert!(matches!(result, Err(Error::InvalidMetadataError(_))));
}

#[test]
fn build_metadata_rejects_duplicate_key() {
    let result = build_metadata(vec![
        ("author".to_string(), "me".to_string()),
        ("author".to_string(), "you".to_string()),
    ]);
    assert!(matches!(result, Err(Error::DuplicateMetadataKeyError(_))));
}

#[test]
fn build_metadata_trims_keys_and_values() {
    let result =
        build_metadata(vec![(" author ".to_string(), " me ".to_string())]).expect("expected Ok");
    assert_eq!(result.get("author"), Some(&"me".to_string()));
}

#[test]
fn build_metadata_builds_ordered_map_from_pairs() {
    let result = build_metadata(vec![
        ("priority".to_string(), "high".to_string()),
        ("author".to_string(), "me".to_string()),
    ])
    .expect("expected Ok");
    assert_eq!(
        result.into_iter().collect::<Vec<_>>(),
        vec![
            ("author".to_string(), "me".to_string()),
            ("priority".to_string(), "high".to_string())
        ]
    );
}

#[test]
fn build_metadata_empty_input_returns_empty_map() {
    assert!(build_metadata(vec![]).expect("expected Ok").is_empty());
}

#[test]
fn format_metadata_joins_pairs_as_key_equals_value() {
    let mut map = std::collections::BTreeMap::new();
    map.insert("author".to_string(), "me".to_string());
    map.insert("priority".to_string(), "high".to_string());
    assert_eq!(format_metadata(&map), "author=me, priority=high");
}

#[test]
fn format_metadata_empty_map_returns_empty_string() {
    assert_eq!(format_metadata(&std::collections::BTreeMap::new()), "");
}

#[test]
fn embedding_text_ignores_metadata() {
    let mut kb1 = make_kb("value");
    kb1.metadata.insert("author".to_string(), "me".to_string());
    let kb2 = make_kb("value");
    assert_eq!(kb1.embedding_text(), kb2.embedding_text());
}

#[test]
fn display_includes_metadata_line() {
    let mut kb = make_kb("some value");
    kb.metadata.insert("author".to_string(), "me".to_string());
    let rendered = kb.to_string();
    assert!(rendered.contains("Metadata  : author=me"));
}

#[test]
fn parse_metadata_input_splits_comma_and_equals() {
    let input = "author=me,priority=high";
    let result = parse_metadata_input(input);
    assert_eq!(
        result,
        vec![
            ("author".to_string(), "me".to_string()),
            ("priority".to_string(), "high".to_string()),
        ]
    );
}

#[test]
fn parse_metadata_input_skips_tokens_without_equals() {
    let input = "author=me,invalid,priority=high";
    let result = parse_metadata_input(input);
    assert_eq!(
        result,
        vec![
            ("author".to_string(), "me".to_string()),
            ("priority".to_string(), "high".to_string()),
        ]
    );
}

#[test]
fn parse_metadata_input_trims_keys_and_values() {
    let input = "  author = me  , priority = high  ";
    let result = parse_metadata_input(input);
    assert_eq!(
        result,
        vec![
            ("author".to_string(), "me".to_string()),
            ("priority".to_string(), "high".to_string()),
        ]
    );
}

#[test]
fn parse_metadata_input_empty_string_returns_empty_vec() {
    let result = parse_metadata_input("");
    assert_eq!(result, vec![]);
}

#[test]
fn parse_metadata_input_only_invalid_tokens_returns_empty_vec() {
    let input = "invalid,also-invalid,no-equals";
    let result = parse_metadata_input(input);
    assert_eq!(result, vec![]);
}

#[test]
fn try_from_add_json_input_builds_new_kb_with_trimmed_fields() {
    let mut input = make_add_json_input();
    input.key = "  complexity-views  ".to_string();
    input.value = "  Fools ignore complexity.  ".to_string();
    input.category = "  quote  ".to_string();
    input.tags = vec!["a".to_string(), "a".to_string(), "b".to_string()];
    let new_kb = NewKb::try_from(input).expect("expected Ok NewKb");
    assert_eq!(new_kb.key, "complexity-views");
    assert_eq!(new_kb.value, "Fools ignore complexity.");
    assert_eq!(new_kb.category, "quote");
    assert_eq!(new_kb.tags, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(new_kb.media_extension, None);
}

#[test]
fn try_from_add_json_input_normalizes_path() {
    let mut input = make_add_json_input();
    input.path = Some("personal/rust".to_string());
    let new_kb = NewKb::try_from(input).expect("expected Ok NewKb");
    assert_eq!(new_kb.path, Some("/personal/rust".to_string()));
}

#[test]
fn try_from_add_json_input_rejects_invalid_path() {
    let mut input = make_add_json_input();
    input.path = Some("../evil".to_string());
    let result = NewKb::try_from(input);
    assert!(matches!(result, Err(Error::InvalidPathError(_))));
}

#[test]
fn try_from_add_json_input_defaults_optional_fields() {
    let input = AddJsonInput {
        key: "k".to_string(),
        value: "v".to_string(),
        category: "concept".to_string(),
        tags: vec!["t".to_string()],
        reference: String::new(),
        notes: String::new(),
        namespace: String::new(),
        path: None,
        parent: None,
        media_url: None,
        metadata: std::collections::BTreeMap::new(),
    };
    let new_kb = NewKb::try_from(input).expect("expected Ok NewKb");
    assert_eq!(new_kb.reference, "");
    assert_eq!(new_kb.notes, "");
    assert_eq!(new_kb.namespace, "");
    assert_eq!(new_kb.parent, None);
    assert_eq!(new_kb.path, None);
    assert_eq!(new_kb.media_url, None);
}

#[test]
fn add_json_input_validate_rejects_blank_metadata_key() {
    let mut input = make_add_json_input();
    input.metadata.insert("  ".to_string(), "x".to_string());
    assert!(matches!(input.validate(), Err(Error::InvalidJsonInput(_))));
}

#[test]
fn try_from_add_json_input_passes_metadata_through_unchanged() {
    let mut input = make_add_json_input();
    input
        .metadata
        .insert("author".to_string(), "me".to_string());
    let new_kb = NewKb::try_from(input).expect("expected Ok NewKb");
    assert_eq!(new_kb.metadata.get("author"), Some(&"me".to_string()));
}
