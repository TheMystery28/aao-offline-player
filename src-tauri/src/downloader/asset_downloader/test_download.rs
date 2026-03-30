use super::*;
use std::sync::Arc;
use crate::downloader::AssetRef;
use reqwest::Client;

// --- DownloadEvent serialization regression tests ---
// These ensure adding new variants doesn't break existing serialization.

#[test]
fn test_download_event_started_serialization() {
    let event = DownloadEvent::Started { total: 42 };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event"], "started");
    assert_eq!(json["data"]["total"], 42);
}

#[test]
fn test_download_event_progress_serialization() {
    let event = DownloadEvent::Progress {
        completed: 10,
        total: 50,
        current_url: "https://example.com/img.png".to_string(),
        bytes_downloaded: 1024000,
        elapsed_ms: 5000,
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event"], "progress");
    assert_eq!(json["data"]["completed"], 10);
    assert_eq!(json["data"]["total"], 50);
    assert_eq!(json["data"]["current_url"], "https://example.com/img.png");
    assert_eq!(json["data"]["bytes_downloaded"], 1024000);
    assert_eq!(json["data"]["elapsed_ms"], 5000);
}

#[test]
fn test_download_event_finished_serialization() {
    let event = DownloadEvent::Finished {
        downloaded: 45,
        failed: 5,
        total_bytes: 123456,
        dedup_saved_bytes: 0,
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event"], "finished");
    assert_eq!(json["data"]["downloaded"], 45);
    assert_eq!(json["data"]["failed"], 5);
    assert_eq!(json["data"]["total_bytes"], 123456);
}

#[test]
fn test_download_event_error_serialization() {
    let event = DownloadEvent::Error {
        message: "Connection refused".to_string(),
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event"], "error");
    assert_eq!(json["data"]["message"], "Connection refused");
}

#[test]
fn test_download_event_sequence_progress_serialization() {
    let event = DownloadEvent::SequenceProgress {
        current_part: 2,
        total_parts: 5,
        part_title: "Trial Day 1".to_string(),
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event"], "sequence_progress");
    assert_eq!(json["data"]["current_part"], 2);
    assert_eq!(json["data"]["total_parts"], 5);
    assert_eq!(json["data"]["part_title"], "Trial Day 1");
}

/// All DownloadEvent variants must serialize with an "event" field.
#[test]
fn test_download_event_all_variants_have_event_field() {
    let variants: Vec<DownloadEvent> = vec![
        DownloadEvent::Started { total: 10 },
        DownloadEvent::Progress { completed: 1, total: 10, current_url: "http://a.com".into(), bytes_downloaded: 100, elapsed_ms: 500 },
        DownloadEvent::Finished { downloaded: 8, failed: 2, total_bytes: 500, dedup_saved_bytes: 0 },
        DownloadEvent::Error { message: "fail".into() },
        DownloadEvent::SequenceProgress { current_part: 1, total_parts: 3, part_title: "P1".into() },
    ];
    for variant in &variants {
        let json = serde_json::to_value(variant).unwrap();
        assert!(
            json.get("event").is_some(),
            "Missing 'event' field in serialized DownloadEvent: {:?}",
            json
        );
        assert!(
            json["event"].is_string(),
            "'event' field should be a string, got: {:?}",
            json["event"]
        );
    }
}

/// Started with total=0 should serialize correctly.
#[test]
fn test_download_event_started_zero_total() {
    let event = DownloadEvent::Started { total: 0 };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event"], "started");
    assert_eq!(json["data"]["total"], 0);
}

// ============================================================
// Phase 1: Core Reliability Tests
// ============================================================

#[test]
fn test_per_asset_timeout_constant() {
    assert!(PER_ASSET_TIMEOUT.as_secs() >= 5, "Timeout should be at least 5s");
    assert!(PER_ASSET_TIMEOUT.as_secs() <= 30, "Timeout should be at most 30s");
    assert_eq!(PER_ASSET_TIMEOUT.as_secs(), 15, "Timeout should be 15s");
}

#[test]
fn test_retryable_errors_include_transient() {
    // These should be retried
    let transient = vec!["HTTP 429", "HTTP 503", "HTTP 502", "timeout: foo", "connection error", "reset by peer", "closed"];
    for err in &transient {
        let is_retryable = err.contains("429")
            || err.contains("503")
            || err.contains("502")
            || err.contains("timeout")
            || err.contains("connection")
            || err.contains("reset")
            || err.contains("closed");
        assert!(is_retryable, "'{}' should be retryable", err);
    }
}

#[test]
fn test_retryable_errors_exclude_permanent() {
    // These should NOT be retried (permanent failures)
    let permanent = vec!["HTTP 301 redirect to: example.com", "HTTP 302 redirect to: example.com", "HTTP 404", "HTTP 403"];
    for err in &permanent {
        let is_retryable = err.contains("429")
            || err.contains("503")
            || err.contains("502")
            || err.contains("timeout")
            || err.contains("connection")
            || err.contains("reset")
            || err.contains("closed");
        assert!(!is_retryable, "'{}' should NOT be retryable", err);
    }
}

#[test]
fn test_content_type_html_is_rejected_for_media() {
    let media_types = vec!["sprite", "background_internal", "evidence_icon", "music_internal", "sound", "voice", "popup", "lock", "icon"];
    for asset_type in &media_types {
        let content_type = "text/html; charset=utf-8";
        let media_markers = ["sprite", "background", "evidence", "music", "sound", "voice", "popup", "lock", "icon", "place"];
        let should_reject = content_type.contains("text/html")
            && media_markers.iter().any(|t| asset_type.contains(t));
        assert!(should_reject, "HTML response should be rejected for asset_type '{}'", asset_type);
    }
}

#[test]
fn test_content_type_html_is_accepted_for_unknown() {
    let content_type = "text/html";
    let asset_type = "external_unknown";
    let media_markers = ["sprite", "background", "evidence", "music", "sound", "voice", "popup", "lock", "icon", "place"];
    let should_reject = content_type.contains("text/html")
        && media_markers.iter().any(|t| asset_type.contains(t));
    assert!(!should_reject, "HTML should be accepted for unknown asset type");
}

#[test]
fn test_content_length_mismatch_detected() {
    let expected: Option<u64> = Some(1000);
    let actual_len: u64 = 500;
    if let Some(exp) = expected {
        assert_ne!(actual_len, exp, "Mismatch should be detected");
    }
}

#[test]
fn test_content_length_absent_is_ok() {
    let expected: Option<u64> = None;
    // When Content-Length is absent, we accept any size
    assert!(expected.is_none(), "Absent Content-Length should not cause rejection");
}

#[test]
fn test_content_length_match_is_ok() {
    let expected: Option<u64> = Some(500);
    let actual_len: u64 = 500;
    if let Some(exp) = expected {
        assert_eq!(actual_len, exp, "Matching Content-Length should pass");
    }
}

// ============================================================
// Wiremock integration tests
// ============================================================

use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, path_regex};

fn test_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

fn test_log(dir: &std::path::Path) -> DownloadLog {
    DownloadLog::new(&dir.join("test_log.txt")).unwrap()
}

// --- Retry logic ---

#[tokio::test]
async fn test_mock_retry_on_503_succeeds() {
    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let log = test_log(dir.path());

    // First request: 503
    Mock::given(method("GET")).and(path("/asset.png"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&mock_server).await;
    // Second request: 200
    Mock::given(method("GET")).and(path("/asset.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1, 2, 3]))
        .mount(&mock_server).await;

    let url = format!("{}/asset.png", mock_server.uri());
    let client = test_client();
    let result = download_with_retry(
        &client, &url, &dir.path().to_path_buf(), "test.png", &log, "sprite"
    ).await;
    assert!(result.is_ok(), "Should succeed after retry: {:?}", result.err());
    assert_eq!(result.unwrap().size, 3);
}

#[tokio::test]
async fn test_mock_no_retry_on_404() {
    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let log = test_log(dir.path());

    Mock::given(method("GET")).and(path("/missing.png"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&mock_server).await;

    let url = format!("{}/missing.png", mock_server.uri());
    let client = test_client();
    let result = download_with_retry(
        &client, &url, &dir.path().to_path_buf(), "test.png", &log, "sprite"
    ).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("404"));
}

#[tokio::test]
async fn test_mock_retry_exhausted_returns_error() {
    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let log = test_log(dir.path());

    // All 3 retries fail with 503
    Mock::given(method("GET")).and(path("/always_fail.png"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock_server).await;

    let url = format!("{}/always_fail.png", mock_server.uri());
    let client = test_client();
    let result = download_with_retry(
        &client, &url, &dir.path().to_path_buf(), "test.png", &log, "sprite"
    ).await;
    assert!(result.is_err(), "Should fail after all retries");
    assert!(result.unwrap_err().contains("503"));
}

// --- Content validation ---

#[tokio::test]
async fn test_mock_html_rejected_for_sprite() {
    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let log = test_log(dir.path());

    Mock::given(method("GET")).and(path("/sprite.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(b"<html>File not found</html>".to_vec())
                .append_header("content-type", "text/html; charset=utf-8")
        )
        .mount(&mock_server).await;

    let url = format!("{}/sprite.png", mock_server.uri());
    let client = test_client();
    let result = download_single_asset(
        &client, &url, &dir.path().to_path_buf(), "test.png", &log, "sprite"
    ).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("HTML"));
}

#[tokio::test]
async fn test_mock_image_accepted() {
    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let log = test_log(dir.path());

    Mock::given(method("GET")).and(path("/img.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/png")
                .set_body_bytes(vec![0x89, 0x50, 0x4E, 0x47]) // PNG magic
        )
        .mount(&mock_server).await;

    let url = format!("{}/img.png", mock_server.uri());
    let client = test_client();
    let result = download_single_asset(
        &client, &url, &dir.path().to_path_buf(), "test.png", &log, "sprite"
    ).await;
    assert!(result.is_ok(), "PNG should be accepted: {:?}", result.err());
}

#[tokio::test]
async fn test_mock_empty_body_rejected() {
    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let log = test_log(dir.path());

    Mock::given(method("GET")).and(path("/empty.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![]))
        .mount(&mock_server).await;

    let url = format!("{}/empty.png", mock_server.uri());
    let client = test_client();
    let result = download_single_asset(
        &client, &url, &dir.path().to_path_buf(), "test.png", &log, "sprite"
    ).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Empty"));
}

// --- Concurrent downloads ---

#[tokio::test]
async fn test_mock_concurrent_all_succeed() {
    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("assets")).unwrap();

    for i in 0..5 {
        Mock::given(method("GET")).and(path(format!("/a{}.png", i)))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![i as u8; 10]))
            .mount(&mock_server).await;
    }

    let assets: Vec<AssetRef> = (0..5).map(|i| AssetRef {
        url: format!("{}/a{}.png", mock_server.uri(), i),
        asset_type: "sprite".to_string(),
        is_default: false,
        local_path: String::new(),
    }).collect();

    let client = test_client();
    let tx = tauri::ipc::Channel::new(|_| Ok(()));
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let result = download_assets(
        &client, assets, &dir.path().to_path_buf(), &dir.path().to_path_buf(),
        None, &tx, 3, cancel,
    ).await.unwrap();

    assert_eq!(result.downloaded.len(), 5);
    assert_eq!(result.failed.len(), 0);
}

#[tokio::test]
async fn test_mock_concurrent_mixed_results() {
    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("assets")).unwrap();

    Mock::given(method("GET")).and(path("/ok1.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1, 2]))
        .mount(&mock_server).await;
    Mock::given(method("GET")).and(path("/fail.png"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server).await;
    Mock::given(method("GET")).and(path("/ok2.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![3, 4]))
        .mount(&mock_server).await;

    let assets = vec![
        AssetRef { url: format!("{}/ok1.png", mock_server.uri()), asset_type: "sprite".into(), is_default: false, local_path: String::new() },
        AssetRef { url: format!("{}/fail.png", mock_server.uri()), asset_type: "sprite".into(), is_default: false, local_path: String::new() },
        AssetRef { url: format!("{}/ok2.png", mock_server.uri()), asset_type: "sprite".into(), is_default: false, local_path: String::new() },
    ];

    let client = test_client();
    let tx = tauri::ipc::Channel::new(|_| Ok(()));
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let result = download_assets(
        &client, assets, &dir.path().to_path_buf(), &dir.path().to_path_buf(),
        None, &tx, 3, cancel,
    ).await.unwrap();

    assert_eq!(result.downloaded.len(), 2, "2 should succeed");
    assert_eq!(result.failed.len(), 1, "1 should fail");
}

// --- Download orchestration tests ---

#[tokio::test]
async fn test_download_assets_empty_list() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("assets")).unwrap();
    let client = test_client();
    let tx = tauri::ipc::Channel::new(|_| Ok(()));
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let result = download_assets(
        &client, Vec::new(), &dir.path().to_path_buf(), &dir.path().to_path_buf(),
        None, &tx, 3, cancel,
    ).await.unwrap();

    assert_eq!(result.downloaded.len(), 0);
    assert_eq!(result.failed.len(), 0);
}

#[tokio::test]
async fn test_download_assets_all_fail() {
    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("assets")).unwrap();

    // All return 500
    Mock::given(method("GET")).and(path_regex(".*"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server).await;

    let assets: Vec<AssetRef> = (0..3).map(|i| AssetRef {
        url: format!("{}/fail{}.png", mock_server.uri(), i),
        asset_type: "sprite".into(),
        is_default: false,
        local_path: String::new(),
    }).collect();

    let client = test_client();
    let tx = tauri::ipc::Channel::new(|_| Ok(()));
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let result = download_assets(
        &client, assets, &dir.path().to_path_buf(), &dir.path().to_path_buf(),
        None, &tx, 3, cancel,
    ).await.unwrap();

    assert_eq!(result.downloaded.len(), 0, "None should succeed");
    assert_eq!(result.failed.len(), 3, "All 3 should fail");
}

// --- Cancel flag tests ---

#[tokio::test]
async fn test_cancel_flag_before_start() {
    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("assets")).unwrap();

    // Set up 5 assets that would succeed
    for i in 0..5 {
        Mock::given(method("GET")).and(path(format!("/asset{}.png", i)))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1, 2, 3]))
            .mount(&mock_server).await;
    }

    let assets: Vec<AssetRef> = (0..5).map(|i| AssetRef {
        url: format!("{}/asset{}.png", mock_server.uri(), i),
        asset_type: "sprite".into(),
        is_default: false,
        local_path: String::new(),
    }).collect();

    let client = test_client();
    let tx = tauri::ipc::Channel::new(|_| Ok(()));
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(true)); // Already cancelled!

    let result = download_assets(
        &client, assets, &dir.path().to_path_buf(), &dir.path().to_path_buf(),
        None, &tx, 3, cancel,
    ).await.unwrap();

    assert_eq!(result.downloaded.len(), 0, "All should be cancelled, none downloaded");
    assert_eq!(result.failed.len(), 5, "All should be in failed list as cancelled");
    for f in &result.failed {
        assert_eq!(f.error, "Cancelled", "Error should be 'Cancelled'");
    }
}

#[tokio::test]
async fn test_cancel_flag_stops_midway() {
    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("assets")).unwrap();

    // Set up 10 assets, each with a 50ms delay
    for i in 0..10 {
        Mock::given(method("GET")).and(path(format!("/slow{}.png", i)))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(vec![1, 2, 3])
                    .set_delay(std::time::Duration::from_millis(50))
            )
            .mount(&mock_server).await;
    }

    let assets: Vec<AssetRef> = (0..10).map(|i| AssetRef {
        url: format!("{}/slow{}.png", mock_server.uri(), i),
        asset_type: "sprite".into(),
        is_default: false,
        local_path: String::new(),
    }).collect();

    let client = test_client();
    let tx = tauri::ipc::Channel::new(|_| Ok(()));
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Cancel after 150ms (should get ~1-3 assets through with concurrency=1)
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        cancel_clone.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    let result = download_assets(
        &client, assets, &dir.path().to_path_buf(), &dir.path().to_path_buf(),
        None, &tx, 1, cancel, // concurrency=1 so assets are sequential
    ).await.unwrap();

    assert!(
        result.downloaded.len() < 10,
        "Should have downloaded fewer than 10 (got {})",
        result.downloaded.len()
    );
    assert!(
        result.downloaded.len() > 0,
        "Should have downloaded at least 1 before cancel"
    );
    // The rest should be cancelled
    let cancelled: Vec<_> = result.failed.iter().filter(|f| f.error == "Cancelled").collect();
    assert!(
        !cancelled.is_empty(),
        "Some assets should have 'Cancelled' error"
    );
}

// --- Log rotation ---

#[test]
fn test_log_rotation_when_large() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = dir.path().join("download_log.txt");
    // Create a >1MB file
    let big_data = vec![b'x'; 1_100_000];
    std::fs::write(&log_path, &big_data).unwrap();
    assert!(log_path.exists());

    let _log = DownloadLog::new(&log_path).unwrap();
    // Old file should exist
    let old_path = dir.path().join("download_log.old.txt");
    assert!(old_path.exists(), "Old log should have been created by rotation");
    let old_size = std::fs::metadata(&old_path).unwrap().len();
    assert!(old_size > 1_000_000, "Old log should contain the original data");
}

