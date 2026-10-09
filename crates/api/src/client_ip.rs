//! The address of the client and the trusted reverse proxies (ADR 0008, ADR 0035).

use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{Extensions, HeaderMap};
use ipnet::IpNet;
use tada_app::problem::ProblemCode;

use crate::ApiState;
use crate::problem::ApiError;

const FORWARDED_FOR: &str = "x-forwarded-for";

/// The address of the client of the request. `Debug` never shows it (ADR 0035).
///
/// Behind a trusted proxy, it is the rightmost address of `X-Forwarded-For` that is not a trusted
/// proxy. Each proxy appends the address of its own peer, so a client can add addresses only on
/// the left. Without a trusted proxy, it is the peer of the connection.
#[derive(Clone, Copy)]
pub(crate) struct ClientIp(pub IpAddr);

impl fmt::Debug for ClientIp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ClientIp(redacted)")
    }
}

impl FromRequestParts<ApiState> for ClientIp {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &ApiState) -> Result<Self, ApiError> {
        // `serve` gives each request its peer. Without it, the server cannot apply a rate limit.
        let peer = peer(&parts.extensions).ok_or_else(|| ApiError::new(ProblemCode::Internal))?;
        if forwarded_by_an_untrusted_peer(peer, &parts.headers, &state.trusted_proxies)
            && !WARNED.swap(true, Ordering::Relaxed)
        {
            // No address: the log never holds one (ADR 0035).
            tracing::warn!(
                "a request from a peer outside TADA_TRUSTED_PROXIES has X-Forwarded-For; \
                 if a reverse proxy is in front of tada, set TADA_TRUSTED_PROXIES to its range, \
                 or all clients share the rate limit of the proxy"
            );
        }
        Ok(Self(client_ip(
            peer,
            &parts.headers,
            &state.trusted_proxies,
        )))
    }
}

/// True after the first warning of `forwarded_by_an_untrusted_peer` in this process.
static WARNED: AtomicBool = AtomicBool::new(false);

/// True if a peer that is not a trusted proxy sends `X-Forwarded-For`. This is the sign of a
/// reverse proxy that `TADA_TRUSTED_PROXIES` does not name: then each client behind it has the
/// address of the proxy, and one client can use up the rate limit of all.
fn forwarded_by_an_untrusted_peer(
    peer: IpAddr,
    headers: &HeaderMap,
    trusted_proxies: &[IpNet],
) -> bool {
    headers.contains_key(FORWARDED_FOR) && !is_trusted_proxy(trusted_proxies, peer)
}

fn client_ip(peer: IpAddr, headers: &HeaderMap, trusted_proxies: &[IpNet]) -> IpAddr {
    if !is_trusted_proxy(trusted_proxies, peer) {
        return peer;
    }
    // Split the bytes, not the text: a client entry that is not text then stops the walk only
    // where it is, not before the entries that the proxies appended.
    let forwarded: Vec<&[u8]> = headers
        .get_all(FORWARDED_FOR)
        .iter()
        .flat_map(|value| value.as_bytes().split(|byte| *byte == b','))
        .collect();
    let mut client = peer;
    for entry in forwarded.iter().rev() {
        // An entry that is not an address stops the walk: the last trusted proxy is the client.
        let Some(address) = std::str::from_utf8(entry)
            .ok()
            .and_then(|entry| entry.trim().parse::<IpAddr>().ok())
        else {
            break;
        };
        client = address;
        if !is_trusted_proxy(trusted_proxies, address) {
            break;
        }
    }
    client
}

/// The address of the peer of the connection. A request outside a server, for example in a test of
/// the router, has no peer.
pub(crate) fn peer(extensions: &Extensions) -> Option<IpAddr> {
    extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(address)| address.ip())
}

/// True if `address` is in one of the ranges of `TADA_TRUSTED_PROXIES`.
pub(crate) fn is_trusted_proxy(trusted_proxies: &[IpNet], address: IpAddr) -> bool {
    trusted_proxies.iter().any(|range| range.contains(&address))
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    const PROXY: &str = "10.0.0.5";

    fn trusted() -> Vec<IpNet> {
        vec!["10.0.0.0/8".parse().unwrap()]
    }

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    fn forwarded(values: &[&str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append(FORWARDED_FOR, HeaderValue::from_str(value).unwrap());
        }
        headers
    }

    #[test]
    fn a_peer_outside_the_trusted_ranges_ignores_x_forwarded_for() {
        let headers = forwarded(&["198.51.100.1"]);
        assert_eq!(
            client_ip(ip("203.0.113.7"), &headers, &trusted()),
            ip("203.0.113.7")
        );
        assert_eq!(client_ip(ip(PROXY), &headers, &[]), ip(PROXY));
    }

    #[test]
    fn a_trusted_peer_gives_the_rightmost_untrusted_address() {
        // The client sent a false first entry; two proxies appended their peers.
        let headers = forwarded(&["192.0.2.66, 203.0.113.7", "10.0.0.9"]);
        assert_eq!(
            client_ip(ip(PROXY), &headers, &trusted()),
            ip("203.0.113.7")
        );
    }

    #[test]
    fn a_trusted_peer_without_x_forwarded_for_is_the_client() {
        assert_eq!(
            client_ip(ip(PROXY), &HeaderMap::new(), &trusted()),
            ip(PROXY)
        );
    }

    #[test]
    fn an_entry_that_is_not_an_address_stops_the_walk() {
        let headers = forwarded(&["203.0.113.7, unknown, 10.0.0.9"]);
        assert_eq!(client_ip(ip(PROXY), &headers, &trusted()), ip("10.0.0.9"));
    }

    /// The proxy appends the peer as text. A client entry that is not text must not hide it, or the
    /// client gets the address of the proxy and a second rate limit.
    #[test]
    fn a_client_entry_that_is_not_text_does_not_hide_the_entry_of_the_proxy() {
        let mut headers = HeaderMap::new();
        headers.append(
            FORWARDED_FOR,
            HeaderValue::from_bytes(b"\xff, 203.0.113.7").unwrap(),
        );
        assert_eq!(
            client_ip(ip(PROXY), &headers, &trusted()),
            ip("203.0.113.7")
        );
    }

    #[test]
    fn x_forwarded_for_from_an_untrusted_peer_is_a_sign_of_a_missing_proxy_setting() {
        let headers = forwarded(&["203.0.113.7"]);
        assert!(forwarded_by_an_untrusted_peer(ip(PROXY), &headers, &[]));
        assert!(forwarded_by_an_untrusted_peer(
            ip("192.168.1.2"),
            &headers,
            &trusted()
        ));
        assert!(!forwarded_by_an_untrusted_peer(
            ip(PROXY),
            &headers,
            &trusted()
        ));
        assert!(!forwarded_by_an_untrusted_peer(
            ip(PROXY),
            &HeaderMap::new(),
            &[]
        ));
    }

    #[test]
    fn the_debug_text_holds_no_address() {
        let text = format!("{:?}", ClientIp(ip("203.0.113.7")));
        assert!(!text.contains("203"), "{text}");
    }
}
