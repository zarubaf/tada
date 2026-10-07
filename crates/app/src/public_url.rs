//! `TADA_PUBLIC_URL` (ADRs 0025 and 0042): the only base of each link that tada sends.

/// `TADA_PUBLIC_URL`. Each link starts with it, and its host is the right part of each
/// `Message-ID`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicUrl {
    /// The URL without the final slash, for example `https://tada.example.org`.
    origin: String,
    host: String,
}

/// The public URL is not an `http` or `https` URL without a path, a query and a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the public URL must be an http or https URL without a path")]
pub struct InvalidPublicUrl;

impl PublicUrl {
    pub fn parse(url: &str) -> Result<Self, InvalidPublicUrl> {
        let authority = ["https://", "http://"]
            .iter()
            .find_map(|scheme| url.strip_prefix(scheme))
            .ok_or(InvalidPublicUrl)?;
        let authority = authority.strip_suffix('/').unwrap_or(authority);
        if authority.is_empty() || authority.contains(['/', '?', '#', '@']) {
            return Err(InvalidPublicUrl);
        }
        // An IPv6 address keeps its brackets, as a `Message-ID` domain literal needs them.
        let host = match authority.find(']') {
            Some(end) if authority.starts_with('[') => &authority[..=end],
            _ => authority.split(':').next().unwrap_or(authority),
        };
        Ok(Self {
            origin: url.strip_suffix('/').unwrap_or(url).to_owned(),
            host: host.to_owned(),
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
        ] {
            let parsed = PublicUrl::parse(url).unwrap();
            assert_eq!((parsed.origin(), parsed.host()), (origin, host));
        }
    }

    #[test]
    fn a_public_url_with_a_path_or_another_scheme_is_invalid() {
        for url in [
            "https://tada.example.org/app",
            "ftp://tada.example.org",
            "https://",
            "https://anna@tada.example.org",
        ] {
            assert_eq!(PublicUrl::parse(url), Err(InvalidPublicUrl), "{url}");
        }
    }
}
