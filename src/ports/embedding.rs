use std::fmt::Debug;

use crate::errors::Error;

/// Outbound port for embedding providers.
/// Implementations generate vector embeddings from text.
pub trait EmbeddingProvider: Debug + Clone {
    /// Returns the number of dimensions produced by this provider.
    fn dimensions(&self) -> usize;

    /// Embeds a single text string into a float vector.
    fn embed(&self, text: &str) -> Result<Vec<f32>, Error>;
}
