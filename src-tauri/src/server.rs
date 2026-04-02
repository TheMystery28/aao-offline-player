//! Custom HTTP server for serving the AAO game engine and case assets.
//!
//! Routes:
//!   GET /player.html              → engine/player.html
//!   GET /bridge.js                → engine/bridge.js
//!   GET /Javascript/*             → engine/Javascript/*
//!   GET /CSS/*                    → engine/CSS/*
//!   GET /Languages/*              → engine/Languages/*
//!   GET /img/*                    → engine/img/*
//!   GET /defaults/*               → engine/defaults/*
//!   GET /case/{id}/*              → engine/case/{id}/*
//!   GET /assets/*                 → engine/assets/* (for current case)
//!
//! All paths are resolved relative to a configurable base directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tiny_http::{Header, Response, Server};

/// Result of serving a file request. Pure data — no tiny_http dependency.
pub(crate) struct ServeResult {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub data: Vec<u8>,
}

impl ServeResult {
    /// Get a header value by case-insensitive key lookup.
    #[cfg(test)]
    fn header(&self, key: &str) -> Option<&str> {
        let key_lower = key.to_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == key_lower)
            .map(|(_, v)| v.as_str())
    }
}

/// Convert a ServeResult into an http::Response for Tauri's protocol handler.
pub(crate) fn serve_result_to_response(result: ServeResult) -> tauri::http::Response<Vec<u8>> {
    let mut builder = tauri::http::Response::builder().status(result.status);
    for (key, value) in &result.headers {
        builder = builder.header(key.as_str(), value.as_str());
    }
    builder.body(result.data).unwrap_or_else(|_| {
        tauri::http::Response::builder()
            .status(500)
            .body(Vec::new())
            .unwrap()
    })
}

/// Serve a file request as a pure function. No tiny_http dependency.
///
/// Handles GET (200/404/500), OPTIONS (204 with CORS), and Range requests (206/416).
/// Returns a `ServeResult` with status, headers, and body data.
pub(crate) fn serve_file(
    config: &ServerConfig,
    url_path: &str,
    method: &str,
    range_header: Option<&str>,
) -> ServeResult {
    if method.eq_ignore_ascii_case("OPTIONS") {
        return handle_options_preflight();
    }

    // Normalize URL path
    let clean_path = url_path.split('?').next().unwrap_or(url_path);
    let decoded = url_decode(clean_path);
    let relative = decoded.trim_start_matches('/');
    let relative = sanitize_path(relative);

    // Resolve to filesystem path
    let file_path = match resolve_path(config, &relative) {
        Some(p) if p.is_file() => p,
        _ => return ServeResult {
            status: 404,
            headers: vec![("Access-Control-Allow-Origin".into(), "*".into())],
            data: b"404 Not Found".to_vec(),
        },
    };

    // File metadata
    let file_size = match fs::metadata(&file_path) {
        Ok(m) => m.len() as usize,
        Err(_) => return ServeResult {
            status: 500,
            headers: vec![],
            data: b"500 Internal Server Error".to_vec(),
        },
    };

    // MIME, cache strategy, base headers
    let mime = mime_type(&file_path);
    let cache_value = if relative.starts_with("case/") || relative.starts_with("defaults/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let is_media = mime.starts_with("audio/") || mime.starts_with("video/") || mime.starts_with("image/");
    let mut headers = vec![
        ("Content-Type".into(), mime.to_string()),
        ("Access-Control-Allow-Origin".into(), "*".into()),
        ("Cache-Control".into(), cache_value.into()),
    ];
    if is_media {
        headers.push(("Accept-Ranges".into(), "bytes".into()));
    }

    // Dispatch to range or full-file handler
    if let Some(range_str) = range_header {
        return handle_range_request(&file_path, file_size, headers, range_str);
    }
    handle_full_response(&file_path, file_size, headers)
}

/// Return a 204 CORS preflight response for OPTIONS requests.
fn handle_options_preflight() -> ServeResult {
    ServeResult {
        status: 204,
        headers: vec![
            ("Access-Control-Allow-Origin".into(), "*".into()),
            ("Access-Control-Allow-Methods".into(), "GET, OPTIONS".into()),
            ("Access-Control-Allow-Headers".into(), "Range, Content-Type".into()),
            ("Access-Control-Expose-Headers".into(), "Content-Range, Content-Length".into()),
        ],
        data: Vec::new(),
    }
}

/// Serve a byte-range slice of a file (HTTP Range request).
///
/// Returns 206 Partial Content with the requested slice,
/// 416 Range Not Satisfiable if the range is invalid,
/// or 500 if the file cannot be opened/read.
///
/// `headers` must already contain Content-Type, CORS, Cache-Control, and
/// Accept-Ranges — this function appends Content-Range and Content-Length.
fn handle_range_request(
    file_path: &Path,
    file_size: usize,
    mut headers: Vec<(String, String)>,
    range_str: &str,
) -> ServeResult {
    let Some((start, end)) = parse_range(range_str, file_size) else {
        return ServeResult {
            status: 416,
            headers: vec![
                ("Content-Range".into(), format!("bytes */{}", file_size)),
                ("Access-Control-Allow-Origin".into(), "*".into()),
            ],
            data: Vec::new(),
        };
    };

    use std::io::{Read, Seek, SeekFrom};
    let slice = match std::fs::File::open(file_path) {
        Ok(mut f) => {
            let len = end - start + 1;
            let mut buf = vec![0u8; len];
            if f.seek(SeekFrom::Start(start as u64)).is_err() {
                return ServeResult { status: 500, headers: vec![], data: b"Seek failed".to_vec() };
            }
            if f.read_exact(&mut buf).is_err() {
                return ServeResult { status: 500, headers: vec![], data: b"Read failed".to_vec() };
            }
            buf
        }
        Err(_) => return ServeResult {
            status: 500,
            headers: vec![],
            data: b"500 Internal Server Error".to_vec(),
        },
    };

    headers.push(("Content-Range".into(), format!("bytes {}-{}/{}", start, end, file_size)));
    headers.push(("Content-Length".into(), slice.len().to_string()));
    ServeResult { status: 206, headers, data: slice }
}

/// Serve the complete contents of a file (HTTP 200 OK).
///
/// Returns 500 if the file cannot be read.
/// `headers` must already contain Content-Type, CORS, and Cache-Control —
/// this function appends Content-Length.
fn handle_full_response(
    file_path: &Path,
    file_size: usize,
    mut headers: Vec<(String, String)>,
) -> ServeResult {
    match fs::read(file_path) {
        Ok(data) => {
            headers.push(("Content-Length".into(), file_size.to_string()));
            ServeResult { status: 200, headers, data }
        }
        Err(_) => ServeResult {
            status: 500,
            headers: vec![],
            data: b"500 Internal Server Error".to_vec(),
        },
    }
}

/// Parse an HTTP Range header value into (start, end) inclusive byte positions.
/// Returns None if the range is invalid or unsatisfiable for the given file size.
fn parse_range(range_str: &str, file_size: usize) -> Option<(usize, usize)> {
    let range_str = range_str.trim();
    let spec = range_str.strip_prefix("bytes=")?;

    if let Some(suffix) = spec.strip_prefix('-') {
        // bytes=-N → last N bytes
        let n: usize = suffix.trim().parse().ok()?;
        if n == 0 || n > file_size {
            return None;
        }
        let start = file_size - n;
        Some((start, file_size - 1))
    } else if spec.ends_with('-') {
        // bytes=N- → from N to EOF
        let start: usize = spec.trim_end_matches('-').trim().parse().ok()?;
        if start >= file_size {
            return None;
        }
        Some((start, file_size - 1))
    } else {
        // bytes=N-M
        let mut parts = spec.splitn(2, '-');
        let start: usize = parts.next()?.trim().parse().ok()?;
        let end: usize = parts.next()?.trim().parse().ok()?;
        if start > end || start >= file_size {
            return None;
        }
        let end = end.min(file_size - 1);
        Some((start, end))
    }
}

/// MIME type lookup based on file extension.
pub(crate) fn mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "html" => "text/html; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "opus" => "audio/opus",
        "wav" => "audio/wav",
        "m4a" | "aac" => "audio/mp4",
        "flac" => "audio/flac",
        "mid" | "midi" => "audio/midi",
        "webm" => "video/webm",
        "mp4" | "m4v" => "video/mp4",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "ico" => "image/x-icon",
        "xml" => "application/xml",
        "txt" => "text/plain; charset=utf-8",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

/// Configuration for the asset server.
pub struct ServerConfig {
    /// Base directory containing the engine files (player.html, Javascript/, CSS/, etc.)
    /// On Android/iOS this is the read-only bundled resources directory.
    pub engine_dir: PathBuf,
    /// Writable data directory for case/, defaults/, and config.json.
    /// On desktop this equals engine_dir. On mobile it's the app's private data directory.
    pub data_dir: PathBuf,
}

/// Start the HTTP server in a background thread.
/// Returns the port number the server is listening on.
pub fn start_server(config: ServerConfig) -> Result<u16, crate::error::AppError> {
    let port = portpicker::pick_unused_port()
        .ok_or_else(|| "No available port found for asset server".to_string())?;
    let config = Arc::new(config);

    let server = Server::http(format!("localhost:{}", port))
        .map_err(|e| format!("Failed to start asset server on port {}: {}", port, e))?;

    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let config = Arc::clone(&config);
            std::thread::spawn(move || {
                handle_request(request, &config);
            });
        }
    });

    Ok(port)
}

