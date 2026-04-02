use std::fs;
use std::path::Path;

use regex::Regex;
use serde_json::Value;

use crate::error::AppError;
use super::shared::ImportedCaseInfo;

/// Check if a destination asset file truly exists — follows VFS pointers.
/// Returns true if the file is a real asset or a VFS pointer with a valid target.
fn dest_asset_exists(dest: &Path, engine_dir: &Path) -> bool {
    if !dest.exists() {
        return false;
    }
    match crate::downloader::vfs::read_vfs_pointer(dest) {
        Some(_) => {
            let resolved = crate::downloader::vfs::resolve_path(dest, engine_dir, engine_dir);
            resolved.is_file() && resolved != dest
        }
        None => true,
    }
}

/// A single default sprite mapping extracted from the aaoffline `getDefaultSpriteUrl` override.
pub(super) struct DefaultSpriteMapping {
    pub(super) base: String,      // e.g. "Phoenix"
    pub(super) sprite_id: u32,    // e.g. 1
    pub(super) status: String,    // "talking", "still", or "startup"
    pub(super) asset_path: String, // e.g. "assets/1-18236344477825908183.gif"
}

/// Parse the overridden `getDefaultSpriteUrl` function from an aaoffline index.html.
///
/// The aaoffline downloaders replace the function body with hardcoded if-statements:
/// ```js
/// if (base === 'Phoenix' && sprite_id === 1 && status === 'talking') return 'assets/1-xxx.gif';
/// ```
pub(super) fn extract_default_sprite_mappings(html: &str) -> Vec<DefaultSpriteMapping> {
    let re = Regex::new(
        r"if\s*\(base\s*===\s*'([^']+)'\s*&&\s*sprite_id\s*===\s*(\d+)\s*&&\s*status\s*===\s*'([^']+)'\)\s*return\s*'([^']+)'"
    ).unwrap();

    re.captures_iter(html)
        .map(|cap| DefaultSpriteMapping {
            base: cap[1].to_string(),
            sprite_id: cap[2].parse().unwrap_or(0),
            status: cap[3].to_string(),
            asset_path: cap[4].to_string(),
        })
        .collect()
}

/// Copy default sprite assets from the aaoffline `assets/` folder to the engine's `defaults/` tree.
///
/// Maps status → subdirectory:
/// - "talking"  → `defaults/images/chars/{base}/{id}.gif`
/// - "still"    → `defaults/images/charsStill/{base}/{id}.gif`
/// - "startup"  → `defaults/images/charsStartup/{base}/{id}.gif`
pub(super) fn copy_default_sprites(
    mappings: &[DefaultSpriteMapping],
    source_dir: &Path,
    engine_dir: &Path,
) -> (usize, u64) {
    let mut copied = 0usize;
    let mut bytes = 0u64;

    for m in mappings {
        let subdir = match m.status.as_str() {
            "talking" => "chars",
            "still" => "charsStill",
            "startup" => "charsStartup",
            _ => continue,
        };

        let dest_dir = engine_dir.join("defaults").join("images").join(subdir).join(&m.base);
        let dest_file = dest_dir.join(format!("{}.gif", m.sprite_id));

        if dest_asset_exists(&dest_file, engine_dir) {
            continue;
        }

        let src_file = source_dir.join(&m.asset_path);
        if !src_file.exists() {
            continue;
        }

        if fs::create_dir_all(&dest_dir).is_err() {
            continue;
        }

        if let Ok(b) = fs::copy(&src_file, &dest_file) {
            copied += 1;
            bytes += b;
        }
    }

    (copied, bytes)
}

/// Copy default sprites searching across multiple asset directories.
///
/// The aaoffline downloader shares sprite files across a sequence — each case's index.html
/// references the same hash filenames, but a given sprite file may only exist in one
/// subfolder's `assets/` directory. This function tries all provided asset directories
/// to find each sprite file.
pub(super) fn copy_default_sprites_from_multiple_dirs(
    mappings: &[DefaultSpriteMapping],
    asset_dirs: &[std::path::PathBuf],
    engine_dir: &Path,
) -> (usize, u64) {
    let mut copied = 0usize;
    let mut bytes = 0u64;

    for m in mappings {
        let subdir = match m.status.as_str() {
            "talking" => "chars",
            "still" => "charsStill",
            "startup" => "charsStartup",
            _ => continue,
        };

        let dest_dir = engine_dir.join("defaults").join("images").join(subdir).join(&m.base);
        let dest_file = dest_dir.join(format!("{}.gif", m.sprite_id));

        if dest_asset_exists(&dest_file, engine_dir) {
            continue;
        }

        // The asset_path is "assets/{hash}.gif" — strip the "assets/" prefix to get filename
        let filename = m.asset_path.strip_prefix("assets/").unwrap_or(&m.asset_path);

        // Search across all asset directories for this file
        let src_file = asset_dirs.iter()
            .map(|dir| dir.join(filename))
            .find(|p| p.exists());

        let src_file = match src_file {
            Some(f) => f,
            None => continue,
        };

        if let Err(_) = fs::create_dir_all(&dest_dir) {
            continue;
        }

        match fs::copy(&src_file, &dest_file) {
            Ok(b) => {
                copied += 1;
                bytes += b;
            }
            Err(_) => {}
        }
    }

    (copied, bytes)
}

