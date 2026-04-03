use std::fmt;
use std::path::Path;

use crate::domain::StoreMediaParams;
use crate::errors::Error;
use crate::ports::MediaStore;

/// Filesystem-backed implementation of [`MediaStore`].
/// Copies local files to the media directory and deletes them on removal.
#[derive(Clone)]
pub struct FileSystemMediaStore;

impl fmt::Debug for FileSystemMediaStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSystemMediaStore").finish()
    }
}

fn expand_tilde(path: &str) -> String {
    if path == "~" {
        return std::env::var("HOME").unwrap_or_else(|_| path.to_string());
    }
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}/{}", home, rest);
        }
    }
    path.to_string()
}

impl MediaStore for FileSystemMediaStore {
    fn store_media(&self, params: &StoreMediaParams) -> Result<String, Error> {
        let expanded = expand_tilde(&params.source);
        let source = Path::new(&expanded);
        let destination = Path::new(&params.destination);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::MediaCopyError(e.to_string()))?;
        }
        std::fs::copy(source, destination).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::MediaFileNotFoundError(params.source.clone())
            } else {
                Error::MediaCopyError(e.to_string())
            }
        })?;
        Ok(params.destination.clone())
    }

    fn delete_media(&self, path: &str) -> Result<(), Error> {
        std::fs::remove_file(path).map_err(|e| Error::MediaDeleteError(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_media_copies_file_to_destination() {
        let source_dir = tempfile::tempdir().expect("temp dir");
        let source_path = source_dir.path().join("source.jpg");
        std::fs::write(&source_path, b"fake image data").expect("write source");

        let dest_dir = tempfile::tempdir().expect("temp dir");
        let dest_path = dest_dir.path().join("subdir").join("dest.jpg");

        let store = FileSystemMediaStore;
        let params = StoreMediaParams {
            source: source_path.to_string_lossy().to_string(),
            destination: dest_path.to_string_lossy().to_string(),
        };

        let result = store.store_media(&params);
        assert!(result.is_ok());
        assert!(dest_path.exists());
        assert_eq!(std::fs::read(dest_path).unwrap(), b"fake image data");
    }

    #[test]
    fn store_media_returns_error_when_source_missing() {
        let store = FileSystemMediaStore;
        let params = StoreMediaParams {
            source: "/nonexistent/path/file.jpg".to_string(),
            destination: "/tmp/dest.jpg".to_string(),
        };
        let result = store.store_media(&params);
        assert!(matches!(result, Err(Error::MediaFileNotFoundError(_))));
    }

    #[test]
    fn store_media_creates_parent_directories() {
        let source_dir = tempfile::tempdir().expect("temp dir");
        let source_path = source_dir.path().join("file.png");
        std::fs::write(&source_path, b"data").expect("write source");

        let dest_dir = tempfile::tempdir().expect("temp dir");
        let dest_path = dest_dir
            .path()
            .join("a")
            .join("b")
            .join("c")
            .join("file.png");

        let store = FileSystemMediaStore;
        let params = StoreMediaParams {
            source: source_path.to_string_lossy().to_string(),
            destination: dest_path.to_string_lossy().to_string(),
        };

        assert!(store.store_media(&params).is_ok());
        assert!(dest_path.exists());
    }

    #[test]
    fn delete_media_removes_existing_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file_path = dir.path().join("file.mp3");
        std::fs::write(&file_path, b"audio").expect("write file");

        let store = FileSystemMediaStore;
        assert!(store.delete_media(&file_path.to_string_lossy()).is_ok());
        assert!(!file_path.exists());
    }

    #[test]
    fn delete_media_returns_error_for_missing_file() {
        let store = FileSystemMediaStore;
        let result = store.delete_media("/nonexistent/path/file.mp3");
        assert!(matches!(result, Err(Error::MediaDeleteError(_))));
    }
}
