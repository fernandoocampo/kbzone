use std::fmt::Debug;

use crate::domain::DEFAULT_SEMANTIC_THRESHOLD;
use crate::errors::Error;

/// Outbound port for embedding providers.
/// Implementations generate vector embeddings from text.
///
/// The `Clone` bound exists for the same reason as `KbStore`: the concrete
/// `FastEmbedProvider` must be cheaply cloneable so it can be shared across services.
pub trait EmbeddingProvider: Debug + Clone {
    /// Returns the number of dimensions produced by this provider.
    fn dimensions(&self) -> usize;

    /// Embeds a single document (an entry being indexed) into a float vector.
    fn embed(&self, text: &str) -> Result<Vec<f32>, Error>;

    /// Embeds a search query. Models that treat queries differently from documents
    /// override this; by default a query is embedded like a document.
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, Error> {
        self.embed(text)
    }

    /// Maximum distance a semantic match may have when the caller gives no threshold.
    fn default_threshold(&self) -> f32 {
        DEFAULT_SEMANTIC_THRESHOLD
    }
}