/// Migration-only HTTP request handler.
/// Only serves `localstorage_migrate.html` for the one-time localStorage migration.
/// All other content is served by the `aao://` protocol handler.
/// Will be removed entirely in the next release once migration period ends.
fn handle_request(request: tiny_http::Request, config: &ServerConfig) {
    let url_path = request.url().to_string();
    let clean_path = url_path.split('?').next().unwrap_or(&url_path);

    if clean_path == "/localstorage_migrate.html" {
        let path = config.engine_dir.join("localstorage_migrate.html");
        if let Ok(data) = std::fs::read(&path) {
            let mut response = Response::from_data(data);
            if let Ok(h) = Header::from_bytes(
                "Content-Type".as_bytes(),
                "text/html; charset=utf-8".as_bytes(),
            ) {
                response.add_header(h);
            }
            if let Ok(h) = Header::from_bytes(
                "Access-Control-Allow-Origin".as_bytes(),
                "*".as_bytes(),
            ) {
                response.add_header(h);
            }
            let _ = request.respond(response);
            return;
        }
    }

    let _ = request.respond(
        Response::from_string("404 Not Found").with_status_code(404),
    );
}

/// Resolve a URL path to a filesystem path.
///
/// Routes writable data (case/, defaults/) to `data_dir` and
/// static engine files (JS, CSS, HTML, img, Languages) to `engine_dir`.
/// On desktop both directories are the same. On Android/iOS they differ:
/// engine_dir is the read-only bundled resources, data_dir is the app's
/// private writable storage.
pub(crate) fn resolve_path(config: &ServerConfig, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() || relative == "/" {
        let path = config.engine_dir.join("player.html");
        return if path.is_file() { Some(path) } else { None };
    }

    if relative.contains("..") {
        return None;
    }

    // Pipeline: find candidate file, then resolve VFS pointers at the end.
    let mut candidate: Option<PathBuf> = None;

    // Route case/, defaults/, plugins/ to writable data directory.
    if relative.starts_with("case/") || relative.starts_with("defaults/") || relative.starts_with("plugins/") {
        let path = config.data_dir.join(relative);
        if path.is_file() {
            candidate = Some(path);
        }
        // Fall through to engine_dir in case defaults are bundled (future-proof)
    }

    // Static engine files (JS, CSS, HTML, img, Languages) from engine_dir.
    if candidate.is_none() {
        let path = config.engine_dir.join(relative);
        if path.is_file() {
            candidate = Some(path);
        }
    }

    // Case-insensitive fallback for Linux/Android (case-sensitive filesystems).
    #[cfg(not(target_os = "windows"))]
    if candidate.is_none() {
        let base = if relative.starts_with("case/") || relative.starts_with("defaults/") {
            config.data_dir.join(relative)
        } else {
            config.engine_dir.join(relative)
        };
        candidate = case_insensitive_resolve(&base);
    }

    // VFS pointer resolution: follow aliases to the real file (supports multi-hop chains).
    if let Some(path) = candidate {
        let resolved = crate::downloader::vfs::resolve_path(&path, &config.data_dir, &config.engine_dir);
        // If the file is a VFS pointer whose target is missing, resolve_path returns the
        // pointer itself — don't serve the pointer content, return 404 instead.
        if resolved == path {
            if crate::downloader::vfs::read_vfs_pointer(&path).is_some() {
                return None; // Broken pointer
            }
        }
        if resolved.is_file() {
            return Some(resolved);
        }
        return None;
    }

    None
}

/// Case-insensitive file resolution: given a candidate path, if the file doesn't
/// exist at the exact case, scan the parent directory for a case-insensitive match.
#[cfg(not(target_os = "windows"))]
fn case_insensitive_resolve(candidate: &std::path::Path) -> Option<PathBuf> {
    let parent = candidate.parent()?;
    if !parent.is_dir() {
        return None;
    }
    let target_name = candidate.file_name()?.to_string_lossy().to_lowercase();
    for entry in std::fs::read_dir(parent).ok()?.flatten() {
        if entry.file_name().to_string_lossy().to_lowercase() == target_name {
            let path = entry.path();
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

/// Canonical path normalization. Delegates to the shared normalize_path function
/// in downloader::paths — ensures server and downloader use identical normalization.
pub(crate) fn sanitize_path(path: &str) -> String {
    crate::downloader::paths::normalize_path(path)
}

/// URL path decoding (handles %XX sequences only).
///
/// NOTE: `+` is kept as a literal character. In URL *paths*, `+` is literal.
/// Only in query strings (application/x-www-form-urlencoded) does `+` mean space.
/// Spaces in URL paths are encoded as `%20`.
pub(crate) fn url_decode(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.bytes();

    while let Some(b) = chars.next() {
        match b {
            b'%' => {
                let hi = chars.next().unwrap_or(b'0');
                let lo = chars.next().unwrap_or(b'0');
                if let (Some(h), Some(l)) = (hex_val(hi), hex_val(lo)) {
                    bytes.push(h << 4 | l);
                } else {
                    bytes.push(b'%');
                    bytes.push(hi);
                    bytes.push(lo);
                }
            }
            _ => bytes.push(b),
        }
    }

    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[SERVER WARN] URL decode produced invalid UTF-8 for input '{}': lossy conversion applied",
                input
            );
            String::from_utf8_lossy(e.as_bytes()).into_owned()
        }
    }
}