/// A voice blip mapping from the aaoffline `getVoiceUrl` override.
pub(super) struct VoiceMapping {
    pub(super) voice_id: u32,    // e.g. 1 (absolute value)
    pub(super) ext: String,      // "opus", "wav", or "mp3"
    pub(super) asset_path: String,
}

/// Parse the overridden `getVoiceUrl` from an aaoffline index.html.
/// Format: `if (-voice_id === 1 && ext === 'opus') return 'assets/voice_singleblip_1-xxx.opus';`
pub(super) fn extract_voice_mappings(html: &str) -> Vec<VoiceMapping> {
    let re = Regex::new(
        r"if\s*\(-voice_id\s*===\s*(\d+)\s*&&\s*ext\s*===\s*'([^']+)'\)\s*return\s*'([^']+)'"
    ).unwrap();

    re.captures_iter(html)
        .map(|cap| VoiceMapping {
            voice_id: cap[1].parse().unwrap_or(0),
            ext: cap[2].to_string(),
            asset_path: cap[3].to_string(),
        })
        .collect()
}

/// Copy voice blip assets to `defaults/voices/`.
pub(super) fn copy_voice_assets(mappings: &[VoiceMapping], source_dir: &Path, engine_dir: &Path) -> (usize, u64) {
    let mut copied = 0usize;
    let mut bytes = 0u64;
    let dest_dir = engine_dir.join("defaults").join("voices");

    for m in mappings {
        let dest_file = dest_dir.join(format!("voice_singleblip_{}.{}", m.voice_id, m.ext));
        if dest_asset_exists(&dest_file, engine_dir) { continue; }

        let src_file = source_dir.join(&m.asset_path);
        if !src_file.exists() { continue; }

        if fs::create_dir_all(&dest_dir).is_err() { continue; }
        if let Ok(b) = fs::copy(&src_file, &dest_file) {
            copied += 1;
            bytes += b;
        }
    }
    (copied, bytes)
}

/// A default place asset mapping from the aaoffline `default_places` variable.
pub(super) struct PlaceAssetMapping {
    /// Original engine path
    pub(super) dest_path: String,
    /// Source path in aaoffline assets
    pub(super) asset_path: String,
}

/// Parse the overridden `default_places` variable from an aaoffline index.html.
/// Extracts image paths that point to `assets/` (downloaded) rather than `Ressources/` (remote).
pub(super) fn extract_default_place_mappings(html: &str) -> Vec<PlaceAssetMapping> {
    let mut mappings = Vec::new();

    // Find all "image":"assets/..." references in the default_places JSON
    let re = Regex::new(
        r#""image"\s*:\s*"(assets/([^"]+))"#
    ).unwrap();

    for cap in re.captures_iter(html) {
        let asset_path = cap[1].to_string(); // "assets/aj_courtroom-123.jpg"
        let filename_with_hash = &cap[2];     // "aj_courtroom-123.jpg"

        // Determine if this is a background or foreground object based on the filename
        // Background filenames: aj_courtroom, pw_judge, pw_court_still, etc.
        // Foreground filenames: aj_courtroom_benches, pw_courtroom_benches, pw_detention_center (glass), aj_judge_bench
        let is_foreground = filename_with_hash.contains("_benches")
            || filename_with_hash.contains("_glass")
            || filename_with_hash.contains("_bench-")
            || (filename_with_hash.contains("detention_center-") && filename_with_hash.ends_with(".gif"));

        let subdir = if is_foreground { "foreground_objects" } else { "backgrounds" };

        // Extract the base name without the hash: "aj_courtroom-123.jpg" → "aj_courtroom.jpg"
        // The hash is the last `-{digits}.ext` part
        let base_name = strip_aaoffline_hash(filename_with_hash);

        let dest_path = format!("defaults/images/defaultplaces/{}/{}", subdir, base_name);
        mappings.push(PlaceAssetMapping { dest_path, asset_path });
    }

    mappings
}

/// Strip the aaoffline hash suffix from a filename.
/// "aj_courtroom-16637394819900123171.jpg" → "aj_courtroom.jpg"
pub(super) fn strip_aaoffline_hash(filename: &str) -> String {
    if let Some(dot_pos) = filename.rfind('.') {
        let name_part = &filename[..dot_pos];
        let ext = &filename[dot_pos..];
        if let Some(dash_pos) = name_part.rfind('-') {
            let after_dash = &name_part[dash_pos + 1..];
            if after_dash.chars().all(|c| c.is_ascii_digit()) && !after_dash.is_empty() {
                return format!("{}{}", &name_part[..dash_pos], ext);
            }
        }
        filename.to_string()
    } else {
        filename.to_string()
    }
}

