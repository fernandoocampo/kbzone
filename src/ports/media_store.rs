use std::fmt::Debug;

use crate::domain::StoreMediaParams;
use crate::errors::Error;

/// Outbound port for local media file operations.
/// Implementations handle copying files to the media directory and deleting them.
pub trait MediaStore: Debug + Clone {
    /// Copies a local file from `params.source` to `params.destination`.
    /// Creates parent directories as needed. Returns the destination path.
    fn store_media(&self, params: &StoreMediaParams) -> Result<String, Error>;

    /// Deletes the media file at the given path.
    fn delete_media(&self, path: &str) -> Result<(), Error>;
}
