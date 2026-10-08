//! Personal API tokens (ADR 0039, ADR 0045): a member gives an external client, for example an MCP client, access in one organization.

/// What a token allows (ADR 0039). There is no `write` scope: an AI behind a token never changes accepted state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenScope {
    /// Queries only.
    Read,
    /// Queries and proposals.
    Propose,
}

impl TokenScope {
    /// The name that the API and the database use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Propose => "propose",
        }
    }

    /// The scope of a name of `as_str`.
    pub fn parse(name: &str) -> Option<Self> {
        [Self::Read, Self::Propose]
            .into_iter()
            .find(|scope| scope.as_str() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_have_stable_names() {
        let names = [TokenScope::Read, TokenScope::Propose].map(TokenScope::as_str);
        assert_eq!(names, ["read", "propose"]);
        for name in names {
            assert_eq!(TokenScope::parse(name).map(TokenScope::as_str), Some(name));
        }
        assert_eq!(TokenScope::parse("write"), None);
    }
}