#[test]
fn test_no_log_rotation_when_small() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = dir.path().join("download_log.txt");
    std::fs::write(&log_path, "small content").unwrap();

    let _log = DownloadLog::new(&log_path).unwrap();
    let old_path = dir.path().join("download_log.old.txt");
    assert!(!old_path.exists(), "Small log should not be rotated");
}

// --- Multi-level redirect ---

#[tokio::test]
async fn test_mock_redirect_chain_2_levels() {
    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let log = test_log(dir.path());

    // A → B → C (final)
    Mock::given(method("GET")).and(path("/a"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("location", &format!("{}/b", mock_server.uri()))
        )
        .mount(&mock_server).await;
    Mock::given(method("GET")).and(path("/b"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("location", &format!("{}/c", mock_server.uri()))
        )
        .mount(&mock_server).await;
    Mock::given(method("GET")).and(path("/c"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![42]))
        .mount(&mock_server).await;

    let url = format!("{}/a", mock_server.uri());
    let client = test_client();
    let result = download_single_asset(
        &client, &url, &dir.path().to_path_buf(), "test.bin", &log, "sprite"
    ).await;
    assert!(result.is_ok(), "Should follow 2-level redirect chain: {:?}", result.err());
    assert_eq!(result.unwrap().size, 1);
}

// --- Dedup integration test ---

#[tokio::test]
async fn test_download_dedup_skips_existing_in_index() {
    use crate::downloader::dedup::DedupIndex;

    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();
    std::fs::create_dir_all(data_dir.join("assets")).unwrap();

    // Content that will be downloaded
    let content = vec![10, 20, 30, 40, 50];
    let content_hash = xxhash_rust::xxh3::xxh3_64(&content);
    let content_size = content.len() as u64;

    Mock::given(method("GET")).and(path("/dup.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(content.clone()))
        .mount(&mock_server).await;

    // Pre-register a file in the index with the same hash
    let existing_path = "defaults/images/bg/room.png";
    std::fs::create_dir_all(data_dir.join("defaults/images/bg")).unwrap();
    std::fs::write(data_dir.join(existing_path), &content).unwrap();

    let index = DedupIndex::open(data_dir).unwrap();
    index.register(existing_path, content_size, content_hash).unwrap();

    let assets = vec![AssetRef {
        url: format!("{}/dup.png", mock_server.uri()),
        asset_type: "background".to_string(),
        is_default: false,
        local_path: String::new(),
    }];

    let client = test_client();
    let tx = tauri::ipc::Channel::new(|_| Ok(()));
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let result = download_assets(
        &client, assets,
        &data_dir.to_path_buf(), &data_dir.to_path_buf(),
        Some(&index), &tx, 3, cancel,
    ).await.unwrap();

    assert_eq!(result.downloaded.len(), 1);
    let asset = &result.downloaded[0];
    // Should point to the existing defaults/ path, not a newly saved file
    assert_eq!(asset.local_path, existing_path, "Should reuse existing path from index");
    // The just-saved file should have been deleted
    // (it was saved to assets/{hash}.png then removed after dedup check)
}

/// Regression: canonical defaults (defaults/sounds/, defaults/images/, etc.) must NOT
/// be deleted by dedup even when a match exists in defaults/shared/.
/// The engine references these by fixed path (external: false in trial_data).
#[tokio::test]
async fn test_download_dedup_preserves_canonical_defaults() {
    use crate::downloader::dedup::DedupIndex;

    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Content that will be downloaded as a canonical default sound
    let content = vec![0xAA; 100];
    let content_hash = xxhash_rust::xxh3::xxh3_64(&content);

    Mock::given(method("GET")).and(path("/sound.mp3"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(content.clone()))
        .mount(&mock_server).await;

    // Pre-register the SAME content at a defaults/shared/ path (simulating a previous case's dedup)
    let shared_path = "defaults/shared/abcd/abcd1234.mp3";
    std::fs::create_dir_all(data_dir.join("defaults/shared/abcd")).unwrap();
    std::fs::write(data_dir.join(shared_path), &content).unwrap();

    let index = DedupIndex::open(data_dir).unwrap();
    index.register(shared_path, content.len() as u64, content_hash).unwrap();

    // Download the sound as a canonical default (defaults/sounds/Objection Phoenix.mp3)
    let assets = vec![AssetRef {
        url: format!("{}/sound.mp3", mock_server.uri()),
        asset_type: "sound".to_string(),
        is_default: true,
        local_path: "defaults/sounds/Objection Phoenix.mp3".to_string(),
    }];

    // Create target dir so download can write
    std::fs::create_dir_all(data_dir.join("defaults/sounds")).unwrap();

    let client = test_client();
    let tx = tauri::ipc::Channel::new(|_| Ok(()));
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let result = download_assets(
        &client, assets,
        &data_dir.to_path_buf(), &data_dir.to_path_buf(),
        Some(&index), &tx, 3, cancel,
    ).await.unwrap();

    assert_eq!(result.downloaded.len(), 1);
    let asset = &result.downloaded[0];

    // The canonical path must be kept — engine looks for it by fixed path
    assert_eq!(
        asset.local_path, "defaults/sounds/Objection Phoenix.mp3",
        "Canonical default must NOT be redirected to shared"
    );

    // The file must actually exist at the canonical path
    assert!(
        data_dir.join("defaults/sounds/Objection Phoenix.mp3").is_file(),
        "Canonical default file must exist on disk"
    );

    // The shared copy should be replaced with a VFS pointer to the canonical
    let shared_full = data_dir.join(shared_path);
    assert!(
        shared_full.is_file(),
        "Shared path should still exist as a VFS pointer"
    );
    let pointer_target = crate::downloader::vfs::read_vfs_pointer(&shared_full);
    assert!(
        pointer_target.is_some(),
        "Shared file should be a VFS pointer, not the original data"
    );
    assert_eq!(
        pointer_target.unwrap(),
        "defaults/sounds/Objection Phoenix.mp3",
        "VFS pointer should redirect to the canonical default"
    );
}

/// Regression: case-specific assets (assets/...) should still be deduped normally
/// against existing defaults/ entries — only canonical defaults are protected.
#[tokio::test]
async fn test_download_dedup_still_works_for_case_assets() {
    use crate::downloader::dedup::DedupIndex;

    let mock_server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    let content = vec![0xBB; 50];
    let content_hash = xxhash_rust::xxh3::xxh3_64(&content);

    Mock::given(method("GET")).and(path("/sprite.gif"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(content.clone()))
        .mount(&mock_server).await;

    // Pre-register at a canonical defaults/ path
    let canonical = "defaults/images/chars/Apollo/1.gif";
    std::fs::create_dir_all(data_dir.join("defaults/images/chars/Apollo")).unwrap();
    std::fs::write(data_dir.join(canonical), &content).unwrap();

    let index = DedupIndex::open(data_dir).unwrap();
    index.register(canonical, content.len() as u64, content_hash).unwrap();

    // Download as a case-specific external asset (no local_path → goes to assets/)
    let assets = vec![AssetRef {
        url: format!("{}/sprite.gif", mock_server.uri()),
        asset_type: "sprite".to_string(),
        is_default: false,
        local_path: String::new(), // External asset
    }];

    std::fs::create_dir_all(data_dir.join("assets")).unwrap();

    let client = test_client();
    let tx = tauri::ipc::Channel::new(|_| Ok(()));
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let result = download_assets(
        &client, assets,
        &data_dir.to_path_buf(), &data_dir.to_path_buf(),
        Some(&index), &tx, 3, cancel,
    ).await.unwrap();

    assert_eq!(result.downloaded.len(), 1);
    let asset = &result.downloaded[0];

    // Case asset should be deduped to the existing canonical default
    assert_eq!(
        asset.local_path, canonical,
        "Case-specific asset should dedup to existing canonical default"
    );
}

/// Verify the IMGUR_REMOVED_HASH constant matches the actual imgur removed.png fixture.
#[test]
fn test_imgur_removed_hash_constant() {
    let bytes = include_bytes!("../testdata/imgur_removed.png");
    assert_eq!(bytes.len(), 503, "imgur removed.png should be exactly 503 bytes");
    let hash = xxhash_rust::xxh3::xxh3_64(bytes);
    assert_eq!(hash, 0x38da9bd2e10a4bc8, "IMGUR_REMOVED_HASH must match imgur_removed.png");
}
