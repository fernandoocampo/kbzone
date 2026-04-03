use std::fmt;

use uuid::Uuid;

use crate::domain::file_extension;
use crate::errors::Error;
use crate::ports::MediaFetcher;

/// HTTP-backed implementation of [`MediaFetcher`].
/// Downloads a URL to a temporary file in the OS temp directory.
#[derive(Clone)]
pub struct HttpMediaFetcher;

impl fmt::Debug for HttpMediaFetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpMediaFetcher").finish()
    }
}

impl MediaFetcher for HttpMediaFetcher {
    fn fetch(&self, url: &str) -> Result<String, Error> {
        let response = ureq::get(url)
            .call()
            .map_err(|e| Error::MediaDownloadError(e.to_string()))?;

        let ext = file_extension(url).unwrap_or_else(|| "tmp".to_string());
        let tmp_path = std::env::temp_dir().join(format!("{}.{}", Uuid::new_v4(), ext));

        let mut file = std::fs::File::create(&tmp_path)
            .map_err(|e| Error::MediaDownloadError(e.to_string()))?;

        let mut reader = response.into_reader();
        std::io::copy(&mut reader, &mut file)
            .map_err(|e| Error::MediaDownloadError(e.to_string()))?;

        Ok(tmp_path.to_string_lossy().to_string())
    }
}