pub(crate) fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test helper: create a ServerConfig with both dirs pointing to the same path.
    fn test_config(dir: &Path) -> ServerConfig {
        ServerConfig {
            engine_dir: dir.to_path_buf(),
            data_dir: dir.to_path_buf(),
        }
    }

    #[test]
    fn test_url_decode_percent_encoding() {
        assert_eq!(url_decode("/path%20with%20spaces"), "/path with spaces");
        assert_eq!(url_decode("Ace%20Attorney%3A%20Miles"), "Ace Attorney: Miles");
    }

    /// In URL paths, `+` is a literal character (NOT a space).
    /// Only in query strings (application/x-www-form-urlencoded) does `+` mean space.
    /// Since url_decode is used on URL paths, `+` must stay as `+`.
    #[test]
    fn test_url_decode_plus_stays_literal() {
        assert_eq!(url_decode("hello+world"), "hello+world");
        assert_eq!(url_decode("/assets/pioggia+car-123.mp3"), "/assets/pioggia+car-123.mp3");
    }

    #[test]
    fn test_url_decode_passthrough() {
        assert_eq!(url_decode("/simple/path.jpg"), "/simple/path.jpg");
    }

    #[test]
    fn test_url_decode_invalid_utf8_is_lossy() {
        // %80%81 are not valid UTF-8 start bytes — should produce replacement chars, not panic
        let result = url_decode("/path/%80%81/file.gif");
        assert!(result.contains('\u{FFFD}') || result.contains('�'),
            "Invalid UTF-8 should produce replacement character, got: {:?}", result);
        assert!(result.contains("file.gif"), "Rest of path should be preserved");
    }

    #[test]
    fn test_sanitize_path_colons() {
        assert_eq!(
            sanitize_path("defaults/music/AA Investigations : ME2/song.mp3"),
            "defaults/music/AA Investigations _ ME2/song.mp3"
        );
    }

    #[test]
    fn test_sanitize_path_no_change() {
        let path = "defaults/images/backgrounds/Court.jpg";
        assert_eq!(sanitize_path(path), path);
    }

    #[test]
    fn test_url_decode_then_sanitize() {
        // Simulates what the server does: decode URL, trim leading /, then sanitize
        let url = "/defaults/music/Ace%20Attorney%20Investigations%20%3A%20Miles/song.mp3";
        let decoded = url_decode(url);
        assert_eq!(decoded, "/defaults/music/Ace Attorney Investigations : Miles/song.mp3");
        let relative = decoded.trim_start_matches('/');
        let sanitized = sanitize_path(relative);
        assert_eq!(sanitized, "defaults/music/Ace Attorney Investigations _ Miles/song.mp3");
    }

    #[test]
    fn test_mime_type() {
        assert_eq!(mime_type(Path::new("file.html")), "text/html; charset=utf-8");
        assert_eq!(mime_type(Path::new("file.js")), "application/javascript; charset=utf-8");
        assert_eq!(mime_type(Path::new("file.png")), "image/png");
        assert_eq!(mime_type(Path::new("file.gif")), "image/gif");
        assert_eq!(mime_type(Path::new("file.mp3")), "audio/mpeg");
        assert_eq!(mime_type(Path::new("file.svg")), "image/svg+xml");
        assert_eq!(mime_type(Path::new("file.unknown")), "application/octet-stream");
    }

    // --- Regression: resolve_path security and correctness ---

    #[test]
    fn test_resolve_path_blocks_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());
        assert!(resolve_path(&config, "../etc/passwd").is_none());
        assert!(resolve_path(&config, "foo/../../etc/passwd").is_none());
    }

    #[test]
    fn test_resolve_path_empty_serves_player_html() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("player.html"), "test").unwrap();
        let config = test_config(dir.path());
        let path = resolve_path(&config, "");
        assert!(path.is_some());
        assert!(path.unwrap().ends_with("player.html"));
    }

    #[test]
    fn test_resolve_path_serves_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("Javascript");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("test.js"), "// js").unwrap();
        let config = test_config(dir.path());
        let path = resolve_path(&config, "Javascript/test.js");
        assert!(path.is_some());
    }

    #[test]
    fn test_resolve_path_returns_none_for_missing() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());
        assert!(resolve_path(&config, "nonexistent/file.txt").is_none());
    }

    /// Regression: music paths with colons must be sanitized before resolve.
    /// The server pipeline is: url_decode → sanitize_path → resolve_path.
    /// This test verifies the full pipeline for a realistic AAO music path.
    #[test]
    fn test_full_pipeline_music_path_with_colon() {
        let dir = tempfile::tempdir().unwrap();
        // Create file with sanitized name (colon → underscore)
        let music_dir = dir.path().join("defaults/music/Ace Attorney Investigations _ Miles Edgeworth 2");
        std::fs::create_dir_all(&music_dir).unwrap();
        std::fs::write(music_dir.join("117 Lamenting People.mp3"), "audio").unwrap();

        let config = test_config(dir.path());

        // Simulate the server pipeline
        let url = "/defaults/music/Ace%20Attorney%20Investigations%20%3A%20Miles%20Edgeworth%202/117%20Lamenting%20People.mp3";
        let decoded = url_decode(url);
        let relative = decoded.trim_start_matches('/');
        let sanitized = sanitize_path(relative);
        let path = resolve_path(&config, &sanitized);

        assert!(path.is_some(), "Server must resolve sanitized music path");
        assert!(path.unwrap().is_file());
    }

    /// Regression: sanitize_path in server.rs must match asset_resolver::sanitize_path.
    /// Both replace the same characters: : * ? " < > |
    #[test]
    fn test_sanitize_path_matches_asset_resolver() {
        let test_cases = [
            "Ace Attorney Investigations : Miles Edgeworth 2/song.mp3",
            "file*name?.txt",
            "path\"with<angles>and|pipes",
            "normal/path/no-special.jpg",
        ];
        for input in &test_cases {
            let server_result = sanitize_path(input);
            // Verify same replacement logic: all : * ? " < > | → _
            let expected: String = input.chars().map(|c| match c {
                ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            }).collect();
            assert_eq!(server_result, expected, "Mismatch for input: {}", input);
        }
    }

    /// Regression: files with `+` in their name must be served correctly.
    /// The aaoffline import copies filenames as-is, which may include `+`.
    /// The server must NOT decode `+` as space when resolving file paths.
    #[test]
    fn test_full_pipeline_file_with_plus_in_name() {
        let dir = tempfile::tempdir().unwrap();
        let assets_dir = dir.path().join("case/102059/assets");
        std::fs::create_dir_all(&assets_dir).unwrap();
        std::fs::write(assets_dir.join("pioggia+car-123456.mp3"), "audio").unwrap();

        let config = test_config(dir.path());

        // Simulate the server pipeline: browser sends + literally in URL path
        let url = "/case/102059/assets/pioggia+car-123456.mp3";
        let decoded = url_decode(url);
        let relative = decoded.trim_start_matches('/');
        let sanitized = sanitize_path(relative);
        let path = resolve_path(&config, &sanitized);

        assert!(path.is_some(), "Server must resolve files with + in their name");
        assert!(path.unwrap().is_file());
    }

    /// Regression: case asset paths under case/{id}/ must resolve correctly.
    #[test]
    fn test_resolve_path_case_assets() {
        let dir = tempfile::tempdir().unwrap();
        let case_dir = dir.path().join("case/123/assets");
        std::fs::create_dir_all(&case_dir).unwrap();
        std::fs::write(case_dir.join("icon-abc123.png"), "img").unwrap();

        let config = test_config(dir.path());
        let path = resolve_path(&config, "case/123/assets/icon-abc123.png");
        assert!(path.is_some());
        assert!(path.unwrap().is_file());
    }

    /// Regression: default sprite paths must resolve correctly (deep nested dirs).
    #[test]
    fn test_resolve_path_default_sprites() {
        let dir = tempfile::tempdir().unwrap();

        // Create sprite file structure matching the engine layout
        let chars_dir = dir.path().join("defaults/images/chars/Apollo");
        std::fs::create_dir_all(&chars_dir).unwrap();
        std::fs::write(chars_dir.join("1.gif"), "gif data").unwrap();

        let still_dir = dir.path().join("defaults/images/charsStill/Apollo");
        std::fs::create_dir_all(&still_dir).unwrap();
        std::fs::write(still_dir.join("1.gif"), "gif data").unwrap();

        let startup_dir = dir.path().join("defaults/images/charsStartup/Apollo");
        std::fs::create_dir_all(&startup_dir).unwrap();
        std::fs::write(startup_dir.join("3.gif"), "gif data").unwrap();

        let config = test_config(dir.path());

        // Test talking sprite
        let path = resolve_path(&config, "defaults/images/chars/Apollo/1.gif");
        assert!(path.is_some(), "Must resolve talking sprite");
        assert!(path.unwrap().is_file());

        // Test still sprite
        let path = resolve_path(&config, "defaults/images/charsStill/Apollo/1.gif");
        assert!(path.is_some(), "Must resolve still sprite");
        assert!(path.unwrap().is_file());

        // Test startup sprite
        let path = resolve_path(&config, "defaults/images/charsStartup/Apollo/3.gif");
        assert!(path.is_some(), "Must resolve startup sprite");
        assert!(path.unwrap().is_file());
    }

    /// Regression: voice files must resolve correctly.
    #[test]
    fn test_resolve_path_default_voices() {
        let dir = tempfile::tempdir().unwrap();
        let voices_dir = dir.path().join("defaults/voices");
        std::fs::create_dir_all(&voices_dir).unwrap();
        std::fs::write(voices_dir.join("voice_singleblip_1.opus"), "audio").unwrap();

        let config = test_config(dir.path());
        let path = resolve_path(&config, "defaults/voices/voice_singleblip_1.opus");
        assert!(path.is_some(), "Must resolve voice file");
        assert!(path.unwrap().is_file());
    }

    // =====================================================================
    // Integration tests: full HTTP server (regression baseline)
    // These tests start the real server and make HTTP requests to verify
    // end-to-end behavior. They capture the current behavior BEFORE
    // performance optimizations (multi-threading, caching, keep-alive).
    // =====================================================================

    /// Helper: Make a blocking HTTP GET request to the test server.
    /// Returns (status_code, headers as lowercase key-value pairs, body bytes).
    fn http_get(port: u16, path: &str) -> (u16, Vec<(String, String)>, Vec<u8>) {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        use std::time::Duration;

        let mut stream = TcpStream::connect(format!("localhost:{}", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        // Use HTTP/1.0 so the server closes the connection after response
        let request = format!(
            "GET {} HTTP/1.0\r\nHost: localhost:{}\r\n\r\n",
            path, port
        );
        stream.write_all(request.as_bytes()).unwrap();

        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();

        // Find header/body boundary
        let boundary = response
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .unwrap_or(response.len());

        let header_section = String::from_utf8_lossy(&response[..boundary]).to_string();
        let body = if boundary + 4 <= response.len() {
            response[boundary + 4..].to_vec()
        } else {
            Vec::new()
        };

        let mut lines = header_section.lines();
        let status_line = lines.next().unwrap_or("");
        let status_code: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let headers: Vec<(String, String)> = lines
            .filter_map(|line| {
                let mut parts = line.splitn(2, ": ");
                let key = parts.next()?.to_lowercase();
                let value = parts.next()?.to_string();
                Some((key, value))
            })
            .collect();

        (status_code, headers, body)
    }

    /// Helper to get a specific header value (case-insensitive key).
    fn get_header<'a>(headers: &'a [(String, String)], key: &str) -> Option<&'a str> {
        let key_lower = key.to_lowercase();
        headers
            .iter()
            .find(|(k, _)| k == &key_lower)
            .map(|(_, v)| v.as_str())
    }

    /// Create a migration-only test server. Only localstorage_migrate.html exists.
    fn setup_migration_server() -> (u16, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        // Only the migration bridge page should be served
        std::fs::write(
            dir.path().join("localstorage_migrate.html"),
            "<html><script>/* migration */</script></html>",
        ).unwrap();
        // Create files the server should NOT serve (protocol handler serves these now)
        std::fs::write(dir.path().join("player.html"), "<html>player</html>").unwrap();
        let js_dir = dir.path().join("Javascript");
        std::fs::create_dir_all(&js_dir).unwrap();
        std::fs::write(js_dir.join("test.js"), "// js").unwrap();

        let config = test_config(dir.path());
        let port = start_server(config).expect("test: failed to start server");
        std::thread::sleep(std::time::Duration::from_millis(50));
        (port, dir)
    }

    // =================================================================
    // Migration-only server integration tests
    // =================================================================

    #[test]
    fn test_migration_server_serves_migrate_html() {
        let (port, _dir) = setup_migration_server();
        let (status, headers, body) = http_get(port, "/localstorage_migrate.html");
        assert_eq!(status, 200);
        assert!(String::from_utf8_lossy(&body).contains("migration"));
        assert_eq!(get_header(&headers, "content-type"), Some("text/html; charset=utf-8"));
        assert_eq!(get_header(&headers, "access-control-allow-origin"), Some("*"));
    }

    #[test]
    fn test_migration_server_strips_query_string() {
        let (port, _dir) = setup_migration_server();
        let (status, _, body) = http_get(port, "/localstorage_migrate.html?id=abc123");
        assert_eq!(status, 200);
        assert!(String::from_utf8_lossy(&body).contains("migration"));
    }

    #[test]
    fn test_migration_server_rejects_player_html() {
        let (port, _dir) = setup_migration_server();
        let (status, _, _) = http_get(port, "/player.html");
        assert_eq!(status, 404);
    }

    #[test]
    fn test_migration_server_rejects_root() {
        let (port, _dir) = setup_migration_server();
        let (status, _, _) = http_get(port, "/");
        assert_eq!(status, 404);
    }

    #[test]
    fn test_migration_server_rejects_js() {
        let (port, _dir) = setup_migration_server();
        let (status, _, _) = http_get(port, "/Javascript/test.js");
        assert_eq!(status, 404);
    }

    #[test]
    fn test_migration_server_rejects_case_assets() {
        let (port, _dir) = setup_migration_server();
        let (status, _, _) = http_get(port, "/case/123/trial_data.json");
        assert_eq!(status, 404);
    }

    #[test]
    fn test_migration_server_rejects_defaults() {
        let (port, _dir) = setup_migration_server();
        let (status, _, _) = http_get(port, "/defaults/images/chars/Apollo/1.gif");
        assert_eq!(status, 404);
    }

    /// Test with the REAL engine directory to detect path issues.
    #[test]
    fn test_resolve_path_real_engine_dir() {
        // Use the actual engine dir to test real files
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let engine_dir = manifest_dir.parent().unwrap().join("engine");
        if !engine_dir.exists() {
            return; // Skip if engine dir doesn't exist (CI)
        }

        let config = ServerConfig { engine_dir: engine_dir.clone(), data_dir: engine_dir.clone() };

        // Test voice file
        let voice_path = engine_dir.join("defaults/voices/voice_singleblip_1.opus");
        if voice_path.exists() {
            let result = resolve_path(&config, "defaults/voices/voice_singleblip_1.opus");
            assert!(
                result.is_some(),
                "Voice file exists at {} but resolve_path returned None. engine_dir={}",
                voice_path.display(),
                engine_dir.display(),
            );
        }

        // Test sprite file
        let sprite_path = engine_dir.join("defaults/images/chars/Apollo/1.gif");
        if sprite_path.exists() {
            let result = resolve_path(&config, "defaults/images/chars/Apollo/1.gif");
            assert!(
                result.is_some(),
                "Sprite file exists at {} but resolve_path returned None. engine_dir={}",
                sprite_path.display(),
                engine_dir.display(),
            );
        }
    }

    /// Test dual-directory routing: case/ and defaults/ resolve from data_dir,
    /// while engine files resolve from engine_dir. This is the Android-compatible split.
    #[test]
    fn test_resolve_path_dual_directory_routing() {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();

        // Engine files in engine_dir
        std::fs::write(engine.path().join("player.html"), "<html>engine</html>").unwrap();
        let js_dir = engine.path().join("Javascript");
        std::fs::create_dir_all(&js_dir).unwrap();
        std::fs::write(js_dir.join("common.js"), "// js").unwrap();

        // Data files in data_dir (separate directory, simulating Android)
        let case_dir = data.path().join("case/42/assets");
        std::fs::create_dir_all(&case_dir).unwrap();
        std::fs::write(case_dir.join("photo.png"), "img data").unwrap();
        let defaults_dir = data.path().join("defaults/images/chars");
        std::fs::create_dir_all(&defaults_dir).unwrap();
        std::fs::write(defaults_dir.join("Apollo.gif"), "gif data").unwrap();

        let config = ServerConfig {
            engine_dir: engine.path().to_path_buf(),
            data_dir: data.path().to_path_buf(),
        };

        // Engine files should resolve from engine_dir
        assert!(resolve_path(&config, "player.html").is_some(), "player.html from engine_dir");
        assert!(resolve_path(&config, "Javascript/common.js").is_some(), "JS from engine_dir");

        // Data files should resolve from data_dir
        assert!(resolve_path(&config, "case/42/assets/photo.png").is_some(), "case asset from data_dir");
        assert!(resolve_path(&config, "defaults/images/chars/Apollo.gif").is_some(), "default from data_dir");

        // Engine-dir should NOT have case/ or defaults/ (they're only in data_dir)
        assert!(!engine.path().join("case/42/assets/photo.png").exists());
        assert!(!engine.path().join("defaults/images/chars/Apollo.gif").exists());

        // Data-dir should NOT have engine files
        assert!(!data.path().join("player.html").exists());
        assert!(!data.path().join("Javascript/common.js").exists());
    }

    // =====================================================================
    // Phase 1a: Regression tests with split engine_dir / data_dir
    // =====================================================================

    /// Helper: create a ServerConfig with separate engine and data directories.
    fn test_config_split(engine: &Path, data: &Path) -> ServerConfig {
        ServerConfig {
            engine_dir: engine.to_path_buf(),
            data_dir: data.to_path_buf(),
        }
    }

    /// plugins/ must resolve from data_dir (not engine_dir) in split-dir mode.
    #[test]
    fn test_resolve_path_split_plugins_from_data_dir() {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let plugins_dir = data.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("myplugin.js"), "// plugin").unwrap();

        let config = test_config_split(engine.path(), data.path());
        let path = resolve_path(&config, "plugins/myplugin.js");
        assert!(path.is_some(), "plugins/ must resolve from data_dir");
        assert!(path.unwrap().starts_with(data.path()), "must come from data_dir");
    }

    /// case/ assets must resolve from data_dir, NOT engine_dir, even if engine_dir
    /// has a file at the same relative path.
    #[test]
    fn test_resolve_path_split_case_prefers_data_dir() {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();

        // Put file in BOTH dirs
        let engine_case = engine.path().join("case/99/assets");
        std::fs::create_dir_all(&engine_case).unwrap();
        std::fs::write(engine_case.join("img.png"), "engine copy").unwrap();

        let data_case = data.path().join("case/99/assets");
        std::fs::create_dir_all(&data_case).unwrap();
        std::fs::write(data_case.join("img.png"), "data copy").unwrap();

        let config = test_config_split(engine.path(), data.path());
        let path = resolve_path(&config, "case/99/assets/img.png");
        assert!(path.is_some());
        // Must resolve from data_dir
        assert!(
            path.as_ref().unwrap().starts_with(data.path()),
            "case/ must resolve from data_dir, got: {}",
            path.unwrap().display()
        );
    }

    /// defaults/ must resolve from data_dir in split mode.
    #[test]
    fn test_resolve_path_split_defaults_from_data_dir() {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();

        let defaults_dir = data.path().join("defaults/sounds");
        std::fs::create_dir_all(&defaults_dir).unwrap();
        std::fs::write(defaults_dir.join("beep.mp3"), "audio").unwrap();

        let config = test_config_split(engine.path(), data.path());
        let path = resolve_path(&config, "defaults/sounds/beep.mp3");
        assert!(path.is_some(), "defaults/ must resolve from data_dir");
        assert!(path.unwrap().starts_with(data.path()));
    }

    /// Engine file only in data_dir must NOT resolve (engine paths only check engine_dir).
    #[test]
    fn test_resolve_path_split_engine_file_not_in_data_dir() {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();

        // Put engine file ONLY in data_dir (wrong location)
        let js_dir = data.path().join("Javascript");
        std::fs::create_dir_all(&js_dir).unwrap();
        std::fs::write(js_dir.join("test.js"), "// js").unwrap();

        let config = test_config_split(engine.path(), data.path());
        let path = resolve_path(&config, "Javascript/test.js");
        assert!(path.is_none(), "Engine file in data_dir only must NOT resolve");
    }

    /// Empty path with split dirs still resolves player.html from engine_dir.
    #[test]
    fn test_resolve_path_split_empty_path_serves_player() {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        std::fs::write(engine.path().join("player.html"), "<html>").unwrap();

        let config = test_config_split(engine.path(), data.path());
        let path = resolve_path(&config, "");
        assert!(path.is_some());
        assert!(path.unwrap().starts_with(engine.path()));
    }

    /// Path traversal blocked in split-dir mode.
    #[test]
    fn test_resolve_path_split_blocks_traversal() {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let config = test_config_split(engine.path(), data.path());
        assert!(resolve_path(&config, "../etc/passwd").is_none());
        assert!(resolve_path(&config, "case/../../../etc/passwd").is_none());
    }

    /// defaults/ falls through to engine_dir if not found in data_dir.
    #[test]
    fn test_resolve_path_split_defaults_fallthrough_to_engine() {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();

        // Put defaults file only in engine_dir (bundled defaults future-proof)
        let defaults_dir = engine.path().join("defaults/images/backgrounds");
        std::fs::create_dir_all(&defaults_dir).unwrap();
        std::fs::write(defaults_dir.join("Court.jpg"), "img").unwrap();

        let config = test_config_split(engine.path(), data.path());
        let path = resolve_path(&config, "defaults/images/backgrounds/Court.jpg");
        assert!(path.is_some(), "defaults/ should fall through to engine_dir");
        assert!(path.unwrap().starts_with(engine.path()));
    }

    /// CSS files resolve from engine_dir only.
    #[test]
    fn test_resolve_path_split_css_from_engine() {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let css_dir = engine.path().join("CSS");
        std::fs::create_dir_all(&css_dir).unwrap();
        std::fs::write(css_dir.join("style.css"), "body{}").unwrap();

        let config = test_config_split(engine.path(), data.path());
        let path = resolve_path(&config, "CSS/style.css");
        assert!(path.is_some());
        assert!(path.unwrap().starts_with(engine.path()));
    }

    /// img/ files resolve from engine_dir.
    #[test]
    fn test_resolve_path_split_img_from_engine() {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let img_dir = engine.path().join("img");
        std::fs::create_dir_all(&img_dir).unwrap();
        std::fs::write(img_dir.join("icon.png"), "png").unwrap();

        let config = test_config_split(engine.path(), data.path());
        let path = resolve_path(&config, "img/icon.png");
        assert!(path.is_some());
        assert!(path.unwrap().starts_with(engine.path()));
    }

    // --- mime_type: full extension coverage ---

    #[test]
    fn test_mime_type_all_supported_extensions() {
        // Text/code types
        assert_eq!(mime_type(Path::new("f.html")), "text/html; charset=utf-8");
        assert_eq!(mime_type(Path::new("f.js")), "application/javascript; charset=utf-8");
        assert_eq!(mime_type(Path::new("f.css")), "text/css; charset=utf-8");
        assert_eq!(mime_type(Path::new("f.json")), "application/json; charset=utf-8");
        assert_eq!(mime_type(Path::new("f.txt")), "text/plain; charset=utf-8");
        assert_eq!(mime_type(Path::new("f.xml")), "application/xml");
        // Image types
        assert_eq!(mime_type(Path::new("f.png")), "image/png");
        assert_eq!(mime_type(Path::new("f.jpg")), "image/jpeg");
        assert_eq!(mime_type(Path::new("f.jpeg")), "image/jpeg");
        assert_eq!(mime_type(Path::new("f.gif")), "image/gif");
        assert_eq!(mime_type(Path::new("f.svg")), "image/svg+xml");
        assert_eq!(mime_type(Path::new("f.ico")), "image/x-icon");
        assert_eq!(mime_type(Path::new("f.webp")), "image/webp");
        assert_eq!(mime_type(Path::new("f.bmp")), "image/bmp");
        assert_eq!(mime_type(Path::new("f.avif")), "image/avif");
        // Audio types
        assert_eq!(mime_type(Path::new("f.mp3")), "audio/mpeg");
        assert_eq!(mime_type(Path::new("f.ogg")), "audio/ogg");
        assert_eq!(mime_type(Path::new("f.oga")), "audio/ogg");
        assert_eq!(mime_type(Path::new("f.opus")), "audio/opus");
        assert_eq!(mime_type(Path::new("f.wav")), "audio/wav");
        assert_eq!(mime_type(Path::new("f.m4a")), "audio/mp4");
        assert_eq!(mime_type(Path::new("f.aac")), "audio/mp4");
        assert_eq!(mime_type(Path::new("f.flac")), "audio/flac");
        assert_eq!(mime_type(Path::new("f.mid")), "audio/midi");
        assert_eq!(mime_type(Path::new("f.midi")), "audio/midi");
        // Video types
        assert_eq!(mime_type(Path::new("f.webm")), "video/webm");
        assert_eq!(mime_type(Path::new("f.mp4")), "video/mp4");
        assert_eq!(mime_type(Path::new("f.m4v")), "video/mp4");
        // Font types
        assert_eq!(mime_type(Path::new("f.woff")), "font/woff");
        assert_eq!(mime_type(Path::new("f.woff2")), "font/woff2");
        assert_eq!(mime_type(Path::new("f.ttf")), "font/ttf");
        assert_eq!(mime_type(Path::new("f.otf")), "font/otf");
        // Archive types
        assert_eq!(mime_type(Path::new("f.zip")), "application/zip");
        // Unknown
        assert_eq!(mime_type(Path::new("f.xyz")), "application/octet-stream");
        assert_eq!(mime_type(Path::new("no_ext")), "application/octet-stream");
    }

    /// mime_type is case-insensitive (extension .PNG should match .png).
    #[test]
    fn test_mime_type_case_insensitive() {
        assert_eq!(mime_type(Path::new("f.PNG")), "image/png");
        assert_eq!(mime_type(Path::new("f.MP3")), "audio/mpeg");
        assert_eq!(mime_type(Path::new("f.Html")), "text/html; charset=utf-8");
    }

    // --- url_decode: edge cases ---

    /// Double-encoded %2520 should only decode once to %20 (not to space).
    #[test]
    fn test_url_decode_double_encoded() {
        assert_eq!(url_decode("file%2520name.txt"), "file%20name.txt");
    }

    /// Truncated percent at end of string: %2 with missing second hex digit.
    #[test]
    fn test_url_decode_truncated_percent() {
        // %2 at end — only one hex digit available, second defaults to '0'
        let result = url_decode("test%2");
        // Implementation consumes next byte as hi, then next as lo (defaults to '0')
        // hi=b'2' → Some(2), lo=b'0' (default) → Some(0), so byte = 0x20 = space
        assert_eq!(result, "test ");
    }

    /// Empty string decodes to empty string.
    #[test]
    fn test_url_decode_empty() {
        assert_eq!(url_decode(""), "");
    }

    /// Single percent at very end.
    #[test]
    fn test_url_decode_percent_at_end() {
        let result = url_decode("test%");
        // % consumed, hi defaults to '0', lo defaults to '0' → byte 0x00
        assert_eq!(result, "test\0");
    }

    // --- sanitize_path: all special characters ---

    /// All illegal Windows characters replaced with underscore.
    #[test]
    fn test_sanitize_path_all_special_chars() {
        assert_eq!(
            sanitize_path("a:b*c?d\"e<f>g|h"),
            "a_b_c_d_e_f_g_h"
        );
    }

    /// Forward slashes preserved, backslashes normalized to forward slashes.
    #[test]
    fn test_sanitize_path_normalizes_backslashes() {
        assert_eq!(
            sanitize_path("dir/sub\\file:name.txt"),
            "dir/sub/file_name.txt"
        );
    }

    // =====================================================================
    // Phase 2a: Contract tests for serve_file
    // =====================================================================

    /// Helper: create a split config with test files for serve_file tests.
    fn setup_serve_file_dirs() -> (tempfile::TempDir, tempfile::TempDir, ServerConfig) {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();

        // Engine files
        std::fs::write(engine.path().join("player.html"), "<html>player</html>").unwrap();
        let js_dir = engine.path().join("Javascript");
        std::fs::create_dir_all(&js_dir).unwrap();
        std::fs::write(js_dir.join("ace.js"), "var ace = {};").unwrap();
        let css_dir = engine.path().join("CSS");
        std::fs::create_dir_all(&css_dir).unwrap();
        std::fs::write(css_dir.join("main.css"), "body{}").unwrap();

        // Data files
        let case_dir = data.path().join("case/42/assets");
        std::fs::create_dir_all(&case_dir).unwrap();
        std::fs::write(case_dir.join("photo.png"), b"PNG_DATA_HERE").unwrap();
        std::fs::write(case_dir.join("bgm.mp3"), vec![0xFFu8; 1000]).unwrap();
        std::fs::write(case_dir.join("sfx.ogg"), vec![0xAA; 500]).unwrap();

        let defaults_dir = data.path().join("defaults/sounds");
        std::fs::create_dir_all(&defaults_dir).unwrap();
        std::fs::write(defaults_dir.join("beep.wav"), vec![0xBB; 200]).unwrap();

        let config = ServerConfig {
            engine_dir: engine.path().to_path_buf(),
            data_dir: data.path().to_path_buf(),
        };

        (engine, data, config)
    }

    /// GET existing file → 200 with correct Content-Type and CORS.
    #[test]
    fn test_serve_file_get_existing() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/player.html", "GET", None);
        assert_eq!(result.status, 200);
        assert_eq!(result.data, b"<html>player</html>");
        assert_eq!(result.header("Content-Type"), Some("text/html; charset=utf-8"));
        assert_eq!(result.header("Access-Control-Allow-Origin"), Some("*"));
    }

    /// GET missing file → 404.
    #[test]
    fn test_serve_file_get_missing() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/nonexistent.txt", "GET", None);
        assert_eq!(result.status, 404);
    }

    /// GET with query string stripped for resolution.
    #[test]
    fn test_serve_file_strips_query_string() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/player.html?trial_id=123", "GET", None);
        assert_eq!(result.status, 200);
        assert_eq!(result.data, b"<html>player</html>");
    }

    /// GET URL-encoded path decoded correctly.
    #[test]
    fn test_serve_file_url_encoded_path() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/case/42/assets/photo.png", "GET", None);
        assert_eq!(result.status, 200);
        assert_eq!(result.data, b"PNG_DATA_HERE");
    }

    /// GET path with colon (sanitized to underscore).
    #[test]
    fn test_serve_file_sanitizes_colon() {
        let (engine, _data, config) = setup_serve_file_dirs();
        // Create file with sanitized name
        let js_dir = engine.path().join("Javascript");
        std::fs::write(js_dir.join("game_utils.js"), "// code").unwrap();
        let result = serve_file(&config, "/Javascript/game:utils.js", "GET", None);
        assert_eq!(result.status, 200);
        assert_eq!(result.data, b"// code");
    }

    /// OPTIONS any path → 204 with CORS headers, empty body.
    #[test]
    fn test_serve_file_options_preflight() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/anything", "OPTIONS", None);
        assert_eq!(result.status, 204);
        assert!(result.data.is_empty(), "OPTIONS body must be empty");
        assert_eq!(result.header("Access-Control-Allow-Origin"), Some("*"));
        assert_eq!(result.header("Access-Control-Allow-Methods"), Some("GET, OPTIONS"));
        assert!(
            result.header("Access-Control-Allow-Headers")
                .map_or(false, |v| v.contains("Range")),
            "Must allow Range header"
        );
    }

    /// Range: bytes=0-99 → 206 with Content-Range and correct slice.
    #[test]
    fn test_serve_file_range_first_100_bytes() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/case/42/assets/bgm.mp3", "GET", Some("bytes=0-99"));
        assert_eq!(result.status, 206);
        assert_eq!(result.data.len(), 100);
        assert!(result.data.iter().all(|&b| b == 0xFF));
        let cr = result.header("Content-Range").expect("must have Content-Range");
        assert!(cr.starts_with("bytes 0-99/1000"), "Content-Range: {}", cr);
    }

    /// Range: bytes=50- (open end) → 206 from offset 50 to EOF.
    #[test]
    fn test_serve_file_range_open_end() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/case/42/assets/sfx.ogg", "GET", Some("bytes=50-"));
        assert_eq!(result.status, 206);
        assert_eq!(result.data.len(), 450); // 500 - 50
        let cr = result.header("Content-Range").expect("must have Content-Range");
        assert!(cr.starts_with("bytes 50-499/500"), "Content-Range: {}", cr);
    }

    /// Range: bytes=-100 (suffix) → 206 last 100 bytes.
    #[test]
    fn test_serve_file_range_suffix() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/case/42/assets/bgm.mp3", "GET", Some("bytes=-100"));
        assert_eq!(result.status, 206);
        assert_eq!(result.data.len(), 100);
        let cr = result.header("Content-Range").expect("must have Content-Range");
        assert!(cr.starts_with("bytes 900-999/1000"), "Content-Range: {}", cr);
    }

    /// Range on missing file → 404 (not 206).
    #[test]
    fn test_serve_file_range_missing_file() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/missing.mp3", "GET", Some("bytes=0-99"));
        assert_eq!(result.status, 404);
    }

    /// Invalid Range (start > end) → 416 Range Not Satisfiable.
    #[test]
    fn test_serve_file_range_invalid_start_gt_end() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/case/42/assets/bgm.mp3", "GET", Some("bytes=500-100"));
        assert_eq!(result.status, 416);
        let cr = result.header("Content-Range").expect("must have Content-Range on 416");
        assert!(cr.contains("*/1000"), "Content-Range should show total size: {}", cr);
    }

    /// Invalid Range (start beyond file size) → 416.
    #[test]
    fn test_serve_file_range_beyond_size() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/case/42/assets/bgm.mp3", "GET", Some("bytes=5000-"));
        assert_eq!(result.status, 416);
    }

    /// Media files (mp3, ogg, wav) include Accept-Ranges: bytes.
    #[test]
    fn test_serve_file_accept_ranges_media() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/case/42/assets/bgm.mp3", "GET", None);
        assert_eq!(result.status, 200);
        assert_eq!(result.header("Accept-Ranges"), Some("bytes"));
    }

    /// Non-media files (html, js) do NOT include Accept-Ranges.
    #[test]
    fn test_serve_file_no_accept_ranges_non_media() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/player.html", "GET", None);
        assert_eq!(result.status, 200);
        assert!(result.header("Accept-Ranges").is_none(),
            "Non-media files should not have Accept-Ranges");
    }

    /// Cache-Control: case/ and defaults/ get immutable, engine paths get no-cache.
    #[test]
    fn test_serve_file_cache_control() {
        let (_engine, _data, config) = setup_serve_file_dirs();

        // Engine file → no-cache
        let result = serve_file(&config, "/player.html", "GET", None);
        assert_eq!(result.header("Cache-Control"), Some("no-cache"));

        // Case asset → immutable
        let result = serve_file(&config, "/case/42/assets/photo.png", "GET", None);
        let cache = result.header("Cache-Control").unwrap_or("");
        assert!(cache.contains("immutable"), "case/ should be immutable, got: {}", cache);

        // Default asset → immutable
        let result = serve_file(&config, "/defaults/sounds/beep.wav", "GET", None);
        let cache = result.header("Cache-Control").unwrap_or("");
        assert!(cache.contains("immutable"), "defaults/ should be immutable, got: {}", cache);
    }

    /// OPTIONS includes Access-Control-Expose-Headers for Range responses.
    #[test]
    fn test_serve_file_options_expose_headers() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/any", "OPTIONS", None);
        assert!(
            result.header("Access-Control-Expose-Headers")
                .map_or(false, |v| v.contains("Content-Range")),
            "Must expose Content-Range header for JS fetch"
        );
    }

    /// Root path (/) resolves to player.html.
    #[test]
    fn test_serve_file_root_path() {
        let (_engine, _data, config) = setup_serve_file_dirs();
        let result = serve_file(&config, "/", "GET", None);
        assert_eq!(result.status, 200);
        assert_eq!(result.data, b"<html>player</html>");
    }

    // =====================================================================
    // Phase 3c: serve_result_to_response conversion tests
    // =====================================================================

    /// Helper to read a header from an http::Response.
    fn resp_header(resp: &tauri::http::Response<Vec<u8>>, key: &str) -> Option<String> {
        resp.headers()
            .get(key)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    }

    /// 200 response preserves status, headers, and body.
    #[test]
    fn test_serve_result_to_response_200() {
        let result = ServeResult {
            status: 200,
            headers: vec![
                ("Content-Type".into(), "text/html; charset=utf-8".into()),
                ("Cache-Control".into(), "no-cache".into()),
            ],
            data: b"<html>test</html>".to_vec(),
        };
        let resp = serve_result_to_response(result);
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(resp_header(&resp, "Content-Type").as_deref(), Some("text/html; charset=utf-8"));
        assert_eq!(resp_header(&resp, "Cache-Control").as_deref(), Some("no-cache"));
        assert_eq!(resp.body(), b"<html>test</html>");
    }

    /// 204 OPTIONS response has empty body and CORS headers.
    #[test]
    fn test_serve_result_to_response_204() {
        let result = ServeResult {
            status: 204,
            headers: vec![
                ("Access-Control-Allow-Origin".into(), "*".into()),
                ("Access-Control-Allow-Methods".into(), "GET, OPTIONS".into()),
            ],
            data: Vec::new(),
        };
        let resp = serve_result_to_response(result);
        assert_eq!(resp.status().as_u16(), 204);
        assert!(resp.body().is_empty());
        assert_eq!(resp_header(&resp, "Access-Control-Allow-Origin").as_deref(), Some("*"));
        assert_eq!(resp_header(&resp, "Access-Control-Allow-Methods").as_deref(), Some("GET, OPTIONS"));
    }

    /// 206 Range response preserves Content-Range header.
    #[test]
    fn test_serve_result_to_response_206() {
        let result = ServeResult {
            status: 206,
            headers: vec![
                ("Content-Range".into(), "bytes 0-99/1000".into()),
                ("Content-Type".into(), "audio/mpeg".into()),
            ],
            data: vec![0xFF; 100],
        };
        let resp = serve_result_to_response(result);
        assert_eq!(resp.status().as_u16(), 206);
        assert_eq!(resp_header(&resp, "Content-Range").as_deref(), Some("bytes 0-99/1000"));
        assert_eq!(resp.body().len(), 100);
    }

    /// 404 response preserves status and body.
    #[test]
    fn test_serve_result_to_response_404() {
        let result = ServeResult {
            status: 404,
            headers: vec![
                ("Access-Control-Allow-Origin".into(), "*".into()),
            ],
            data: b"404 Not Found".to_vec(),
        };
        let resp = serve_result_to_response(result);
        assert_eq!(resp.status().as_u16(), 404);
        assert_eq!(resp.body(), b"404 Not Found");
    }

    /// 416 Range Not Satisfiable preserves Content-Range with total size.
    #[test]
    fn test_serve_result_to_response_416() {
        let result = ServeResult {
            status: 416,
            headers: vec![
                ("Content-Range".into(), "bytes */5000".into()),
            ],
            data: Vec::new(),
        };
        let resp = serve_result_to_response(result);
        assert_eq!(resp.status().as_u16(), 416);
        assert_eq!(resp_header(&resp, "Content-Range").as_deref(), Some("bytes */5000"));
        assert!(resp.body().is_empty());
    }

    // =====================================================================
    // Phase 4a: E2E test — all asset path types resolve via serve_file
    // =====================================================================

    /// Comprehensive test with realistic split engine_dir/data_dir layout
    /// covering every path type the AAO engine uses.
    #[test]
    fn test_serve_file_e2e_all_asset_types() {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();

        // --- Engine files (static, read-only on mobile) ---
        std::fs::write(engine.path().join("player.html"), "<html>player</html>").unwrap();
        std::fs::write(engine.path().join("bridge.js"), "var bridge = {};").unwrap();

        let js_dir = engine.path().join("Javascript");
        std::fs::create_dir_all(&js_dir).unwrap();
        std::fs::write(js_dir.join("player.js"), "// player code").unwrap();

        let css_dir = engine.path().join("CSS");
        std::fs::create_dir_all(&css_dir).unwrap();
        std::fs::write(css_dir.join("style.css"), "body{}").unwrap();

        let lang_dir = engine.path().join("Languages");
        std::fs::create_dir_all(&lang_dir).unwrap();
        std::fs::write(lang_dir.join("en.json"), r#"{"ok":"OK"}"#).unwrap();

        let img_dir = engine.path().join("img");
        std::fs::create_dir_all(&img_dir).unwrap();
        std::fs::write(img_dir.join("icon.png"), b"PNG").unwrap();

        // --- Data files (writable, runtime-created) ---
        let case_dir = data.path().join("case/69063");
        std::fs::create_dir_all(case_dir.join("assets")).unwrap();
        std::fs::write(case_dir.join("trial_data.json"), r#"{"id":69063}"#).unwrap();
        std::fs::write(case_dir.join("assets/sprite-abc123.gif"), b"GIF89a").unwrap();

        let chars_dir = data.path().join("defaults/images/chars/Apollo");
        std::fs::create_dir_all(&chars_dir).unwrap();
        std::fs::write(chars_dir.join("1.gif"), b"GIF89a").unwrap();

        let music_dir = data.path().join("defaults/music/Ace Attorney Investigations _ Miles");
        std::fs::create_dir_all(&music_dir).unwrap();
        std::fs::write(music_dir.join("song.mp3"), vec![0xFF; 500]).unwrap();

        let sounds_dir = data.path().join("defaults/sounds");
        std::fs::create_dir_all(&sounds_dir).unwrap();
        std::fs::write(sounds_dir.join("sfx.mp3"), vec![0xAA; 200]).unwrap();

        let voices_dir = data.path().join("defaults/voices");
        std::fs::create_dir_all(&voices_dir).unwrap();
        std::fs::write(voices_dir.join("voice_singleblip_1.opus"), b"OggS").unwrap();

        let plugins_dir = data.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("myplugin.js"), "// plugin").unwrap();

        let config = ServerConfig {
            engine_dir: engine.path().to_path_buf(),
            data_dir: data.path().to_path_buf(),
        };

        // Test all engine paths → 200 with correct Content-Type
        let cases = vec![
            ("/player.html", 200, "text/html"),
            ("/bridge.js", 200, "application/javascript"),
            ("/Javascript/player.js", 200, "application/javascript"),
            ("/CSS/style.css", 200, "text/css"),
            ("/Languages/en.json", 200, "application/json"),
            ("/img/icon.png", 200, "image/png"),
            // Data paths
            ("/case/69063/trial_data.json", 200, "application/json"),
            ("/case/69063/assets/sprite-abc123.gif", 200, "image/gif"),
            ("/defaults/images/chars/Apollo/1.gif", 200, "image/gif"),
            ("/defaults/music/Ace%20Attorney%20Investigations%20_%20Miles/song.mp3", 200, "audio/mpeg"),
            ("/defaults/sounds/sfx.mp3", 200, "audio/mpeg"),
            ("/defaults/voices/voice_singleblip_1.opus", 200, "audio/opus"),
            ("/plugins/myplugin.js", 200, "application/javascript"),
            // Missing file
            ("/nonexistent.txt", 404, ""),
        ];

        for (path, expected_status, expected_mime) in &cases {
            let result = serve_file(&config, path, "GET", None);
            assert_eq!(
                result.status, *expected_status,
                "Path '{}': expected status {}, got {}",
                path, expected_status, result.status
            );
            if *expected_status == 200 {
                let ct = result.header("Content-Type").unwrap_or("");
                assert!(
                    ct.starts_with(expected_mime),
                    "Path '{}': expected Content-Type starting with '{}', got '{}'",
                    path, expected_mime, ct
                );
            }
        }

        // Range request on audio file → 206
        let result = serve_file(&config, "/defaults/sounds/sfx.mp3", "GET", Some("bytes=0-49"));
        assert_eq!(result.status, 206, "Range request should return 206");
        assert_eq!(result.data.len(), 50);
        assert!(result.header("Content-Range").unwrap().starts_with("bytes 0-49/200"));
        assert_eq!(result.header("Accept-Ranges"), Some("bytes"));

        // OPTIONS → 204
        let result = serve_file(&config, "/anything", "OPTIONS", None);
        assert_eq!(result.status, 204);
        assert!(result.data.is_empty());
        assert_eq!(result.header("Access-Control-Allow-Origin"), Some("*"));
    }

    // =====================================================================
    // VFS pointer resolution tests
    // =====================================================================

    /// VFS pointer: resolve_path follows pointer to the real file.
    #[test]
    fn test_resolve_path_vfs_pointer() {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();

        // Real file at the talking sprite path
        let chars_dir = data.path().join("defaults/images/chars/Apollo");
        std::fs::create_dir_all(&chars_dir).unwrap();
        std::fs::write(chars_dir.join("3.gif"), b"GIF89a real sprite data").unwrap();

        // VFS pointer at the still sprite path
        let still_dir = data.path().join("defaults/images/charsStill/Apollo");
        std::fs::create_dir_all(&still_dir).unwrap();
        crate::downloader::vfs::write_vfs_pointer(
            &still_dir.join("3.gif"),
            "defaults/images/chars/Apollo/3.gif",
        ).unwrap();

        let config = test_config_split(engine.path(), data.path());
        let result = resolve_path(&config, "defaults/images/charsStill/Apollo/3.gif");
        assert!(result.is_some(), "VFS pointer must resolve to the target file");
        let resolved = result.unwrap();
        assert!(resolved.ends_with("defaults/images/chars/Apollo/3.gif"),
            "Should resolve to talking sprite, got: {}", resolved.display());
    }

    /// VFS pointer with broken target returns None (404).
    #[test]
    fn test_resolve_path_vfs_pointer_broken() {
        let data = tempfile::tempdir().unwrap();

        // VFS pointer to nonexistent target
        let still_dir = data.path().join("defaults/images/charsStill/Apollo");
        std::fs::create_dir_all(&still_dir).unwrap();
        crate::downloader::vfs::write_vfs_pointer(
            &still_dir.join("3.gif"),
            "defaults/images/chars/Apollo/3.gif", // doesn't exist
        ).unwrap();

        let config = test_config(data.path());
        let result = resolve_path(&config, "defaults/images/charsStill/Apollo/3.gif");
        assert!(result.is_none(), "Broken VFS pointer should return None (404)");
    }

    /// serve_file with VFS pointer returns 200 with the target's content.
    #[test]
    fn test_serve_file_vfs_pointer() {
        let engine = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();

        let gif_content = b"GIF89a this is Apollo talking sprite 3";

        // Real file
        let chars_dir = data.path().join("defaults/images/chars/Apollo");
        std::fs::create_dir_all(&chars_dir).unwrap();
        std::fs::write(chars_dir.join("3.gif"), gif_content).unwrap();

        // VFS pointer
        let still_dir = data.path().join("defaults/images/charsStill/Apollo");
        std::fs::create_dir_all(&still_dir).unwrap();
        crate::downloader::vfs::write_vfs_pointer(
            &still_dir.join("3.gif"),
            "defaults/images/chars/Apollo/3.gif",
        ).unwrap();

        // Engine needs player.html (for root path)
        std::fs::write(engine.path().join("player.html"), "<html>").unwrap();

        let config = ServerConfig {
            engine_dir: engine.path().to_path_buf(),
            data_dir: data.path().to_path_buf(),
        };

        // Request the still path (VFS pointer) → should get the talking sprite content
        let result = serve_file(&config, "/defaults/images/charsStill/Apollo/3.gif", "GET", None);
        assert_eq!(result.status, 200, "VFS pointer path should return 200");
        assert_eq!(result.data, gif_content, "Body should be the real GIF content, not pointer text");
        assert_eq!(result.header("Content-Type"), Some("image/gif"));

        // Request the real path → same content
        let result2 = serve_file(&config, "/defaults/images/chars/Apollo/3.gif", "GET", None);
        assert_eq!(result2.status, 200);
        assert_eq!(result2.data, gif_content);

        // Both return identical data
        assert_eq!(result.data, result2.data, "Both paths should serve the same content");
    }
}
