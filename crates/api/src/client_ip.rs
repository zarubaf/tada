//! The address of the client and the trusted reverse proxies (ADR 0008, ADR 0035).

use std::net::{IpAddr, SocketAddr};

use axum::extract::ConnectInfo;
use axum::http::Extensions;
use ipnet::IpNet;

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
