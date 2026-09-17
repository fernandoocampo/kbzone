use chrono::Local;
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::errors::Error;

/// Category value for media entries. Compared case-insensitively via [`is_media_category`].
pub const MEDIA_CATEGORY: &str = "media";

/// Returns `true` if the given category string (case-insensitive) identifies a media entry.
pub fn is_media_category(category: &str) -> bool {
    category.eq_ignore_ascii_case(MEDIA_CATEGORY)
}

/// Extracts the file extension from a path or URL string.
/// Returns `None` if no extension is present.
pub fn file_extension(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_string)
}

/// Full KB entity as stored in the database.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Kb {
    /// Internal UUID, auto-generated on creation.
    pub id: String,
    /// User-provided unique string identifier (e.g. `"rust-ownership"`).
    pub key: String,
    /// The content/answer of this entry.
    pub value: String,
    /// Freeform large text with extra context or elaboration.
    pub notes: String,
    /// Entry type, e.g. `quote`, `bookmark`, `command`, `concept` (user-extensible).
    pub category: String,
    /// The source: a person, company, book, investigation, magazine name.
    pub reference: String,
    /// Grouping scope (like Kubernetes namespaces); organises entries that share a context.
    pub namespace: String,
    /// Searchable keywords; drives FTS5 tag search.
    pub tags: Vec<String>,
    /// Freeform key-value metadata pairs, unique by key.
    pub metadata: BTreeMap<String, String>,
    /// ISO-8601 creation timestamp.
    pub created_on: String,
    /// Internal UUID of the parent KB item, if any.
    pub parent: Option<String>,
    /// Optional Unix-style hierarchical path (e.g. `/personal/cars/engines`).
    pub path: Option<String>,
    /// File extension of the stored media file (e.g. `"jpg"`, `"pdf"`). Only set for category `media`.
    pub media_extension: Option<String>,
}

impl Kb {
    /// Joins tags with a space for SQLite storage.
    pub fn tags_as_string(&self) -> String {
        self.tags.join(" ")
    }

    /// Builds the text used for embedding: key + category + namespace + reference + tags + first 200 chars of value.
    pub fn embedding_text(&self) -> String {
        let truncated_value: String = self.value.chars().take(200).collect();
        format!(
            "{} {} {} {} {} {}",
            self.key,
            self.category,
            self.namespace,
            self.reference,
            self.tags.join(" "),
            truncated_value
        )
    }
}

/// Formats a BTreeMap of metadata as comma-separated `key=value` pairs.
pub fn format_metadata(metadata: &BTreeMap<String, String>) -> String {
    metadata
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl std::fmt::Display for Kb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "ID        : {}", self.id)?;
        writeln!(f, "Key       : {}", self.key)?;
        writeln!(f, "Value     : {}", self.value)?;
        writeln!(f, "Category  : {}", self.category)?;
        writeln!(f, "Namespace : {}", self.namespace)?;
        writeln!(f, "Reference : {}", self.reference)?;
        writeln!(f, "Tags      : {}", self.tags.join(", "))?;
        writeln!(f, "Metadata  : {}", format_metadata(&self.metadata))?;
        writeln!(f, "Created   : {}", self.created_on)?;
        if let Some(ref p) = self.path {
            writeln!(f, "Path      : {}", p)?;
        }
        if let Some(ref p) = self.parent {
            writeln!(f, "Parent    : {}", p)?;
        }
        if !self.notes.is_empty() {
            writeln!(f, "Notes     :\n{}", self.notes)?;
        }
        Ok(())
    }
}

/// Structured output format for `kb get --out`. Absence of `--out` (plain
/// text) is represented as `None` at the call site, not as a variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Yaml,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(OutputFormat::Json),
            "yaml" => Ok(OutputFormat::Yaml),
            other => Err(format!(
                "invalid output format: {other} (expected json|yaml)"
            )),
        }
    }
}

/// Minimal JSON response for a successful delete operation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeleteConfirmation {
    pub id: String,
    pub deleted: bool,
}

/// JSON error response for a failed delete operation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeleteErrorResponse {
    pub error: String,
}

/// JSON error response for a failed `categories` operation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CategoriesErrorResponse {
    pub error: String,
}

/// JSON error response for a failed `random` operation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RandomErrorResponse {
    pub error: String,
}

