use std::fmt::Debug;

use crate::errors::Error;

/// Outbound port for embedding providers.
/// Implementations generate vector embeddings from text.
///
/// The `Clone` bound exists for the same reason as `KbStore`: the concrete
/// `FastEmbedProvider` must be cheaply cloneable so it can be shared across services.
pub trait EmbeddingProvider: Debug + Clone {
    /// Returns the number of dimensions produced by this provider.
    fn dimensions(&self) -> usize;

    /// Embeds a single text string into a float vector.
    fn embed(&self, text: &str) -> Result<Vec<f32>, Error>;
}
