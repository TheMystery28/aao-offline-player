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

/// MIME type lookup based on file extension.
fn mime_type(path: &Path) -> &'static str {
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
        "mp3" => "audio/mpeg",
        "ogg" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "ico" => "image/x-icon",
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
pub fn start_server(config: ServerConfig) -> u16 {
    let port = portpicker::pick_unused_port().expect("failed to find unused port");
    let config = Arc::new(config);

    std::thread::spawn(move || {
        let server = Server::http(format!("localhost:{}", port))
            .expect("Unable to start asset server");

        for request in server.incoming_requests() {
            let config = Arc::clone(&config);
            std::thread::spawn(move || {
                handle_request(request, &config);
            });
        }
    });

    port
}

/// Handle a single HTTP request (runs in its own thread).
fn handle_request(request: tiny_http::Request, config: &ServerConfig) {
    let url_path = request.url().to_string();
    let method = request.method().to_string();
    // Strip query string for file resolution
    let clean_path = url_path.split('?').next().unwrap_or(&url_path);
    // URL-decode the path
    let decoded_path = url_decode(clean_path);
    // Remove leading slash
    let relative = decoded_path.trim_start_matches('/');
    // Sanitize illegal Windows characters (e.g. colons in AAO music paths)
    let relative = sanitize_path(relative);

    let file_path = resolve_path(config, &relative);

    if cfg!(debug_assertions) {
        let is_found = file_path.is_some();
        if is_found {
            eprintln!(
                "[SERVER 200] {} {} | relative=\"{}\" | resolved={}",
                method,
                url_path,
                relative,
                file_path.as_ref().unwrap().display()
            );
        } else {
            // For case/defaults paths, show the data_dir path in debug output
            let attempted = if relative.starts_with("case/") || relative.starts_with("defaults/") {
                config.data_dir.join(&relative)
            } else {
                config.engine_dir.join(&relative)
            };
            let exists = attempted.exists();
            let is_file = attempted.is_file();
            let is_dir = attempted.is_dir();
            // List parent directory contents for sprite 404s to help diagnose
            let parent_listing = attempted
                .parent()
                .map(|p| {
                    if p.is_dir() {
                        match std::fs::read_dir(p) {
                            Ok(entries) => {
                                let names: Vec<String> = entries
                                    .filter_map(|e| e.ok())
                                    .take(20)
                                    .map(|e| e.file_name().to_string_lossy().to_string())
                                    .collect();
                                format!("[{}]", names.join(", "))
                            }
                            Err(e) => format!("(readdir err: {})", e),
                        }
                    } else {
                        format!("(parent not a dir: {})", p.display())
                    }
                })
                .unwrap_or_else(|| "(no parent)".to_string());
            eprintln!(
                "[SERVER 404] {} {} | relative=\"{}\" | path={} | exists={} | is_file={} | is_dir={} | engine_dir={} | parent_contents={}",
                method, url_path, relative, attempted.display(), exists, is_file, is_dir,
                config.engine_dir.display(), parent_listing
            );
        }
    }

    match file_path {
        Some(path) if path.is_file() => {
            match fs::read(&path) {
                Ok(data) => {
                    let mime = mime_type(&path);
                    // Determine caching strategy based on path
                    let cache_value =
                        if relative.starts_with("case/") || relative.starts_with("defaults/") {
                            // Case assets and defaults NEVER change after download
                            "public, max-age=31536000, immutable"
                        } else {
                            // Engine files (JS, CSS, HTML) — allow cache but revalidate
                            "no-cache"
                        };

                    let mut response = Response::from_data(data);
                    // Content-Type
                    if let Ok(h) =
                        Header::from_bytes("Content-Type".as_bytes(), mime.as_bytes())
                    {
                        response.add_header(h);
                    }
                    // CORS — allow same-origin access
                    if let Ok(h) = Header::from_bytes(
                        "Access-Control-Allow-Origin".as_bytes(),
                        "*".as_bytes(),
                    ) {
                        response.add_header(h);
                    }
                    // Cache-Control
                    if let Ok(h) = Header::from_bytes(
                        "Cache-Control".as_bytes(),
                        cache_value.as_bytes(),
                    ) {
                        response.add_header(h);
                    }
                    // Note: Connection keep-alive is handled by tiny_http at the protocol level.
                    // HTTP/1.1 defaults to keep-alive; no explicit header needed.
                    let _ = request.respond(response);
                }
                Err(e) => {
                    if cfg!(debug_assertions) {
                        eprintln!(
                            "[SERVER 500] {} {} | read error: {}",
                            method, url_path, e
                        );
                    }
                    let _ = request.respond(
                        Response::from_string("500 Internal Server Error")
                            .with_status_code(500),
                    );
                }
            }
        }
        _ => {
            let _ =
                request.respond(Response::from_string("404 Not Found").with_status_code(404));
        }
    }
}