/// DTO for creating a new KB entry.
#[derive(Debug, Clone)]
pub struct NewKb {
    pub key: String,
    pub value: String,
    pub notes: String,
    pub category: String,
    pub reference: String,
    pub namespace: String,
    pub tags: Vec<String>,
    /// Freeform key-value metadata pairs, unique by key.
    pub metadata: BTreeMap<String, String>,
    /// Internal UUID of the parent KB item, if any.
    pub parent: Option<String>,
    /// Optional Unix-style hierarchical path (e.g. `/personal/rust`).
    pub path: Option<String>,
    /// Original media source (URL or local file path). Required when category is `media`.
    /// This field is transient — used during `add` to download/copy the file. Not persisted.
    pub media_url: Option<String>,
    /// File extension override. Takes precedence over the extension derived from `media_url`.
    /// Used by import, where the file extension is already known and no URL is present.
    pub media_extension: Option<String>,
}

impl From<NewKb> for Kb {
    /// Converts into a full `Kb`, generating UUID and timestamp, normalising
    /// key / category / namespace to lowercase.
    fn from(new: NewKb) -> Self {
        let media_extension = new
            .media_extension
            .or_else(|| new.media_url.as_deref().and_then(file_extension));
        Kb {
            id: Uuid::new_v4().to_string(),
            key: new.key.to_lowercase(),
            value: new.value,
            notes: new.notes,
            category: new.category.to_lowercase(),
            reference: new.reference,
            namespace: new.namespace.to_lowercase(),
            tags: new.tags,
            metadata: new.metadata,
            created_on: Local::now().format("%Y-%m-%dT%H:%M:%S%z").to_string(),
            parent: new.parent,
            path: new.path,
            media_extension,
        }
    }
}

/// DTO for updating an existing KB entry (all fields are optional except `id`).
#[derive(Debug, Clone)]
pub struct KbUpdate {
    pub id: String,
    pub key: Option<String>,
    pub value: Option<String>,
    pub notes: Option<String>,
    pub category: Option<String>,
    pub namespace: Option<String>,
    pub reference: Option<String>,
    pub tags: Option<Vec<String>>,
    /// Set a new parent (by internal UUID). `None` = keep existing.
    pub parent: Option<String>,
    /// Set a new path. `None` = keep existing. Empty string = clear path.
    pub path: Option<String>,
    /// New metadata as `key=value,key=value` pairs. `None` = keep existing.
    /// Empty string = clear all metadata. Otherwise replaces the entire map
    /// (whole-replace, like `tags`) after validation via `build_metadata`.
    pub metadata: Option<String>,
}

/// Parameters for list / search operations.
#[derive(Debug, Clone, Default)]
pub struct KbFilter {
    pub category: Option<String>,
    pub namespace: Option<String>,
    pub tags: Option<Vec<String>>,
    pub keyword: Option<String>,
    pub reference: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Lightweight result row for list / search output.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct KbItem {
    pub id: String,
    pub key: String,
    pub category: String,
    pub namespace: String,
    pub tags: Vec<String>,
}

/// A KbItem paired with its semantic similarity distance (lower = more similar).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScoredKbItem {
    pub item: KbItem,
    pub score: f32,
}

/// Parameters for a semantic / vector search query.
#[derive(Debug, Clone)]
pub struct SemanticQuery {
    pub text: String,
    pub limit: Option<i64>,
    /// Maximum distance to include (0.0–1.0). Results with a distance above this value are excluded.
    pub threshold: Option<f32>,
    /// Filter to entries in this category (applied as a post-filter after the KNN search).
    pub category: Option<String>,
    /// Filter to entries in this namespace (applied as a `vec0` partition key filter —
    /// narrows the nearest-neighbor search itself, not just the results).
    pub namespace: Option<String>,
}

/// Input for generating and storing an embedding for a KB entry.
#[derive(Debug, Clone)]
pub struct EmbeddingInput {
    pub kb_id: String,
    pub text: String,
    /// Stored as the `kb_embeddings` `vec0` table's partition key.
    pub namespace: String,
}

/// YAML-serialisable representation of a KB entry produced by `kb export`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportKbItem {
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "Value")]
    pub value: String,
    #[serde(rename = "Notes")]
    pub notes: String,
    #[serde(rename = "Category")]
    pub category: String,
    #[serde(rename = "Reference")]
    pub reference: String,
    #[serde(rename = "Namespace")]
    pub namespace: String,
    #[serde(rename = "Tags")]
    pub tags: Vec<String>,
    /// Only present when the parent is also in the exported set.
    #[serde(rename = "Parent", skip_serializing_if = "Option::is_none")]
    pub parent_key: Option<String>,
    #[serde(rename = "Path", skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(rename = "MediaExtension", skip_serializing_if = "Option::is_none")]
    pub media_extension: Option<String>,
}

