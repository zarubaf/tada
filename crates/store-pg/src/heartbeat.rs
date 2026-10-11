//! The worker heartbeat (ADR 0025). The database clock gives the times.

use async_trait::async_trait;
use jiff::SignedDuration;
use sqlx::types::Uuid;
use tada_app::build_info::{BuildInfo, Role};
use tada_app::caller::MemberCaller;
use tada_app::store::StoreError;
use tada_app::system::{Heartbeat, VersionStore};

use crate::Database;
use crate::error::store_error;

impl Database {
    /// Records that the process `worker_id` with the role `role` completed a loop (ADR 0025).
    /// The row holds the version of the build (ADR 0075).
    /// Only the `worker` row holds the number of due jobs and the age of the oldest one (ADR 0035),
    /// so the other roles do not count the job table.
    pub async fn record_heartbeat(
        &self,
        worker_id: Uuid,
        role: Role,
        build: &BuildInfo,
    ) -> Result<(), sqlx::Error> {
        let query = match role {
            Role::Worker => sqlx::query!(
                "INSERT INTO worker_heartbeat
                     (worker_id, started_at, last_loop_at, queue_depth, oldest_due_seconds, role, version, commit)
                 SELECT $1, now(), now(), count(*), extract(epoch FROM now() - min(run_at))::bigint, $2, $3, $4
                 FROM job
                 WHERE failed_at IS NULL AND run_at <= now()
                 ON CONFLICT (worker_id) DO UPDATE
                 SET last_loop_at = now(), queue_depth = excluded.queue_depth,
                     oldest_due_seconds = excluded.oldest_due_seconds,
                     role = excluded.role, version = excluded.version, commit = excluded.commit",
                worker_id,
                role.as_str(),
                build.version,
                build.commit,
            ),
            Role::Serve | Role::Telegram => sqlx::query!(
                "INSERT INTO worker_heartbeat (worker_id, started_at, last_loop_at, role, version, commit)
                 VALUES ($1, now(), now(), $2, $3, $4)
                 ON CONFLICT (worker_id) DO UPDATE
                 SET last_loop_at = now(),
                     role = excluded.role, version = excluded.version, commit = excluded.commit",
                worker_id,
                role.as_str(),
                build.version,
                build.commit,
            ),
        };
        query.execute(&self.pool).await.map(|_| ())
    }

    /// Removes the heartbeat of a worker that stops cleanly.
    pub async fn remove_heartbeat(&self, worker_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "DELETE FROM worker_heartbeat WHERE worker_id = $1",
            worker_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
    }
}

#[async_trait]
impl VersionStore for Database {
    async fn schema_version(&self, _caller: &MemberCaller) -> Result<i64, StoreError> {
        sqlx::query_scalar!(
            r#"SELECT max(version) AS "version?" FROM _sqlx_migrations WHERE success"#
        )
        .fetch_one(&self.pool)
        .await
        .map(|version| version.unwrap_or(0))
        .map_err(store_error)
    }

    async fn running_heartbeats(
        &self,
        _caller: &MemberCaller,
        max_age: SignedDuration,
    ) -> Result<Vec<Heartbeat>, StoreError> {
        // The database clock gives the age, as it gives the time of the heartbeat (ADR 0038).
        let rows = sqlx::query!(
            r#"SELECT role, version, commit, last_loop_at AS "last_loop_at: jiff_sqlx::Timestamp"
               FROM worker_heartbeat
               WHERE last_loop_at > now() - make_interval(secs => $1)
               ORDER BY started_at, worker_id"#,
            max_age.as_secs_f64(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(rows
            .into_iter()
            .map(|row| Heartbeat {
                role: row.role,
                version: row.version,
                commit: row.commit,
                last_seen: row.last_loop_at.to_jiff(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use tada_app::domain::identity::OrganizationRole;
    use tada_app::domain::ids::{OrganizationId, UserId};

    use super::*;
    use crate::testing::TestDatabase;

    const NEW: BuildInfo = BuildInfo {
        version: "0.1.0",
        commit: "2222222",
        built_at: "2026-10-01T00:00:00Z",
    };

    const OLD: BuildInfo = BuildInfo {
        version: "0.0.9",
        commit: "1111111",
        built_at: "2026-01-01T00:00:00Z",
    };

    fn caller() -> MemberCaller {
        MemberCaller::new(
            UserId::from_uuid(Uuid::from_u128(1)),
            OrganizationId::from_uuid(Uuid::from_u128(2)),
            OrganizationRole::Member,
        )
    }

    #[tokio::test]
    async fn the_heartbeat_holds_role_version_and_commit() {
        let test = TestDatabase::start().await;
        let (serve, worker) = (Uuid::now_v7(), Uuid::now_v7());
        let db = &test.database;
        db.record_heartbeat(serve, Role::Serve, &NEW).await.unwrap();
        db.record_heartbeat(worker, Role::Worker, &OLD)
            .await
            .unwrap();

        let beats = db
            .running_heartbeats(&caller(), SignedDuration::from_secs(60))
            .await
            .unwrap();
        let described: Vec<_> = beats
            .iter()
            .map(|beat| {
                (
                    beat.role.as_deref(),
                    beat.version.as_deref(),
                    beat.commit.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            described,
            [
                (Some("serve"), Some("0.1.0"), Some("2222222")),
                (Some("worker"), Some("0.0.9"), Some("1111111")),
            ]
        );
        // A heartbeat older than the age is not running any more.
        assert!(
            db.running_heartbeats(&caller(), SignedDuration::from_secs(-60))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn the_schema_version_is_the_last_migration() {
        let test = TestDatabase::start().await;
        assert!(test.database.schema_version(&caller()).await.unwrap() >= 27);
    }
}
