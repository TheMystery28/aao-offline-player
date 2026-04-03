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
    /// Whether the one-time localStorage migration from http://localhost to aao:// has completed.
    #[serde(default)]
    pub migration_complete: bool,
    /// Unix timestamp (seconds since epoch) of the last successful Optimize & Fix run.
    /// None means it has never been run.
    #[serde(default)]
    pub last_optimized_at: Option<u64>,
    /// Selected UI theme name ("default", "gba", "ds").
    #[serde(default = "default_theme")]
    pub theme: String,
    /// User-supplied CSS injected into the launcher UI after the theme preset.
    #[serde(default)]
    pub custom_css: String,
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
fn default_theme() -> String {
    "default".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            concurrent_downloads: default_concurrent_downloads(),
            auto_save: default_auto_save(),
            blur_spoilers: default_blur_spoilers(),
            migration_complete: false,
            last_optimized_at: None,
            theme: default_theme(),
            custom_css: String::new(),
        }
    }
}

const VALID_LANGUAGES: &[&str] = &["en", "fr", "de", "es"];
const VALID_THEMES: &[&str] = &["default", "gba", "ds"];

/// Validate and clamp config values to acceptable ranges.
pub fn validate(config: &mut AppConfig) {
    config.concurrent_downloads = config.concurrent_downloads.clamp(1, 10);
    if !VALID_LANGUAGES.contains(&config.language.as_str()) {
        config.language = default_language();
    }
    if !VALID_THEMES.contains(&config.theme.as_str()) {
        config.theme = default_theme();
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
pub fn save_config(data_dir: &Path, config: &AppConfig) -> Result<(), crate::error::AppError> {
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
    pub cases_assets_bytes: u64,
    pub cases_metadata_bytes: u64,
    pub cases_plugins_bytes: u64,
    pub defaults_size_bytes: u64,
    pub defaults_sprites_bytes: u64,
    pub defaults_music_bytes: u64,
    pub defaults_sounds_bytes: u64,
    pub defaults_voices_bytes: u64,
    pub defaults_shared_bytes: u64,
    pub defaults_shared_count: usize,
    pub defaults_shared_images_bytes: u64,
    pub defaults_shared_audio_bytes: u64,
    pub defaults_shared_other_bytes: u64,
    pub defaults_other_bytes: u64,
    pub total_size_bytes: u64,
}

/// Compute storage usage for cases and default asset cache.
pub fn compute_storage_info(engine_dir: &Path) -> StorageInfo {
    let cases_dir = engine_dir.join("case");
    let defaults_dir = engine_dir.join("defaults");

    let (cases_count, cases_size, cases_assets, cases_metadata, cases_plugins) = if cases_dir.exists() {
        count_cases_and_size(&cases_dir)
    } else {
        (0, 0u64, 0u64, 0u64, 0u64)
    };

    // Break down defaults by category
    let mut sprites: u64 = 0;
    let mut music: u64 = 0;
    let mut sounds: u64 = 0;
    let mut voices: u64 = 0;
    let mut shared: u64 = 0;
    let mut shared_count: usize = 0;
    let mut shared_images: u64 = 0;
    let mut shared_audio: u64 = 0;
    let mut shared_other_sub: u64 = 0;
    let mut other: u64 = 0;

    if defaults_dir.exists() {
        let images_dir = defaults_dir.join("images");
        if images_dir.exists() {
            sprites = dir_size(&images_dir);
        }
        let music_dir = defaults_dir.join("music");
        if music_dir.exists() {
            music = dir_size(&music_dir);
        }
        let sounds_dir = defaults_dir.join("sounds");
        if sounds_dir.exists() {
            sounds = dir_size(&sounds_dir);
        }
        let voices_dir = defaults_dir.join("voices");
        if voices_dir.exists() {
            voices = dir_size(&voices_dir);
        }
        let shared_dir = defaults_dir.join("shared");
        if shared_dir.exists() {
            shared = dir_size(&shared_dir);
            classify_shared(&shared_dir, &mut shared_count, &mut shared_images, &mut shared_audio, &mut shared_other_sub);
        }
        let known = sprites + music + sounds + voices + shared;
        let total_defaults = dir_size(&defaults_dir);
        other = total_defaults.saturating_sub(known);
    }

    let defaults_size = sprites + music + sounds + voices + shared + other;

    StorageInfo {
        data_dir: engine_dir.to_string_lossy().to_string(),
        cases_count,
        cases_size_bytes: cases_size,
        cases_assets_bytes: cases_assets,
        cases_metadata_bytes: cases_metadata,
        cases_plugins_bytes: cases_plugins,
        defaults_size_bytes: defaults_size,
        defaults_sprites_bytes: sprites,
        defaults_music_bytes: music,
        defaults_sounds_bytes: sounds,
        defaults_voices_bytes: voices,
        defaults_shared_bytes: shared,
        defaults_shared_count: shared_count,
        defaults_shared_images_bytes: shared_images,
        defaults_shared_audio_bytes: shared_audio,
        defaults_shared_other_bytes: shared_other_sub,
        defaults_other_bytes: other,
        total_size_bytes: cases_size + defaults_size,
    }
}

/// Recursively classify files under `dir` into images, audio, or other size buckets.
/// Used by [`compute_storage_info`] to break down the shared defaults directory.
fn classify_shared(
    dir: &Path,
    count: &mut usize,
    images: &mut u64,
    audio: &mut u64,
    other: &mut u64,
) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                classify_shared(&path, count, images, audio, other);
            } else if path.is_file() {
                let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                *count += 1;
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                match ext.as_str() {
                    "gif" | "png" | "jpg" | "jpeg" | "webp" | "bmp" | "svg" => *images += size,
                    "mp3" | "ogg" | "opus" | "wav" | "m4a" | "flac" | "aac" => *audio += size,
                    _ => *other += size,
                }
            }
        }
    }
}

