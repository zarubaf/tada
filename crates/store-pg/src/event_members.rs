//! The `EventMemberStore` adapter (ADR 0052).

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::identity::{DisplayName, EventRole};
use tada_app::domain::ids::{EventId, UserId};
use tada_app::event_members::{Added, Changed, EventMember, EventMemberStore};
use tada_app::store::StoreError;

use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error};

struct MemberRow {
    user_id: Uuid,
    display_name: String,
    event_role: String,
    version: i64,
    created_at: jiff_sqlx::Timestamp,
}

impl TryFrom<MemberRow> for EventMember {
    type Error = InvalidRow;

    fn try_from(row: MemberRow) -> Result<Self, InvalidRow> {
        Ok(EventMember {
            user_id: UserId::from_uuid(row.user_id),
            display_name: DisplayName::parse(&row.display_name)
                .map_err(|_| InvalidRow("app_user.display_name"))?,
            event_role: EventRole::parse(&row.event_role)
                .ok_or(InvalidRow("event_membership.event_role"))?,
            version: RecordVersion::new(row.version)
                .ok_or(InvalidRow("event_membership.version"))?,
            created_at: row.created_at.to_jiff(),
        })
    }
}

/// The event membership of `user` in `event`, read in the transaction of a change.
async fn read_member(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    user: UserId,
) -> Result<Option<EventMember>, StoreError> {
    let row = sqlx::query_as!(
        MemberRow,
        r#"SELECT m.user_id, u.display_name, m.event_role, m.version,
                  m.created_at AS "created_at: jiff_sqlx::Timestamp"
           FROM event_membership m JOIN app_user u ON u.id = m.user_id
           WHERE m.organization_id = $1 AND m.event_id = $2 AND m.user_id = $3"#,
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        user.as_uuid(),
    )
    .fetch_optional(conn)
    .await
    .map_err(store_error)?;
    Ok(row.map(EventMember::try_from).transpose()?)
}

/// The result of a change that matched no row: the membership is missing or has another version.
async fn unchanged<T>(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    user: UserId,
) -> Result<Changed<T>, StoreError> {
    Ok(match read_member(conn, scope, event, user).await? {
        None => Changed::NotFound,
        Some(_) => Changed::VersionConflict,
    })
}

