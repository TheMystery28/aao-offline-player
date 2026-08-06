//! Path normalization and sanitization utilities.
//!
//! This module ensures that all relative paths used for assets are consistent
//! across platforms, Unicode-normalized, and safe for use on Windows.

use relative_path::RelativePath;
use unicode_normalization::UnicodeNormalization;

/// True when `normalize_path` would return `s` unchanged — used to skip the
/// NFC scan, backslash pass, RelativePath parse and illegal-char map for the
/// overwhelmingly common ASCII, well-formed path.
fn is_fast_path(s: &str) -> bool {
    if s.is_empty() || !s.is_ascii() {
        return false;
    }
    if s.starts_with('/') || s.ends_with('/') || s.contains("//") || s.contains('\\') {
        return false;
    }
    for ch in s.chars() {
        if matches!(ch, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            return false;
        }
    }
    // Any "." or ".." path segment would be resolved by RelativePath
    for part in s.split('/') {
        if part == "." || part == ".." {
            return false;
        }
    }
    true
}

/// Normalizes a raw path string into a canonical, cross-platform format.
///
/// Steps taken:
/// 1. Unicode NFC normalization (composed characters).
/// 2. Convert backslashes to forward slashes.
/// 3. Resolve `.` and `..` segments lexically.
/// 4. Replace Windows-illegal characters (`:`, `*`, `?`, etc.) with `_`.
pub fn normalize_path(raw: &str) -> String {
    if is_fast_path(raw) {
        return raw.to_string();
    }

    // 1. Unicode NFC normalization
    let nfc: String = raw.nfc().collect();

    // 2. Convert backslashes to forward slashes (RelativePath v2 treats \ as literal)
    let forward_slashed = nfc.replace('\\', "/");

    // 3. Parse as RelativePath → resolves . and .., guarantees forward slashes
    let normalized = RelativePath::new(&forward_slashed).normalize();

    // 4. Replace Windows-illegal characters (safe because all our paths are relative)
    normalized
        .as_str()
        .chars()
        .map(|c| match c {
            ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_nfc() {
        // NFD input (decomposed é = e + combining accent) → NFC output (composed é)
        let nfd = "caf\u{0065}\u{0301}/menu.mp3";
        let result = normalize_path(nfd);
        assert!(result.contains("café"), "Should normalize NFD to NFC, got: {}", result);
        assert_eq!(result, "caf\u{00e9}/menu.mp3");
    }

    #[test]
    fn test_normalize_path_backslash() {
        assert_eq!(
            normalize_path("defaults\\music\\song.mp3"),
            "defaults/music/song.mp3"
        );
    }

    #[test]
    fn test_normalize_path_dotdot() {
        assert_eq!(
            normalize_path("assets/../defaults/file.gif"),
            "defaults/file.gif"
        );
    }

    #[test]
    fn test_normalize_path_illegal_chars() {
        assert_eq!(
            normalize_path("music/Game : Title/song.mp3"),
            "music/Game _ Title/song.mp3"
        );
        assert_eq!(
            normalize_path("a:b*c?d\"e<f>g|h"),
            "a_b_c_d_e_f_g_h"
        );
    }

    #[test]
    fn test_normalize_path_passthrough() {
        let path = "defaults/images/chars/Apollo/1.gif";
        assert_eq!(normalize_path(path), path);
    }

    #[test]
    fn test_normalize_path_mixed() {
        // Backslash + colon + .. all at once
        assert_eq!(
            normalize_path("assets\\..\\defaults\\music\\Game : Title\\song.mp3"),
            "defaults/music/Game _ Title/song.mp3"
        );
    }

    #[test]
    fn test_normalize_path_preserves_unicode() {
        assert_eq!(
            normalize_path("逆転裁判/テーマ.mp3"),
            "逆転裁判/テーマ.mp3"
        );
        assert_eq!(
            normalize_path("Ace Attorney/Thème été.mp3"),
            "Ace Attorney/Thème été.mp3"
        );
    }

    #[test]
    fn test_normalize_path_dot_resolution() {
        assert_eq!(normalize_path("./file.gif"), "file.gif");
        assert_eq!(normalize_path("a/./b/./c.gif"), "a/b/c.gif");
    }

    #[test]
    fn test_fast_path_conditions() {
        assert!(is_fast_path("a/b.gif"));
        assert!(is_fast_path("defaults/images/chars/Apollo/1.gif"));
        assert!(is_fast_path("case/42/assets/file-hash.mp3"));
        assert!(is_fast_path("hidden/.gitkeep"), "dotfile names are not dot segments");
        // Anything the slow path would transform must NOT take the fast path
        assert!(!is_fast_path("a/../b.gif"));
        assert!(!is_fast_path("a/./b.gif"));
        assert!(!is_fast_path("a:b"));
        assert!(!is_fast_path("a\\b"));
        assert!(!is_fast_path("caf\u{00e9}"), "non-ASCII must use NFC slow path");
        assert!(!is_fast_path("a//b"));
        assert!(!is_fast_path("a/"));
        assert!(!is_fast_path("/a"));
        assert!(!is_fast_path(""), "empty string must use the slow path");
    }

    #[test]
    fn test_normalize_fast_path_equivalence() {
        // Fast-path outputs must equal their inputs
        let cases = [
            "simple/path.gif",
            "defaults/images/chars/Apollo/1.gif",
            "case/42/assets/record-scratch-abc123.mp3",
            "hidden/.gitkeep",
            "a.b/c.d",
            "music/Game_Song/track.ogg",
        ];
        for input in cases {
            assert_eq!(normalize_path(input), input, "fast path must be identity for: {}", input);
        }
        // Slow-path cases must still be transformed as before
        assert_eq!(normalize_path("a: b"), "a_ b");
        assert_eq!(normalize_path("x\\y"), "x/y");
        assert_eq!(normalize_path("caf\u{00e9}"), "caf\u{00e9}");
    }
}
