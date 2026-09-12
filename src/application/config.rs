use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the embedding provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Which provider to use. Supported values: `"fastembed"` (default).
    #[serde(default = "default_provider")]
    pub provider: String,
}

fn default_provider() -> String {
    "fastembed".to_string()
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
        }
    }
}

/// Configuration loaded from `~/kbzona/config.yaml`
/// (or `$KBZONA_HOME/config.yaml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Path to the SQLite database file. Tilde is expanded.
    pub db_path: String,
    /// Embedding provider configuration. Defaults to fastembed (local).
    #[serde(default)]
    pub embedding: EmbeddingConfig,
}

impl Config {
    /// Returns the config file path, honouring `$KBZONA_HOME`.
    pub fn config_file_path() -> PathBuf {
        let kbzona_home = std::env::var("KBZONA_HOME").ok();
        let base = resolve_base_dir(kbzona_home.as_deref(), "kbzona");
        PathBuf::from(base).join("config.yaml")
    }

    /// Loads config from disk, creating a default file if absent.
    // Tech debt: serde_yaml is archived/deprecated upstream (frozen at 0.9.34
    // since March 2024) but kept intentionally — its output format underpins
    // both this config file and the documented `kb export`/`kb import` CLI
    // contract, so swapping the YAML engine risks a subtle format drift.
    // Revisit only if a maintained fork proves format-compatible.
    pub fn load() -> Result<Self, String> {
        let path = Self::config_file_path();

        if !path.exists() {
            let cfg = Self::default_config()?;
            cfg.save(&path)?;
            return Ok(cfg);
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read config at {}: {}", path.display(), e))?;

        let cfg: Config =
            serde_yaml::from_str(&content).map_err(|e| format!("invalid config YAML: {}", e))?;

        Ok(cfg)
    }

    /// Validates that the parent directory of `db_path` exists and is writable.
    pub fn validate(&self) -> Result<(), String> {
        let expanded = expand_tilde(&self.db_path);
        let path = PathBuf::from(&expanded);
        let parent = path
            .parent()
            .ok_or_else(|| format!("invalid db_path: {}", expanded))?;

        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create db directory {}: {}", parent.display(), e))?;
        }

        // Quick write-access check: try opening the parent dir.
        let meta = std::fs::metadata(parent)
            .map_err(|e| format!("cannot stat db directory {}: {}", parent.display(), e))?;

        if meta.permissions().readonly() {
            return Err(format!("db directory {} is read-only", parent.display()));
        }

        Ok(())
    }

    /// Returns the expanded (tilde-resolved) `db_path`.
    pub fn resolved_db_path(&self) -> String {
        expand_tilde(&self.db_path)
    }

    /// Returns the directory used to cache embedding model files.
    /// Defaults to `~/.kbzona/fastembed_cache/`.
    pub fn model_cache_dir(&self) -> PathBuf {
        let expanded = expand_tilde(&self.db_path);
        PathBuf::from(expanded)
            .parent()
            .map(|p| p.join("fastembed_cache"))
            .unwrap_or_else(|| PathBuf::from("fastembed_cache"))
    }

    fn default_config() -> Result<Self, String> {
        let kbzona_home = std::env::var("KBZONA_HOME").ok();
        let base = resolve_base_dir(kbzona_home.as_deref(), ".kbzona");
        Ok(Config {
            db_path: format!("{base}/kbzona.db"),
            embedding: EmbeddingConfig::default(),
        })
    }

    fn save(&self, path: &PathBuf) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create config dir: {}", e))?;
        }
        let yaml =
            serde_yaml::to_string(self).map_err(|e| format!("cannot serialise config: {}", e))?;
        std::fs::write(path, yaml)
            .map_err(|e| format!("cannot write config to {}: {}", path.display(), e))?;
        Ok(())
    }
}

/// Resolves `~` to the current user's home directory.
pub fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") || path == "~" {
        let home = dirs_next();
        path.replacen('~', &home, 1)
    } else {
        path.to_string()
    }
}

/// Returns the home directory as a string, falling back to `/tmp`.
fn dirs_next() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
}

/// Resolves the base directory for either the config file or the DB. When
/// `kbzona_home` is set (i.e. `$KBZONA_HOME`), it IS the base directory —
/// used as-is, with no `default_suffix` appended — so everything a fresh
/// config points at (config file and, by extension, `db_path`) is rooted
/// under the same override. Otherwise falls back to `$HOME/<default_suffix>`.
fn resolve_base_dir(kbzona_home: Option<&str>, default_suffix: &str) -> String {
    match kbzona_home {
        Some(dir) => dir.to_string(),
        None => format!("{}/{default_suffix}", dirs_next()),
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
