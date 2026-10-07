//! The `EventStore` adapter.

use async_trait::async_trait;
use jiff_sqlx::ToSqlx;
use sqlx::types::Uuid;
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::events::{Event, EventKey, EventName, EventTimeZone};
use tada_app::domain::ids::{EventId, OrganizationId, UserId};
use tada_app::events::{EventCursor, EventStore, Inserted};
use tada_app::store::StoreError;

use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error};

struct EventRow {
    id: Uuid,
    organization_id: Uuid,
    key: String,
    name: String,
    time_zone: String,
    version: i64,
    created_at: jiff_sqlx::Timestamp,
}

impl TryFrom<EventRow> for Event {
    type Error = InvalidRow;

    fn try_from(row: EventRow) -> Result<Self, InvalidRow> {
        Ok(Event {
            id: EventId::from_uuid(row.id),
            organization_id: OrganizationId::from_uuid(row.organization_id),
            key: EventKey::parse(&row.key).map_err(|_| InvalidRow("event.key"))?,
            name: EventName::parse(&row.name).map_err(|_| InvalidRow("event.name"))?,
            time_zone: EventTimeZone::parse(&row.time_zone)
                .map_err(|_| InvalidRow("event.time_zone"))?,
            version: RecordVersion::new(row.version).ok_or(InvalidRow("event.version"))?,
            created_at: row.created_at.to_jiff(),
        })
    }
}

