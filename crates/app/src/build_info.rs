//! The identity of this build: the product version, the commit and the build time (ADR 0075).

/// What the build wrote into the binary. The product version is the version of the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildInfo {
    /// `MAJOR.MINOR.PATCH`.
    pub version: &'static str,
    /// The short Git commit, or `unknown` when the build had no Git and no `TADA_BUILD_COMMIT`.
    pub commit: &'static str,
    /// The build time as `YYYY-MM-DDTHH:MM:SSZ`.
    pub built_at: &'static str,
}

impl BuildInfo {
    /// The build of this binary.
    pub const CURRENT: Self = Self {
        version: env!("CARGO_PKG_VERSION"),
        commit: env!("TADA_BUILD_COMMIT"),
        built_at: env!("TADA_BUILT_AT"),
    };
}

/// A process role of the binary. Each role writes a heartbeat with its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Serve,
    Worker,
    Telegram,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Serve => "serve",
            Self::Worker => "worker",
            Self::Telegram => "telegram",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_knows_its_version_commit_and_time() {
        let build = BuildInfo::CURRENT;
        assert_eq!(build.version, env!("CARGO_PKG_VERSION"));
        assert!(!build.commit.is_empty());
        let built_at: jiff::Timestamp = build.built_at.parse().unwrap();
        assert!(built_at.as_second() > 1_700_000_000);
    }

    #[test]
    fn a_role_has_a_stable_name() {
        let names = [Role::Serve, Role::Worker, Role::Telegram].map(Role::as_str);
        assert_eq!(names, ["serve", "worker", "telegram"]);
    }
}
