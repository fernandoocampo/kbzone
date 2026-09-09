use std::sync::{Arc, Mutex};

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use crate::errors::Error;
use crate::ports::EmbeddingProvider;

/// `EmbeddingProvider` backed by a local fastembed model.
/// Uses BAAI/bge-small-en-v1.5 (384 dimensions, ~50 MB, downloaded on first use).
/// `TextEmbedding::embed` takes `&mut self` (fastembed 6.x); the `Mutex` provides
/// the interior mutability needed to keep `EmbeddingProvider::embed` at `&self`,
/// mirroring the `Arc<Mutex<Connection>>` pattern used by `SqliteStore`.
#[derive(Clone)]
pub struct FastEmbedProvider {
    model: Arc<Mutex<TextEmbedding>>,
    dims: usize,
}

impl FastEmbedProvider {
    /// Initialises the provider, loading (and downloading if needed) the model.
    /// `cache_dir` is where the model files are stored (e.g. `~/.kbzona/fastembed_cache`).
    pub fn new(cache_dir: std::path::PathBuf) -> Result<Self, Error> {
        let opts = InitOptions::new(EmbeddingModel::BGESmallENV15).with_cache_dir(cache_dir);
        let model =
            TextEmbedding::try_new(opts).map_err(|e| Error::EmbeddingError(e.to_string()))?;
        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            dims: 384,
        })
    }
}

impl std::fmt::Debug for FastEmbedProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FastEmbedProvider")
            .field("dims", &self.dims)
            .finish()
    }
}

impl EmbeddingProvider for FastEmbedProvider {
    fn dimensions(&self) -> usize {
        self.dims
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, Error> {
        let mut model = self.model.lock().expect("mutex poisoned");
        let mut embeddings = model
            .embed(vec![text.to_string()], None)
            .map_err(|e| Error::EmbeddingError(e.to_string()))?;
        embeddings
            .pop()
            .ok_or_else(|| Error::EmbeddingError("empty embedding result".to_string()))
    }
}
