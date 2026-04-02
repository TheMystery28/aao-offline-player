/// Unified error type for all Tauri commands and internal functions.
///
/// Replaces `.map_err(|e| format!(...))` boilerplate across the codebase.
/// Serialized as a plain string to the frontend (same contract as the previous
/// `Result<T, String>` pattern — no JS changes needed).
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Database error: {0}")]
    Db(#[from] redb::Error),

    #[error("Database storage error: {0}")]
    DbStorage(#[from] redb::StorageError),

    #[error("Database table error: {0}")]
    DbTable(#[from] redb::TableError),

    #[error("Database commit error: {0}")]
    DbCommit(#[from] redb::CommitError),

    #[error("Database transaction error: {0}")]
    DbTransaction(#[from] redb::TransactionError),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),

    /// Catch-all for contextual error messages and gradual migration.
    #[error("{0}")]
    Other(String),
}

/// Required by Tauri — command error types must implement serde::Serialize.
/// Serializes as a plain string (same format the frontend already expects).
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

/// Allow `?` from String errors (gradual migration support).
impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Other(s)
    }
}

/// Bridge: convert existing DownloaderError to AppError at module boundaries.
impl From<crate::downloader::DownloaderError> for AppError {
    fn from(e: crate::downloader::DownloaderError) -> Self {
        AppError::Other(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_serializes_as_string() {
        let err = AppError::Other("Something went wrong".to_string());
        let json = serde_json::to_value(&err).unwrap();
        assert!(json.is_string());
        assert_eq!(json.as_str().unwrap(), "Something went wrong");
    }

    #[test]
    fn test_io_error_serialization() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = AppError::from(io_err);
        let json = serde_json::to_value(&err).unwrap();
        assert!(json.is_string());
        assert!(json.as_str().unwrap().contains("file missing"));
    }

    #[test]
    fn test_json_error_serialization() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err = AppError::from(json_err);
        let json = serde_json::to_value(&err).unwrap();
        assert!(json.is_string());
        assert!(json.as_str().unwrap().contains("JSON error"));
    }

    #[test]
    fn test_from_string() {
        let err: AppError = "custom error".to_string().into();
        assert_eq!(err.to_string(), "custom error");
    }

    #[test]
    fn test_from_downloader_error() {
        let dl_err = crate::downloader::DownloaderError::Other("download failed".to_string());
        let err: AppError = dl_err.into();
        assert_eq!(err.to_string(), "download failed");
    }

    #[test]
    fn test_serialization_matches_string_contract() {
        let msg = "Failed to read file: No such file or directory";
        let string_json = serde_json::to_value(msg).unwrap();
        let app_json = serde_json::to_value(&AppError::Other(msg.to_string())).unwrap();
        assert_eq!(string_json, app_json);
    }
}
