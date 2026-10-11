//! The versions of tada and of its running components (ADR 0075).
//!
//! Only a signed-in member can read them: a version helps an attacker to select an exploit.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::{SignedDuration, Timestamp};

use crate::build_info::BuildInfo;
use crate::caller::MemberCaller;
use crate::store::StoreError;

/// The version of the HTTP API: the major version in the path (ADR 0017).
pub const API_VERSION: &str = "v1";
/// A role whose heartbeat is older than this is not running.
pub const ROLE_FRESHNESS: SignedDuration = SignedDuration::from_secs(60);

/// The last heartbeat of one process, as the process wrote it.
/// A process of a previous release writes no role, no version and no commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heartbeat {
    pub role: Option<String>,
    pub version: Option<String>,
    pub commit: Option<String>,
    pub last_seen: Timestamp,
}

/// The read port of the schema version and of the heartbeats.
#[async_trait]
pub trait VersionStore: Debug + Send + Sync {
    /// The number of the last applied migration.
    async fn schema_version(&self, caller: &MemberCaller) -> Result<i64, StoreError>;

    /// The heartbeats that are not older than `max_age` by the clock of the database, the oldest process first.
    async fn running_heartbeats(
        &self,
        caller: &MemberCaller,
        max_age: SignedDuration,
    ) -> Result<Vec<Heartbeat>, StoreError>;
}

/// One running process role with its version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleVersion {
    pub role: String,
    pub version: String,
    pub commit: String,
    pub last_seen: Timestamp,
}

/// The versions of all components, as the serving process sees them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemVersion {
    pub product: String,
    pub commit: String,
    pub schema: i64,
    pub api: &'static str,
    pub mcp: String,
    pub roles: Vec<RoleVersion>,
    /// True if a running role has another version or commit than this process.
    pub mismatch: bool,
}

/// The versions of the product and of the running roles. `build` is the build of the serving process.
pub async fn system_version(
    caller: &MemberCaller,
    store: &dyn VersionStore,
    build: &BuildInfo,
) -> Result<SystemVersion, StoreError> {
    let schema = store.schema_version(caller).await?;
    let roles: Vec<RoleVersion> = store
        .running_heartbeats(caller, ROLE_FRESHNESS)
        .await?
        .into_iter()
        .map(|heartbeat| RoleVersion {
            // Only the worker of a previous release wrote a heartbeat without a role.
            role: heartbeat.role.unwrap_or_else(|| "worker".to_owned()),
            version: heartbeat.version.unwrap_or_else(|| "unknown".to_owned()),
            commit: heartbeat.commit.unwrap_or_else(|| "unknown".to_owned()),
            last_seen: heartbeat.last_seen,
        })
        .collect();
    let mismatch = roles
        .iter()
        .any(|role| role.version != build.version || role.commit != build.commit);
    Ok(SystemVersion {
        product: build.version.to_owned(),
        commit: build.commit.to_owned(),
        schema,
        api: API_VERSION,
        mcp: build.version.to_owned(),
        roles,
        mismatch,
    })
}

#[cfg(test)]
mod tests {
    use tada_domain::identity::OrganizationRole;
    use tada_domain::ids::{OrganizationId, UserId};
    use uuid::Uuid;

    use super::*;

    fn caller() -> MemberCaller {
        MemberCaller::new(
            UserId::from_uuid(Uuid::from_u128(1)),
            OrganizationId::from_uuid(Uuid::from_u128(2)),
            OrganizationRole::Member,
        )
    }

    #[derive(Debug)]
    struct Fake(Vec<Heartbeat>);

    #[async_trait]
    impl VersionStore for Fake {
        async fn schema_version(&self, _: &MemberCaller) -> Result<i64, StoreError> {
            Ok(27)
        }
        async fn running_heartbeats(
            &self,
            _: &MemberCaller,
            _: SignedDuration,
        ) -> Result<Vec<Heartbeat>, StoreError> {
            Ok(self.0.clone())
        }
    }

    const BUILD: BuildInfo = BuildInfo {
        version: "0.2.0",
        commit: "abc1234",
        built_at: "2026-10-11T08:00:00Z",
    };

    fn heartbeat(role: Option<&str>, version: Option<&str>, commit: Option<&str>) -> Heartbeat {
        Heartbeat {
            role: role.map(str::to_owned),
            version: version.map(str::to_owned),
            commit: commit.map(str::to_owned),
            last_seen: Timestamp::UNIX_EPOCH,
        }
    }

    async fn show(heartbeats: Vec<Heartbeat>) -> SystemVersion {
        system_version(&caller(), &Fake(heartbeats), &BUILD)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn equal_roles_are_no_mismatch() {
        let shown = show(vec![heartbeat(
            Some("worker"),
            Some("0.2.0"),
            Some("abc1234"),
        )])
        .await;
        assert!(!shown.mismatch);
        assert_eq!(shown.product, "0.2.0");
        assert_eq!(shown.mcp, "0.2.0");
        assert_eq!(shown.api, "v1");
        assert_eq!(shown.schema, 27);
        assert_eq!(shown.roles[0].role, "worker");
    }

    #[tokio::test]
    async fn another_version_or_commit_is_a_mismatch() {
        let other_version = show(vec![heartbeat(
            Some("telegram"),
            Some("0.1.0"),
            Some("abc1234"),
        )])
        .await;
        let other_commit = show(vec![heartbeat(
            Some("serve"),
            Some("0.2.0"),
            Some("def5678"),
        )])
        .await;
        assert!(other_version.mismatch);
        assert!(other_commit.mismatch);
    }

    #[tokio::test]
    async fn a_heartbeat_of_a_previous_release_is_a_worker_of_unknown_version() {
        let shown = show(vec![heartbeat(None, None, None)]).await;
        assert!(shown.mismatch);
        assert_eq!(shown.roles[0].role, "worker");
        assert_eq!(shown.roles[0].version, "unknown");
    }
}
