use std::collections::HashSet;

use super::super::{AssetRef, AAONLINE_BASE};

/// Canonical path normalization: NFC unicode, forward slashes, `.`/`..` resolution,
/// Windows-illegal character replacement. Delegates to `paths::normalize_path`.
pub use super::super::paths::normalize_path as sanitize_path;

/// Offline cfg paths (must match engine/bridge.js cfg).
pub(super) struct LocalPaths;
impl LocalPaths {
    pub(super) fn icon() -> &'static str { "defaults/images/chars/" }
    pub(super) fn talking() -> &'static str { "defaults/images/chars/" }
    pub(super) fn still() -> &'static str { "defaults/images/charsStill/" }
    pub(super) fn startup() -> &'static str { "defaults/images/charsStartup/" }
    pub(super) fn evidence() -> &'static str { "defaults/images/evidence/" }
    pub(super) fn bg() -> &'static str { "defaults/images/backgrounds/" }
    pub(super) fn defaultplaces_bg() -> &'static str { "defaults/images/defaultplaces/backgrounds/" }
    pub(super) fn defaultplaces_fg() -> &'static str { "defaults/images/defaultplaces/foreground_objects/" }
    pub(super) fn popups() -> &'static str { "defaults/images/popups/" }
    pub(super) fn locks() -> &'static str { "defaults/images/psycheLocks/" }
    pub(super) fn music() -> &'static str { "defaults/music/" }
    pub(super) fn sounds() -> &'static str { "defaults/sounds/" }
    pub(super) fn voices() -> &'static str { "defaults/voices/" }
}

pub(super) fn build_url(path: &str) -> String {
    // Use url::Url::join for proper path joining and percent-encoding
    match url::Url::parse(AAONLINE_BASE) {
        Ok(base) => {
            // Ensure base has trailing slash for proper joining
            let base_str = if base.as_str().ends_with('/') {
                base.to_string()
            } else {
                format!("{}/", base)
            };
            match url::Url::parse(&base_str).and_then(|b| b.join(path)) {
                Ok(u) => u.to_string(),
                Err(_) => format!("{}/{}", AAONLINE_BASE, path),
            }
        }
        Err(_) => format!("{}/{}", AAONLINE_BASE, path),
    }
}

/// Add an asset with a local_path (for internal assets) or empty local_path (for external).
pub(super) fn add_asset(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    url: String,
    asset_type: &str,
    is_default: bool,
    local_path: String,
) {
    if url.is_empty() {
        return;
    }
    if seen.contains(&url) {
        // Upgrade: if existing entry is external (no local_path) but this one has a default path,
        // update the existing entry so the file gets saved to the correct default location.
        // Same URL = same bytes, so using the default path avoids file duplication.
        if !local_path.is_empty() {
            if let Some(existing) = assets.iter_mut().find(|a| a.url == url && a.local_path.is_empty()) {
                existing.local_path = local_path;
                existing.is_default = is_default;
            }
        }
        return;
    }
    seen.insert(url.clone());
    assets.push(AssetRef {
        url,
        asset_type: asset_type.to_string(),
        is_default,
        local_path,
    });
}

/// Add a non-external asset from AAO's library.
/// `server_dir` = the AAO server path (for building download URL).
/// `local_dir` = the offline bridge.js cfg path (for local save path).
pub(super) fn add_internal(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    server_dir: &str,
    local_dir: &str,
    name: &str,
    default_ext: &str,
    asset_type: &str,
    is_default: bool,
) {
    if name.is_empty() || name.contains("..") {
        return;
    }
    let has_ext = name.contains('.');
    let filename = if has_ext {
        name.to_string()
    } else {
        format!("{}.{}", name, default_ext)
    };
    let url = build_url(&format!("{}{}", server_dir, filename));
    let local_path = sanitize_path(&format!("{}{}", local_dir, filename));
    if local_path.starts_with("..") {
        return; // Reject paths that escape the sandbox after normalization
    }
    add_asset(assets, seen, url, asset_type, is_default, local_path);
}

/// Add an external asset (full URL or relative path).
/// External assets have no local_path -- they get hashed filenames.
pub(super) fn add_external(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    url_or_path: &str,
    asset_type: &str,
) {
    if url_or_path.is_empty() || url_or_path.contains("..") {
        return;
    }
    let full_url = if url_or_path.starts_with("http://") || url_or_path.starts_with("https://") {
        url_or_path.to_string()
    } else {
        build_url(url_or_path)
    };
    add_asset(assets, seen, full_url, asset_type, false, String::new());
}

pub(super) fn is_external(val: &serde_json::Value) -> bool {
    val.as_bool()
        .or_else(|| val.as_i64().map(|n| n != 0))
        .unwrap_or(false)
}
