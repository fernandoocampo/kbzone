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
}

impl Kb {
    /// Joins tags with a space for SQLite storage.
    pub fn tags_as_string(&self) -> String {
        self.tags.join(" ")
    }

    /// Builds the text used for embedding: key + category + namespace + tags + first 200 chars of value.
    pub fn embedding_text(&self) -> String {
        let truncated_value: String = self.value.chars().take(200).collect();
        format!(
            "{} {} {} {} {}",
            self.key,
            self.category,
            self.namespace,
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
}

/// Parameters for list / search operations.
#[derive(Debug, Clone, Default)]
pub struct KbFilter {
    pub category: Option<String>,
    pub namespace: Option<String>,
    pub tags: Option<Vec<String>>,
    pub keyword: Option<String>,
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
}

/// Input for generating and storing an embedding for a KB entry.
#[derive(Debug, Clone)]
pub struct EmbeddingInput {
    pub kb_id: String,
    pub text: String,
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
        }
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
mod tests {
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
}
