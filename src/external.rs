//! Explicit, local-only external target syntax. Parsing never dereferences an address.

pub(crate) fn address(target: &str) -> Option<&str> {
    target.strip_prefix("external:")
}

pub(crate) fn valid_address(address: &str) -> bool {
    let authority = address
        .strip_prefix("http://")
        .or_else(|| address.strip_prefix("https://"))
        .and_then(|rest| rest.split(['/', '?', '#', '\\']).next());
    if authority.is_none_or(|authority| authority.is_empty() || authority.contains('@')) {
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
        && (address.starts_with("http://") || address.starts_with("https://"))
}
