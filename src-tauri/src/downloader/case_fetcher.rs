use std::sync::LazyLock;

use regex::Regex;
use reqwest::Client;
use serde_json::Value;

use crate::error::AppError;
use super::{CaseInfo, DownloaderError, SitePaths, AAONLINE_BASE};

/// Quick check if aaonline.fr is reachable (HEAD with short timeout).
pub async fn is_aaonline_reachable(client: &Client) -> bool {
    let check_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| client.clone());

    match check_client.head(AAONLINE_BASE).send().await {
        Ok(resp) => resp.status().is_success() || resp.status().is_redirection(),
        Err(_) => false,
    }
}

/// Unescape a JavaScript string literal (content between quotes).
fn unescape_js_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('\'') => result.push('\''),
                Some('/') => result.push('/'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Extract the string argument from `var {var_name} = JSON.parse("..." or '...')`.
/// Handles both single and double quoted JS strings with escape sequences.
fn extract_json_parse_arg(text: &str, var_name: &str) -> Option<String> {
    let search = format!("var {} = JSON.parse(", var_name);
    let start = text.find(&search)?;
    let after_parse = &text[start + search.len()..];

    // Determine quote character
    let quote = after_parse.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }

    // Find matching close quote, handling escapes
    let content = &after_parse[1..];
    let mut end = 0;
    let mut escaped = false;
    let mut found = false;
    for (i, c) in content.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == quote {
            end = i;
            found = true;
            break;
        }
    }

    if !found {
        return None;
    }

    let raw = &content[..end];
    Some(unescape_js_string(raw))
}

/// Pre-compiled regex for parsing the `var cfg = {...}` block in bridge.js.php.
static CFG_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)var cfg = (\{.+?\});").expect("CFG_REGEX pattern is valid")
});

/// Parse bridge.js.php response text into SitePaths.
/// Extracted for testability — the HTTP fetch is separate.
pub(crate) fn parse_bridge_js_response(text: &str) -> Result<SitePaths, DownloaderError> {
    let captures = CFG_REGEX
        .captures(text)
        .ok_or_else(|| DownloaderError::Other("Could not find cfg variable in bridge.js.php".to_string()))?;

    let cfg_json = captures
        .get(1)
        .ok_or_else(|| DownloaderError::Other("Could not extract cfg JSON".to_string()))?
        .as_str();

    let cfg: Value = serde_json::from_str(cfg_json)?;

    Ok(SitePaths {
        picture_dir: cfg["picture_dir"].as_str().unwrap_or("").to_string(),
        icon_subdir: cfg["icon_subdir"].as_str().unwrap_or("chars/").to_string(),
        talking_subdir: cfg["talking_subdir"]
            .as_str()
            .unwrap_or("chars/")
            .to_string(),
        still_subdir: cfg["still_subdir"]
            .as_str()
            .unwrap_or("charsStill/")
            .to_string(),
        startup_subdir: cfg["startup_subdir"]
            .as_str()
            .unwrap_or("charsStartup/")
            .to_string(),
        evidence_subdir: cfg["evidence_subdir"]
            .as_str()
            .unwrap_or("evidence/")
            .to_string(),
        bg_subdir: cfg["bg_subdir"]
            .as_str()
            .unwrap_or("backgrounds/")
            .to_string(),
        defaultplaces_subdir: cfg["defaultplaces_subdir"]
            .as_str()
            .unwrap_or("defaultplaces/")
            .to_string(),
        popups_subdir: cfg["popups_subdir"]
            .as_str()
            .unwrap_or("popups/")
            .to_string(),
        locks_subdir: cfg["locks_subdir"]
            .as_str()
            .unwrap_or("psycheLocks/")
            .to_string(),
        music_dir: cfg["music_dir"].as_str().unwrap_or("").to_string(),
        sounds_dir: cfg["sounds_dir"].as_str().unwrap_or("").to_string(),
        voices_dir: cfg["voices_dir"].as_str().unwrap_or("").to_string(),
    })
}

/// Fetch site paths (cfg) from bridge.js.php on aaonline.fr.
pub async fn fetch_site_paths(client: &Client) -> Result<SitePaths, AppError> {
    let url = format!("{}/bridge.js.php", AAONLINE_BASE);
    let text = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch bridge.js.php: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read bridge.js.php response: {}", e))?;

    Ok(parse_bridge_js_response(&text)?)
}

