/// Encode URL-unsafe characters in a URL path.
/// Uses url::Url::parse as the primary validation — if the URL is already valid, return as-is.
/// Falls back to manual encoding for URLs that fail to parse (unencoded spaces, brackets, etc.).
pub(super) fn encode_url(raw_url: &str) -> String {
    // Try to parse with url crate for normalization (encodes spaces, Unicode, etc.)
    let base = if let Ok(parsed) = url::Url::parse(raw_url) {
        parsed.to_string()
    } else {
        raw_url.to_string()
    };
    // Url::parse doesn't encode all chars that HTTP clients need encoded (brackets, pipes, etc.)
    // Apply additional encoding for chars that are valid in URLs but problematic in practice
    if base.contains('[') || base.contains(']') || base.contains('|')
        || base.contains('{') || base.contains('}') || base.contains('^')
        || base.contains('`') || base.contains('\\')
    {
        return base
            .replace('[', "%5B")
            .replace(']', "%5D")
            .replace('{', "%7B")
            .replace('}', "%7D")
            .replace('|', "%7C")
            .replace('\\', "%5C")
            .replace('^', "%5E")
            .replace('`', "%60");
    }
    base
}
