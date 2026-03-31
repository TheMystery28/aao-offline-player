use super::*;

#[test]
fn test_extract_descriptors_basic() {
    let code = r#"
EnginePlugins.register({
    name: "test_plugin",
    params: {
        volume: { type: "number", default: 0.8, min: 0, max: 1, step: 0.1, label: "Volume" },
        enabled: { type: "checkbox", default: true, label: "Enable" }
    },
    init: function(config, events, api) {}
});
"#;
    let result = extract_plugin_descriptors(code);
    assert!(result.is_some(), "Should extract descriptors from basic plugin");
    let desc = result.unwrap();
    assert_eq!(desc["volume"]["type"], "number");
    assert_eq!(desc["volume"]["min"], 0);
    assert_eq!(desc["volume"]["max"], 1);
    assert_eq!(desc["enabled"]["type"], "checkbox");
    assert_eq!(desc["enabled"]["default"], true);
}

#[test]
fn test_extract_descriptors_with_select() {
    let code = r#"
EnginePlugins.register({
    name: "theme_plugin",
    params: {
        theme: { type: "select", default: "dark", options: ["dark", "light", "auto"], label: "Theme" }
    },
    init: function() {}
});
"#;
    let result = extract_plugin_descriptors(code);
    assert!(result.is_some(), "Should extract select descriptors");
    let desc = result.unwrap();
    assert_eq!(desc["theme"]["type"], "select");
    let opts = desc["theme"]["options"].as_array().unwrap();
    assert_eq!(opts.len(), 3);
    assert_eq!(opts[0], "dark");
}

#[test]
fn test_extract_descriptors_no_params() {
    let code = r#"
EnginePlugins.register({
    name: "no_params",
    init: function() {}
});
"#;
    let result = extract_plugin_descriptors(code);
    assert!(result.is_none(), "Plugin without params should return None");
}

#[test]
fn test_extract_descriptors_malformed() {
    let result = extract_plugin_descriptors("this is not valid JS at all");
    assert!(result.is_none(), "Malformed code should return None");

    let result2 = extract_plugin_descriptors("");
    assert!(result2.is_none(), "Empty code should return None");
}

#[test]
fn test_extract_descriptors_single_quotes() {
    let code = r#"
EnginePlugins.register({
    name: "test",
    params: { theme: { type: 'select', default: 'dark', options: ['dark', 'light'] } },
    init: function(c,e,a) {}
});
"#;
    let result = extract_plugin_descriptors(code);
    assert!(result.is_some(), "Should parse single-quoted strings");
    let desc = result.unwrap();
    assert_eq!(desc["theme"]["type"], "select");
    assert_eq!(desc["theme"]["default"], "dark");
    let opts = desc["theme"]["options"].as_array().unwrap();
    assert_eq!(opts.len(), 2);
    assert_eq!(opts[0], "dark");
    assert_eq!(opts[1], "light");
}

#[test]
fn test_extract_descriptors_already_quoted_keys() {
    let code = r#"
EnginePlugins.register({
    name: "test",
    params: { "volume": { "type": "number", "min": 0, "max": 1 } },
    init: function(c,e,a) {}
});
"#;
    let result = extract_plugin_descriptors(code);
    assert!(result.is_some(), "Should parse already-quoted keys");
    let desc = result.unwrap();
    assert_eq!(desc["volume"]["type"], "number");
    assert_eq!(desc["volume"]["min"], 0);
    assert_eq!(desc["volume"]["max"], 1);
}

#[test]
fn test_extract_descriptors_with_comments() {
    let code = r#"
EnginePlugins.register({
    name: "test",
    params: {
        // Volume control
        volume: { type: "number", default: 0.8 },
        /* Theme selector
           supports dark and light */
        theme: { type: "select", default: "dark" }
    },
    init: function(c,e,a) {}
});
"#;
    let result = extract_plugin_descriptors(code);
    assert!(result.is_some(), "Should parse params with comments");
    let desc = result.unwrap();
    assert_eq!(desc["volume"]["type"], "number");
    assert_eq!(desc["theme"]["type"], "select");
}

#[test]
fn test_duplicate_found_in_global() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    std::fs::create_dir_all(engine_dir.join("plugins")).unwrap();
    std::fs::write(engine_dir.join("plugins/test.js"), "console.log('hello');").unwrap();

    let matches = check_plugin_duplicate("console.log('hello');", engine_dir);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].filename, "test.js");
    assert_eq!(matches[0].location, "global");
}

#[test]
fn test_duplicate_found_in_case() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    std::fs::create_dir_all(engine_dir.join("case/555/plugins")).unwrap();
    std::fs::write(engine_dir.join("case/555/plugins/p.js"), "// dup").unwrap();

    let matches = check_plugin_duplicate("// dup", engine_dir);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].location, "case 555");
}

