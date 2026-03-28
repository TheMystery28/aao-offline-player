use super::*;

#[test]
fn test_encode_url_spaces() {
    assert_eq!(
        encode_url("http://example.com/sounds/Apollo - (french) hold it.mp3"),
        "http://example.com/sounds/Apollo%20-%20(french)%20hold%20it.mp3"
    );
}

#[test]
fn test_encode_url_already_encoded() {
    let url = "http://example.com/sounds/Konrad%20-%20(french)%20Objection.mp3";
    assert_eq!(encode_url(url), url);
}

#[test]
fn test_encode_url_no_spaces() {
    let url = "https://aaonline.fr/uploads/sprites/chars/Phoenix/1.gif";
    assert_eq!(encode_url(url), url);
}

#[test]
fn test_encode_url_brackets() {
    let result = encode_url("http://example.com/file[1].png");
    assert_eq!(result, "http://example.com/file%5B1%5D.png");
}

#[test]
fn test_encode_url_pipe() {
    let result = encode_url("http://example.com/a|b.png");
    assert_eq!(result, "http://example.com/a%7Cb.png");
}

#[test]
fn test_encode_url_already_encoded_unchanged() {
    let result = encode_url("http://example.com/a%20b.png");
    assert_eq!(result, "http://example.com/a%20b.png", "Should not double-encode");
}

#[test]
fn test_encode_url_multiple_unsafe_chars() {
    let result = encode_url("http://example.com/a b[1]|c.png");
    assert!(result.contains("%20"), "Space should be encoded");
    assert!(result.contains("%5B"), "[ should be encoded");
    assert!(result.contains("%7C"), "| should be encoded");
}

#[test]
fn test_encode_url_valid_url_passes_through() {
    let url = "https://example.com/path/to/file.png?q=1&r=2";
    assert_eq!(encode_url(url), url, "Valid URL should pass through unchanged");
}

#[test]
fn test_encode_url_already_percent_encoded_passes() {
    let url = "https://example.com/path%20with%20spaces/file%5B1%5D.png";
    assert_eq!(encode_url(url), url, "Already-encoded URL should pass through");
}

#[test]
fn test_encode_url_preserves_query_string() {
    // URL with spaces in path but valid query string
    let raw = "http://example.com/my file.png?type=bg&id=1";
    let result = encode_url(raw);
    assert!(result.contains("%20"), "Space in path should be encoded");
    assert!(result.contains("?type=bg&id=1"), "Query string should be preserved");
}

#[test]
fn test_url_parse_detects_valid_aao_url() {
    // Real AAO URL format
    let url = "https://aaonline.fr/sprites/00000.png";
    assert!(url::Url::parse(url).is_ok(), "Standard AAO URL should parse");
}

#[test]
fn test_url_join_resolves_relative_path() {
    let base = url::Url::parse("https://example.com/old/path").unwrap();
    let resolved = base.join("/new-path").unwrap();
    assert_eq!(resolved.as_str(), "https://example.com/new-path");
}

#[test]
fn test_url_join_resolves_relative_file() {
    let base = url::Url::parse("https://example.com/dir/").unwrap();
    let resolved = base.join("file.png").unwrap();
    assert_eq!(resolved.as_str(), "https://example.com/dir/file.png");
}

mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn encode_url_never_panics(input in "\\PC{0,200}") {
            let _ = encode_url(&input);
        }
    }
}
