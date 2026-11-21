//! The only network access of Mara (REQ-ONLY-CODE-ADD-DOWNLOADS): `code add` fetches a grammar's
//! files over HTTPS, or plain HTTP from a loopback address, through `ureq` with rustls
//! (ADR-UREQ-RUSTLS-FETCH). Nothing else calls this module.

use std::net::IpAddr;
use std::time::Duration;

use mara_engine::config::{Repository, repository};
use ureq::http::Uri;
use ureq::{Proxy, ProxyProtocol};

/// The most bytes a grammar's file may have: tree-sitter gives a module at most 128 MiB of memory.
const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// How long a connection may take to open, after its host name is resolved.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// How long fetching one file may take in all, resolving its host name included.
const FETCH_TIMEOUT: Duration = Duration::from_secs(300);
/// Redirects an `https://` fetch follows, each to `https://` only, as release downloads need.
const MAX_REDIRECTS: u32 = 5;
/// The variables that name a proxy, in the order `ureq` reads them (`Proxy::try_from_env`): the
/// first one set to a URL it can read is its proxy for every request.
const PROXY_VARIABLES: [&str; 6] = [
    "ALL_PROXY",
    "all_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
];

/// The bytes at `url`, or why they could not be fetched. The URL must be one that
/// [`repository`] reads, and `ureq`'s own parser must read the same scheme, host and port from
/// it. A proxy that any proxy variable names and the client cannot use refuses every fetch. An
/// `https://` URL verifies the server's certificate against the WebPKI roots, follows redirects
/// only to `https://` and uses the environment's proxy; a loopback `http://` URL follows no
/// redirect and no proxy.
pub fn get(url: &str) -> Result<Vec<u8>, String> {
    let target = repository(url)?;
    let uri = same_address(url, &target)?;
    usable_proxy()?;
    let builder = ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(FETCH_TIMEOUT))
        .http_status_as_error(false);
    let config = match target.secure {
        true => builder.https_only(true).max_redirects(MAX_REDIRECTS),
        false => builder.proxy(None).max_redirects(0),
    }
    .build();
    let agent = ureq::Agent::new_with_config(config);
    let mut response = agent.get(uri).call().map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("the server answered {status}"));
    }
    response
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()
        .map_err(|error| match error {
            ureq::Error::BodyExceedsLimit(_) => {
                format!("it has more than {} MiB", MAX_BYTES / 1024 / 1024)
            }
            error => error.to_string(),
        })
}

/// `url` as the HTTP client parses it, refused unless that names the scheme, host and port that
/// Mara's check read as `target`: the client's parser decides where it connects.
fn same_address(url: &str, target: &Repository) -> Result<Uri, String> {
    let other = || {
        format!(
            "the HTTP client reads {url} as another address than {}:{}, so Mara does not \
             connect to it",
            target.host, target.port
        )
    };
    let uri: Uri = url.parse().map_err(|_| other())?;
    let secure = match uri.scheme_str().map(str::to_ascii_lowercase).as_deref() {
        Some("https") => true,
        Some("http") => false,
        _ => return Err(other()),
    };
    let host = uri.host().ok_or_else(other)?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let host = host.strip_suffix('.').unwrap_or(host);
    let same_host = match (host.parse::<IpAddr>(), target.host.parse::<IpAddr>()) {
        (Ok(read), Ok(expected)) => read == expected,
        (Err(_), Err(_)) => host.eq_ignore_ascii_case(&target.host),
        _ => false,
    };
    let port = uri.port_u16().unwrap_or(if secure { 443 } else { 80 });
    match secure == target.secure && same_host && port == target.port {
        true => Ok(uri),
        false => Err(other()),
    }
}

/// Refuses every fetch when any variable that the client reads names a proxy it cannot use: one
/// it cannot read, or a SOCKS proxy, which this build of the client would bypass. `NO_PROXY`
/// changes nothing here, since a redirect could lead to a host it does not exempt.
fn usable_proxy() -> Result<(), String> {
    for name in PROXY_VARIABLES {
        let Some(value) = std::env::var(name).ok().filter(|value| !value.is_empty()) else {
            continue;
        };
        let scheme = value.split_once("://").map_or("http", |(scheme, _)| scheme);
        let why = match Proxy::new(&value) {
            Err(_) => "which Mara's HTTP client cannot read",
            Ok(proxy) if is_socks(proxy.protocol()) => "which Mara's HTTP client cannot use",
            Ok(_) => continue,
        };
        return Err(format!(
            "{name} names a {scheme} proxy, {why}: unset {name} for this command, or set it to an \
             http:// or https:// proxy"
        ));
    }
    Ok(())
}

fn is_socks(protocol: ProxyProtocol) -> bool {
    matches!(
        protocol,
        ProxyProtocol::Socks4
            | ProxyProtocol::Socks4A
            | ProxyProtocol::Socks5
            | ProxyProtocol::Socks5h
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connects_only_where_the_check_and_the_client_agree() {
        for url in [
            "https://example.com/g/rust/1/grammar.wasm",
            "HTTPS://Example.com:443/g",
            "http://127.0.0.1:8080/g",
            "http://[::1]:9/g",
            "https://example.org./g",
        ] {
            let target = repository(url).unwrap();
            assert!(same_address(url, &target).is_ok(), "{url}");
        }
        let mut elsewhere = repository("https://example.com/g").unwrap();
        elsewhere.port = 8443;
        assert!(same_address("https://example.com/g", &elsewhere).is_err());
        elsewhere = repository("https://example.com/g").unwrap();
        elsewhere.host = "example.org".to_owned();
        assert!(same_address("https://example.com/g", &elsewhere).is_err());
    }
}