/// Parameters for the media file export operation.
pub struct ExportMediaParams {
    pub target_dir: String,
    pub category: Option<String>,
    pub namespace: Option<String>,
    pub items: Vec<ExportKbItem>,
}

/// YAML-serialisable representation of a single KB entry used by `kb import`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImportKbItem {
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "Value")]
    pub value: String,
    #[serde(rename = "Notes", default)]
    pub notes: String,
    #[serde(rename = "Category", default)]
    pub category: String,
    #[serde(rename = "Reference", default)]
    pub reference: String,
    #[serde(rename = "Namespace", default)]
    pub namespace: String,
    #[serde(rename = "Tags", default)]
    pub tags: Vec<String>,
    /// Key of the parent KB item. Resolved to internal UUID at import time.
    /// Accepts `Parent` too — the field name `kb export`'s `ExportKbItem`
    /// writes it under — while still serializing back out as `ParentKey`
    /// (used by the failed-items file format).
    #[serde(rename = "ParentKey", alias = "Parent", default)]
    pub parent_key: Option<String>,
    #[serde(rename = "Path", default)]
    pub path: Option<String>,
    #[serde(rename = "MediaExtension", default)]
    pub media_extension: Option<String>,
}

impl ImportKbItem {
    /// Returns `Some(reason)` if required fields are missing/blank, `None` if valid.
    pub fn validate(&self) -> Option<String> {
        if self.key.trim().is_empty() {
            return Some("Key is empty".to_string());
        }
        if self.value.trim().is_empty() {
            return Some("Value is empty".to_string());
        }
        if let Some(ref p) = self.path
            && !p.is_empty()
            && let Err(e) = normalize_path(p)
        {
            return Some(e.to_string());
        }
        None
    }
}

impl From<ImportKbItem> for NewKb {
    fn from(item: ImportKbItem) -> Self {
        NewKb {
            key: item.key,
            value: item.value,
            notes: item.notes,
            category: item.category,
            reference: item.reference,
            namespace: item.namespace,
            tags: item.tags,
            metadata: BTreeMap::new(),
            parent: None, // parent_key is resolved to UUID in the service layer
            path: item.path,
            media_url: None,
            media_extension: item.media_extension,
        }
    }
}

/// Input DTO for `kb add --json`. Deserialized directly from the CLI-supplied
/// JSON string. Unlike [`ImportKbItem`] (batch-tolerant YAML import), a single
/// invalid `AddJsonInput` is a hard error, not a skip-and-report-later case.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AddJsonInput {
    pub key: String,
    pub value: String,
    pub category: String,
    pub tags: Vec<String>,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub media_url: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl AddJsonInput {
    /// Hard-fails when a mandatory field (`key`/`value`/`category` blank
    /// after trim, or `tags` empty) is missing. Category and tags are
    /// strict because both feed the embedding text and back the vector index.
    pub fn validate(&self) -> Result<(), Error> {
        if self.key.trim().is_empty() {
            return Err(Error::InvalidJsonInput(
                "missing required field: key".to_string(),
            ));
        }
        if self.value.trim().is_empty() {
            return Err(Error::InvalidJsonInput(
                "missing required field: value".to_string(),
            ));
        }
        if self.category.trim().is_empty() {
            return Err(Error::InvalidJsonInput(
                "missing required field: category".to_string(),
            ));
        }
        if self.tags.is_empty() {
            return Err(Error::InvalidJsonInput(
                "tags must be a non-empty array".to_string(),
            ));
        }
        if self.metadata.keys().any(|k| k.trim().is_empty()) {
            return Err(Error::InvalidJsonInput(
                "metadata keys must not be blank".to_string(),
            ));
        }
        Ok(())
    }
}

/// Trims each tag, rejects any blank entry, and dedupes exact-match
/// duplicates while preserving first-occurrence order.
fn dedup_tags(tags: Vec<String>) -> Result<Vec<String>, Error> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::with_capacity(tags.len());
    for tag in tags {
        let trimmed = tag.trim().to_string();
        if trimmed.is_empty() {
            return Err(Error::InvalidTagError(
                "blank tag entry is not allowed".to_string(),
            ));
        }
        if seen.insert(trimmed.clone()) {
            result.push(trimmed);
        }
    }
    Ok(result)
}

