//! The identity of a build: the product version, the commit and the build time (ADR 0075).
//! The `tada` binary writes the values of its own build; `app` only knows their shape.

/// What the build wrote into the binary. The product version is the version of the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildInfo {
    /// `MAJOR.MINOR.PATCH`.
    pub version: &'static str,
    /// The short Git commit, or `unknown` when the build had no Git and no `TADA_BUILD_COMMIT`.
    pub commit: &'static str,
    /// The build time as `YYYY-MM-DDTHH:MM:SSZ`.
    pub built_at: &'static str,
    /// The random ID of this build (ADR 0078). The web client of the same build has the same ID, and the
    /// client compares the two. Nobody can compute the version or the commit from the ID, but the ID of a public
    /// image is easy to look up.
    pub build_id: &'static str,
}

/// A process role of the binary. Each role writes a heartbeat with its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessRole {
    Serve,
    Worker,
    Telegram,
}

impl ProcessRole {
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
    fn a_role_has_a_stable_name() {
        let names = [
            ProcessRole::Serve,
            ProcessRole::Worker,
            ProcessRole::Telegram,
        ]
        .map(ProcessRole::as_str);
        assert_eq!(names, ["serve", "worker", "telegram"]);
    }
}
