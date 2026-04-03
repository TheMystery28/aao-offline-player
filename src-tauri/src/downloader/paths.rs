//! Path normalization and sanitization utilities.
//!
//! This module ensures that all relative paths used for assets are consistent
//! across platforms, Unicode-normalized, and safe for use on Windows.

use relative_path::RelativePath;
use unicode_normalization::UnicodeNormalization;

/// Normalizes a raw path string into a canonical, cross-platform format.
///
/// Steps taken:
/// 1. Unicode NFC normalization (composed characters).
/// 2. Convert backslashes to forward slashes.
/// 3. Resolve `.` and `..` segments lexically.
/// 4. Replace Windows-illegal characters (`:`, `*`, `?`, etc.) with `_`.
pub fn normalize_path(raw: &str) -> String {
    // 1. Unicode NFC normalization
    let nfc: String = raw.nfc().collect();

    // 2. Convert backslashes to forward slashes (RelativePath v2 treats \ as literal)
    let forward_slashed = nfc.replace('\\', "/");

    // 3. Parse as RelativePath → resolves . and .., guarantees forward slashes
    let normalized = RelativePath::new(&forward_slashed).normalize();

    // 3. Replace Windows-illegal characters (safe because all our paths are relative)
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
}