/// Copy default place assets to `defaults/images/defaultplaces/`.
pub(super) fn copy_place_assets(mappings: &[PlaceAssetMapping], source_dir: &Path, engine_dir: &Path) -> (usize, u64) {
    let mut copied = 0usize;
    let mut bytes = 0u64;

    for m in mappings {
        let dest_file = engine_dir.join(&m.dest_path);
        if dest_asset_exists(&dest_file, engine_dir) { continue; }

        let src_file = source_dir.join(&m.asset_path);
        if !src_file.exists() { continue; }

        if let Some(parent) = dest_file.parent() {
            if fs::create_dir_all(parent).is_err() { continue; }
        }
        if let Ok(b) = fs::copy(&src_file, &dest_file) {
            copied += 1;
            bytes += b;
        }
    }
    (copied, bytes)
}

/// Read a text file from inside a ZIP archive.
/// Extract `var trial_information = {...};` from the HTML.
pub(super) fn extract_trial_information(html: &str) -> Result<ImportedCaseInfo, AppError> {
    let re = Regex::new(r"var\s+trial_information\s*=\s*(\{[^;]*\})\s*;")
        .map_err(|e| format!("Regex error: {}", e))?;

    let caps = re
        .captures(html)
        .ok_or_else(|| AppError::Other("Could not find 'var trial_information = {...}' in index.html. Is this an aaoffline download?".to_string()))?;

    let json_str = caps.get(1).unwrap().as_str();
    let value: Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse trial_information JSON: {}", e))?;

    let id = value["id"]
        .as_u64()
        .ok_or_else(|| AppError::Other("trial_information missing 'id' field".to_string()))? as u32;
    let title = value["title"]
        .as_str()
        .unwrap_or("Unknown Title")
        .to_string();
    let author = value["author"]
        .as_str()
        .unwrap_or("Unknown Author")
        .to_string();
    let language = value["language"]
        .as_str()
        .unwrap_or("en")
        .to_string();
    let format = value["format"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let last_edit_date = value["last_edit_date"].as_u64().unwrap_or(0);
    let sequence = value.get("sequence").cloned().filter(|v| !v.is_null());

    Ok(ImportedCaseInfo {
        id,
        title,
        author,
        language,
        format,
        last_edit_date,
        sequence,
    })
}

/// Extract `var initial_trial_data = {...};` from the HTML.
///
/// The trial_data JSON can be very large (several MB) and may contain nested
/// braces, so we use a brace-counting approach instead of a simple regex.
pub(super) fn extract_trial_data(html: &str) -> Result<Value, AppError> {
    let marker = "var initial_trial_data = ";
    let start = html
        .find(marker)
        .ok_or_else(|| AppError::Other("Could not find 'var initial_trial_data = ' in index.html.".to_string()))?;

    let json_start = start + marker.len();
    let bytes = html.as_bytes();

    // Find the matching closing brace
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape_next = false;
    let mut end_pos = json_start;

    for (i, &b) in bytes[json_start..].iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match b {
            b'\\' if in_string => {
                escape_next = true;
            }
            b'"' => {
                in_string = !in_string;
            }
            b'{' if !in_string => {
                depth += 1;
            }
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    end_pos = json_start + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }

    if depth != 0 {
        return Err("Malformed initial_trial_data: unbalanced braces.".to_string().into());
    }

    let json_str = &html[json_start..end_pos];
    serde_json::from_str(json_str)
        .map_err(|e| AppError::Other(format!("Failed to parse initial_trial_data JSON: {}", e)))
}

/// Sanitize a filename for safe use in URLs and on disk.
///
/// Uses the same character policy as `generate_filename()` in asset_downloader:
/// only alphanumeric, `-`, `_` are allowed in the name part. Everything else → `-`.
/// The extension (after the last `.`) is preserved and lowercased.
pub(super) fn sanitize_imported_filename(filename: &str) -> String {
    let (name, ext) = match filename.rfind('.') {
        Some(pos) => (&filename[..pos], Some(&filename[pos + 1..])),
        None => (filename, None),
    };

    let sanitized_name: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();

    match ext {
        Some(e) => format!("{}.{}", sanitized_name, e.to_lowercase()),
        None => sanitized_name,
    }
}

/// Build trial_info.json value from extracted case info.
pub(super) fn build_trial_info_json(info: &ImportedCaseInfo) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("id".into(), Value::Number(info.id.into()));
    map.insert("title".into(), Value::String(info.title.clone()));
    map.insert("author".into(), Value::String(info.author.clone()));
    map.insert("language".into(), Value::String(info.language.clone()));
    map.insert("format".into(), Value::String(info.format.clone()));
    map.insert(
        "last_edit_date".into(),
        Value::Number(info.last_edit_date.into()),
    );
    map.insert(
        "sequence".into(),
        info.sequence.clone().unwrap_or(Value::Null),
    );
    Value::Object(map)
}