#[async_trait]
impl EventMemberStore for Database {
    async fn list(&self, scope: OrgScope, event: EventId) -> Result<Vec<EventMember>, StoreError> {
        let rows = sqlx::query_as!(
            MemberRow,
            r#"SELECT m.user_id, u.display_name, m.event_role, m.version,
                      m.created_at AS "created_at: jiff_sqlx::Timestamp"
               FROM event_membership m JOIN app_user u ON u.id = m.user_id
               WHERE m.organization_id = $1 AND m.event_id = $2
               ORDER BY u.display_name, m.user_id"#,
            scope.organization_id().as_uuid(),
            event.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(rows
            .into_iter()
            .map(EventMember::try_from)
            .collect::<Result<_, _>>()?)
    }

    async fn add(
        &self,
        scope: OrgScope,
        event: EventId,
        user: UserId,
        role: EventRole,
        now: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Added, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let inserted = sqlx::query!(
            "INSERT INTO event_membership (organization_id, event_id, user_id, event_role, version, created_at)
             VALUES ($1, $2, $3, $4, 1, $5)",
            scope.organization_id().as_uuid(),
            event.as_uuid(),
            user.as_uuid(),
            role.as_str(),
            now.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await;
        match inserted {
            Ok(_) => {}
            Err(sqlx::Error::Database(error))
                if error.constraint() == Some("event_membership_pkey") =>
            {
                return Ok(Added::Taken);
            }
            Err(sqlx::Error::Database(error))
                if error.constraint() == Some("event_membership_organization_id_user_id_fkey") =>
            {
                return Ok(Added::NotMember);
            }
            Err(error) => return Err(store_error(error)),
        }
        let member = read_member(&mut tx, scope, event, user)
            .await?
            .ok_or(InvalidRow("event_membership"))?;
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Added::Added(member))
    }

    async fn change_role(
        &self,
        scope: OrgScope,
        event: EventId,
        user: UserId,
        role: EventRole,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Changed<EventMember>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let updated = sqlx::query!(
            "UPDATE event_membership SET event_role = $4, version = version + 1
             WHERE organization_id = $1 AND event_id = $2 AND user_id = $3 AND version = $5",
            scope.organization_id().as_uuid(),
            event.as_uuid(),
            user.as_uuid(),
            role.as_str(),
            expected_version.get(),
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        if updated.rows_affected() == 0 {
            return unchanged(&mut tx, scope, event, user).await;
        }
        let member = read_member(&mut tx, scope, event, user)
            .await?
            .ok_or(InvalidRow("event_membership"))?;
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Changed::Changed(member))
    }

    async fn remove(
        &self,
        scope: OrgScope,
        event: EventId,
        user: UserId,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Changed<()>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let deleted = sqlx::query!(
            "DELETE FROM event_membership
             WHERE organization_id = $1 AND event_id = $2 AND user_id = $3 AND version = $4",
            scope.organization_id().as_uuid(),
            event.as_uuid(),
            user.as_uuid(),
            expected_version.get(),
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        if deleted.rows_affected() == 0 {
            return unchanged(&mut tx, scope, event, user).await;
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Changed::Changed(()))
    }
}

#[cfg(test)]
mod tests {
    use tada_app::audit::AuditAction;
    use tada_app::caller::{MemberCaller, OrganizationRole};
    use tada_app::domain::identity::Email;

    use super::*;
    use crate::testing::TestDatabase;

    struct Fixture {
        test: TestDatabase,
        event: EventId,
        anna: UserId,
        owner: MemberCaller,
    }

    impl Fixture {
        /// Testwil with one event, its owner and Anna, a member without an event role.
        async fn start() -> Self {
            let test = TestDatabase::start().await;
            let testwil = test.create_organization("testwil").await;
            let anna = test
                .create_user(
                    &DisplayName::parse("Anna Muster").unwrap(),
                    &Email::parse("anna@example.org").unwrap(),
                )
                .await;
            test.add_membership(testwil, anna, OrganizationRole::Member)
                .await;
            let event = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO event (id, organization_id, key, name, time_zone, version, created_at)
                 VALUES ($1, $2, 'TEST30', 'Open Day Testwil', 'Europe/Zurich', 1, now())",
            )
            .bind(event)
            .bind(testwil.as_uuid())
            .execute(&test.database.pool)
            .await
            .unwrap();
            let owner = MemberCaller::new(
                UserId::from_uuid(Uuid::now_v7()),
                testwil,
                OrganizationRole::Owner,
            );
            Self {
                test,
                event: EventId::from_uuid(event),
                anna,
                owner,
            }
        }

        fn scope(&self) -> OrgScope {
            self.owner.scope()
        }

        fn audit(&self, action: AuditAction) -> AuditEvent {
            AuditEvent::new(
                self.owner.actor(),
                action,
                Some(self.event.as_uuid()),
                Some(self.scope()),
            )
        }

        async fn audit_actions(&self) -> Vec<String> {
            sqlx::query_scalar("SELECT action FROM audit_event ORDER BY occurred_at, id")
                .fetch_all(&self.test.database.pool)
                .await
                .unwrap()
        }

        async fn add(&self, user: UserId, role: EventRole) -> Added {
            self.test
                .database
                .add(
                    self.scope(),
                    self.event,
                    user,
                    role,
                    "2030-05-18T08:00:00.123456Z".parse().unwrap(),
                    &self.audit(AuditAction::EventMembershipAdd),
                )
                .await
                .unwrap()
        }
    }

    #[tokio::test]
    async fn adds_lists_changes_and_removes_an_event_membership_with_audit_events() {
        let f = Fixture::start().await;
        let db = &f.test.database;

        let Added::Added(added) = f.add(f.anna, EventRole::EventViewer).await else {
            panic!("not added");
        };
        assert_eq!(added.user_id, f.anna);
        assert_eq!(added.display_name.as_str(), "Anna Muster");
        assert_eq!(added.event_role, EventRole::EventViewer);
        assert_eq!(added.version, RecordVersion::FIRST);
        assert_eq!(
            added.created_at,
            "2030-05-18T08:00:00.123456Z".parse::<Timestamp>().unwrap()
        );
        assert_eq!(
            db.list(f.scope(), f.event).await.unwrap(),
            std::slice::from_ref(&added)
        );

        let changed = db
            .change_role(
                f.scope(),
                f.event,
                f.anna,
                EventRole::EventManager,
                RecordVersion::FIRST,
                &f.audit(AuditAction::EventMembershipChangeRole),
            )
            .await
            .unwrap();
        let Changed::Changed(changed) = changed else {
            panic!("not changed");
        };
        assert_eq!(changed.event_role, EventRole::EventManager);
        assert_eq!(changed.version, RecordVersion::new(2).unwrap());

        let removed = db
            .remove(
                f.scope(),
                f.event,
                f.anna,
                changed.version,
                &f.audit(AuditAction::EventMembershipRemove),
            )
            .await
            .unwrap();
        assert_eq!(removed, Changed::Changed(()));
        assert!(db.list(f.scope(), f.event).await.unwrap().is_empty());
        assert_eq!(
            f.audit_actions().await,
            [
                "event_membership.add",
                "event_membership.change_role",
                "event_membership.remove"
            ]
        );
    }

    #[tokio::test]
    async fn a_failed_change_writes_no_audit_event() {
        let f = Fixture::start().await;
        let db = &f.test.database;
        let stranger = UserId::from_uuid(Uuid::now_v7());
        assert_eq!(
            f.add(stranger, EventRole::EventViewer).await,
            Added::NotMember
        );
        f.add(f.anna, EventRole::EventViewer).await;
        assert_eq!(f.add(f.anna, EventRole::EventManager).await, Added::Taken);

        let second = RecordVersion::new(2).unwrap();
        let conflict = db
            .change_role(
                f.scope(),
                f.event,
                f.anna,
                EventRole::EventManager,
                second,
                &f.audit(AuditAction::EventMembershipChangeRole),
            )
            .await
            .unwrap();
        assert_eq!(conflict, Changed::VersionConflict);
        let conflict = db
            .remove(
                f.scope(),
                f.event,
                f.anna,
                second,
                &f.audit(AuditAction::EventMembershipRemove),
            )
            .await
            .unwrap();
        assert_eq!(conflict, Changed::VersionConflict);
        let missing = db
            .remove(
                f.scope(),
                f.event,
                stranger,
                RecordVersion::FIRST,
                &f.audit(AuditAction::EventMembershipRemove),
            )
            .await
            .unwrap();
        assert_eq!(missing, Changed::NotFound);

        assert_eq!(f.audit_actions().await, ["event_membership.add"]);
    }

    /// A scope of another organization neither sees nor changes the membership (ADR 0006).
    #[tokio::test]
    async fn keeps_event_memberships_inside_the_organization() {
        let f = Fixture::start().await;
        let db = &f.test.database;
        f.add(f.anna, EventRole::EventViewer).await;
        let musterhausen = f.test.create_organization("musterhausen").await;
        let other = MemberCaller::new(f.anna, musterhausen, OrganizationRole::Owner).scope();

        assert!(db.list(other, f.event).await.unwrap().is_empty());
        let changed = db
            .change_role(
                other,
                f.event,
                f.anna,
                EventRole::EventManager,
                RecordVersion::FIRST,
                &f.audit(AuditAction::EventMembershipChangeRole),
            )
            .await
            .unwrap();
        assert_eq!(changed, Changed::NotFound);
        let removed = db
            .remove(
                other,
                f.event,
                f.anna,
                RecordVersion::FIRST,
                &f.audit(AuditAction::EventMembershipRemove),
            )
            .await
            .unwrap();
        assert_eq!(removed, Changed::NotFound);
        assert_eq!(db.list(f.scope(), f.event).await.unwrap().len(), 1);
    }
}
