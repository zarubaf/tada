//! The `WorkstreamStore` adapter (ADR 0067).

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::types::Uuid;
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::ids::{EventId, UserId, WorkstreamId};
use tada_app::domain::work::{WorkstreamName, WorkstreamStatus};
use tada_app::store::StoreError;
use tada_app::workstreams::{Changed, Created, Workstream, WorkstreamStore, WorkstreamUpdate};

use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error};

struct WorkstreamRow {
    id: Uuid,
    event_id: Uuid,
    name: String,
    lead_user_id: Uuid,
    status: String,
    version: i64,
}

impl TryFrom<WorkstreamRow> for Workstream {
    type Error = InvalidRow;

    fn try_from(row: WorkstreamRow) -> Result<Self, InvalidRow> {
        Ok(Workstream {
            id: WorkstreamId::from_uuid(row.id),
            event_id: EventId::from_uuid(row.event_id),
            name: WorkstreamName::parse(&row.name).map_err(|_| InvalidRow("workstream.name"))?,
            lead: UserId::from_uuid(row.lead_user_id),
            status: WorkstreamStatus::parse(&row.status).ok_or(InvalidRow("workstream.status"))?,
            version: RecordVersion::new(row.version).ok_or(InvalidRow("workstream.version"))?,
        })
    }
}

fn violates(error: &sqlx::Error, constraint: &str) -> bool {
    matches!(error, sqlx::Error::Database(error) if error.constraint() == Some(constraint))
}