/// Resolve a URL path to a filesystem path.
///
/// Routes writable data (case/, defaults/) to `data_dir` and
/// static engine files (JS, CSS, HTML, img, Languages) to `engine_dir`.
/// On desktop both directories are the same. On Android/iOS they differ:
/// engine_dir is the read-only bundled resources, data_dir is the app's
/// private writable storage.
fn resolve_path(config: &ServerConfig, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() || relative == "/" {
        // Serve index/player.html by default
        let path = config.engine_dir.join("player.html");
        return if path.is_file() { Some(path) } else { None };
    }

    // Security: prevent path traversal
    if relative.contains("..") {
        return None;
    }

    // Route case/ and defaults/ to the writable data directory.
    // These are downloaded at runtime and must be writable (important on Android
    // where bundled resources are read-only).
    if relative.starts_with("case/") || relative.starts_with("defaults/") {
        let path = config.data_dir.join(relative);
        if path.is_file() {
            return Some(path);
        }
        // Fall through to engine_dir in case defaults are bundled (future-proof)
    }

    // Static engine files (JS, CSS, HTML, img, Languages) from engine_dir.
    let path = config.engine_dir.join(relative);

    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

/// Sanitize path for Windows by replacing illegal characters.
/// Must match the sanitization in asset_resolver::sanitize_path.
fn sanitize_path(path: &str) -> String {
    path.chars()
        .map(|c| match c {
            ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// URL path decoding (handles %XX sequences only).
///
/// NOTE: `+` is kept as a literal character. In URL *paths*, `+` is literal.
/// Only in query strings (application/x-www-form-urlencoded) does `+` mean space.
/// Spaces in URL paths are encoded as `%20`.
fn url_decode(input: &str) -> String {
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

    String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn hex_val(b: u8) -> Option<u8> {
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
        // Simulates what the server does: decode URL, then sanitize
        let url = "/defaults/music/Ace%20Attorney%20Investigations%20%3A%20Miles/song.mp3";
        let decoded = url_decode(url);
        assert_eq!(decoded, "/defaults/music/Ace Attorney Investigations : Miles/song.mp3");
        let sanitized = sanitize_path(&decoded);
        assert_eq!(sanitized, "/defaults/music/Ace Attorney Investigations _ Miles/song.mp3");
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

    /// Create a test server with temporary files. Returns (port, _tempdir_guard).
    fn setup_test_server() -> (u16, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        // Create various test files matching the engine layout
        std::fs::write(dir.path().join("player.html"), "<html>test</html>").unwrap();

        let js_dir = dir.path().join("Javascript");
        std::fs::create_dir_all(&js_dir).unwrap();
        std::fs::write(js_dir.join("test.js"), "console.log('test');").unwrap();

        let css_dir = dir.path().join("CSS");
        std::fs::create_dir_all(&css_dir).unwrap();
        std::fs::write(css_dir.join("style.css"), "body { color: red; }").unwrap();

        let case_dir = dir.path().join("case/123/assets");
        std::fs::create_dir_all(&case_dir).unwrap();
        std::fs::write(case_dir.join("image.png"), b"PNG fake data").unwrap();
        std::fs::write(case_dir.join("music.mp3"), b"MP3 fake data").unwrap();

        let defaults_dir = dir.path().join("defaults/images/chars");
        std::fs::create_dir_all(&defaults_dir).unwrap();
        std::fs::write(defaults_dir.join("sprite.gif"), b"GIF fake data").unwrap();

        let config = test_config(dir.path());
        let port = start_server(config);

        // Give the server a moment to start accepting connections
        std::thread::sleep(std::time::Duration::from_millis(50));

        (port, dir)
    }

    #[test]
    fn test_server_serves_file_with_200() {
        let (port, _dir) = setup_test_server();
        let (status, _, body) = http_get(port, "/player.html");
        assert_eq!(status, 200);
        assert_eq!(String::from_utf8_lossy(&body), "<html>test</html>");
    }

    #[test]
    fn test_server_root_serves_player_html() {
        let (port, _dir) = setup_test_server();
        let (status, _, body) = http_get(port, "/");
        assert_eq!(status, 200);
        assert_eq!(String::from_utf8_lossy(&body), "<html>test</html>");
    }

    #[test]
    fn test_server_serves_js_with_correct_content_type() {
        let (port, _dir) = setup_test_server();
        let (status, headers, _) = http_get(port, "/Javascript/test.js");
        assert_eq!(status, 200);
        assert_eq!(
            get_header(&headers, "content-type"),
            Some("application/javascript; charset=utf-8")
        );
    }

    #[test]
    fn test_server_serves_css_with_correct_content_type() {
        let (port, _dir) = setup_test_server();
        let (status, headers, _) = http_get(port, "/CSS/style.css");
        assert_eq!(status, 200);
        assert_eq!(
            get_header(&headers, "content-type"),
            Some("text/css; charset=utf-8")
        );
    }

    #[test]
    fn test_server_returns_cors_header() {
        let (port, _dir) = setup_test_server();
        let (_, headers, _) = http_get(port, "/player.html");
        assert_eq!(
            get_header(&headers, "access-control-allow-origin"),
            Some("*")
        );
    }

    /// Regression: verify Cache-Control headers for different path types.
    /// After Phase 1A, case/* and defaults/* should get immutable caching.
    #[test]
    fn test_server_cache_control_engine_files() {
        let (port, _dir) = setup_test_server();
        let (_, headers, _) = http_get(port, "/Javascript/test.js");
        let cache = get_header(&headers, "cache-control").unwrap_or("");
        // Engine files should use no-cache (revalidate on each load)
        assert!(
            cache.contains("no-cache"),
            "Engine JS files should have no-cache, got: {}",
            cache
        );
    }

    /// Case assets (case/*) should get immutable caching — they never change after download.
    #[test]
    fn test_server_cache_control_case_assets() {
        let (port, _dir) = setup_test_server();
        let (_, headers, _) = http_get(port, "/case/123/assets/image.png");
        let cache = get_header(&headers, "cache-control").unwrap_or("");
        assert!(
            cache.contains("immutable"),
            "Case assets should have immutable caching, got: {}",
            cache
        );
    }

    /// Default assets (defaults/*) should get immutable caching — they never change after download.
    #[test]
    fn test_server_cache_control_default_assets() {
        let (port, _dir) = setup_test_server();
        let (_, headers, _) = http_get(port, "/defaults/images/chars/sprite.gif");
        let cache = get_header(&headers, "cache-control").unwrap_or("");
        assert!(
            cache.contains("immutable"),
            "Default assets should have immutable caching, got: {}",
            cache
        );
    }

    /// Verify multi-threaded server handles many concurrent requests correctly.
    /// This exercises the thread-per-request model with higher concurrency.
    #[test]
    fn test_server_high_concurrency() {
        let (port, _dir) = setup_test_server();

        let handles: Vec<_> = (0..20)
            .map(|i| {
                let path = match i % 4 {
                    0 => "/player.html",
                    1 => "/Javascript/test.js",
                    2 => "/CSS/style.css",
                    _ => "/case/123/assets/image.png",
                };
                let path = path.to_string();
                std::thread::spawn(move || http_get(port, &path))
            })
            .collect();

        for handle in handles {
            let (status, _, _) = handle.join().unwrap();
            assert_eq!(status, 200, "High-concurrency request failed");
        }
    }

    #[test]
    fn test_server_returns_404_for_missing_file() {
        let (port, _dir) = setup_test_server();
        let (status, _, body) = http_get(port, "/nonexistent.txt");
        assert_eq!(status, 404);
        assert_eq!(String::from_utf8_lossy(&body), "404 Not Found");
    }

    /// Regression: concurrent requests must all succeed.
    /// Before Phase 1B (multi-threading), the server is single-threaded
    /// and processes requests sequentially. All must still return 200.
    #[test]
    fn test_server_handles_concurrent_requests() {
        let (port, _dir) = setup_test_server();

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let path = match i % 3 {
                    0 => "/player.html",
                    1 => "/Javascript/test.js",
                    _ => "/CSS/style.css",
                };
                let path = path.to_string();
                std::thread::spawn(move || http_get(port, &path))
            })
            .collect();

        for handle in handles {
            let (status, _, _) = handle.join().unwrap();
            assert_eq!(status, 200, "Concurrent request failed");
        }
    }

    #[test]
    fn test_server_serves_case_assets_with_correct_mime() {
        let (port, _dir) = setup_test_server();
        let (status, headers, body) = http_get(port, "/case/123/assets/image.png");
        assert_eq!(status, 200);
        assert_eq!(get_header(&headers, "content-type"), Some("image/png"));
        assert_eq!(body, b"PNG fake data");

        let (status, headers, _) = http_get(port, "/case/123/assets/music.mp3");
        assert_eq!(status, 200);
        assert_eq!(get_header(&headers, "content-type"), Some("audio/mpeg"));
    }

    #[test]
    fn test_server_serves_default_assets() {
        let (port, _dir) = setup_test_server();
        let (status, headers, _) = http_get(port, "/defaults/images/chars/sprite.gif");
        assert_eq!(status, 200);
        assert_eq!(get_header(&headers, "content-type"), Some("image/gif"));
    }

    /// Regression: large files (>1MB) must be served correctly.
    #[test]
    fn test_server_serves_large_file() {
        let dir = tempfile::tempdir().unwrap();
        let large_data = vec![0x42u8; 2 * 1024 * 1024]; // 2MB
        let case_dir = dir.path().join("case/1/assets");
        std::fs::create_dir_all(&case_dir).unwrap();
        std::fs::write(case_dir.join("big.mp3"), &large_data).unwrap();

        let config = test_config(dir.path());
        let port = start_server(config);
        std::thread::sleep(std::time::Duration::from_millis(50));

        let (status, _, body) = http_get(port, "/case/1/assets/big.mp3");
        assert_eq!(status, 200);
        assert_eq!(body.len(), 2 * 1024 * 1024);
        assert!(body.iter().all(|&b| b == 0x42));
    }

    /// Regression: query strings must be stripped for file resolution.
    #[test]
    fn test_server_strips_query_string() {
        let (port, _dir) = setup_test_server();
        let (status, _, body) = http_get(port, "/player.html?v=12345");
        assert_eq!(status, 200);
        assert_eq!(String::from_utf8_lossy(&body), "<html>test</html>");
    }

    /// Regression: URL-encoded paths must be decoded correctly.
    #[test]
    fn test_server_handles_url_encoded_paths() {
        let dir = tempfile::tempdir().unwrap();
        let music_dir = dir.path().join("defaults/music/My Song");
        std::fs::create_dir_all(&music_dir).unwrap();
        std::fs::write(music_dir.join("track.mp3"), b"audio").unwrap();

        let config = test_config(dir.path());
        let port = start_server(config);
        std::thread::sleep(std::time::Duration::from_millis(50));

        let (status, _, body) = http_get(port, "/defaults/music/My%20Song/track.mp3");
        assert_eq!(status, 200);
        assert_eq!(body, b"audio");
    }

    /// Regression: the full pipeline (decode → sanitize → resolve) for special chars.
    #[test]
    fn test_server_full_pipeline_colon_in_path() {
        let dir = tempfile::tempdir().unwrap();
        // Colon gets sanitized to underscore
        let music_dir = dir.path().join("defaults/music/Game _ Title");
        std::fs::create_dir_all(&music_dir).unwrap();
        std::fs::write(music_dir.join("song.mp3"), b"audio").unwrap();

        let config = test_config(dir.path());
        let port = start_server(config);
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Browser sends colon URL-encoded as %3A
        let (status, _, _) = http_get(port, "/defaults/music/Game%20%3A%20Title/song.mp3");
        assert_eq!(status, 200);
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
}
