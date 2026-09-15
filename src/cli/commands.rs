use clap::{Parser, Subcommand};

/// Parses a single `KEY=VALUE` token, splitting on the first `=` only.
fn parse_key_val(s: &str) -> Result<(String, String), String> {
    s.split_once('=')
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .ok_or_else(|| format!("invalid KEY=VALUE pair (missing '='): {s}"))
}

/// kbzone — local knowledge base CLI
#[derive(Parser, Debug)]
#[command(name = "kb", about = "Manage your local knowledge base")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Add a new knowledge base entry.
    Add {
        #[arg(long, help = "Unique string key (e.g. rust-ownership)")]
        key: Option<String>,

        #[arg(long, help = "Content / answer for this entry")]
        value: Option<String>,

        #[arg(long, default_value = "", help = "Extended notes or elaboration")]
        notes: String,

        #[arg(
            long,
            default_value = "",
            help = "Entry type (concept, quote, command, …)"
        )]
        category: String,

        #[arg(long, default_value = "", help = "Grouping namespace")]
        namespace: String,

        #[arg(long, default_value = "", help = "Source (book, person, URL, …)")]
        reference: String,

        /// Comma-separated tags, e.g. `rust,memory,concepts`
        #[arg(long, value_delimiter = ',', help = "Comma-separated search tags")]
        tags: Vec<String>,

        /// Comma-separated key=value metadata pairs, e.g. `author=me,priority=high`
        #[arg(long, value_delimiter = ',', value_parser = parse_key_val, help = "Comma-separated key=value metadata pairs (e.g. author=me,priority=high)")]
        metadata: Vec<(String, String)>,

        #[arg(long, help = "Prompt for missing required fields interactively")]
        interactive: bool,

        #[arg(long, help = "UUID of the parent KB entry")]
        parent: Option<String>,

        #[arg(
            long,
            default_value = "",
            help = "Optional hierarchical path (e.g. /personal/rust). Leading / is added automatically."
        )]
        path: String,

        #[arg(
            long,
            default_value = "",
            help = "URL or local file path for media category entries (e.g. https://… or /path/to/file.jpg)"
        )]
        media_url: String,

        /// Full entry as a JSON object, e.g.
        /// `{"key":"k","value":"v","category":"quote","tags":["a","b"]}`.
        /// `key`, `value`, `category` and `tags` are required; `tags` must be
        /// a non-empty array with no blank entries (duplicates are silently
        /// deduped). Mutually exclusive with every other `add` flag — skips
        /// all prompts and prints only the created entry as JSON.
        #[arg(
            long,
            help = "Full entry as a JSON object (mutually exclusive with the other add flags)"
        )]
        json: Option<String>,
    },

    /// Get a single entry by key or ID.
    Get {
        #[arg(long, help = "Unique string key", conflicts_with = "id")]
        key: Option<String>,

        #[arg(long, help = "UUID", conflicts_with = "key")]
        id: Option<String>,

        #[arg(long, help = "Output format: json | yaml (default: plain text)")]
        out: Option<String>,

        #[arg(
            long,
            help = "Show outgoing relationships (entries this one connects to)"
        )]
        with_out_connections: bool,

        #[arg(
            long,
            help = "Show incoming relationships (entries connected to this one)"
        )]
        with_in_connections: bool,

        #[arg(long, help = "Show both outgoing and incoming relationships")]
        with_all_connections: bool,
    },

    /// Update an existing entry (identified by --id).
    Update {
        #[arg(long, required = true, help = "UUID of the entry to update")]
        id: String,

        #[arg(long, help = "New key")]
        key: Option<String>,

        #[arg(long, help = "New value")]
        value: Option<String>,

        #[arg(long, help = "New notes")]
        notes: Option<String>,

        #[arg(long, help = "New category")]
        category: Option<String>,

        #[arg(long, help = "New namespace")]
        namespace: Option<String>,

        #[arg(long, help = "New reference")]
        reference: Option<String>,

        /// Comma-separated tags
        #[arg(
            long,
            value_delimiter = ',',
            help = "New tags (comma-separated); replaces existing"
        )]
        tags: Option<Vec<String>>,

        #[arg(long, help = "UUID of the new parent KB entry")]
        parent: Option<String>,

        #[arg(
            long,
            help = "New path (optional). Leading / is added automatically. Pass empty string to clear."
        )]
        path: Option<String>,

        #[arg(
            long,
            help = "New metadata as key=value pairs (comma-separated); replaces existing. Pass empty string to clear."
        )]
        metadata: Option<String>,

        #[arg(long, help = "Output format: json | yaml (default: plain text)")]
        out: Option<String>,
    },

    /// Delete an entry by ID.
    Delete {
        #[arg(long, required = true, help = "UUID of the entry to delete")]
        id: String,

        #[arg(long, help = "Output format: json (optional, default: plain text)")]
        out: Option<String>,
    },

    /// Search entries by keyword, category, namespace, tags, or reference.
    Search {
        #[arg(long, help = "Keyword to search for in tags (FTS5)")]
        keyword: Option<String>,

        #[arg(long, help = "Filter by category")]
        category: Option<String>,

        #[arg(long, help = "Filter by namespace")]
        namespace: Option<String>,

        /// Comma-separated tags to filter by
        #[arg(long, value_delimiter = ',', help = "Filter by tags (comma-separated)")]
        tags: Vec<String>,

        #[arg(
            long,
            help = "Filter results to entries whose reference contains this string (case-insensitive)"
        )]
        reference: Option<String>,

        #[arg(long, default_value = "20", help = "Maximum rows to return")]
        limit: i64,

        #[arg(long, default_value = "0", help = "Row offset for pagination")]
        offset: i64,

        #[arg(long, help = "Output format: json | yaml (default: plain text)")]
        out: Option<String>,
    },

    /// Semantic / vector search using natural language.
    Ask {
        #[arg(help = "Natural language query (e.g. \"how do I list kubernetes pods\")")]
        query: String,

        #[arg(long, default_value = "10", help = "Maximum results to return")]
        limit: i64,

        #[arg(
            long,
            default_value = "0.9",
            help = "Maximum distance to include (0.0–1.0). Lower = stricter matching. Results with a distance above this threshold are excluded. Use a higher value (e.g. 0.95) to see more results."
        )]
        threshold: f32,

        #[arg(long, help = "Output format: json | yaml (default: plain text)")]
        out: Option<String>,
    },

    /// Re-generate embeddings for all existing KB entries.
    Reindex,

    /// Print a random quote from the knowledge base.
    Quote,

    /// List all distinct category values in the knowledge base.
    Categories {
        #[arg(long, help = "Only list categories used within this namespace")]
        namespace: Option<String>,
    },

    /// Print version information (git hash, build date).
    Version,

    /// Export KB entries to a multi-document YAML file.
    Export {
        #[arg(
            long,
            help = "Name for the exported YAML file (default: exported-kb-<yyyy-mm-dd-hh-mi-ss>.yaml)"
        )]
        file_name: Option<String>,
        #[arg(
            long,
            required = true,
            help = "Directory where the YAML file (and any media files) will be written"
        )]
        folder_output: String,
        #[arg(long, help = "Export only entries with this category")]
        category: Option<String>,
        #[arg(long, help = "Export only entries with this namespace")]
        namespace: Option<String>,
        #[arg(long, help = "Maximum number of entries to export")]
        limit: Option<i64>,
        #[arg(long, help = "Row offset for pagination")]
        offset: Option<i64>,
    },

    /// Import KB entries and relationships from a YAML file (see `kb export`'s output shape).
    Import {
        #[arg(long, required = true, help = "Path to the YAML file to import")]
        file: String,
        #[arg(
            long,
            default_value = "wrong-kb-items.yaml",
            help = "Output file for items that failed to import (same YAML format)"
        )]
        failed_items_file: String,
        #[arg(
            long,
            default_value = "wrong-kb-edges.yaml",
            help = "Output file for relationships that failed to import (same YAML format)"
        )]
        failed_edges_file: String,
    },

    /// Link two existing KB entries (creates a directed edge).
    Link {
        #[arg(help = "Key or UUID of the source entry")]
        from: String,

        #[arg(help = "Key or UUID of the target entry")]
        to: String,

        #[arg(
            long,
            default_value = "",
            help = "Free-text note describing the relationship"
        )]
        note: String,
    },

    /// Remove the link between two KB entries.
    Unlink {
        #[arg(help = "Key or UUID of the source entry")]
        from: String,

        #[arg(help = "Key or UUID of the target entry")]
        to: String,
    },

    /// Show one-hop outgoing/incoming relationships for an entry.
    Related {
        #[arg(help = "Key or UUID of the entry")]
        key_or_id: String,

        #[arg(long, default_value = "both", help = "out | in | both")]
        direction: String,

        #[arg(long, help = "Output as JSON (full NOTE text, no truncation)")]
        json: bool,
    },

    /// Show the transitive relationship tree for an entry.
    Tree {
        #[arg(help = "Key or UUID of the entry")]
        key_or_id: String,

        #[arg(long, default_value = "out", help = "out | in")]
        direction: String,

        #[arg(long, default_value = "10", help = "Maximum traversal depth")]
        depth: i64,

        #[arg(long, help = "Output as JSON (full NOTE text, no truncation)")]
        json: bool,
    },

    /// Generate an interactive HTML graph view and open it in your browser
    /// (drag nodes, click to inspect, see relationships highlighted).
    Graph {
        #[arg(help = "Key or UUID of the entry")]
        key_or_id: String,

        #[arg(long, default_value = "both", help = "out | in | both")]
        direction: String,

        #[arg(long, default_value = "2", help = "Traversal depth from the root")]
        depth: i64,
    },
}