#[async_trait]
impl EventStore for Database {
    async fn insert(
        &self,
        scope: OrgScope,
        event: &Event,
        manager: UserId,
        audit: &[AuditEvent],
    ) -> Result<Inserted, StoreError> {
        debug_assert_eq!(scope.organization_id(), event.organization_id);
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let result = sqlx::query!(
            "INSERT INTO event (id, organization_id, key, name, time_zone, version, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            event.id.as_uuid(),
            scope.organization_id().as_uuid(),
            event.key.as_str(),
            event.name.as_str(),
            event.time_zone.as_str(),
            event.version.get(),
            event.created_at.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await;
        match result {
            Ok(_) => {}
            Err(sqlx::Error::Database(error)) if error.constraint() == Some("event_pkey") => {
                return Ok(Inserted::IdTaken);
            }
            Err(sqlx::Error::Database(error)) if error.constraint() == Some("event_key_unique") => {
                return Ok(Inserted::KeyTaken);
            }
            Err(error) => return Err(store_error(error)),
        }
        // An event has at least one event manager: its creator (ADR 0052).
        sqlx::query!(
            "INSERT INTO event_membership
                 (organization_id, event_id, user_id, event_role, version, created_at)
             VALUES ($1, $2, $3, 'event-manager', 1, $4)",
            scope.organization_id().as_uuid(),
            event.id.as_uuid(),
            manager.as_uuid(),
            event.created_at.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        for entry in audit {
            audit::record(&mut tx, entry).await.map_err(store_error)?;
        }
        tx.commit().await.map_err(store_error)?;
        Ok(Inserted::Inserted)
    }

    async fn get(&self, scope: OrgScope, id: EventId) -> Result<Option<Event>, StoreError> {
        let row = sqlx::query_as!(
            EventRow,
            r#"SELECT id, organization_id, key, name, time_zone, version,
                      created_at AS "created_at: jiff_sqlx::Timestamp"
               FROM event
               WHERE organization_id = $1 AND id = $2"#,
            scope.organization_id().as_uuid(),
            id.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(row.map(Event::try_from).transpose()?)
    }

    async fn list(
        &self,
        scope: OrgScope,
        after: Option<&EventCursor>,
        limit: u32,
    ) -> Result<Vec<Event>, StoreError> {
        let rows = sqlx::query_as!(
            EventRow,
            r#"SELECT id, organization_id, key, name, time_zone, version,
                      created_at AS "created_at: jiff_sqlx::Timestamp"
               FROM event
               WHERE organization_id = $1
                 AND ($2::text IS NULL OR (key, id) > ($2, $3))
               ORDER BY key, id
               LIMIT $4"#,
            scope.organization_id().as_uuid(),
            after.map(|cursor| cursor.key.as_str()),
            after.map(|cursor| cursor.id.as_uuid()),
            i64::from(limit),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(rows
            .into_iter()
            .map(Event::try_from)
            .collect::<Result<_, _>>()?)
    }

    async fn list_of_member(
        &self,
        scope: OrgScope,
        member: UserId,
        after: Option<&EventCursor>,
        limit: u32,
    ) -> Result<Vec<Event>, StoreError> {
        let rows = sqlx::query_as!(
            EventRow,
            r#"SELECT e.id, e.organization_id, e.key, e.name, e.time_zone, e.version,
                      e.created_at AS "created_at: jiff_sqlx::Timestamp"
               FROM event e
               JOIN event_membership m ON m.organization_id = e.organization_id AND m.event_id = e.id
               WHERE e.organization_id = $1
                 AND m.user_id = $2
                 AND ($3::text IS NULL OR (e.key, e.id) > ($3, $4))
               ORDER BY e.key, e.id
               LIMIT $5"#,
            scope.organization_id().as_uuid(),
            member.as_uuid(),
            after.map(|cursor| cursor.key.as_str()),
            after.map(|cursor| cursor.id.as_uuid()),
            i64::from(limit),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(rows
            .into_iter()
            .map(Event::try_from)
            .collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use tada_app::audit::{AuditAction, AuditRole};
    use tada_app::caller::MemberCaller;
    use tada_app::caller::OrganizationRole::{Member, Owner};
    use tada_app::domain::identity::{DisplayName, Email, EventRole};

    use super::*;
    use crate::testing::TestDatabase;

    fn scope(organization: OrganizationId) -> OrgScope {
        MemberCaller::new(UserId::from_uuid(Uuid::now_v7()), organization, Owner).scope()
    }

    /// A new owner of the organization, with a user and a membership: the creator of events.
    async fn owner(test: &TestDatabase, organization: OrganizationId) -> MemberCaller {
        let number = Uuid::now_v7().simple();
        let user = test
            .create_user(
                &DisplayName::parse("Olivia Owner").unwrap(),
                &Email::parse(&format!("owner-{number}@example.org")).unwrap(),
            )
            .await;
        test.add_membership(organization, user, Owner).await;
        MemberCaller::new(user, organization, Owner)
    }

    /// Inserts an event of `owner` without audit events.
    async fn insert(test: &TestDatabase, owner: &MemberCaller, event: &Event) -> Inserted {
        test.database
            .insert(owner.scope(), event, owner.user_id(), &[])
            .await
            .unwrap()
    }

    fn event(organization: OrganizationId, key: &str) -> Event {
        Event {
            id: EventId::from_uuid(Uuid::now_v7()),
            organization_id: organization,
            key: EventKey::parse(key).unwrap(),
            name: EventName::parse("Open Day Testwil").unwrap(),
            time_zone: EventTimeZone::default_zone(),
            version: RecordVersion::FIRST,
            created_at: "2030-05-18T08:00:00.123456Z".parse::<Timestamp>().unwrap(),
        }
    }

    #[tokio::test]
    async fn stores_and_reads_an_event() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let event = event(organization, "TEST30");

        let inserted = insert(&test, &owner(&test, organization).await, &event).await;
        assert_eq!(inserted, Inserted::Inserted);
        let read = test
            .database
            .get(scope(organization), event.id)
            .await
            .unwrap();
        assert_eq!(read, Some(event));
    }

    /// The creator of an event becomes its event manager in the transaction of the insert, with
    /// the audit events of both (ADR 0052, ADR 0061).
    #[tokio::test]
    async fn the_creator_becomes_the_event_manager_of_a_new_event() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let owner = owner(&test, organization).await;
        let event = event(organization, "TEST30");
        let audit = [
            AuditEvent::new(
                owner.actor(),
                AuditAction::EventCreate,
                Some(event.id.as_uuid()),
                Some(owner.scope()),
            ),
            AuditEvent::new(
                owner.actor(),
                AuditAction::EventMembershipAdd,
                Some(event.id.as_uuid()),
                Some(owner.scope()),
            )
            .about(owner.user_id())
            .with_roles(None, Some(AuditRole::Event(EventRole::EventManager))),
        ];
        let db = &test.database;
        let inserted = db
            .insert(owner.scope(), &event, owner.user_id(), &audit)
            .await
            .unwrap();
        assert_eq!(inserted, Inserted::Inserted);
        // A retry with the same ID and a new event with the same key add nothing.
        let retry = db
            .insert(owner.scope(), &event, owner.user_id(), &audit)
            .await
            .unwrap();
        assert_eq!(retry, Inserted::IdTaken);
        let same_key = Event {
            id: EventId::from_uuid(Uuid::now_v7()),
            ..event.clone()
        };
        let taken = db
            .insert(owner.scope(), &same_key, owner.user_id(), &audit)
            .await
            .unwrap();
        assert_eq!(taken, Inserted::KeyTaken);

        let members: Vec<(Uuid, Uuid, String, i64)> =
            sqlx::query_as("SELECT event_id, user_id, event_role, version FROM event_membership")
                .fetch_all(&db.pool)
                .await
                .unwrap();
        assert_eq!(
            members,
            [(
                event.id.as_uuid(),
                owner.user_id().as_uuid(),
                "event-manager".to_owned(),
                1
            )]
        );
        let actions: Vec<String> = sqlx::query_scalar("SELECT action FROM audit_event ORDER BY id")
            .fetch_all(&db.pool)
            .await
            .unwrap();
        assert_eq!(actions, ["event.create", "event_membership.add"]);
    }

    #[tokio::test]
    async fn reports_a_taken_id_and_a_taken_key() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let first = event(organization, "TEST30");
        insert(&test, &owner(&test, organization).await, &first).await;

        let same_id = Event {
            key: EventKey::parse("OTHER").unwrap(),
            ..first.clone()
        };
        assert_eq!(
            insert(&test, &owner(&test, organization).await, &same_id).await,
            Inserted::IdTaken
        );
        let same_key = event(organization, "TEST30");
        assert_eq!(
            insert(&test, &owner(&test, organization).await, &same_key).await,
            Inserted::KeyTaken
        );
    }

    /// Two organizations with the same key see only their own event (ADR 0006).
    #[tokio::test]
    async fn keeps_the_events_of_two_organizations_apart() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let musterhausen = test.create_organization("musterhausen").await;
        let theirs = event(testwil, "TEST30");
        let ours = event(musterhausen, "TEST30");
        insert(&test, &owner(&test, testwil).await, &theirs).await;
        insert(&test, &owner(&test, musterhausen).await, &ours).await;

        let listed = test
            .database
            .list(scope(musterhausen), None, 10)
            .await
            .unwrap();
        assert_eq!(listed, [ours]);
        assert_eq!(
            test.database
                .get(scope(musterhausen), theirs.id)
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn lists_after_the_cursor_in_the_order_of_the_keys() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        for key in ["CC", "AA", "BB", "DD"] {
            insert(
                &test,
                &owner(&test, organization).await,
                &event(organization, key),
            )
            .await;
        }
        let first = test
            .database
            .list(scope(organization), None, 2)
            .await
            .unwrap();
        let keys: Vec<_> = first.iter().map(|event| event.key.as_str()).collect();
        assert_eq!(keys, ["AA", "BB"]);

        let cursor = EventCursor {
            key: first[1].key.clone(),
            id: first[1].id,
        };
        let second = test
            .database
            .list(scope(organization), Some(&cursor), 2)
            .await
            .unwrap();
        let keys: Vec<_> = second.iter().map(|event| event.key.as_str()).collect();
        assert_eq!(keys, ["CC", "DD"]);
    }

    #[tokio::test]
    async fn lists_only_the_events_with_a_membership_of_the_member() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let anna = test
            .create_user(
                &DisplayName::parse("Anna Muster").unwrap(),
                &Email::parse("anna@example.org").unwrap(),
            )
            .await;
        test.add_membership(organization, anna, Member).await;
        let mut events = Vec::new();
        for key in ["CC", "AA", "BB"] {
            let event = event(organization, key);
            insert(&test, &owner(&test, organization).await, &event).await;
            events.push(event);
        }
        for event in events.iter().filter(|event| event.key.as_str() != "AA") {
            sqlx::query(
                "INSERT INTO event_membership (organization_id, event_id, user_id, event_role, created_at)
                 VALUES ($1, $2, $3, 'event-viewer', now())",
            )
            .bind(organization.as_uuid())
            .bind(event.id.as_uuid())
            .bind(anna.as_uuid())
            .execute(&test.database.pool)
            .await
            .unwrap();
        }

        let first = test
            .database
            .list_of_member(scope(organization), anna, None, 1)
            .await
            .unwrap();
        let keys: Vec<_> = first.iter().map(|event| event.key.as_str()).collect();
        assert_eq!(keys, ["BB"]);
        let cursor = EventCursor {
            key: first[0].key.clone(),
            id: first[0].id,
        };
        let rest = test
            .database
            .list_of_member(scope(organization), anna, Some(&cursor), 10)
            .await
            .unwrap();
        let keys: Vec<_> = rest.iter().map(|event| event.key.as_str()).collect();
        assert_eq!(keys, ["CC"]);

        let other = UserId::from_uuid(Uuid::now_v7());
        let none = test
            .database
            .list_of_member(scope(organization), other, None, 10)
            .await
            .unwrap();
        assert!(none.is_empty());
    }
}
