/// Domain and storage errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("KB not found")]
    KBNotFound,
    #[error("unable to get KB: {0}")]
    GetKBError(String),
    #[error("unable to create KB: {0}")]
    CreateKBError(String),
    #[error("unable to update KB: {0}")]
    UpdateKBError(String),
    #[error("KB was not updated")]
    KBWasNotUpdatedError,
    #[error("unable to delete KB: {0}")]
    DeleteKBError(String),
    #[error("KB already exists")]
    DuplicateKBError,
    #[error("unable to search: {0}")]
    SearchError(String),
    #[error("unable to list entries: {0}")]
    ListError(String),
    #[error("repository query failed: {0}")]
    DatabaseQueryError(String),
    #[error("storage init failed: {0}")]
    StorageInitError(String),
    #[error("embedding error: {0}")]
    EmbeddingError(String),
    #[error("vector search failed: {0}")]
    VectorSearchError(String),
    #[error("vector store init failed: {0}")]
    VectorStoreInitError(String),
    #[error("reindex failed: {0}")]
    ReindexError(String),
    #[error("failed to read import file: {0}")]
    ImportFileError(String),
    #[error("failed to parse import file: {0}")]
    ParseImportFileError(String),
    #[error("failed to write failed items file: {0}")]
    WriteFailedItemsError(String),
    #[error("export failed: {0}")]
    ExportError(String),
    #[error("no quotes found in the knowledge base")]
    QuoteNotFound,
    #[error("quote error: {0}")]
    QuoteError(String),
    #[error("missing required field: {0}")]
    MissingRequiredField(String),
    #[error("interactive input error: {0}")]
    InteractiveInputError(String),
    #[error("parent KB not found")]
    ParentKBNotFound,
    #[error("KB has children, delete them first: {0}")]
    KBHasChildrenError(String),
    #[error("invalid path: {0}")]
    InvalidPathError(String),
    #[error("media download failed: {0}")]
    MediaDownloadError(String),
    #[error("media file not found: {0}")]
    MediaFileNotFoundError(String),
    #[error("media copy failed: {0}")]
    MediaCopyError(String),
    #[error("media delete failed: {0}")]
    MediaDeleteError(String),
    #[error("media URL is required: {0}")]
    MediaUrlRequired(String),
    #[error("path update not allowed for media entries: {0}")]
    MediaPathUpdateNotAllowed(String),
    #[error("edge already exists between these entries")]
    DuplicateEdgeError,
    #[error("edge not found")]
    EdgeNotFound,
    #[error("an entry cannot be linked to itself")]
    SelfLoopNotAllowed,
    #[error("unable to add edge: {0}")]
    AddEdgeError(String),
    #[error("unable to remove edge: {0}")]
    RemoveEdgeError(String),
    #[error("graph query failed: {0}")]
    GraphQueryError(String),
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
