//! `TADA_PUBLIC_URL` (ADRs 0025 and 0042): the only base of each link that tada sends.
//!
//! This module is the only place that knows the form of a link: its path and its token.

use secrecy::{ExposeSecret, SecretString};

/// The path of the web page that signs in with the token of a magic link.
pub const MAGIC_LINK_PATH: &str = "/sign-in/link";
/// The path of the web page that accepts an invitation with the token of an invitation link.
pub const INVITATION_PATH: &str = "/invitation";

/// `TADA_PUBLIC_URL`. Each link starts with it, and its host is the right part of each
/// `Message-ID`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicUrl {
    /// The URL without the final slash, for example `https://tada.example.org`.
    origin: String,
    host: String,
}

/// Why a text is not a valid public URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidPublicUrl {
    #[error("the public URL is not a URL: {0}")]
    Syntax(url::ParseError),
    #[error("the public URL must start with http:// or https://")]
    Scheme,
    #[error("the public URL must have no path, query or fragment")]
    Path,
    #[error("the public URL must not contain a user name or a password")]
    Credentials,
}

impl PublicUrl {
    /// Parses the URL and normalizes it as a browser does: the scheme and the host in lowercase,
    /// and no default port. So the origin is equal to the `Origin` header of a browser.
    pub fn parse(text: &str) -> Result<Self, InvalidPublicUrl> {
        let url = url::Url::parse(text).map_err(InvalidPublicUrl::Syntax)?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(InvalidPublicUrl::Scheme);
        }
        if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
            return Err(InvalidPublicUrl::Path);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(InvalidPublicUrl::Credentials);
        }
        // An `http` or `https` URL always has a host. An IPv6 host keeps its brackets,
        // as a `Message-ID` domain literal needs them.
        let host = url.host_str().ok_or(InvalidPublicUrl::Scheme)?.to_owned();
        Ok(Self {
            origin: url.origin().ascii_serialization(),
            host,
        })
    }

    /// The scheme, the host and the port, without the final slash, for example
    /// `https://tada.example.org`.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// The host, for example `tada.example.org`. An IPv6 address keeps its brackets.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The link of a magic link with its token.
    pub fn magic_link(&self, token: &SecretString) -> SecretString {
        self.link(MAGIC_LINK_PATH, token)
    }

    /// The link of an invitation with its token.
    pub fn invitation_link(&self, token: &SecretString) -> SecretString {
        self.link(INVITATION_PATH, token)
    }

    /// The token goes into the fragment, so that the browser never sends it in a GET request
    /// (ADR 0008). The link holds the token, so it is a secret too.
    fn link(&self, path: &str, token: &SecretString) -> SecretString {
        SecretString::from(format!(
            "{}{path}#token={}",
            self.origin,
            token.expose_secret()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_public_url_gives_the_origin_and_the_host() {
        for (url, origin, host) in [
            (
                "https://tada.example.org/",
                "https://tada.example.org",
                "tada.example.org",
            ),
            (
                "http://localhost:5173",
                "http://localhost:5173",
                "localhost",
            ),
            ("http://[::1]:8080/", "http://[::1]:8080", "[::1]"),
            // A browser sends this origin, so the Origin check can compare it (ADR 0025).
            (
                "HTTPS://Tada.Example.org:443/",
                "https://tada.example.org",
                "tada.example.org",
            ),
            ("http://LOCALHOST:80", "http://localhost", "localhost"),
        ] {
            let parsed = PublicUrl::parse(url).unwrap();
            assert_eq!((parsed.origin(), parsed.host()), (origin, host));
        }
    }

    #[test]
    fn a_link_has_its_path_and_the_token_in_the_fragment() {
        let url = PublicUrl::parse("https://tada.example.org/").unwrap();
        let token = SecretString::from("abc");
        assert_eq!(
            url.magic_link(&token).expose_secret(),
            "https://tada.example.org/sign-in/link#token=abc"
        );
        assert_eq!(
            url.invitation_link(&token).expose_secret(),
            "https://tada.example.org/invitation#token=abc"
        );
    }

    #[test]
    fn a_public_url_with_a_path_or_another_scheme_is_invalid() {
        for url in [
            "https://tada.example.org/app",
            "ftp://tada.example.org",
            "https://",
            "https://anna@tada.example.org",
            "https://anna:secret@tada.example.org",
            "https://tada.example.org/?a=1",
            "https://tada.example.org/#x",
            "tada.example.org",
        ] {
            assert!(PublicUrl::parse(url).is_err(), "{url}");
        }
    }
}
