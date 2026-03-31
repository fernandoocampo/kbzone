use clap::{Parser, Subcommand};

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
    },

    /// Get a single entry by key or ID.
    Get {
        #[arg(long, help = "Unique string key", conflicts_with = "id")]
        key: Option<String>,

        #[arg(long, help = "UUID", conflicts_with = "key")]
        id: Option<String>,
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
    },

    /// Delete an entry by ID.
    Delete {
        #[arg(long, required = true, help = "UUID of the entry to delete")]
        id: String,
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
    },

    /// Re-generate embeddings for all existing KB entries.
    Reindex,

    /// Print a random quote from the knowledge base.
    Quote,

    /// Print version information (git hash, build date).
    Version,

    /// Export KB entries to a multi-document YAML file.
    Export {
        #[arg(long, required = true, help = "Output path for the YAML export file")]
        file: String,
        #[arg(long, help = "Export only entries with this category")]
        category: Option<String>,
        #[arg(long, help = "Export only entries with this namespace")]
        namespace: Option<String>,
        #[arg(long, help = "Maximum number of entries to export")]
        limit: Option<i64>,
        #[arg(long, help = "Row offset for pagination")]
        offset: Option<i64>,
    },

    /// Import KB entries from a multi-document YAML file.
    Import {
        #[arg(long, required = true, help = "Path to the YAML file to import")]
        file: String,
        #[arg(
            long,
            default_value = "wrong-kb-items.yaml",
            help = "Output file for items that failed to import (same YAML format)"
        )]
        failed_items_file: String,
    },
}
