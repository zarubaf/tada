//! The rate limits of sign-in requests (ADR 0008, ADR 0056).
//!
//! A limit counts the requests of one subject in a fixed window.
//! The counters are in the database, so that all `serve` processes share them (ADR 0025).
//!
//! The client limit refuses a request. The mail limit only stops the mail: the answer stays the
//! same, so nobody can lock a member out of the sign-in with requests for the member's address.
//! Each mail that the mail limit allows holds a link that is valid longer than the cooldown, so
//! the member finds a valid link in the inbox whenever the cooldown stops a mail.

use std::fmt;
use std::net::{IpAddr, Ipv6Addr};

use jiff::{SignedDuration, Timestamp};
use tada_domain::identity::Email;

/// The sign-in requests from one client network in one `WINDOW`.
pub const SIGN_IN_PER_IP: u32 = 30;
/// The window of the client limit. The counters of a window stay at most one more window
/// (ADR 0065).
pub const WINDOW: SignedDuration = SignedDuration::from_hours(1);
/// The shortest time between two magic-link mails to one address: at most one mail in each
/// window of this length. It is shorter than the life of a magic link (`MAGIC_LINK_LIFETIME`).
pub const MAIL_COOLDOWN: SignedDuration = SignedDuration::from_mins(5);

/// The network that one client limit counts: an IPv4 address, or the /64 prefix of an IPv6
/// address. A host with IPv6 usually has a whole /64 and can send from each address of it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ClientNetwork(IpAddr);

impl ClientNetwork {
    /// The network of the client `address`. An IPv4 client of an IPv6 socket counts as IPv4.
    pub fn of(address: IpAddr) -> Self {
        Self(match address.to_canonical() {
            IpAddr::V4(address) => IpAddr::V4(address),
            IpAddr::V6(address) => {
                IpAddr::V6(Ipv6Addr::from(u128::from(address) & !u128::from(u64::MAX)))
            }
        })
    }

    /// The first address of the network.
    pub fn address(self) -> IpAddr {
        self.0
    }
}

impl fmt::Debug for ClientNetwork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ClientNetwork(redacted)")
    }
}

/// What a limit counts. The store keeps only a keyed hash of it (ADR 0056).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RateSubject<'a> {
    Email(&'a Email),
    Ip(ClientNetwork),
}

impl fmt::Debug for RateSubject<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // An IP address never goes to a log (ADR 0035).
        match self {
            Self::Email(_) => f.write_str("Email(redacted)"),
            Self::Ip(_) => f.write_str("Ip(redacted)"),
        }
    }
}

/// The most requests of one subject in one window of the length `window`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit<'a> {
    pub subject: RateSubject<'a>,
    pub limit: u32,
    pub window: SignedDuration,
}

/// The limits of a sign-in request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignInLimits<'a> {
    /// Refuses the request if the client sent too many.
    pub client: RateLimit<'a>,
    /// Stops the mail, but not the request, if the address got a mail in the same cooldown.
    pub mail: RateLimit<'a>,
}

/// The limits of a sign-in request for `email` from `client_ip`.
pub fn sign_in_limits(email: &Email, client_ip: IpAddr) -> SignInLimits<'_> {
    SignInLimits {
        client: RateLimit {
            subject: RateSubject::Ip(ClientNetwork::of(client_ip)),
            limit: SIGN_IN_PER_IP,
            window: WINDOW,
        },
        mail: RateLimit {
            subject: RateSubject::Email(email),
            limit: 1,
            window: MAIL_COOLDOWN,
        },
    }
}

/// The fixed window of a length that contains a time. The windows start at multiples of their
/// length after the Unix epoch, so all processes count in the same windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateWindow {
    pub start: Timestamp,
    pub length: SignedDuration,
}

impl RateWindow {
    pub fn containing(now: Timestamp, length: SignedDuration) -> Self {
        let seconds = length.as_secs();
        let second = now.as_second();
        let start =
            Timestamp::from_second(second - second.rem_euclid(seconds)).unwrap_or(Timestamp::MIN);
        Self { start, length }
    }

