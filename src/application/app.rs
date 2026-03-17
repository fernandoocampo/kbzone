use clap::Parser;

use crate::adapters::fastembed::FastEmbedProvider;
use crate::adapters::sqlite::SqliteStore;
use crate::application::config::Config;
use crate::cli::commands::{Cli, Command};
use crate::cli::handlers::{self, GetParams, ImportParams, ListParams};
use crate::domain::{KbUpdate, NewKb, SemanticQuery};
use crate::errors::AppError;
use crate::ports::{EmbeddingProvider, KbStore, VectorStore};
use crate::service::{KBService, SemanticDeps};

/// Top-level application object. Owns the unified service and drives the CLI.
pub struct App {
    svc: KBService<SqliteStore, SqliteStore, FastEmbedProvider>,
}

impl App {
    /// Loads config, initialises storage and the embedding provider, wires up services.
    pub fn build() -> Result<Self, AppError> {
        let config = Config::load().map_err(AppError::ConfigError)?;
        config.validate().map_err(AppError::ConfigError)?;

        let db_path = config.resolved_db_path();
        let store =
            SqliteStore::new(&db_path).map_err(|e| AppError::StorageError(e.to_string()))?;
        store
            .initialize()
            .map_err(|e| AppError::StorageError(e.to_string()))?;

        let embedder = FastEmbedProvider::new(config.model_cache_dir())
            .map_err(|e| AppError::EmbeddingSetupError(e.to_string()))?;

        store
            .initialize_vectors(embedder.dimensions())
            .map_err(|e| AppError::StorageError(e.to_string()))?;

        let svc = KBService::new(
            store.clone(),
            SemanticDeps {
                vector_store: store,
                embedder,
            },
        );

        Ok(App { svc })
    }

    /// Parses the CLI arguments and dispatches to the appropriate handler.
    pub fn run(self) -> Result<(), AppError> {
        let cli = Cli::parse();

        match cli.command {
            Command::Add {
                key,
                value,
                notes,
                category,
                namespace,
                reference,
                tags,
            } => handlers::handle_add(
                &self.svc,
                NewKb {
                    key,
                    value,
                    notes,
                    category,
                    namespace,
                    reference,
                    tags,
                },
            )?,

            Command::Get { key, id } => handlers::handle_get(&self.svc, GetParams { key, id })?,

            Command::List {
                category,
                namespace,
                tags,
                limit,
                offset,
            } => handlers::handle_list(
                &self.svc,
                ListParams {
                    category,
                    namespace,
                    tags,
                    limit,
                    offset,
                },
            )?,

            Command::Update {
                id,
                key,
                value,
                notes,
                category,
                namespace,
                reference,
                tags,
            } => handlers::handle_update(
                &self.svc,
                KbUpdate {
                    id,
                    key,
                    value,
                    notes,
                    category,
                    namespace,
                    reference,
                    tags,
                },
            )?,

            Command::Delete { id } => handlers::handle_delete(&self.svc, id)?,

            Command::Search { keyword } => handlers::handle_search(&self.svc, keyword)?,

            Command::Ask { query, limit } => handlers::handle_ask(
                &self.svc,
                SemanticQuery {
                    text: query,
                    limit: Some(limit),
                },
            )?,

            Command::Quote => handlers::handle_quote(&self.svc)?,

            Command::Reindex => handlers::handle_reindex(&self.svc)?,

            Command::Import {
                file,
                failed_items_file,
            } => handlers::handle_import(
                &self.svc,
                ImportParams {
                    file,
                    failed_items_file,
                },
            )?,

            Command::Version => handlers::handle_version()?,
        }

        Ok(())
    }
}