/// Parse trial.js.php response text into (CaseInfo, trial_data, raw_info_json, raw_data_json).
/// Extracted for testability — the HTTP fetch is separate.
pub(crate) fn parse_trial_js_response(text: &str, fallback_id: u32) -> Result<(CaseInfo, Value, String, String), DownloaderError> {
    let info_json = extract_json_parse_arg(text, "trial_information")
        .ok_or_else(|| DownloaderError::Other("Could not find trial_information in response".to_string()))?;

    let info_value: Value = serde_json::from_str(&info_json)?;

    let case_info = CaseInfo {
        id: info_value["id"].as_u64().unwrap_or(fallback_id as u64) as u32,
        title: info_value["title"]
            .as_str()
            .unwrap_or("Unknown")
            .to_string(),
        author: info_value["author"]
            .as_str()
            .unwrap_or("[UNKNOWN]")
            .to_string(),
        language: info_value["language"]
            .as_str()
            .unwrap_or("en")
            .to_string(),
        last_edit_date: info_value["last_edit_date"].as_u64().unwrap_or(0),
        format: info_value["format"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        sequence: info_value.get("sequence").cloned(),
    };

    let data_json = extract_json_parse_arg(text, "initial_trial_data")
        .ok_or_else(|| DownloaderError::Other("Could not find initial_trial_data in response".to_string()))?;

    let trial_data: Value = serde_json::from_str(&data_json)?;

    Ok((case_info, trial_data, info_json, data_json))
}

/// Fetch case data from trial.js.php on aaonline.fr.
/// Returns (CaseInfo, trial_data as JSON Value, raw trial_information JSON, raw trial_data JSON).
pub async fn fetch_case(
    client: &Client,
    case_id: u32,
) -> Result<(CaseInfo, Value, String, String), AppError> {
    let url = format!("{}/trial.js.php?trial_id={}", AAONLINE_BASE, case_id);
    let text = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch trial.js.php: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read trial.js.php response: {}", e))?;

    Ok(parse_trial_js_response(&text, case_id)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bridge_js_response() {
        let response = r#"
// Some JS comment
var cfg = {"picture_dir":"Ressources/Images/","icon_subdir":"persos/","talking_subdir":"persos/","still_subdir":"persosMuets/","startup_subdir":"persosStartup/","evidence_subdir":"dossier/","bg_subdir":"cinematiques/","defaultplaces_subdir":"lieux/","popups_subdir":"persos/Cour/","locks_subdir":"persos/Cour/psyche_locks/","music_dir":"Ressources/Musiques/","sounds_dir":"Ressources/Sons/","voices_dir":"Ressources/Voix/"};
// More JS
"#;
        let paths = parse_bridge_js_response(response).unwrap();
        assert_eq!(paths.picture_dir, "Ressources/Images/");
        assert_eq!(paths.talking_subdir, "persos/");
        assert_eq!(paths.still_subdir, "persosMuets/");
        assert_eq!(paths.music_dir, "Ressources/Musiques/");
        assert_eq!(paths.sounds_dir, "Ressources/Sons/");
        assert_eq!(paths.voices_dir, "Ressources/Voix/");
    }

    #[test]
    fn test_parse_bridge_js_malformed() {
        let result = parse_bridge_js_response("this is not valid JS");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Could not find cfg"));
    }

    #[test]
    fn test_parse_trial_js_response() {
        let response = r#"
var trial_information = JSON.parse("{\"id\":12345,\"title\":\"Test Case\",\"author\":\"TestAuthor\",\"language\":\"fr\",\"last_edit_date\":1700000000,\"format\":\"Def6\",\"sequence\":null}");
var initial_trial_data = JSON.parse("{\"profiles\":[0],\"frames\":[0],\"evidence\":[0]}");
"#;
        let (info, data, _, _) = parse_trial_js_response(response, 0).unwrap();
        assert_eq!(info.id, 12345);
        assert_eq!(info.title, "Test Case");
        assert_eq!(info.author, "TestAuthor");
        assert_eq!(info.language, "fr");
        assert_eq!(info.last_edit_date, 1700000000);
        assert_eq!(info.format, "Def6");
        assert!(info.sequence.is_some()); // null is Some(Value::Null)
        assert!(data["profiles"].is_array());
    }

    #[test]
    fn test_parse_trial_js_malformed() {
        let result = parse_trial_js_response("garbage text no JSON here", 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Could not find trial_information"));
    }

    #[test]
    fn test_parse_trial_js_with_escaped_quotes() {
        // Real AAO responses have escaped quotes inside JSON.parse("...")
        let response = r#"var trial_information = JSON.parse("{\"id\":99,\"title\":\"Phoenix's \\\"Turnabout\\\"\",\"author\":\"Writer\",\"language\":\"en\",\"last_edit_date\":0,\"format\":\"v6\",\"sequence\":null}");
var initial_trial_data = JSON.parse("{\"profiles\":[0]}");
"#;
        let (info, _, _, _) = parse_trial_js_response(response, 0).unwrap();
        assert_eq!(info.id, 99);
        assert!(info.title.contains("Turnabout"));
    }
}