#[async_trait]
impl WorkstreamStore for Database {
    async fn create(
        &self,
        scope: OrgScope,
        workstream: &Workstream,
        now: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let inserted = sqlx::query!(
            "INSERT INTO workstream
                 (id, organization_id, event_id, name, lead_user_id, status, version, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)",
            workstream.id.as_uuid(),
            scope.organization_id().as_uuid(),
            workstream.event_id.as_uuid(),
            workstream.name.as_str(),
            workstream.lead.as_uuid(),
            workstream.status.as_str(),
            workstream.version.get(),
            now.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await;
        match inserted {
            Ok(_) => {}
            Err(error) if violates(&error, "workstream_pkey") => return Ok(Created::IdTaken),
            Err(error) if violates(&error, "workstream_name") => return Ok(Created::NameTaken),
            Err(error) => return Err(store_error(error)),
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Created::Created)
    }

    async fn change(
        &self,
        scope: OrgScope,
        event: EventId,
        id: WorkstreamId,
        update: &WorkstreamUpdate,
        expected: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Changed, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        // The row lock serializes two changes. The second one sees the new version and conflicts.
        let current = sqlx::query_scalar!(
            "SELECT version FROM workstream
             WHERE organization_id = $1 AND event_id = $2 AND id = $3
             FOR UPDATE",
            scope.organization_id().as_uuid(),
            event.as_uuid(),
            id.as_uuid(),
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        let Some(current) = current else {
            return Ok(Changed::NotFound);
        };
        if current != expected.get() {
            return Ok(Changed::VersionConflict);
        }
        let updated = sqlx::query_as!(
            WorkstreamRow,
            "UPDATE workstream
             SET name = COALESCE($4, name),
                 lead_user_id = COALESCE($5, lead_user_id),
                 status = COALESCE($6, status),
                 version = version + 1,
                 updated_at = now()
             WHERE organization_id = $1 AND event_id = $2 AND id = $3
             RETURNING id, event_id, name, lead_user_id, status, version",
            scope.organization_id().as_uuid(),
            event.as_uuid(),
            id.as_uuid(),
            update.name.as_ref().map(WorkstreamName::as_str),
            update.lead.map(UserId::as_uuid),
            update.status.map(WorkstreamStatus::as_str),
        )
        .fetch_one(&mut *tx)
        .await;
        let updated = match updated {
            Ok(row) => row,
            Err(error) if violates(&error, "workstream_name") => return Ok(Changed::NameTaken),
            Err(error) => return Err(store_error(error)),
        };
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Changed::Changed(Workstream::try_from(updated)?))
    }

    async fn get(
        &self,
        scope: OrgScope,
        event: EventId,
        id: WorkstreamId,
    ) -> Result<Option<Workstream>, StoreError> {
        let row = sqlx::query_as!(
            WorkstreamRow,
            "SELECT id, event_id, name, lead_user_id, status, version FROM workstream
             WHERE organization_id = $1 AND event_id = $2 AND id = $3",
            scope.organization_id().as_uuid(),
            event.as_uuid(),
            id.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(row.map(Workstream::try_from).transpose()?)
    }

    async fn list(&self, scope: OrgScope, event: EventId) -> Result<Vec<Workstream>, StoreError> {
        let rows = sqlx::query_as!(
            WorkstreamRow,
            "SELECT id, event_id, name, lead_user_id, status, version FROM workstream
             WHERE organization_id = $1 AND event_id = $2
             ORDER BY lower(name), id",
            scope.organization_id().as_uuid(),
            event.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(rows
            .into_iter()
            .map(Workstream::try_from)
            .collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use tada_app::audit::AuditAction;
    use tada_app::caller::{MemberCaller, OrganizationRole};
    use tada_app::domain::identity::{DisplayName, Email};

    use super::*;
    use crate::testing::TestDatabase;

    struct Fixture {
        test: TestDatabase,
        scope: OrgScope,
        caller: MemberCaller,
        event: EventId,
        lead: UserId,
    }

    impl Fixture {
        async fn start() -> Self {
            let test = TestDatabase::start().await;
            let testwil = test.create_organization("testwil").await;
            let event = test.create_event(testwil, "TEST30").await;
            let lead = test
                .create_user(
                    &DisplayName::parse("Anna Muster").unwrap(),
                    &Email::parse("anna@example.org").unwrap(),
                )
                .await;
            let caller = MemberCaller::new(lead, testwil, OrganizationRole::Owner);
            Self {
                test,
                scope: caller.scope(),
                caller,
                event,
                lead,
            }
        }

        fn workstream(&self, name: &str) -> Workstream {
            Workstream {
                id: WorkstreamId::from_uuid(Uuid::now_v7()),
                event_id: self.event,
                name: WorkstreamName::parse(name).unwrap(),
                lead: self.lead,
                status: WorkstreamStatus::Active,
                version: RecordVersion::FIRST,
            }
        }

        fn audit(&self, action: AuditAction, id: WorkstreamId) -> AuditEvent {
            AuditEvent::new(
                self.caller.actor(),
                action,
                Some(id.as_uuid()),
                Some(self.scope),
            )
        }

        async fn create(&self, workstream: &Workstream) -> Created {
            self.test
                .database
                .create(
                    self.scope,
                    workstream,
                    "2030-05-18T08:00:00Z".parse().unwrap(),
                    &self.audit(AuditAction::WorkstreamCreate, workstream.id),
                )
                .await
                .unwrap()
        }

        async fn change(
            &self,
            event: EventId,
            id: WorkstreamId,
            update: &WorkstreamUpdate,
            expected: RecordVersion,
        ) -> Changed {
            self.test
                .database
                .change(
                    self.scope,
                    event,
                    id,
                    update,
                    expected,
                    &self.audit(AuditAction::WorkstreamChange, id),
                )
                .await
                .unwrap()
        }

        async fn audit_actions(&self) -> Vec<String> {
            sqlx::query_scalar("SELECT action FROM audit_event ORDER BY occurred_at, id")
                .fetch_all(&self.test.database.pool)
                .await
                .unwrap()
        }
    }

    fn rename(name: &str) -> WorkstreamUpdate {
        WorkstreamUpdate {
            name: Some(WorkstreamName::parse(name).unwrap()),
            lead: None,
            status: None,
        }
    }

    #[tokio::test]
    async fn creates_lists_and_changes_a_workstream_with_audit_events() {
        let f = Fixture::start().await;
        let db = &f.test.database;
        let ground = f.workstream("Gelände");
        assert_eq!(f.create(&ground).await, Created::Created);
        assert_eq!(f.create(&f.workstream("Aufbau")).await, Created::Created);
        assert_eq!(
            db.get(f.scope, f.event, ground.id).await.unwrap(),
            Some(ground.clone())
        );
        let names: Vec<_> = db
            .list(f.scope, f.event)
            .await
            .unwrap()
            .into_iter()
            .map(|w| w.name.as_str().to_owned())
            .collect();
        assert_eq!(names, ["Aufbau", "Gelände"]);

        let update = WorkstreamUpdate {
            name: Some(WorkstreamName::parse("Aussengelände").unwrap()),
            lead: None,
            status: Some(WorkstreamStatus::Closed),
        };
        let Changed::Changed(changed) = f
            .change(f.event, ground.id, &update, RecordVersion::FIRST)
            .await
        else {
            panic!("not changed");
        };
        assert_eq!(changed.name.as_str(), "Aussengelände");
        assert_eq!(changed.status, WorkstreamStatus::Closed);
        assert_eq!(changed.lead, f.lead);
        assert_eq!(changed.version.get(), 2);
        assert_eq!(
            f.audit_actions().await,
            [
                "workstream.create",
                "workstream.create",
                "workstream.change"
            ]
        );
    }

    #[tokio::test]
    async fn a_refused_write_changes_nothing_and_writes_no_audit_event() {
        let f = Fixture::start().await;
        let ground = f.workstream("Gelände");
        f.create(&ground).await;
        assert_eq!(f.create(&ground).await, Created::IdTaken);
        assert_eq!(f.create(&f.workstream("GELÄNDE")).await, Created::NameTaken);

        let kitchen = f.workstream("Küche");
        f.create(&kitchen).await;
        assert_eq!(
            f.change(
                f.event,
                kitchen.id,
                &rename("gelände"),
                RecordVersion::FIRST
            )
            .await,
            Changed::NameTaken
        );
        let second = RecordVersion::new(2).unwrap();
        assert_eq!(
            f.change(f.event, kitchen.id, &rename("Bar"), second).await,
            Changed::VersionConflict
        );
        let missing = WorkstreamId::from_uuid(Uuid::now_v7());
        assert_eq!(
            f.change(f.event, missing, &rename("Bar"), RecordVersion::FIRST)
                .await,
            Changed::NotFound
        );
        assert_eq!(
            f.audit_actions().await,
            ["workstream.create", "workstream.create"]
        );
    }

    /// Another event and another organization neither see nor change the workstream (ADR 0006).
    #[tokio::test]
    async fn keeps_workstreams_inside_their_event_and_organization() {
        let f = Fixture::start().await;
        let db = &f.test.database;
        let ground = f.workstream("Gelände");
        f.create(&ground).await;

        let other_event = f
            .test
            .create_event(f.scope.organization_id(), "TEST31")
            .await;
        assert_eq!(db.get(f.scope, other_event, ground.id).await.unwrap(), None);
        assert!(db.list(f.scope, other_event).await.unwrap().is_empty());
        assert_eq!(
            f.change(other_event, ground.id, &rename("Bar"), RecordVersion::FIRST)
                .await,
            Changed::NotFound
        );

        let musterhausen = f.test.create_organization("musterhausen").await;
        let other = MemberCaller::new(f.lead, musterhausen, OrganizationRole::Owner).scope();
        assert_eq!(db.get(other, f.event, ground.id).await.unwrap(), None);
        assert!(db.list(other, f.event).await.unwrap().is_empty());
        let changed = db
            .change(
                other,
                f.event,
                ground.id,
                &rename("Bar"),
                RecordVersion::FIRST,
                &f.audit(AuditAction::WorkstreamChange, ground.id),
            )
            .await
            .unwrap();
        assert_eq!(changed, Changed::NotFound);
    }

    /// Two changes with the same expected version: exactly one wins.
    #[tokio::test]
    async fn two_changes_with_one_version_leave_one_winner() {
        let f = Fixture::start().await;
        let ground = f.workstream("Gelände");
        f.create(&ground).await;
        let (alpha, beta) = (rename("Alpha"), rename("Beta"));
        let (a, b) = tokio::join!(
            f.change(f.event, ground.id, &alpha, RecordVersion::FIRST),
            f.change(f.event, ground.id, &beta, RecordVersion::FIRST)
        );
        let winners = [&a, &b]
            .iter()
            .filter(|changed| matches!(changed, Changed::Changed(_)))
            .count();
        assert_eq!(winners, 1, "{a:?} {b:?}");
        assert!([&a, &b].contains(&&Changed::VersionConflict));
    }
}
