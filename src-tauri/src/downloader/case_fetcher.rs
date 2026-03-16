use regex::Regex;
use reqwest::Client;
use serde_json::Value;

use super::{CaseInfo, SitePaths, AAONLINE_BASE};

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

/// Fetch site paths (cfg) from bridge.js.php on aaonline.fr.
pub async fn fetch_site_paths(client: &Client) -> Result<SitePaths, String> {
    let url = format!("{}/bridge.js.php", AAONLINE_BASE);
    let text = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch bridge.js.php: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read bridge.js.php response: {}", e))?;

    // Extract cfg object: var cfg = {...};
    let re = Regex::new(r"(?s)var cfg = (\{.+?\});")
        .map_err(|e| format!("Regex error: {}", e))?;

    let captures = re
        .captures(&text)
        .ok_or("Could not find cfg variable in bridge.js.php")?;

    let cfg_json = captures
        .get(1)
        .ok_or("Could not extract cfg JSON")?
        .as_str();

    let cfg: Value = serde_json::from_str(cfg_json)
        .map_err(|e| format!("Failed to parse cfg JSON: {}", e))?;

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

/// Fetch case data from trial.js.php on aaonline.fr.
/// Returns (CaseInfo, trial_data as JSON Value, raw trial_information JSON, raw trial_data JSON).
pub async fn fetch_case(
    client: &Client,
    case_id: u32,
) -> Result<(CaseInfo, Value, String, String), String> {
    let url = format!("{}/trial.js.php?trial_id={}", AAONLINE_BASE, case_id);
    let text = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch trial.js.php: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read trial.js.php response: {}", e))?;

    // Extract trial_information
    let info_json = extract_json_parse_arg(&text, "trial_information")
        .ok_or("Could not find trial_information in response")?;

    let info_value: Value = serde_json::from_str(&info_json)
        .map_err(|e| format!("Failed to parse trial_information: {}", e))?;

    let case_info = CaseInfo {
        id: info_value["id"].as_u64().unwrap_or(case_id as u64) as u32,
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

    // Extract initial_trial_data
    let data_json = extract_json_parse_arg(&text, "initial_trial_data")
        .ok_or("Could not find initial_trial_data in response")?;

    let trial_data: Value = serde_json::from_str(&data_json)
        .map_err(|e| format!("Failed to parse initial_trial_data: {}", e))?;

    Ok((case_info, trial_data, info_json, data_json))
}
