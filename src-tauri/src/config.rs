use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// User-configurable app settings, persisted as config.json in the data directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Player UI language code (en, fr, de, es).
    #[serde(default = "default_language")]
    pub language: String,
    /// Number of concurrent asset downloads (1-10).
    #[serde(default = "default_concurrent_downloads")]
    pub concurrent_downloads: usize,
    /// Automatically save game progress when leaving the player.
    #[serde(default = "default_auto_save")]
    pub auto_save: bool,
    /// Blur asset filenames in download progress to avoid spoilers.
    #[serde(default = "default_blur_spoilers")]
    pub blur_spoilers: bool,
    /// Keep shared default assets (sprites, backgrounds, sounds) in cache even when
    /// no downloaded case references them. When false (default), deleting the last
    /// case that uses a shared asset automatically cleans it up.
    #[serde(default = "default_keep_shared_assets")]
    pub keep_shared_assets: bool,
}

fn default_language() -> String {
    "en".to_string()
}
fn default_concurrent_downloads() -> usize {
    3
}
fn default_auto_save() -> bool {
    false
}
fn default_blur_spoilers() -> bool {
    true
}
fn default_keep_shared_assets() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            concurrent_downloads: default_concurrent_downloads(),
            auto_save: default_auto_save(),
            blur_spoilers: default_blur_spoilers(),
            keep_shared_assets: default_keep_shared_assets(),
        }
    }
}

const VALID_LANGUAGES: &[&str] = &["en", "fr", "de", "es"];

/// Validate and clamp config values to acceptable ranges.
pub fn validate(config: &mut AppConfig) {
    config.concurrent_downloads = config.concurrent_downloads.clamp(1, 10);
    if !VALID_LANGUAGES.contains(&config.language.as_str()) {
        config.language = default_language();
    }
}

fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("config.json")
}

/// Load config from disk. Returns default if file missing or corrupt.
pub fn load_config(data_dir: &Path) -> AppConfig {
    let path = config_path(data_dir);
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(data) => {
                let mut config: AppConfig =
                    serde_json::from_str(&data).unwrap_or_default();
                validate(&mut config);
                config
            }
            Err(_) => AppConfig::default(),
        }
    } else {
        AppConfig::default()
    }
}

/// Persist config to disk.
pub fn save_config(data_dir: &Path, config: &AppConfig) -> Result<(), String> {
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(config_path(data_dir), json)
        .map_err(|e| format!("Failed to write config: {}", e))?;
    Ok(())
}

/// Storage usage summary for the UI.
#[derive(Debug, Serialize)]
pub struct StorageInfo {
    pub data_dir: String,
    pub cases_count: usize,
    pub cases_size_bytes: u64,
    pub defaults_size_bytes: u64,
    pub total_size_bytes: u64,
}

/// Compute storage usage for cases and default asset cache.
pub fn compute_storage_info(engine_dir: &Path) -> StorageInfo {
    let cases_dir = engine_dir.join("case");
    let defaults_dir = engine_dir.join("defaults");

    let (cases_count, cases_size) = if cases_dir.exists() {
        count_cases_and_size(&cases_dir)
    } else {
        (0, 0)
    };

    let defaults_size = if defaults_dir.exists() {
        dir_size(&defaults_dir)
    } else {
        0
    };

    StorageInfo {
        data_dir: engine_dir.to_string_lossy().to_string(),
        cases_count,
        cases_size_bytes: cases_size,
        defaults_size_bytes: defaults_size,
        total_size_bytes: cases_size + defaults_size,
    }
}

fn count_cases_and_size(cases_dir: &Path) -> (usize, u64) {
    let mut count = 0usize;
    let mut size = 0u64;
    if let Ok(entries) = std::fs::read_dir(cases_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("manifest.json").exists() {
                count += 1;
                size += dir_size(&path);
            }
        }
    }
    (count, size)
}

/// Recursively compute total file size of a directory.
pub fn dir_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                total += std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            } else if path.is_dir() {
                total += dir_size(&path);
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.language, "en");
        assert_eq!(config.concurrent_downloads, 3);
    }

    #[test]
    fn test_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig {
            language: "fr".to_string(),
            concurrent_downloads: 5,
            auto_save: true,
            blur_spoilers: false,
            keep_shared_assets: false,
        };
        save_config(dir.path(), &config).unwrap();
        let loaded = load_config(dir.path());
        assert_eq!(loaded.language, "fr");
        assert_eq!(loaded.concurrent_downloads, 5);
    }

    #[test]
    fn test_load_config_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = load_config(dir.path());
        assert_eq!(config.language, "en");
        assert_eq!(config.concurrent_downloads, 3);
    }

    #[test]
    fn test_load_config_partial_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.json"), r#"{"language":"de"}"#).unwrap();
        let config = load_config(dir.path());
        assert_eq!(config.language, "de");
        assert_eq!(config.concurrent_downloads, 3);
    }

    #[test]
    fn test_validate_clamps_concurrency() {
        let mut config = AppConfig {
            language: "en".to_string(),
            concurrent_downloads: 99,
            auto_save: true,
            blur_spoilers: true,
            keep_shared_assets: false,
        };
        validate(&mut config);
        assert_eq!(config.concurrent_downloads, 10);

        config.concurrent_downloads = 0;
        validate(&mut config);
        assert_eq!(config.concurrent_downloads, 1);
    }

    #[test]
    fn test_validate_rejects_invalid_language() {
        let mut config = AppConfig {
            language: "xx".to_string(),
            concurrent_downloads: 3,
            auto_save: true,
            blur_spoilers: true,
            keep_shared_assets: false,
        };
        validate(&mut config);
        assert_eq!(config.language, "en");
    }

    #[test]
    fn test_storage_info_empty() {
        let dir = tempfile::tempdir().unwrap();
        let info = compute_storage_info(dir.path());
        assert_eq!(info.cases_count, 0);
        assert_eq!(info.cases_size_bytes, 0);
        assert_eq!(info.defaults_size_bytes, 0);
    }

    #[test]
    fn test_storage_info_with_data() {
        let dir = tempfile::tempdir().unwrap();
        let case_dir = dir.path().join("case/123");
        std::fs::create_dir_all(&case_dir).unwrap();
        std::fs::write(case_dir.join("manifest.json"), "{}").unwrap();
        std::fs::write(case_dir.join("trial_data.json"), "x".repeat(1000)).unwrap();

        let defaults_dir = dir.path().join("defaults/images");
        std::fs::create_dir_all(&defaults_dir).unwrap();
        std::fs::write(defaults_dir.join("test.gif"), "y".repeat(500)).unwrap();

        let info = compute_storage_info(dir.path());
        assert_eq!(info.cases_count, 1);
        assert!(info.cases_size_bytes > 0);
        assert!(info.defaults_size_bytes > 0);
        assert_eq!(
            info.total_size_bytes,
            info.cases_size_bytes + info.defaults_size_bytes
        );
    }
}
