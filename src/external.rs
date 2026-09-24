//! Explicit, local-only external target syntax. Parsing never dereferences an address.

pub(crate) fn address(target: &str) -> Option<&str> {
    target.strip_prefix("external:")
}

pub(crate) fn valid_address(address: &str) -> bool {
    let Some((scheme, rest)) = address.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return false;
    }
    let authority = rest.split(['/', '?', '#', '\\']).next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return false;
    }
    if address
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '[' | ']' | '<' | '>' | '\\'))
    {
        return false;
    }
    let Ok(url) = url::Url::parse(address) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url.has_host()
        && url.username().is_empty()
        && url.password().is_none()
}
