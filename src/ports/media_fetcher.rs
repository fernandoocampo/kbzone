use std::fmt::Debug;

use crate::errors::Error;

/// Outbound port for downloading remote media files.
/// Implementations fetch a URL and write it to a temporary local file.
pub trait MediaFetcher: Debug + Clone {
    /// Downloads the resource at `url` to a temporary file.
    /// Returns the path of the temporary file on success.
    fn fetch(&self, url: &str) -> Result<String, Error>;
}