fn count_cases_and_size(cases_dir: &Path) -> (usize, u64, u64, u64, u64) {
    let mut count = 0usize;
    let mut size = 0u64;
    let mut assets_total = 0u64;
    let mut metadata_total = 0u64;
    let mut plugins_total = 0u64;
    if let Ok(entries) = std::fs::read_dir(cases_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("manifest.json").exists() {
                count += 1;
                let case_size = dir_size(&path);
                size += case_size;
                let assets_dir = path.join("assets");
                let a = if assets_dir.exists() { dir_size(&assets_dir) } else { 0 };
                let plugins_dir = path.join("plugins");
                let p = if plugins_dir.exists() { dir_size(&plugins_dir) } else { 0 };
                assets_total += a;
                plugins_total += p;
                metadata_total += case_size.saturating_sub(a + p);
            }
        }
    }
    (count, size, assets_total, metadata_total, plugins_total)
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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

    /// migration_complete defaults to false for new installs and existing configs.
    #[test]
    fn test_migration_complete_defaults_false() {
        let config = AppConfig::default();
        assert!(!config.migration_complete);
    }

    /// migration_complete: existing config.json without this field defaults to false.
    #[test]
    fn test_migration_complete_missing_from_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.json"), r#"{"language":"en"}"#).unwrap();
        let config = load_config(dir.path());
        assert!(!config.migration_complete, "Old configs without migration_complete should default to false");
    }

    /// migration_complete roundtrips through save/load.
    #[test]
    fn test_migration_complete_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig {
            migration_complete: true,
            ..Default::default()
        };
        save_config(dir.path(), &config).unwrap();
        let loaded = load_config(dir.path());
        assert!(loaded.migration_complete);
    }

    // --- classify_shared regression tests ---
    // classify_shared runs only when defaults/shared/ exists.
    // None of the existing tests create that directory, so these tests
    // are the first to exercise every branch of the function.
    // They all go through compute_storage_info so they remain valid
    // regardless of whether classify_shared is nested or module-level.

    #[test]
    fn test_classify_shared_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("defaults/shared")).unwrap();
        let info = compute_storage_info(dir.path());
        assert_eq!(info.defaults_shared_count, 0);
        assert_eq!(info.defaults_shared_images_bytes, 0);
        assert_eq!(info.defaults_shared_audio_bytes, 0);
        assert_eq!(info.defaults_shared_other_bytes, 0);
        assert_eq!(info.defaults_shared_bytes, 0);
    }

    #[test]
    fn test_classify_shared_images() {
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("defaults/shared");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(shared.join("a.png"), vec![0u8; 100]).unwrap();
        std::fs::write(shared.join("b.gif"), vec![0u8; 200]).unwrap();
        std::fs::write(shared.join("c.jpg"), vec![0u8; 300]).unwrap();
        let info = compute_storage_info(dir.path());
        assert_eq!(info.defaults_shared_count, 3);
        assert_eq!(info.defaults_shared_images_bytes, 600);
        assert_eq!(info.defaults_shared_audio_bytes, 0);
        assert_eq!(info.defaults_shared_other_bytes, 0);
    }

    #[test]
    fn test_classify_shared_audio() {
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("defaults/shared");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(shared.join("track.mp3"), vec![0u8; 400]).unwrap();
        std::fs::write(shared.join("sound.ogg"), vec![0u8; 500]).unwrap();
        let info = compute_storage_info(dir.path());
        assert_eq!(info.defaults_shared_count, 2);
        assert_eq!(info.defaults_shared_images_bytes, 0);
        assert_eq!(info.defaults_shared_audio_bytes, 900);
        assert_eq!(info.defaults_shared_other_bytes, 0);
    }

    #[test]
    fn test_classify_shared_other() {
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("defaults/shared");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(shared.join("data.json"), vec![0u8; 150]).unwrap();
        let info = compute_storage_info(dir.path());
        assert_eq!(info.defaults_shared_count, 1);
        assert_eq!(info.defaults_shared_images_bytes, 0);
        assert_eq!(info.defaults_shared_audio_bytes, 0);
        assert_eq!(info.defaults_shared_other_bytes, 150);
    }

    #[test]
    fn test_classify_shared_recursive_subdirs() {
        // classify_shared must recurse into subdirectories and count all files.
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("defaults/shared");
        let sub = shared.join("chars");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(shared.join("root.png"), vec![0u8; 100]).unwrap();
        std::fs::write(sub.join("char1.png"), vec![0u8; 200]).unwrap();
        std::fs::write(sub.join("char2.gif"), vec![0u8; 300]).unwrap();
        let info = compute_storage_info(dir.path());
        assert_eq!(info.defaults_shared_count, 3);
        assert_eq!(info.defaults_shared_images_bytes, 600);
        assert_eq!(info.defaults_shared_audio_bytes, 0);
        assert_eq!(info.defaults_shared_other_bytes, 0);
    }

    #[test]
    fn test_classify_shared_mixed_types() {
        // All three categories present at once; totals must match dir_size.
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("defaults/shared");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(shared.join("img.png"), vec![0u8; 100]).unwrap();
        std::fs::write(shared.join("aud.mp3"), vec![0u8; 200]).unwrap();
        std::fs::write(shared.join("misc.dat"), vec![0u8; 50]).unwrap();
        let info = compute_storage_info(dir.path());
        assert_eq!(info.defaults_shared_count, 3);
        assert_eq!(info.defaults_shared_images_bytes, 100);
        assert_eq!(info.defaults_shared_audio_bytes, 200);
        assert_eq!(info.defaults_shared_other_bytes, 50);
        // The three buckets must sum to the total shared byte count.
        assert_eq!(
            info.defaults_shared_bytes,
            info.defaults_shared_images_bytes
                + info.defaults_shared_audio_bytes
                + info.defaults_shared_other_bytes,
        );
    }

    #[test]
    fn test_classify_shared_all_image_extensions() {
        // Every image extension in the match arm must be classified correctly.
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("defaults/shared");
        std::fs::create_dir_all(&shared).unwrap();
        for ext in &["png", "gif", "jpg", "jpeg", "webp", "bmp", "svg"] {
            std::fs::write(shared.join(format!("f.{ext}")), vec![1u8; 10]).unwrap();
        }
        let info = compute_storage_info(dir.path());
        assert_eq!(info.defaults_shared_count, 7);
        assert_eq!(info.defaults_shared_images_bytes, 70);
        assert_eq!(info.defaults_shared_audio_bytes, 0);
        assert_eq!(info.defaults_shared_other_bytes, 0);
    }

    #[test]
    fn test_classify_shared_all_audio_extensions() {
        // Every audio extension in the match arm must be classified correctly.
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("defaults/shared");
        std::fs::create_dir_all(&shared).unwrap();
        for ext in &["mp3", "ogg", "opus", "wav", "m4a", "flac", "aac"] {
            std::fs::write(shared.join(format!("f.{ext}")), vec![1u8; 10]).unwrap();
        }
        let info = compute_storage_info(dir.path());
        assert_eq!(info.defaults_shared_count, 7);
        assert_eq!(info.defaults_shared_images_bytes, 0);
        assert_eq!(info.defaults_shared_audio_bytes, 70);
        assert_eq!(info.defaults_shared_other_bytes, 0);
    }

    // --- theme field regression tests ---
    // These tests are written BEFORE the `theme` field is added to AppConfig.
    // They will fail to compile until the field is added, confirming we are in
    // the TDD red phase. Once the field and validation are implemented, they pass.

    #[test]
    fn test_theme_defaults_to_default() {
        let config = AppConfig::default();
        assert_eq!(config.theme, "default");
    }

    #[test]
    fn test_theme_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig {
            theme: "gba".to_string(),
            ..Default::default()
        };
        save_config(dir.path(), &config).unwrap();
        let loaded = load_config(dir.path());
        assert_eq!(loaded.theme, "gba");
    }

    #[test]
    fn test_theme_missing_from_json_defaults_to_default() {
        // Old config.json without the theme field must deserialize to "default".
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.json"), r#"{"language":"en"}"#).unwrap();
        let config = load_config(dir.path());
        assert_eq!(config.theme, "default");
    }

    #[test]
    fn test_validate_rejects_invalid_theme() {
        let mut config = AppConfig {
            theme: "dark_mode".to_string(),
            ..Default::default()
        };
        validate(&mut config);
        assert_eq!(config.theme, "default");
    }

    #[test]
    fn test_validate_accepts_valid_themes() {
        for name in &["default", "gba", "ds"] {
            let mut config = AppConfig {
                theme: name.to_string(),
                ..Default::default()
            };
            validate(&mut config);
            assert_eq!(&config.theme, name);
        }
    }

    // --- custom_css regression test ---
    // Written before the field is added: old config.json without custom_css must
    // deserialize to an empty string (serde default).

    #[test]
    fn test_custom_css_missing_from_json_defaults_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.json"), r#"{"language":"en"}"#).unwrap();
        let config = load_config(dir.path());
        assert_eq!(config.custom_css, "");
    }

    #[test]
    fn test_custom_css_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig {
            custom_css: ":root { --bg-body: #ff0000; }".to_string(),
            ..Default::default()
        };
        save_config(dir.path(), &config).unwrap();
        let loaded = load_config(dir.path());
        assert_eq!(loaded.custom_css, ":root { --bg-body: #ff0000; }");
    }
}
