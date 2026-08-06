//! Parsed default-data tables from `engine/Javascript/default_data.js`, cached
//! per engine directory so downloads do not read+regex-parse the file 3×.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use regex::Regex;
use serde_json::Value;

static DEFAULT_PLACES_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"defaults/images/defaultplaces/(backgrounds|foreground_objects)/([^"]+)"#)
        .expect("DEFAULT_PLACES_PATH_RE pattern is valid")
});

/// All tables the downloader needs from `default_data.js`, in one parse.
#[derive(Debug, Default)]
pub struct DefaultData {
    pub profiles_nb: HashMap<String, u32>,
    pub profiles_startup: HashSet<String>,
    /// (category, filename) — "backgrounds" | "foreground_objects"
    pub place_files: Vec<(String, String)>,
}

impl DefaultData {
    /// Parse `default_data.js` from the engine dir. Missing/unreadable file
    /// yields an empty default (callers skip sprite/place extraction then).
    pub fn load(engine_dir: &Path) -> Self {
        let content = match std::fs::read_to_string(engine_dir.join("Javascript").join("default_data.js"))
        {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };

        let profiles_nb = parse_js_object_assignment(&content, "default_profiles_nb")
            .and_then(|json| serde_json::from_str::<HashMap<String, u32>>(&json).ok())
            .unwrap_or_default();

        let profiles_startup = parse_js_object_assignment(&content, "default_profiles_startup")
            .and_then(|json| serde_json::from_str::<HashMap<String, Value>>(&json).ok())
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default();

        let place_files = DEFAULT_PLACES_PATH_RE
            .captures_iter(&content)
            .map(|cap| (cap[1].to_string(), cap[2].to_string()))
            .collect();

        Self { profiles_nb, profiles_startup, place_files }
    }

    /// Process-wide cache keyed by engine dir (bounded to 4 entries).
    pub fn for_engine(engine_dir: &Path) -> Arc<Self> {
        static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<DefaultData>>>> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let key = engine_dir.to_path_buf();
        let Ok(mut guard) = cache.lock() else {
            // Poisoned — fall back to an uncached parse
            return Arc::new(Self::load(engine_dir));
        };
        if let Some(entry) = guard.get(&key) {
            return Arc::clone(entry);
        }
        let loaded = Arc::new(Self::load(engine_dir));
        if guard.len() >= 4 {
            guard.clear();
        }
        guard.insert(key, Arc::clone(&loaded));
        loaded
    }
}

/// Extract the JSON object from a `var name = {...};` assignment in JS source.
pub(crate) fn parse_js_object_assignment(source: &str, var_name: &str) -> Option<String> {
    let prefix = format!("var {} = ", var_name);
    let start = source.find(&prefix)?;
    let rest = &source[start + prefix.len()..];
    let obj_start = rest.find('{')?;
    let mut depth = 0;
    for (i, ch) in rest[obj_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(rest[obj_start..obj_start + i + 1].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let js_dir = dir.path().join("Javascript");
        std::fs::create_dir_all(&js_dir).unwrap();
        std::fs::write(
            js_dir.join("default_data.js"),
            r#"var default_profiles_nb = {"Juge2": 6, "Phoenix": 20, "Inconnu": 0};
var default_profiles_startup = {"Juge2/2": 880, "Phoenix/1": 900};
var default_places = {"backgrounds": {"pw_courtroom": "defaults/images/defaultplaces/backgrounds/pw_courtroom.jpg"}};"#,
        ).unwrap();
        dir
    }

    #[test]
    fn test_load_parses_all_sections() {
        let dir = fixture_dir();
        let data = DefaultData::load(dir.path());
        assert_eq!(data.profiles_nb["Juge2"], 6);
        assert_eq!(data.profiles_nb["Inconnu"], 0);
        assert!(data.profiles_startup.contains("Juge2/2"));
        assert!(!data.profiles_startup.contains("Juge2/3"));
    }

    #[test]
    fn test_load_missing_file_is_empty_default() {
        let dir = tempfile::tempdir().unwrap();
        let data = DefaultData::load(dir.path());
        assert!(data.profiles_nb.is_empty());
        assert!(data.profiles_startup.is_empty());
        assert!(data.place_files.is_empty());
    }

    #[test]
    fn test_for_engine_caches_same_arc() {
        let dir = fixture_dir();
        let a = DefaultData::for_engine(dir.path());
        let b = DefaultData::for_engine(dir.path());
        assert!(Arc::ptr_eq(&a, &b), "second load for same dir must reuse the cached Arc");
        let other = tempfile::tempdir().unwrap();
        let c = DefaultData::for_engine(other.path());
        assert!(!Arc::ptr_eq(&a, &c), "different dirs must not share");
    }

    // --- parse_js_object_assignment (moved from asset_resolver/tests/test_defaults.rs) ---

    #[test]
    fn test_parse_js_object_assignment_simple() {
        let src = r#"var foo = {"a": 1, "b": 2};"#;
        let result = parse_js_object_assignment(src, "foo");
        assert_eq!(result.unwrap(), r#"{"a": 1, "b": 2}"#);
    }

    #[test]
    fn test_parse_js_object_assignment_not_found() {
        let src = "var foo = 42;";
        assert!(parse_js_object_assignment(src, "bar").is_none());
    }
}
