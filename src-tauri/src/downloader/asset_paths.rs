/// Typed path constructors for asset index paths.
/// Single source of truth — avoids format!() path strings scattered across the codebase.

/// Case-specific asset path: `case/{id}/assets/{filename}`
pub fn case_asset(case_id: u32, filename: &str) -> String {
    format!("case/{}/assets/{}", case_id, filename)
}

/// Case-relative path: `case/{id}/{relative}` (for assets/, plugins/, etc.)
pub fn case_relative(case_id: u32, relative: &str) -> String {
    format!("case/{}/{}", case_id, relative)
}

/// Case prefix for bulk operations: `case/{id}/`
pub fn case_prefix(case_id: u32) -> String {
    format!("case/{}/", case_id)
}

/// Promoted shared asset path: `defaults/shared/{hash_hex[0..4]}/{hash_hex}.{ext}`
pub fn shared_asset(content_hash: u64, ext: &str) -> String {
    let hash_hex = format!("{:016x}", content_hash);
    let subdir = &hash_hex[0..4];
    if ext.is_empty() {
        format!("defaults/shared/{}/{}", subdir, hash_hex)
    } else {
        format!("defaults/shared/{}/{}.{}", subdir, hash_hex, ext)
    }
}

/// Flat shared asset path (legacy optimize format): `defaults/shared/{hash_hex}.{ext}`
pub fn shared_asset_flat(content_hash: u64, ext: &str) -> String {
    format!("defaults/shared/{:016x}.{}", content_hash, ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_case_asset() {
        assert_eq!(case_asset(42, "sprite.gif"), "case/42/assets/sprite.gif");
        assert_eq!(case_asset(100187, "record scratch-abc.mp3"), "case/100187/assets/record scratch-abc.mp3");
    }

    #[test]
    fn test_case_relative() {
        assert_eq!(case_relative(42, "assets/sprite.gif"), "case/42/assets/sprite.gif");
        assert_eq!(case_relative(42, "plugins/manifest.json"), "case/42/plugins/manifest.json");
    }

    #[test]
    fn test_case_prefix() {
        assert_eq!(case_prefix(42), "case/42/");
        assert_eq!(case_prefix(100187), "case/100187/");
    }

    #[test]
    fn test_shared_asset() {
        let hash: u64 = 0x8f0f583ee399f9bf;
        assert_eq!(shared_asset(hash, "mp3"), "defaults/shared/8f0f/8f0f583ee399f9bf.mp3");
        assert_eq!(shared_asset(hash, ""), "defaults/shared/8f0f/8f0f583ee399f9bf");
    }

    #[test]
    fn test_shared_asset_flat() {
        let hash: u64 = 0x1234567890abcdef;
        assert_eq!(shared_asset_flat(hash, "png"), "defaults/shared/1234567890abcdef.png");
    }
}
