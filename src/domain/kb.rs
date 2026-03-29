use chrono::Local;
use uuid::Uuid;

/// Full KB entity as stored in the database.
#[derive(Debug, Clone, PartialEq)]
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
    /// ISO-8601 creation timestamp.
    pub created_on: String,
    /// Internal UUID of the parent KB item, if any.
    pub parent: Option<String>,
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

impl std::fmt::Display for Kb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "ID        : {}", self.id)?;
        writeln!(f, "Key       : {}", self.key)?;
        writeln!(f, "Value     : {}", self.value)?;
        writeln!(f, "Category  : {}", self.category)?;
        writeln!(f, "Namespace : {}", self.namespace)?;
        writeln!(f, "Reference : {}", self.reference)?;
        writeln!(f, "Tags      : {}", self.tags.join(", "))?;
        writeln!(f, "Created   : {}", self.created_on)?;
        if let Some(ref p) = self.parent {
            writeln!(f, "Parent    : {}", p)?;
        }
        if !self.notes.is_empty() {
            writeln!(f, "Notes     :\n{}", self.notes)?;
        }
        Ok(())
    }
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
    /// Internal UUID of the parent KB item, if any.
    pub parent: Option<String>,
}

impl From<NewKb> for Kb {
    /// Converts into a full `Kb`, generating UUID and timestamp, normalising
    /// key / category / namespace to lowercase.
    fn from(new: NewKb) -> Self {
        Kb {
            id: Uuid::new_v4().to_string(),
            key: new.key.to_lowercase(),
            value: new.value,
            notes: new.notes,
            category: new.category.to_lowercase(),
            reference: new.reference,
            namespace: new.namespace.to_lowercase(),
            tags: new.tags,
            created_on: Local::now().format("%Y-%m-%dT%H:%M:%S%z").to_string(),
            parent: new.parent,
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
#[derive(Debug, Clone, PartialEq)]
pub struct KbItem {
    pub id: String,
    pub key: String,
    pub category: String,
    pub namespace: String,
    pub tags: Vec<String>,
}

/// A KbItem paired with its semantic similarity distance (lower = more similar).
#[derive(Debug, Clone)]
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
}

/// Input for generating and storing an embedding for a KB entry.
#[derive(Debug, Clone)]
pub struct EmbeddingInput {
    pub kb_id: String,
    pub text: String,
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
    #[serde(rename = "ParentKey", default)]
    pub parent_key: Option<String>,
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
            parent: None, // parent_key is resolved to UUID in the service layer
        }
    }
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