/// Trims each key/value, rejects a blank trimmed key, and rejects an
/// exact-match duplicate trimmed key. Unlike `dedup_tags`, a duplicate
/// key is a hard error, not a silent drop.
pub fn build_metadata(pairs: Vec<(String, String)>) -> Result<BTreeMap<String, String>, Error> {
    let mut result = BTreeMap::new();
    for (key, value) in pairs {
        let key = key.trim().to_string();
        let value = value.trim().to_string();
        if key.is_empty() {
            return Err(Error::InvalidMetadataError(
                "blank metadata key is not allowed".to_string(),
            ));
        }
        if result.insert(key.clone(), value).is_some() {
            return Err(Error::DuplicateMetadataKeyError(key));
        }
    }
    Ok(result)
}

/// Splits a free-text `key=value,key=value` string into raw pairs.
/// Tokens without `=` are silently skipped (lenient).
pub fn parse_metadata_input(input: &str) -> Vec<(String, String)> {
    input
        .split(',')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

impl TryFrom<AddJsonInput> for NewKb {
    type Error = Error;

    fn try_from(input: AddJsonInput) -> Result<Self, Error> {
        input.validate()?;
        let tags = dedup_tags(input.tags)?;
        let path = match input.path.filter(|p| !p.is_empty()) {
            Some(p) => Some(normalize_path(&p)?),
            None => None,
        };
        Ok(NewKb {
            key: input.key.trim().to_string(),
            value: input.value.trim().to_string(),
            notes: input.notes,
            category: input.category.trim().to_string(),
            reference: input.reference,
            namespace: input.namespace,
            tags,
            metadata: input.metadata,
            parent: input.parent,
            path,
            media_url: input.media_url,
            media_extension: None,
        })
    }
}

/// Parameters for computing the media file storage path.
pub struct MediaPathParams<'a> {
    pub base_dir: &'a str,
    pub namespace: &'a str,
    pub path: Option<&'a str>,
    pub key: &'a str,
    /// File extension without the leading dot (e.g. `"jpg"`, `"pdf"`). `None` = no extension.
    pub extension: Option<&'a str>,
}

/// Computes the full filesystem path where a media file should be stored.
///
/// Path: `{base_dir}/media/{namespace}/{path}/{key}.{ext}`
/// Without path: `{base_dir}/media/{namespace}/{key}.{ext}`
pub fn media_file_path(params: &MediaPathParams<'_>) -> String {
    let file_name = match params.extension {
        Some(ext) if !ext.is_empty() => format!("{}.{}", params.key, ext),
        _ => params.key.to_string(),
    };

    let mut result = format!("{}/media/{}", params.base_dir, params.namespace);
    if let Some(p) = params.path {
        let trimmed = p.trim_start_matches('/');
        if !trimmed.is_empty() {
            result = format!("{}/{}", result, trimmed);
        }
    }
    format!("{}/{}", result, file_name)
}

/// Parameters for storing a media file (source → destination copy).
pub struct StoreMediaParams {
    pub source: String,
    pub destination: String,
}

/// Normalises a raw path string into a valid Unix-style path.
///
/// - Prepends `/` if the string does not already start with one.
/// - Rejects empty components (double slashes), `.`, `..`, and null bytes.
/// - Returns the normalised path on success or [`Error::InvalidPathError`] on failure.
pub fn normalize_path(raw: &str) -> Result<String, Error> {
    let with_slash = if raw.starts_with('/') {
        raw.to_string()
    } else {
        format!("/{}", raw)
    };

    for component in with_slash.trim_start_matches('/').split('/') {
        if component.is_empty() {
            return Err(Error::InvalidPathError(
                "path must not contain empty components (e.g. double slashes); \
                 use a format like /personal or /personal/cars/engines"
                    .to_string(),
            ));
        }
        if component == "." || component == ".." {
            return Err(Error::InvalidPathError(format!(
                "path component '{}' is not allowed; \
                 use a format like /personal or /personal/cars/engines",
                component
            )));
        }
        if component.contains('\0') {
            return Err(Error::InvalidPathError(
                "path must not contain null bytes".to_string(),
            ));
        }
    }

    Ok(with_slash)
}

/// Result of a reindex operation.
pub struct ReindexResult {
    /// (kb_key, kb_id) for entries successfully re-indexed.
    pub succeeded: Vec<(String, String)>,
    /// (kb_key_or_id, error_message) for entries that failed.
    pub failed: Vec<(String, String)>,
}

impl ReindexResult {
    pub fn total(&self) -> usize {
        self.succeeded.len() + self.failed.len()
    }
}

/// Result of a batch import operation.
pub struct ImportBatchResult {
    pub saved: Vec<Kb>,
    pub failed: Vec<FailedImportItem>,
}

/// An item that could not be imported, with the reason.
pub struct FailedImportItem {
    pub item: ImportKbItem,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "kb_tests.rs"]
mod tests;
