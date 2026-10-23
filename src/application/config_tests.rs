use super::*;

// `resolve_base_dir` is the shared home-resolution logic behind both
// `Config::config_file_path` and `Config::default_config` — when
// `$KBZONA_HOME` is set it must be used as-is (no default suffix appended),
// so a freshly created config's `db_path` lands under the same root as the
// config file itself, not back under the real `$HOME`.

#[test]
fn resolve_base_dir_uses_kbzona_home_as_is_when_set() {
    let base = resolve_base_dir(Some("/scratch/kbzona"), ".kbzona");
    assert_eq!(base, "/scratch/kbzona");
}

#[test]
fn resolve_base_dir_ignores_default_suffix_when_kbzona_home_set() {
    // The whole point of the override: no ".kbzona"/"kbzona" suffix is
    // appended on top of $KBZONA_HOME, regardless of which caller asks.
    let for_config = resolve_base_dir(Some("/scratch/kbzona"), "kbzona");
    let for_db = resolve_base_dir(Some("/scratch/kbzona"), ".kbzona");
    assert_eq!(for_config, for_db);
}

#[test]
fn resolve_base_dir_falls_back_to_home_plus_suffix_when_unset() {
    let base = resolve_base_dir(None, ".kbzona");
    assert_eq!(base, format!("{}/.kbzona", dirs_next()));
}

#[test]
fn default_config_db_path_is_rooted_under_kbzona_home_when_set() {
    let base = resolve_base_dir(Some("/scratch/kbzona"), ".kbzona");
    let db_path = format!("{base}/kbzona.db");
    assert_eq!(db_path, "/scratch/kbzona/kbzona.db");
}

// ---- embedding model selection ----

fn parse(yaml: &str) -> Config {
    serde_yaml::from_str(yaml).expect("valid config yaml")
}

#[test]
fn legacy_config_without_embedding_section_uses_default_model() {
    let cfg = parse("db_path: ~/.kbzona/kbzona.db\n");
    assert_eq!(cfg.embedding.model, "bge-small-en-v1.5");
    assert!(cfg.index_is_current());
}

#[test]
fn legacy_config_with_only_provider_uses_default_model_and_is_current() {
    let cfg = parse("db_path: x.db\nembedding:\n  provider: fastembed\n");
    assert_eq!(cfg.embedding.model, "bge-small-en-v1.5");
    assert_eq!(cfg.embedding.indexed_model, None);
    assert!(cfg.index_is_current());
}

#[test]
fn model_without_indexed_model_is_stale_unless_it_is_the_default() {
    // A legacy DB can only have been built with the default model.
    let cfg = parse("db_path: x.db\nembedding:\n  model: bge-base-en-v1.5\n");
    assert!(!cfg.index_is_current());
    assert_eq!(cfg.indexed_model_name(), "bge-small-en-v1.5");
}

#[test]
fn index_is_current_when_model_equals_indexed_model() {
    let cfg = parse(
        "db_path: x.db\nembedding:\n  model: bge-base-en-v1.5\n  indexed_model: bge-base-en-v1.5\n",
    );
    assert!(cfg.index_is_current());
}

#[test]
fn index_is_stale_when_model_differs_from_indexed_model() {
    let cfg = parse(
        "db_path: x.db\nembedding:\n  model: nomic-embed-text-v1.5\n  indexed_model: bge-small-en-v1.5\n",
    );
    assert!(!cfg.index_is_current());
}

#[test]
fn index_comparison_ignores_case_and_whitespace() {
    let cfg = parse(
        "db_path: x.db\nembedding:\n  model: BGE-Base-EN-v1.5\n  indexed_model: bge-base-en-v1.5\n",
    );
    assert!(cfg.index_is_current());
}

#[test]
fn mark_indexed_makes_a_stale_index_current() {
    let mut cfg = parse(
        "db_path: x.db\nembedding:\n  model: nomic-embed-text-v1.5\n  indexed_model: bge-small-en-v1.5\n",
    );
    cfg.mark_indexed();
    assert_eq!(
        cfg.embedding.indexed_model.as_deref(),
        Some("nomic-embed-text-v1.5")
    );
    assert!(cfg.index_is_current());
}

#[test]
fn default_embedding_config_names_the_model_and_marks_it_indexed() {
    let cfg = EmbeddingConfig::default();
    assert_eq!(cfg.model, "bge-small-en-v1.5");
    assert_eq!(cfg.indexed_model.as_deref(), Some("bge-small-en-v1.5"));
}

#[test]
fn serialized_default_config_lists_model_and_indexed_model() {
    let cfg = Config {
        db_path: "x.db".to_string(),
        embedding: EmbeddingConfig::default(),
    };
    let yaml = serde_yaml::to_string(&cfg).unwrap();
    assert!(yaml.contains("model: bge-small-en-v1.5"));
    assert!(yaml.contains("indexed_model: bge-small-en-v1.5"));
}
