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

    /// Formats the entry for `kb get` output.
    pub fn display(&self) {
        println!("ID        : {}", self.id);
        println!("Key       : {}", self.key);
        println!("Value     : {}", self.value);
        println!("Category  : {}", self.category);
        println!("Namespace : {}", self.namespace);
        println!("Reference : {}", self.reference);
        println!("Tags      : {}", self.tags.join(", "));
        println!("Created   : {}", self.created_on);
        if !self.notes.is_empty() {
            println!("Notes     :\n{}", self.notes);
        }
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

impl NewKb {
    /// Converts into a full `Kb`, generating UUID and timestamp, normalising
    /// key / category / namespace to lowercase.
    pub fn to_kb(self) -> Kb {
        Kb {
            id: Uuid::new_v4().to_string(),
            key: self.key.to_lowercase(),
            value: self.value,
            notes: self.notes,
            category: self.category.to_lowercase(),
            reference: self.reference,
            namespace: self.namespace.to_lowercase(),
            tags: self.tags,
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
}
