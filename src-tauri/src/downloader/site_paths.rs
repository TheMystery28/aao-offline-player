//! TTL-cached AAO site paths (bridge.js.php cfg).
//! Site config changes rarely — caching it avoids one HTTP roundtrip per
//! download/update command. On fetch failure, a stale entry is reused.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::error::AppError;

use super::{SitePaths, AAONLINE_BASE};

pub(crate) const SITE_PATHS_TTL: Duration = Duration::from_secs(6 * 60 * 60);

static CACHE: OnceLock<Mutex<Option<(Instant, SitePaths)>>> = OnceLock::new();

pub fn is_fresh(age: Duration, ttl: Duration) -> bool {
    age < ttl
}

/// Fetch site paths, using the cached copy if it is fresher than `ttl`.
/// Falls back to a stale cached copy when the network fetch fails.
pub(crate) async fn fetch_site_paths_cached_impl(
    client: &reqwest::Client,
    base: &str,
    ttl: Duration,
) -> Result<SitePaths, AppError> {
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    {
        let Ok(guard) = cache.lock() else {
            return super::case_fetcher::fetch_site_paths_from(client, base).await;
        };
        if let Some((at, paths)) = guard.as_ref() {
            if is_fresh(at.elapsed(), ttl) {
                return Ok(paths.clone());
            }
        }
    }

    match super::case_fetcher::fetch_site_paths_from(client, base).await {
        Ok(paths) => {
            let Ok(mut guard) = cache.lock() else { return Ok(paths) };
            *guard = Some((Instant::now(), paths.clone()));
            Ok(paths)
        }
        Err(e) => {
            let Ok(guard) = cache.lock() else { return Err(e) };
            if let Some((_, paths)) = guard.as_ref() {
                log::warn!("Site paths fetch failed, using stale cache: {}", e);
                return Ok(paths.clone());
            }
            Err(e)
        }
    }
}

pub async fn fetch_site_paths_cached(client: &reqwest::Client) -> Result<SitePaths, AppError> {
    fetch_site_paths_cached_impl(client, AAONLINE_BASE, SITE_PATHS_TTL).await
}

/// Clear the process-wide cache (tests).
#[cfg(test)]
pub fn site_paths_cache_reset() {
    if let Some(cache) = CACHE.get() {
        let _ = cache.lock().map(|mut g| *g = None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::method;

    const CFG: &str = r#"var cfg = {"picture_dir":"P/","icon_subdir":"i/","talking_subdir":"t/","still_subdir":"s/","startup_subdir":"st/","evidence_subdir":"e/","bg_subdir":"b/","defaultplaces_subdir":"dp/","popups_subdir":"po/","locks_subdir":"l/","music_dir":"M/","sounds_dir":"S/","voices_dir":"V/"};"#;

    #[test]
    fn test_is_fresh_logic() {
        assert!(is_fresh(Duration::from_secs(1), Duration::from_secs(3600)));
        assert!(!is_fresh(Duration::from_secs(7200), Duration::from_secs(3600)));
        assert!(
            !is_fresh(Duration::ZERO, Duration::ZERO),
            "zero TTL expires immediately (age < ttl)"
        );
    }

    #[tokio::test]
    async fn test_cached_site_paths_single_fetch_within_ttl() {
        site_paths_cache_reset();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(wiremock::matchers::path("/bridge.js.php"))
            .respond_with(ResponseTemplate::new(200).set_body_string(CFG))
            .expect(1) // exactly one fetch while cached
            .mount(&server).await;

        let client = reqwest::Client::new();
        let base = server.uri();
        let ttl = Duration::from_secs(3600);
        let paths1 = fetch_site_paths_cached_impl(&client, &base, ttl).await.unwrap();
        let paths2 = fetch_site_paths_cached_impl(&client, &base, ttl).await.unwrap();
        assert_eq!(paths1.music_dir, "M/");
        assert_eq!(paths2.music_dir, "M/");
    }

    #[tokio::test]
    async fn test_cache_expiry_after_ttl() {
        site_paths_cache_reset();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(CFG))
            .expect(2)
            .mount(&server).await;
        let client = reqwest::Client::new();
        let base = server.uri();
        let short_ttl = Duration::from_millis(5);
        let _ = fetch_site_paths_cached_impl(&client, &base, short_ttl).await.unwrap();
        std::thread::sleep(Duration::from_millis(10)); // > short_ttl
        let _ = fetch_site_paths_cached_impl(&client, &base, short_ttl).await.unwrap();
    }
}
