use std::fmt;
use std::str::FromStr;

use fastembed::EmbeddingModel;

use crate::domain::DEFAULT_SEMANTIC_THRESHOLD;
use crate::errors::Error;

/// The curated set of fastembed models a user may select in `config.yaml`
/// (`embedding.model`) or via `kb model set`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedModel {
    BgeSmallEnV15,
    BgeBaseEnV15,
    NomicEmbedTextV15,
    MultilingualE5Small,
}

impl SupportedModel {
    /// Model used when the config does not name one.
    pub const DEFAULT: Self = Self::BgeSmallEnV15;

    pub const ALL: [Self; 4] = [
        Self::BgeSmallEnV15,
        Self::BgeBaseEnV15,
        Self::NomicEmbedTextV15,
        Self::MultilingualE5Small,
    ];

    /// The name used in `config.yaml` and on the command line.
    pub fn name(&self) -> &'static str {
        match self {
            Self::BgeSmallEnV15 => "bge-small-en-v1.5",
            Self::BgeBaseEnV15 => "bge-base-en-v1.5",
            Self::NomicEmbedTextV15 => "nomic-embed-text-v1.5",
            Self::MultilingualE5Small => "multilingual-e5-small",
        }
    }

    /// The fastembed model this entry loads.
    pub fn embedding_model(&self) -> EmbeddingModel {
        match self {
            Self::BgeSmallEnV15 => EmbeddingModel::BGESmallENV15,
            Self::BgeBaseEnV15 => EmbeddingModel::BGEBaseENV15,
            Self::NomicEmbedTextV15 => EmbeddingModel::NomicEmbedTextV15,
            Self::MultilingualE5Small => EmbeddingModel::MultilingualE5Small,
        }
    }

    /// Number of dimensions of the vectors the model produces.
    pub fn dimensions(&self) -> usize {
        match self {
            Self::BgeSmallEnV15 | Self::MultilingualE5Small => 384,
            Self::BgeBaseEnV15 | Self::NomicEmbedTextV15 => 768,
        }
    }

    /// Prefix the model expects on search queries.
    pub fn query_prefix(&self) -> &'static str {
        match self {
            Self::NomicEmbedTextV15 => "search_query: ",
            Self::MultilingualE5Small => "query: ",
            Self::BgeSmallEnV15 | Self::BgeBaseEnV15 => "",
        }
    }

    /// Prefix the model expects on the documents being indexed.
    pub fn passage_prefix(&self) -> &'static str {
        match self {
            Self::NomicEmbedTextV15 => "search_document: ",
            Self::MultilingualE5Small => "passage: ",
            Self::BgeSmallEnV15 | Self::BgeBaseEnV15 => "",
        }
    }

    /// Maximum distance a semantic match may have when `kb ask` gets no `--threshold`.
    ///
    /// Distances are not comparable across models, so each model carries its own cutoff.
    /// bge-small keeps the long-standing default. The others were placed inside the gap
    /// between the weakest correct top hit and the closest unrelated top hit, measured on
    /// a small sample (10 entries, 10 relevant and 4 unrelated queries): bge-base
    /// 0.955 / 1.040, nomic 0.914 / 1.010. multilingual-e5-small has no such gap
    /// (0.632 / 0.633), so its value is a conservative guess: rely on its ranking.
    pub fn default_threshold(&self) -> f32 {
        match self {
            Self::BgeSmallEnV15 => DEFAULT_SEMANTIC_THRESHOLD,
            Self::BgeBaseEnV15 => 1.0,
            Self::NomicEmbedTextV15 => 0.96,
            Self::MultilingualE5Small => 0.6,
        }
    }
}

impl fmt::Display for SupportedModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for SupportedModel {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let wanted = s.trim();
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(wanted))
            .ok_or_else(|| {
                let valid: Vec<&str> = Self::ALL.iter().map(Self::name).collect();
                Error::EmbeddingError(format!(
                    "unknown embedding model '{wanted}'; supported models: {}",
                    valid.join(", ")
                ))
            })
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