#[test]
fn test_no_duplicate_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let matches = check_plugin_duplicate("unique code", dir.path());
    assert!(matches.is_empty());
}

#[test]
fn test_duplicate_whitespace_trimmed() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    std::fs::create_dir_all(engine_dir.join("plugins")).unwrap();
    std::fs::write(engine_dir.join("plugins/t.js"), "  code  \n").unwrap();

    let matches = check_plugin_duplicate("\n  code  ", engine_dir);
    assert_eq!(matches.len(), 1);
}

#[test]
fn test_set_params_default() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    std::fs::create_dir_all(engine_dir.join("plugins")).unwrap();
    std::fs::write(engine_dir.join("plugins/manifest.json"),
        r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":true},"params":{}}}}"#).unwrap();

    set_global_plugin_params("a.js", "default", "", &serde_json::json!({"font":"Arial"}), engine_dir).unwrap();

    let text = std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["plugins"]["a.js"]["params"]["default"]["font"], "Arial");
}

#[test]
fn test_set_params_by_case() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    std::fs::create_dir_all(engine_dir.join("plugins")).unwrap();
    std::fs::write(engine_dir.join("plugins/manifest.json"),
        r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":true},"params":{}}}}"#).unwrap();

    set_global_plugin_params("a.js", "by_case", "69063", &serde_json::json!({"font":"Mono"}), engine_dir).unwrap();

    let text = std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["plugins"]["a.js"]["params"]["by_case"]["69063"]["font"], "Mono");
}

// =====================================================================
// parse_plugin_assets tests
// =====================================================================

#[test]
fn test_parse_plugin_assets_valid_block() {
    let code = r#"
/**
 * My Plugin
 *
 * @assets
 * voice_blip1.opus = https://example.com/voice_blip1.opus
 * voice_blip2.opus = https://example.com/voice_blip2.opus
 * voice_blip3.opus = https://example.com/voice_blip3.opus
 */
EnginePlugins.register({ name: 'test', init: function() {} });
"#;
    let assets = parse_plugin_assets(code);
    assert_eq!(assets.len(), 3);
    assert_eq!(assets[0].0, "voice_blip1.opus");
    assert_eq!(assets[0].1, "https://example.com/voice_blip1.opus");
    assert_eq!(assets[2].0, "voice_blip3.opus");
}

#[test]
fn test_parse_plugin_assets_no_block() {
    let code = r#"
/**
 * Plugin without assets
 */
EnginePlugins.register({ name: 'test', init: function() {} });
"#;
    let assets = parse_plugin_assets(code);
    assert!(assets.is_empty());
}

#[test]
fn test_parse_plugin_assets_malformed_lines() {
    let code = r#"
/**
 * @assets
 * no_equals_here
 * = https://example.com/no_filename
 * valid.opus = https://example.com/valid.opus
 *
 * just_text
 */
"#;
    let assets = parse_plugin_assets(code);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].0, "valid.opus");
}

#[test]
fn test_parse_plugin_assets_extra_whitespace() {
    let code = r#"
/**
 *   @assets
 *   voice.opus   =   https://example.com/voice.opus
 *    font.woff =https://example.com/font.woff
 */
"#;
    let assets = parse_plugin_assets(code);
    assert_eq!(assets.len(), 2);
    assert_eq!(assets[0].0, "voice.opus");
    assert_eq!(assets[0].1, "https://example.com/voice.opus");
    assert_eq!(assets[1].0, "font.woff");
    assert_eq!(assets[1].1, "https://example.com/font.woff");
}

#[test]
fn test_parse_plugin_assets_ignores_single_line_comments() {
    let code = r#"
// @assets
// voice.opus = https://example.com/voice.opus
EnginePlugins.register({ name: 'test', init: function() {} });
"#;
    let assets = parse_plugin_assets(code);
    assert!(assets.is_empty(), "Single-line comments should not be parsed for @assets");
}

#[test]
fn test_parse_plugin_assets_stops_at_next_tag() {
    let code = r#"
/**
 * @assets
 * voice.opus = https://example.com/voice.opus
 * @param config - some param
 * font.woff = https://example.com/font.woff
 */
"#;
    let assets = parse_plugin_assets(code);
    assert_eq!(assets.len(), 1, "@param should end the @assets section");
    assert_eq!(assets[0].0, "voice.opus");
}

#[test]
fn test_parse_plugin_assets_non_http_url_skipped() {
    let code = r#"
/**
 * @assets
 * valid.opus = https://example.com/valid.opus
 * local.opus = ./local/path.opus
 * ftp.opus = ftp://example.com/file.opus
 */
"#;
    let assets = parse_plugin_assets(code);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].0, "valid.opus");
}
