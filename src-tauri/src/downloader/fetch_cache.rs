//! Short-TTL cache of fetched trial payloads.
//!
//! The frontend pre-fetches `trial.js.php` via `fetch_case_info` to detect
//! sequences, then the download command fetches the SAME payload again.
//! This cache makes the second fetch free. Payloads are large, so the cache
//! is small (8 entries) and short-lived (10 minutes).
//!
//! `update_case` deliberately bypasses this cache — freshness is its purpose.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::error::AppError;
use super::CaseInfo;
use super::case_fetcher::fetch_case_from;

const FETCH_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_ENTRIES: usize = 8;

pub struct FetchedCase {
    pub case_info: CaseInfo,
    pub trial_data: serde_json::Value,
    pub info_json: String,
    pub data_json: String,
}

impl Clone for FetchedCase {
    fn clone(&self) -> Self {
        FetchedCase {
            case_info: self.case_info.clone(),
            trial_data: self.trial_data.clone(),
            info_json: self.info_json.clone(),
            data_json: self.data_json.clone(),
        }
    }
}

type Payload = (Instant, FetchedCase);

static CACHE: OnceLock<Mutex<HashMap<u32, Payload>>> = OnceLock::new();

pub fn cache_key_age_is_valid(age: Duration, ttl: Duration) -> bool {
    age < ttl
}

/// Fetch a case, reusing a cached payload when present and fresh.
pub async fn fetch_case_cached(client: &reqwest::Client, case_id: u32) -> Result<FetchedCase, AppError> {
    fetch_case_cached_impl(client, case_id, &crate::downloader::AAONLINE_BASE, FETCH_TTL).await
}

pub(crate) async fn fetch_case_cached_impl(
    client: &reqwest::Client,
    case_id: u32,
    base: &str,
    ttl: Duration,
) -> Result<FetchedCase, AppError> {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let cached: Option<Payload> = match cache.lock() {
        Ok(guard) => guard.get(&case_id).cloned(),
        Err(_) => None, // poisoned — fetch fresh
    };
    if let Some((at, fetched)) = cached {
        if cache_key_age_is_valid(at.elapsed(), ttl) {
            return Ok(fetched);
        }
    }

    let (case_info, trial_data, info_json, data_json) =
        fetch_case_from(client, base, case_id).await?;
    let fetched = FetchedCase { case_info, trial_data, info_json, data_json };
    if let Ok(mut guard) = cache.lock() {
        if guard.len() >= MAX_ENTRIES {
            guard.clear();
        }
        guard.insert(case_id, (Instant::now(), fetched.clone()));
    }
    Ok(fetched)
}

#[cfg(test)]
pub fn fetch_case_cache_reset() {
    if let Some(cache) = CACHE.get() {
        let _ = cache.lock().map(|mut g| g.clear());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::method;

    const SAMPLE: &str = r#"var trial_information = JSON.parse("{\"id\":42,\"title\":\"T\",\"author\":\"A\",\"language\":\"en\",\"last_edit_date\":0,\"format\":\"v6\",\"sequence\":null}");
var initial_trial_data = JSON.parse("{\"profiles\":[0]}");"#;

    #[test]
    fn test_cache_age_validity() {
        assert!(cache_key_age_is_valid(Duration::from_secs(1), Duration::from_secs(600)));
        assert!(!cache_key_age_is_valid(Duration::from_secs(1200), Duration::from_secs(600)));
    }

    #[tokio::test]
    async fn test_fetch_case_cached_second_call_no_network() {
        fetch_case_cache_reset();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(wiremock::matchers::path("/trial.js.php"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SAMPLE))
            .expect(1)
            .mount(&server).await;

        let client = reqwest::Client::new();
        let base = server.uri();
        let ttl = Duration::from_secs(600);
        let first = fetch_case_cached_impl(&client, 42, &base, ttl).await.unwrap();
        let second = fetch_case_cached_impl(&client, 42, &base, ttl).await.unwrap();
        assert_eq!(first.case_info.id, 42);
        assert_eq!(second.case_info.id, 42);
        assert_eq!(second.data_json, first.data_json);
    }
}
