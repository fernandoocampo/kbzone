use std::sync::{Arc, Mutex};

use fastembed::{InitOptions, TextEmbedding};

use crate::errors::Error;
use crate::ports::EmbeddingProvider;

use super::model::SupportedModel;

/// `EmbeddingProvider` backed by a local fastembed model chosen from `SupportedModel`
/// (default: BAAI/bge-small-en-v1.5, 384 dimensions, downloaded on first use).
/// `TextEmbedding::embed` takes `&mut self` (fastembed 6.x); the `Mutex` provides
/// the interior mutability needed to keep `EmbeddingProvider::embed` at `&self`,
/// mirroring the `Arc<Mutex<Connection>>` pattern used by `SqliteStore`.
#[derive(Clone)]
pub struct FastEmbedProvider {
    model: Arc<Mutex<TextEmbedding>>,
    supported: SupportedModel,
}

/// Inputs for `FastEmbedProvider::new`.
pub struct FastEmbedParams {
    pub model: SupportedModel,
    /// Where the model files are stored (e.g. `~/.kbzona/fastembed_cache`).
    pub cache_dir: std::path::PathBuf,
}

impl FastEmbedProvider {
    /// Initialises the provider, loading (and downloading if needed) the model.
    pub fn new(params: FastEmbedParams) -> Result<Self, Error> {
        let opts =
            InitOptions::new(params.model.embedding_model()).with_cache_dir(params.cache_dir);
        let model =
            TextEmbedding::try_new(opts).map_err(|e| Error::EmbeddingError(e.to_string()))?;
        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            supported: params.model,
        })
    }

    /// Embeds `prefix` followed by `text`.
    fn embed_with_prefix(&self, prefix: &str, text: &str) -> Result<Vec<f32>, Error> {
        let mut model = self.model.lock().expect("mutex poisoned");
        let mut embeddings = model
            .embed(vec![format!("{prefix}{text}")], None)
            .map_err(|e| Error::EmbeddingError(e.to_string()))?;
        embeddings
            .pop()
            .ok_or_else(|| Error::EmbeddingError("empty embedding result".to_string()))
    }
}

impl std::fmt::Debug for FastEmbedProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FastEmbedProvider")
            .field("model", &self.supported.name())
            .field("dims", &self.supported.dimensions())
            .finish()
    }
}

impl EmbeddingProvider for FastEmbedProvider {
    fn dimensions(&self) -> usize {
        self.supported.dimensions()
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, Error> {
        self.embed_with_prefix(self.supported.passage_prefix(), text)
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>, Error> {
        self.embed_with_prefix(self.supported.query_prefix(), text)
    }

    fn default_threshold(&self) -> f32 {
        self.supported.default_threshold()
    }
}
