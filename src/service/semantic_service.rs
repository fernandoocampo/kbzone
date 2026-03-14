use crate::domain::{EmbeddingInput, ScoredKbItem, SemanticQuery};
use crate::errors::Error;
use crate::ports::{EmbeddingProvider, VectorStore};

/// Service that coordinates embedding generation and vector search.
#[derive(Debug, Clone)]
pub struct SemanticService<V: VectorStore, E: EmbeddingProvider> {
    vector_store: V,
    embedder: E,
}

impl<V: VectorStore, E: EmbeddingProvider> SemanticService<V, E> {
    pub fn new(vector_store: V, embedder: E) -> Self {
        Self {
            vector_store,
            embedder,
        }
    }

    /// Embeds the text in `input` and stores the resulting vector.
    pub fn index_kb(&self, input: &EmbeddingInput) -> Result<(), Error> {
        let embedding = self.embedder.embed(&input.text)?;
        self.vector_store.save_embedding(input, &embedding)
    }

    /// Removes the stored embedding for the given KB entry.
    pub fn remove_index(&self, kb_id: &str) -> Result<(), Error> {
        self.vector_store.delete_embedding(kb_id)
    }

    /// Embeds the query and returns the nearest KB entries by vector distance.
    pub fn ask(&self, query: &SemanticQuery) -> Result<Vec<ScoredKbItem>, Error> {
        let embedding = self.embedder.embed(&query.text)?;
        self.vector_store.search_similar(query, &embedding)
    }
}

// ---------------------------------------------------------------------------
// Unit tests using mock implementations
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Debug, Clone)]
    struct MockEmbeddingProvider {
        dims: usize,
    }

    impl EmbeddingProvider for MockEmbeddingProvider {
        fn dimensions(&self) -> usize {
            self.dims
        }

        fn embed(&self, _text: &str) -> Result<Vec<f32>, Error> {
            Ok(vec![0.1; self.dims])
        }
    }

    #[derive(Debug, Clone, Default)]
    struct MockVectorStore {
        data: RefCell<HashMap<String, Vec<f32>>>,
    }

    impl VectorStore for MockVectorStore {
        fn initialize_vectors(&self, _dimensions: usize) -> Result<(), Error> {
            Ok(())
        }

        fn save_embedding(&self, input: &EmbeddingInput, embedding: &[f32]) -> Result<(), Error> {
            self.data
                .borrow_mut()
                .insert(input.kb_id.clone(), embedding.to_vec());
            Ok(())
        }

        fn delete_embedding(&self, kb_id: &str) -> Result<(), Error> {
            self.data.borrow_mut().remove(kb_id);
            Ok(())
        }

        fn search_similar(
            &self,
            _query: &SemanticQuery,
            _embedding: &[f32],
        ) -> Result<Vec<ScoredKbItem>, Error> {
            Ok(vec![])
        }
    }

    fn make_svc() -> SemanticService<MockVectorStore, MockEmbeddingProvider> {
        SemanticService::new(
            MockVectorStore::default(),
            MockEmbeddingProvider { dims: 4 },
        )
    }

    #[test]
    fn index_kb_stores_embedding() {
        let svc = make_svc();
        let input = EmbeddingInput {
            kb_id: "id-1".to_string(),
            text: "rust ownership concept".to_string(),
        };
        assert!(svc.index_kb(&input).is_ok());
        assert!(svc.vector_store.data.borrow().contains_key("id-1"));
    }

    #[test]
    fn remove_index_deletes_embedding() {
        let svc = make_svc();
        let input = EmbeddingInput {
            kb_id: "id-1".to_string(),
            text: "rust".to_string(),
        };
        svc.index_kb(&input).unwrap();
        assert!(svc.remove_index("id-1").is_ok());
        assert!(!svc.vector_store.data.borrow().contains_key("id-1"));
    }

    #[test]
    fn ask_returns_results_without_error() {
        let svc = make_svc();
        let query = SemanticQuery {
            text: "kubernetes pod status".to_string(),
            limit: Some(5),
        };
        assert!(svc.ask(&query).is_ok());
    }
}
