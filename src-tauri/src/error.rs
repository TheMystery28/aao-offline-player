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

    /// HTTP error with a non-success status code, at the URL that failed.
    #[error("HTTP {status}")]
    HttpStatus { status: u16, url: String },

    /// Request timed out (message keeps the historical "timeout: " prefix).
    #[error("timeout: {0}")]
    Timeout(String),

    /// Connection-level failure (message keeps the historical "connection error: " prefix).
    #[error("connection error: {0}")]
    Network(String),

    /// Operation was cancelled by the user.
    #[error("Cancelled")]
    Cancelled,

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

impl AppError {
    /// Stable machine-readable classifier for the error.
    /// The frontend can branch on this without parsing message strings.
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Io(_) => "io",
            AppError::Json(_) => "json",
            AppError::Zip(_) => "zip",
            AppError::Db(_) => "db",
            AppError::DbStorage(_) => "db_storage",
            AppError::DbTable(_) => "db_table",
            AppError::DbCommit(_) => "db_commit",
            AppError::DbTransaction(_) => "db_transaction",
            AppError::Http(_) => "http",
            AppError::Regex(_) => "regex",
            AppError::Url(_) => "url",
            AppError::HttpStatus { .. } => "http_status",
            AppError::Timeout(_) => "timeout",
            AppError::Network(_) => "network",
            AppError::Cancelled => "cancelled",
            AppError::Other(_) => "other",
        }
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
    fn test_serialization_matches_string_contract() {
        let msg = "Failed to read file: No such file or directory";
        let string_json = serde_json::to_value(msg).unwrap();
        let app_json = serde_json::to_value(&AppError::Other(msg.to_string())).unwrap();
        assert_eq!(string_json, app_json);
    }

    #[test]
    fn test_http_status_variant_message_and_kind() {
        let err = AppError::HttpStatus { status: 404, url: "https://aaonline.fr/x.png".to_string() };
        assert_eq!(err.to_string(), "HTTP 404", "message must match the old Other(...) text");
        assert_eq!(err.kind(), "http_status");
    }

    #[test]
    fn test_timeout_variant_message_and_kind() {
        let err = AppError::Timeout("operation timed out".to_string());
        assert!(err.to_string().starts_with("timeout: "), "got: {}", err);
        assert_eq!(err.kind(), "timeout");
    }

    #[test]
    fn test_network_variant_message_and_kind() {
        let err = AppError::Network("connection refused".to_string());
        assert_eq!(err.to_string(), "connection error: connection refused");
        assert_eq!(err.kind(), "network");
    }

    #[test]
    fn test_cancelled_variant() {
        let err = AppError::Cancelled;
        assert_eq!(err.to_string(), "Cancelled");
        assert_eq!(err.kind(), "cancelled");
    }

    #[test]
    fn test_kind_covers_all_variants() {
        // Every variant whose inner error has a public constructor.
        // reqwest::Error and the redb error types have NO public constructor,
        // so their kind() arms are guaranteed by the exhaustive match in kind()
        // (the compiler rejects a missing arm).
        let cases: Vec<AppError> = vec![
            AppError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "x")),
            AppError::Json(serde_json::from_str::<serde_json::Value>("nope").unwrap_err()),
            AppError::Zip(zip::result::ZipError::FileNotFound),
            AppError::Regex(regex::Regex::new("(").unwrap_err()),
            AppError::Url(url::Url::parse("http://[::1").unwrap_err()),
            AppError::HttpStatus { status: 500, url: "u".to_string() },
            AppError::Timeout("t".to_string()),
            AppError::Network("n".to_string()),
            AppError::Cancelled,
            AppError::Other("misc".to_string()),
        ];
        for err in &cases {
            assert!(!err.kind().is_empty(), "kind() must never return empty for {:?}", err);
        }
        assert_eq!(AppError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "x")).kind(), "io");
        assert_eq!(AppError::HttpStatus { status: 404, url: "u".to_string() }.kind(), "http_status");
        assert_eq!(AppError::Timeout("t".to_string()).kind(), "timeout");
        assert_eq!(AppError::Network("n".to_string()).kind(), "network");
        assert_eq!(AppError::Other("x".into()).kind(), "other");
        assert_eq!(AppError::Cancelled.kind(), "cancelled");
    }
}
