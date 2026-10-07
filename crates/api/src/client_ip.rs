//! The address of the client and the trusted reverse proxies (ADR 0008, ADR 0035).

use std::fmt;
use std::net::{IpAddr, SocketAddr};

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
        Ok(Self(client_ip(
            peer,
            &parts.headers,
            &state.trusted_proxies,
        )))
    }
}

fn client_ip(peer: IpAddr, headers: &HeaderMap, trusted_proxies: &[IpNet]) -> IpAddr {
    if !is_trusted_proxy(trusted_proxies, peer) {
        return peer;
    }
    let forwarded: Vec<&str> = headers
        .get_all(FORWARDED_FOR)
        .iter()
        // A value that is not text cannot be parsed and stops the walk below.
        .flat_map(|value| value.to_str().unwrap_or("").split(','))
        .map(str::trim)
        .collect();
    let mut client = peer;
    for entry in forwarded.iter().rev() {
        // An entry that is not an address stops the walk: the last trusted proxy is the client.
        let Ok(address) = entry.parse::<IpAddr>() else {
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

    #[test]
    fn the_debug_text_holds_no_address() {
        let text = format!("{:?}", ClientIp(ip("203.0.113.7")));
        assert!(!text.contains("203"), "{text}");
    }
}
