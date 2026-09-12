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