    pub fn end(self) -> Timestamp {
        self.start
            .saturating_add(self.length)
            .unwrap_or(Timestamp::MAX)
    }
}

/// If a request may go on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateDecision {
    Allowed,
    /// The client can try again after `retry_after`.
    Limited {
        retry_after: SignedDuration,
    },
}

impl RateDecision {
    /// The decision of `limit` for the request number `count` of its subject in the window of `now`.
    pub fn of(count: u32, limit: &RateLimit<'_>, now: Timestamp) -> Self {
        if count <= limit.limit {
            Self::Allowed
        } else {
            Self::Limited {
                retry_after: now.duration_until(RateWindow::containing(now, limit.window).end()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Timestamp {
        text.parse().unwrap()
    }

    #[test]
    fn a_window_starts_at_a_multiple_of_its_length() {
        let window = RateWindow::containing(at("2030-05-18T08:59:59Z"), WINDOW);
        assert_eq!(window.start, at("2030-05-18T08:00:00Z"));
        assert_eq!(window.end(), at("2030-05-18T09:00:00Z"));
        assert_eq!(
            RateWindow::containing(at("2030-05-18T09:00:00Z"), WINDOW).start,
            at("2030-05-18T09:00:00Z")
        );
        let cooldown = RateWindow::containing(at("2030-05-18T08:59:59Z"), MAIL_COOLDOWN);
        assert_eq!(cooldown.start, at("2030-05-18T08:55:00Z"));
        assert_eq!(cooldown.end(), at("2030-05-18T09:00:00Z"));
    }

    #[test]
    fn the_request_after_the_limit_waits_for_the_next_window() {
        let email = Email::parse("anna@example.org").unwrap();
        let limit = sign_in_limits(&email, "203.0.113.7".parse().unwrap()).client;
        let now = at("2030-05-18T08:45:00Z");
        assert_eq!(RateDecision::of(30, &limit, now), RateDecision::Allowed);
        assert_eq!(
            RateDecision::of(31, &limit, now),
            RateDecision::Limited {
                retry_after: SignedDuration::from_mins(15)
            }
        );
    }

    #[test]
    fn the_cooldown_allows_one_mail_and_is_shorter_than_a_magic_link() {
        let email = Email::parse("anna@example.org").unwrap();
        let mail = sign_in_limits(&email, "203.0.113.7".parse().unwrap()).mail;
        let now = at("2030-05-18T08:45:00Z");
        assert_eq!(RateDecision::of(1, &mail, now), RateDecision::Allowed);
        assert!(matches!(
            RateDecision::of(2, &mail, now),
            RateDecision::Limited { .. }
        ));
        // A suppressed request finds the link of the last mail still valid for this time or more.
        assert!(
            crate::outbound::MAGIC_LINK_LIFETIME - MAIL_COOLDOWN >= SignedDuration::from_mins(10)
        );
    }

    fn network(address: &str) -> IpAddr {
        ClientNetwork::of(address.parse().unwrap()).address()
    }

    #[test]
    fn an_ipv6_client_counts_by_its_64_prefix_and_an_ipv4_client_by_its_address() {
        assert_eq!(
            network("2001:db8:1:2:aaaa:bbbb:cccc:dddd"),
            network("2001:db8:1:2::1")
        );
        assert_eq!(
            network("2001:db8:1:2::1"),
            "2001:db8:1:2::".parse::<IpAddr>().unwrap()
        );
        assert_ne!(network("2001:db8:1:2::1"), network("2001:db8:1:3::1"));
        assert_eq!(
            network("203.0.113.7"),
            "203.0.113.7".parse::<IpAddr>().unwrap()
        );
        assert_ne!(network("203.0.113.7"), network("203.0.113.8"));
        assert_eq!(network("::ffff:203.0.113.7"), network("203.0.113.7"));
    }

    #[test]
    fn the_debug_text_holds_no_address() {
        let email = Email::parse("anna@example.org").unwrap();
        let text = format!(
            "{:?}",
            sign_in_limits(&email, "203.0.113.7".parse().unwrap())
        );
        assert!(!text.contains("anna"), "{text}");
        assert!(!text.contains("203.0.113.7"), "{text}");
    }
}
