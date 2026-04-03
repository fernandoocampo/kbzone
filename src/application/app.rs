use clap::Parser;

use crate::adapters::fastembed::FastEmbedProvider;
use crate::adapters::filesystem::FileSystemMediaStore;
use crate::adapters::http::HttpMediaFetcher;
use crate::adapters::sqlite::SqliteStore;
use crate::application::config::Config;
use crate::cli::commands::{Cli, Command};
use crate::cli::handlers::{self, AddParams, ExportParams, GetParams, ImportParams, SearchParams};
use crate::domain::{KbUpdate, SemanticQuery};
use crate::errors::AppError;
use crate::ports::{EmbeddingProvider, KbStore, VectorStore};
use crate::service::{KBService, ServiceDeps};

/// Top-level application object. Owns the unified service and drives the CLI.
pub struct App {
    svc: KBService<
        SqliteStore,
        SqliteStore,
        FastEmbedProvider,
        FileSystemMediaStore,
        HttpMediaFetcher,
    >,
    base_dir: String,
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

        let base_dir = std::path::Path::new(&db_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let svc = KBService::new(
            store.clone(),
            ServiceDeps {
                vector_store: store,
                embedder,
                media_store: FileSystemMediaStore,
                media_fetcher: HttpMediaFetcher,
                base_dir: base_dir.clone(),
            },
        );

        Ok(App { svc, base_dir })
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
                interactive,
                parent,
                path,
                media_url,
            } => handlers::handle_add(
                &self.svc,
                AddParams {
                    key,
                    value,
                    notes,
                    category,
                    namespace,
                    reference,
                    tags,
                    interactive,
                    parent,
                    path: (!path.is_empty()).then_some(path),
                    media_url: (!media_url.is_empty()).then_some(media_url),
                },
            )?,

            Command::Get { key, id } => handlers::handle_get(
                &self.svc,
                GetParams {
                    key,
                    id,
                    base_dir: self.base_dir.clone(),
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
                parent,
                path,
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
                    parent,
                    path,
                },
            )?,

            Command::Delete { id } => handlers::handle_delete(&self.svc, id)?,

            Command::Search {
                keyword,
                category,
                namespace,
                tags,
                reference,
                limit,
                offset,
            } => handlers::handle_search(
                &self.svc,
                SearchParams {
                    keyword,
                    category,
                    namespace,
                    tags,
                    reference,
                    limit,
                    offset,
                },
            )?,

            Command::Ask {
                query,
                limit,
                threshold,
            } => handlers::handle_ask(
                &self.svc,
                SemanticQuery {
                    text: query,
                    limit: Some(limit),
                    threshold: Some(threshold),
                },
            )?,

            Command::Quote => handlers::handle_quote(&self.svc)?,

            Command::Reindex => handlers::handle_reindex(&self.svc)?,

            Command::Export {
                file,
                category,
                namespace,
                limit,
                offset,
            } => handlers::handle_export(
                &self.svc,
                ExportParams {
                    file,
                    category,
                    namespace,
                    limit,
                    offset,
                },
            )?,

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
