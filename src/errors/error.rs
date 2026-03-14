/// Domain and storage errors.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("KB not found")]
    KBNotFound,
    #[error("unable to get this KB")]
    GetKBError,
    #[error("unable to create KB")]
    CreateKBError,
    #[error("unable to update KB")]
    UpdateKBError,
    #[error("KB was not updated")]
    KBWasNotUpdatedError,
    #[error("unable to delete KB")]
    DeleteKBError,
    #[error("KB already exists")]
    DuplicateKBError,
    #[error("unable to search")]
    SearchError,
    #[error("unable to list entries")]
    ListError,
    #[error("repository query failed")]
    DatabaseQueryError,
    #[error("storage init failed: {0}")]
    StorageInitError(String),
    #[error("embedding error: {0}")]
    EmbeddingError(String),
    #[error("vector search failed")]
    VectorSearchError,
    #[error("vector store init failed: {0}")]
    VectorStoreInitError(String),
    #[error("reindex failed: {0}")]
    ReindexError(String),
}

/// Application-level startup / configuration errors.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("config error: {0}")]
    ConfigError(String),
    #[error("storage error: {0}")]
    StorageError(String),
    #[error("embedding setup error: {0}")]
    EmbeddingSetupError(String),
    #[error("command failed: {0}")]
    CommandError(#[from] Error),
}
