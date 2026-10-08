//! The `MemberStore` adapter: organization memberships and invitations (ADR 0056).

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::audit::{AuditAction, AuditEvent, AuditRole};
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::identity::{DisplayName, Email, EventRole};
use tada_app::domain::ids::{EventId, InvitationId, OrganizationId, UserId};
use tada_app::event_members::takes_last_manager;
use tada_app::members::{
    Invitation, InvitationInsert, LockedMembership, MemberCursor, MemberStore, OrganizationMember,
    Refusal, Remover,
};
use tada_app::outbound::Purpose;
use tada_app::store::StoreError;

use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error};
use crate::identity::organization_role;
use crate::outbound::queue_outbound;

/// Revokes the pending invitations `ids` of the organization and deletes their tokens.
/// A revoked invitation keeps no token, so no mailed or printed link of it works any more.
/// Returns the IDs of the invitations that were pending; the other IDs stay as they are.
pub(crate) async fn revoke_invitations(
    conn: &mut PgConnection,
    organization_id: OrganizationId,
    ids: &[Uuid],
    now: Timestamp,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let revoked = sqlx::query_scalar!(
        "UPDATE invitation SET status = 'revoked', revoked_at = $3
         WHERE organization_id = $1 AND id = ANY($2) AND status = 'pending'
         RETURNING id",
        organization_id.as_uuid(),
        ids,
        now.to_sqlx() as _,
    )
    .fetch_all(&mut *conn)
    .await?;
    sqlx::query!(
        "DELETE FROM invitation_token WHERE organization_id = $1 AND invitation_id = ANY($2)",
        organization_id.as_uuid(),
        &revoked,
    )
    .execute(&mut *conn)
    .await?;
    Ok(revoked)
}

struct MemberRow {
    user_id: Uuid,
    display_name: String,
    email: Option<String>,
    role: String,
    version: i64,
}

impl TryFrom<MemberRow> for OrganizationMember {
    type Error = InvalidRow;

    fn try_from(row: MemberRow) -> Result<Self, InvalidRow> {
        Ok(OrganizationMember {
            user_id: UserId::from_uuid(row.user_id),
            display_name: DisplayName::parse(&row.display_name)
                .map_err(|_| InvalidRow("app_user.display_name"))?,
            email: row
                .email
                .as_deref()
                .map(Email::parse)
                .transpose()
                .map_err(|_| InvalidRow("email_identity.email"))?,
            role: organization_role(&row.role)?,
            version: RecordVersion::new(row.version)
                .ok_or(InvalidRow("organization_membership.version"))?,
        })
    }
}

struct InvitationRow {
    id: Uuid,
    email: String,
    display_name: String,
    role: String,
    created_at: jiff_sqlx::Timestamp,
}

impl TryFrom<InvitationRow> for Invitation {
    type Error = InvalidRow;

    fn try_from(row: InvitationRow) -> Result<Self, InvalidRow> {
        Ok(Invitation {
            id: InvitationId::from_uuid(row.id),
            email: Email::parse(&row.email).map_err(|_| InvalidRow("invitation.email"))?,
            display_name: DisplayName::parse(&row.display_name)
                .map_err(|_| InvalidRow("invitation.display_name"))?,
            role: organization_role(&row.role)?,
            created_at: row.created_at.to_jiff(),
        })
    }
}

/// Locks the membership of `member` and all owner memberships of the organization, in the order
/// of the user IDs, and all event manager rows of the organization and all event memberships of
/// `member`, in the order of the event and user IDs. `None` if the user is not a member.
///
/// The event rows take the same ordered lock as a change of an event role (`event_members.rs`), so a
/// removal and a concurrent demotion of the other event manager cannot leave an event without an
/// event manager. READ COMMITTED checks the `WHERE` again on a row that another transaction
/// changed, so the second one sees one manager less (ADR 0052). The same holds for the owners.
async fn lock_membership(
    conn: &mut PgConnection,
    scope: OrgScope,
    member: UserId,
    remover: Remover,
) -> Result<Option<Locked>, StoreError> {
    let organization = scope.organization_id().as_uuid();
    // The row of the remover is locked too, so the removal uses the current role of the remover.
    let rows = sqlx::query!(
        "SELECT user_id, role, version FROM organization_membership
         WHERE organization_id = $1 AND (user_id = $2 OR user_id = $3 OR role = 'owner')
         ORDER BY user_id
         FOR UPDATE",
        organization,
        member.as_uuid(),
        remover.user_id().as_uuid(),
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?;
    let owners = rows.iter().filter(|row| row.role == "owner").count();
    let remover_role = rows
        .iter()
        .find(|row| row.user_id == remover.user_id().as_uuid())
        .map(|row| organization_role(&row.role))
        .transpose()?;
    let Some(row) = rows.into_iter().find(|row| row.user_id == member.as_uuid()) else {
        return Ok(None);
    };
    let events = sqlx::query!(
        "SELECT event_id, event_role, event_role = 'event-manager' AS \"manager!\",
                user_id = $2 AS \"member!\"
         FROM event_membership
         WHERE organization_id = $1 AND (user_id = $2 OR event_role = 'event-manager')
         ORDER BY event_id, user_id
         FOR UPDATE",
        organization,
        member.as_uuid(),
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?;
    let managers_of = |event: Uuid| {
        events
            .iter()
            .filter(|row| row.event_id == event && row.manager)
            .count()
    };
    let event_roles = events
        .iter()
        .filter(|row| row.member)
        .map(|row| {
            let role = EventRole::parse(&row.event_role)
                .ok_or(InvalidRow("event_membership.event_role"))?;
            Ok((EventId::from_uuid(row.event_id), role))
        })
        .collect::<Result<Vec<_>, InvalidRow>>()?;
    let only_manager = event_roles
        .iter()
        .any(|(event, role)| takes_last_manager(*role, None, managers_of(event.as_uuid())));
    Ok(Some(Locked {
        membership: LockedMembership {
            role: organization_role(&row.role)?,
            version: RecordVersion::new(row.version)
                .ok_or(InvalidRow("organization_membership.version"))?,
            owners,
            only_manager,
            remover_role,
        },
        event_roles,
    }))
}

/// The locked rows of a removal: the membership and the event roles of the member.
struct Locked {
    membership: LockedMembership,
    event_roles: Vec<(EventId, EventRole)>,
}

#[async_trait]
impl MemberStore for Database {
    async fn list(
        &self,
        scope: OrgScope,
        after: Option<MemberCursor>,
        limit: u32,
    ) -> Result<Vec<OrganizationMember>, StoreError> {
        // The cursor names the last member of the page before; its display name is the sort key.
        // The cursor comes from the client, so only a member of the organization counts.
        // Trade-off: if that member left between two pages, the list ends there.
        let rows = sqlx::query_as!(
            MemberRow,
            r#"SELECT m.user_id, u.display_name, e.email AS "email?", m.role, m.version
               FROM organization_membership m
               JOIN app_user u ON u.id = m.user_id
               LEFT JOIN email_identity e ON e.user_id = m.user_id
               WHERE m.organization_id = $1
                 AND ($2::uuid IS NULL OR (u.display_name, u.id) >
                      (SELECT c.display_name, c.id FROM app_user c
                       JOIN organization_membership cm
                           ON cm.user_id = c.id AND cm.organization_id = $1
                       WHERE c.id = $2))
               ORDER BY u.display_name, u.id
               LIMIT $3"#,
            scope.organization_id().as_uuid(),
            after.map(|cursor| cursor.0.as_uuid()),
            i64::from(limit),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(rows
            .into_iter()
            .map(OrganizationMember::try_from)
            .collect::<Result<_, _>>()?)
    }

    async fn invite(
        &self,
        scope: OrgScope,
        invitation: &Invitation,
        invited_by: UserId,
        audit: &AuditEvent,
    ) -> Result<InvitationInsert, StoreError> {
        let organization_id = scope.organization_id();
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        // The lock serializes the invitations of one organization, so two invitations of one
        // address cannot both stay pending. Other inserts that refer to the organization pass.
        sqlx::query!(
            "SELECT id FROM organization WHERE id = $1 FOR NO KEY UPDATE",
            organization_id.as_uuid(),
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(store_error)?;
        let id_taken = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM invitation WHERE id = $1) AS "exists!""#,
            invitation.id.as_uuid(),
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(store_error)?;
        if id_taken {
            return Ok(InvitationInsert::IdTaken);
        }
        let member = sqlx::query_scalar!(
            r#"SELECT EXISTS (
                   SELECT 1 FROM organization_membership m
                   JOIN email_identity e ON e.user_id = m.user_id
                   WHERE m.organization_id = $1 AND e.email = $2
               ) AS "exists!""#,
            organization_id.as_uuid(),
            invitation.email.as_str(),
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(store_error)?;
        if member {
            return Ok(InvitationInsert::AlreadyMember);
        }

        let pending = sqlx::query_scalar!(
            "SELECT id FROM invitation
             WHERE organization_id = $1 AND email = $2 AND status = 'pending'",
            organization_id.as_uuid(),
            invitation.email.as_str(),
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(store_error)?;
        let replaced =
            revoke_invitations(&mut tx, organization_id, &pending, invitation.created_at)
                .await
                .map_err(store_error)?;
        let inserted = sqlx::query!(
            "INSERT INTO invitation
                 (id, organization_id, email, display_name, role, invited_by, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            invitation.id.as_uuid(),
            organization_id.as_uuid(),
            invitation.email.as_str(),
            invitation.display_name.as_str(),
            invitation.role.as_str(),
            invited_by.as_uuid(),
            invitation.created_at.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await;
        match inserted {
            Ok(_) => {}
            // The check above serializes inside one organization only. Another organization can
            // take the ID in between; the rollback then also undoes the revocations.
            Err(sqlx::Error::Database(error)) if error.constraint() == Some("invitation_pkey") => {
                return Ok(InvitationInsert::IdTaken);
            }
            Err(error) => return Err(store_error(error)),
        }
        queue_outbound(
            &mut tx,
            &Purpose::Invitation {
                organization_id,
                invitation_id: invitation.id,
            },
            audit.actor().request_id(),
        )
        .await
        .map_err(store_error)?;
        for id in replaced {
            let event = audit.for_record(AuditAction::InvitationReplace, id);
            audit::record(&mut tx, &event).await.map_err(store_error)?;
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(InvitationInsert::Inserted)
    }

    async fn invitation(
        &self,
        scope: OrgScope,
        id: InvitationId,
    ) -> Result<Option<Invitation>, StoreError> {
        let row = sqlx::query_as!(
            InvitationRow,
            r#"SELECT id, email, display_name, role,
                      created_at AS "created_at: jiff_sqlx::Timestamp"
               FROM invitation WHERE organization_id = $1 AND id = $2"#,
            scope.organization_id().as_uuid(),
            id.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(row.map(Invitation::try_from).transpose()?)
    }

    async fn pending_invitations(&self, scope: OrgScope) -> Result<Vec<Invitation>, StoreError> {
        let rows = sqlx::query_as!(
            InvitationRow,
            r#"SELECT id, email, display_name, role,
                      created_at AS "created_at: jiff_sqlx::Timestamp"
               FROM invitation WHERE organization_id = $1 AND status = 'pending'
               ORDER BY created_at, id"#,
            scope.organization_id().as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(rows
            .into_iter()
            .map(Invitation::try_from)
            .collect::<Result<_, _>>()?)
    }

    async fn revoke(
        &self,
        scope: OrgScope,
        id: InvitationId,
        now: Timestamp,
        audit: &AuditEvent,
    ) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let revoked = revoke_invitations(&mut tx, scope.organization_id(), &[id.as_uuid()], now)
            .await
            .map_err(store_error)?;
        if revoked.is_empty() {
            return Ok(false);
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(true)
    }

    async fn remove(
        &self,
        scope: OrgScope,
        remover: Remover,
        member: UserId,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Option<Refusal>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let Some(locked) = lock_membership(&mut tx, scope, member, remover).await? else {
            return Ok(Some(Refusal::NotFound));
        };
        if let Some(refusal) = remover.refusal(member, expected_version, &locked.membership) {
            return Ok(Some(refusal));
        }
        // The event memberships of the member go with it (ON DELETE CASCADE).
        sqlx::query!(
            "DELETE FROM organization_membership WHERE organization_id = $1 AND user_id = $2",
            scope.organization_id().as_uuid(),
            member.as_uuid(),
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        // The log of each event shows that the member left it (ADR 0061).
        for (event, role) in locked.event_roles {
            let left = audit
                .for_record(AuditAction::EventMembershipRemove, event.as_uuid())
                .about(member)
                .with_roles(Some(AuditRole::Event(role)), None);
            audit::record(&mut tx, &left).await.map_err(store_error)?;
        }
        let audit = audit
            .clone()
            .with_roles(Some(AuditRole::Organization(locked.membership.role)), None);
        audit::record(&mut tx, &audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tada_app::caller::MemberCaller;
    use tada_app::domain::identity::OrganizationRole;
    use tada_app::event_members::{self, Added, Changed};
    use tada_app::outbound::OutboundStore;

    use super::*;
    use crate::testing::TestDatabase;

    const NOW: &str = "2030-05-18T08:00:00Z";

    struct Fixture {
        test: TestDatabase,
        owner: MemberCaller,
    }

    impl Fixture {
        /// Testwil with one owner.
        async fn start() -> Self {
            let test = TestDatabase::start().await;
            let testwil = test.create_organization("testwil").await;
            let owner = test
                .create_user(
                    &DisplayName::parse("Olga Owner").unwrap(),
                    &Email::parse("olga@example.org").unwrap(),
                )
                .await;
            test.add_membership(testwil, owner, OrganizationRole::Owner)
                .await;
            Self {
                test,
                owner: MemberCaller::new(owner, testwil, OrganizationRole::Owner),
            }
        }

        fn db(&self) -> &Database {
            &self.test.database
        }

        fn scope(&self) -> OrgScope {
            self.owner.scope()
        }

        /// A new member of Testwil with the name `name`.
        async fn member(&self, name: &str, role: OrganizationRole) -> UserId {
            let number = Uuid::now_v7().simple();
            let user = self
                .test
                .create_user(
                    &DisplayName::parse(name).unwrap(),
                    &Email::parse(&format!("m-{number}@example.org")).unwrap(),
                )
                .await;
            self.test
                .add_membership(self.scope().organization_id(), user, role)
                .await;
            user
        }

        fn invitation(&self, email: &str, role: OrganizationRole) -> Invitation {
            Invitation {
                id: InvitationId::from_uuid(Uuid::now_v7()),
                email: Email::parse(email).unwrap(),
                display_name: DisplayName::parse("Anna Muster").unwrap(),
                role,
                created_at: NOW.parse().unwrap(),
            }
        }

        async fn invite(&self, invitation: &Invitation) -> InvitationInsert {
            self.invite_as(&self.owner, invitation).await.unwrap()
        }

        /// An invitation of `inviter` into the organization of `inviter`.
        async fn invite_as(
            &self,
            inviter: &MemberCaller,
            invitation: &Invitation,
        ) -> Result<InvitationInsert, StoreError> {
            let audit = AuditEvent::new(
                inviter.actor(),
                AuditAction::InvitationCreate,
                Some(invitation.id.as_uuid()),
                Some(inviter.scope()),
            );
            self.db()
                .invite(inviter.scope(), invitation, inviter.user_id(), &audit)
                .await
        }

        async fn remove_as(&self, remover: &MemberCaller, member: UserId) -> Option<Refusal> {
            let audit = AuditEvent::new(
                remover.actor(),
                AuditAction::OrganizationMembershipRemove,
                Some(self.scope().organization_id().as_uuid()),
                Some(self.scope()),
            )
            .about(member);
            self.db()
                .remove(
                    self.scope(),
                    Remover::of(remover),
                    member,
                    RecordVersion::FIRST,
                    &audit,
                )
                .await
                .unwrap()
        }

        async fn remove(&self, member: UserId) -> Option<Refusal> {
            self.remove_as(&self.owner, member).await
        }

        /// The action, the record ID, the subject and the detail of each audit event, in order.
        async fn audit_rows(
            &self,
        ) -> Vec<(
            String,
            Option<Uuid>,
            Option<Uuid>,
            Option<serde_json::Value>,
        )> {
            sqlx::query_as(
                "SELECT action, record_id, subject_user_id, detail FROM audit_event
                 ORDER BY occurred_at, id",
            )
            .fetch_all(&self.db().pool)
            .await
            .unwrap()
        }

        async fn count(&self, sql: &str) -> i64 {
            self.test.scalar(sql).await
        }

        async fn event_with_managers(&self, managers: &[UserId]) -> EventId {
            let event = self
                .test
                .create_event(self.scope().organization_id(), "TEST30")
                .await;
            for manager in managers {
                sqlx::query(
                    "INSERT INTO event_membership
                         (organization_id, event_id, user_id, event_role, created_at)
                     VALUES ($1, $2, $3, 'event-manager', now())",
                )
                .bind(self.scope().organization_id().as_uuid())
                .bind(event.as_uuid())
                .bind(manager.as_uuid())
                .execute(&self.db().pool)
                .await
                .unwrap();
            }
            event
        }
    }

    #[tokio::test]
    async fn an_invitation_replaces_the_pending_invitation_of_its_address() {
        let f = Fixture::start().await;
        let first = f.invitation("anna@example.org", OrganizationRole::Member);
        assert_eq!(f.invite(&first).await, InvitationInsert::Inserted);
        let scope = f.scope();
        let token = f
            .db()
            .issue_invitation_token(scope, first.id, NOW.parse().unwrap())
            .await
            .unwrap();
        assert!(token.is_some());
        let other = f.invitation("berta@example.org", OrganizationRole::Admin);
        f.invite(&other).await;

        let second = f.invitation("anna@example.org", OrganizationRole::Admin);
        assert_eq!(f.invite(&second).await, InvitationInsert::Inserted);

        let pending = f.db().pending_invitations(scope).await.unwrap();
        assert_eq!(pending, [other.clone(), second.clone()]);
        assert_eq!(
            f.db().invitation(scope, first.id).await.unwrap(),
            Some(first.clone())
        );
        let status: String = f
            .test
            .scalar(&format!(
                "SELECT status FROM invitation WHERE id = '{}'",
                first.id
            ))
            .await;
        assert_eq!(status, "revoked");
        assert_eq!(f.count("SELECT count(*) FROM invitation_token").await, 0);
        assert_eq!(
            f.count("SELECT count(*) FROM outbound_intent WHERE purpose = 'invitation'")
                .await,
            3
        );
        let invited_by: Uuid = f
            .test
            .scalar(&format!(
                "SELECT invited_by FROM invitation WHERE id = '{}'",
                second.id
            ))
            .await;
        assert_eq!(invited_by, f.owner.user_id().as_uuid());
        let actions: Vec<_> = f
            .audit_rows()
            .await
            .into_iter()
            .map(|(action, record, ..)| (action, record))
            .collect();
        let id = |invitation: &Invitation| Some(invitation.id.as_uuid());
        assert_eq!(
            actions,
            [
                ("invitation.create".to_owned(), id(&first)),
                ("invitation.create".to_owned(), id(&other)),
                ("invitation.replace".to_owned(), id(&first)),
                ("invitation.create".to_owned(), id(&second)),
            ]
        );
    }

    #[tokio::test]
    async fn an_invitation_needs_a_free_id_and_an_address_without_membership() {
        let f = Fixture::start().await;
        let first = f.invitation("anna@example.org", OrganizationRole::Member);
        f.invite(&first).await;
        let again = Invitation {
            email: Email::parse("berta@example.org").unwrap(),
            ..first.clone()
        };
        assert_eq!(f.invite(&again).await, InvitationInsert::IdTaken);
        let member = f.invitation("OLGA@example.org", OrganizationRole::Admin);
        assert_eq!(f.invite(&member).await, InvitationInsert::AlreadyMember);
        assert_eq!(
            f.db().pending_invitations(f.scope()).await.unwrap(),
            [first]
        );
        assert_eq!(f.audit_rows().await.len(), 1);
    }

    /// Two organizations that insert an invitation with the same client ID at the same moment: the
    /// primary key decides, and the second one gets `IdTaken`, not an error.
    #[tokio::test]
    async fn the_same_id_in_two_organizations_at_once_is_taken_once() {
        let f = Fixture::start().await;
        let musterhausen = f.test.create_organization("musterhausen").await;
        let other = MemberCaller::new(f.owner.user_id(), musterhausen, OrganizationRole::Owner);
        for _ in 0..5 {
            let invitation = f.invitation("anna@example.org", OrganizationRole::Member);
            let (first, second) = tokio::join!(
                f.invite_as(&f.owner, &invitation),
                f.invite_as(&other, &invitation)
            );
            let mut results = [first.unwrap(), second.unwrap()];
            results.sort_by_key(|result| *result == InvitationInsert::IdTaken);
            assert_eq!(
                results,
                [InvitationInsert::Inserted, InvitationInsert::IdTaken]
            );
        }
    }

    #[tokio::test]
    async fn a_revocation_deletes_the_tokens_and_works_once() {
        let f = Fixture::start().await;
        let invitation = f.invitation("anna@example.org", OrganizationRole::Member);
        f.invite(&invitation).await;
        f.db()
            .issue_invitation_token(f.scope(), invitation.id, NOW.parse().unwrap())
            .await
            .unwrap();
        let audit = AuditEvent::new(
            f.owner.actor(),
            AuditAction::InvitationRevoke,
            Some(invitation.id.as_uuid()),
            Some(f.scope()),
        );
        let now = NOW.parse().unwrap();
        assert!(
            f.db()
                .revoke(f.scope(), invitation.id, now, &audit)
                .await
                .unwrap()
        );
        assert!(
            !f.db()
                .revoke(f.scope(), invitation.id, now, &audit)
                .await
                .unwrap()
        );
        assert_eq!(f.count("SELECT count(*) FROM invitation_token").await, 0);
        assert!(
            f.db()
                .pending_invitations(f.scope())
                .await
                .unwrap()
                .is_empty()
        );
        let actions: Vec<_> = f.audit_rows().await.into_iter().map(|row| row.0).collect();
        assert_eq!(actions, ["invitation.create", "invitation.revoke"]);

        // Another organization cannot revoke it.
        let other = f.invitation("berta@example.org", OrganizationRole::Member);
        f.invite(&other).await;
        let musterhausen = f.test.create_organization("musterhausen").await;
        let stranger =
            MemberCaller::new(f.owner.user_id(), musterhausen, OrganizationRole::Owner).scope();
        assert!(
            !f.db()
                .revoke(stranger, other.id, now, &audit)
                .await
                .unwrap()
        );
        assert_eq!(f.db().invitation(stranger, other.id).await.unwrap(), None);
    }

    #[tokio::test]
    async fn lists_the_members_by_display_name_in_pages() {
        let f = Fixture::start().await;
        let berta = f.member("Berta Beispiel", OrganizationRole::Member).await;
        let anna = f.member("Anna Muster", OrganizationRole::Admin).await;
        let first = f.db().list(f.scope(), None, 2).await.unwrap();
        let names: Vec<_> = first.iter().map(|m| m.display_name.as_str()).collect();
        assert_eq!(names, ["Anna Muster", "Berta Beispiel"]);
        assert_eq!(first[0].user_id, anna);
        assert_eq!(first[0].role, OrganizationRole::Admin);
        assert_eq!(first[0].version, RecordVersion::FIRST);
        assert!(first[0].email.is_some());
        let rest = f
            .db()
            .list(f.scope(), Some(MemberCursor(berta)), 2)
            .await
            .unwrap();
        let names: Vec<_> = rest.iter().map(|m| m.display_name.as_str()).collect();
        assert_eq!(names, ["Olga Owner"]);

        let musterhausen = f.test.create_organization("musterhausen").await;
        let other = MemberCaller::new(anna, musterhausen, OrganizationRole::Owner).scope();
        assert!(f.db().list(other, None, 10).await.unwrap().is_empty());

        // A cursor with a user of another organization reads nothing of that user.
        let stranger = f
            .test
            .create_user(
                &DisplayName::parse("Aaron Fremd").unwrap(),
                &Email::parse("aaron@example.org").unwrap(),
            )
            .await;
        f.test
            .add_membership(musterhausen, stranger, OrganizationRole::Member)
            .await;
        let foreign = f
            .db()
            .list(f.scope(), Some(MemberCursor(stranger)), 10)
            .await
            .unwrap();
        assert!(foreign.is_empty(), "{foreign:?}");
    }

    #[tokio::test]
    async fn a_removal_takes_the_event_memberships_and_records_the_old_role() {
        let f = Fixture::start().await;
        let anna = f.member("Anna Muster", OrganizationRole::Admin).await;
        let event = f.event_with_managers(&[f.owner.user_id()]).await;
        let added = event_members::EventMemberStore::add(
            f.db(),
            f.scope(),
            event,
            anna,
            EventRole::EventContributor,
            NOW.parse().unwrap(),
            &AuditEvent::new(
                f.owner.actor(),
                AuditAction::EventMembershipAdd,
                Some(event.as_uuid()),
                Some(f.scope()),
            ),
        )
        .await
        .unwrap();
        assert!(matches!(added, Added::Added(_)));

        assert_eq!(f.remove(anna).await, None);
        assert_eq!(f.remove(anna).await, Some(Refusal::NotFound));
        assert_eq!(
            f.count("SELECT count(*) FROM event_membership").await,
            1,
            "only the manager stays"
        );
        // Each event that the member leaves records it, before the organization membership.
        let rows = f.audit_rows().await;
        assert_eq!(
            rows[rows.len() - 2..],
            [
                (
                    "event_membership.remove".to_owned(),
                    Some(event.as_uuid()),
                    Some(anna.as_uuid()),
                    Some(json!({"old_role": "event-contributor"}))
                ),
                (
                    "organization_membership.remove".to_owned(),
                    Some(f.scope().organization_id().as_uuid()),
                    Some(anna.as_uuid()),
                    Some(json!({"old_role": "admin"}))
                )
            ]
        );
    }

    #[tokio::test]
    async fn the_last_owner_and_the_only_event_manager_stay() {
        let f = Fixture::start().await;
        let olga = f.owner.user_id();
        assert_eq!(f.remove(olga).await, Some(Refusal::LastOwner));
        let otto = f.member("Otto Owner", OrganizationRole::Owner).await;
        let anna = f.member("Anna Muster", OrganizationRole::Member).await;
        f.event_with_managers(&[anna]).await;
        assert_eq!(f.remove(anna).await, Some(Refusal::LastManager));
        let admin = MemberCaller::new(
            f.member("Adam Admin", OrganizationRole::Admin).await,
            f.scope().organization_id(),
            OrganizationRole::Admin,
        );
        assert_eq!(f.remove_as(&admin, otto).await, Some(Refusal::Forbidden));
        assert_eq!(f.remove(otto).await, None);
        assert_eq!(f.remove(olga).await, Some(Refusal::LastOwner));
        let removals: i64 = f
            .count(
                "SELECT count(*) FROM audit_event WHERE action = 'organization_membership.remove'",
            )
            .await;
        assert_eq!(removals, 1);
    }

    /// An owner of another organization neither sees nor removes a membership (ADR 0006).
    #[tokio::test]
    async fn a_removal_stays_inside_the_organization() {
        let f = Fixture::start().await;
        let anna = f.member("Anna Muster", OrganizationRole::Member).await;
        let musterhausen = f.test.create_organization("musterhausen").await;
        let stranger = f
            .test
            .create_user(
                &DisplayName::parse("Otto Fremd").unwrap(),
                &Email::parse("otto@example.org").unwrap(),
            )
            .await;
        f.test
            .add_membership(musterhausen, stranger, OrganizationRole::Owner)
            .await;
        let other = MemberCaller::new(stranger, musterhausen, OrganizationRole::Owner);
        let audit = AuditEvent::new(
            other.actor(),
            AuditAction::OrganizationMembershipRemove,
            Some(musterhausen.as_uuid()),
            Some(other.scope()),
        )
        .about(anna);
        let removed = f
            .db()
            .remove(
                other.scope(),
                Remover::of(&other),
                anna,
                RecordVersion::FIRST,
                &audit,
            )
            .await
            .unwrap();
        assert_eq!(removed, Some(Refusal::NotFound));
        assert_eq!(
            f.count("SELECT count(*) FROM organization_membership WHERE role = 'member'")
                .await,
            1
        );
    }

    /// The role of the remover counts as it is in the database, not as the session saw it.
    #[tokio::test]
    async fn a_demoted_or_removed_admin_cannot_remove() {
        let f = Fixture::start().await;
        let anna = f.member("Anna Muster", OrganizationRole::Member).await;
        let adam = f.member("Adam Admin", OrganizationRole::Admin).await;
        let admin = MemberCaller::new(adam, f.scope().organization_id(), OrganizationRole::Admin);
        sqlx::query("UPDATE organization_membership SET role = 'member' WHERE user_id = $1")
            .bind(adam.as_uuid())
            .execute(&f.db().pool)
            .await
            .unwrap();
        assert_eq!(f.remove_as(&admin, anna).await, Some(Refusal::Forbidden));
        sqlx::query("DELETE FROM organization_membership WHERE user_id = $1")
            .bind(adam.as_uuid())
            .execute(&f.db().pool)
            .await
            .unwrap();
        assert_eq!(f.remove_as(&admin, anna).await, Some(Refusal::Forbidden));
        assert_eq!(
            f.count("SELECT count(*) FROM organization_membership")
                .await,
            2
        );
    }

    /// Two owners who remove each other at the same time: the lock on the owner rows lets exactly
    /// one of them go.
    #[tokio::test]
    async fn two_owners_who_remove_each_other_leave_one_owner() {
        let f = Fixture::start().await;
        let olga = f.owner.clone();
        let otto = MemberCaller::new(
            f.member("Otto Owner", OrganizationRole::Owner).await,
            f.scope().organization_id(),
            OrganizationRole::Owner,
        );
        let (first, second) = tokio::join!(
            f.remove_as(&olga, otto.user_id()),
            f.remove_as(&otto, olga.user_id())
        );
        // The second one finds that the remover is no longer a member.
        let mut results = [first, second];
        results.sort_by_key(Option::is_some);
        assert_eq!(results, [None, Some(Refusal::Forbidden)]);
        assert_eq!(
            f.count("SELECT count(*) FROM organization_membership WHERE role = 'owner'")
                .await,
            1
        );
    }

    /// A removal of one event manager and a demotion of the other one at the same time: the
    /// ordered lock on the event manager rows lets exactly one of them happen (N8).
    #[tokio::test]
    async fn a_removal_and_a_demotion_of_two_managers_leave_one_manager() {
        let f = Fixture::start().await;
        let anna = f.member("Anna Muster", OrganizationRole::Member).await;
        let ben = f.member("Ben Beispiel", OrganizationRole::Member).await;
        let event = f.event_with_managers(&[anna, ben]).await;
        let demotion = AuditEvent::new(
            f.owner.actor(),
            AuditAction::EventMembershipChangeRole,
            Some(event.as_uuid()),
            Some(f.scope()),
        )
        .about(ben);
        let (removed, demoted) = tokio::join!(
            f.remove(anna),
            event_members::EventMemberStore::change_role(
                f.db(),
                f.scope(),
                event,
                ben,
                EventRole::EventViewer,
                RecordVersion::FIRST,
                &demotion,
            )
        );
        let demoted = demoted.unwrap();
        let refused = usize::from(removed == Some(Refusal::LastManager))
            + usize::from(demoted == Changed::LastManager);
        assert_eq!(refused, 1, "{removed:?} {demoted:?}");
        assert_eq!(
            f.count("SELECT count(*) FROM event_membership WHERE event_role = 'event-manager'")
                .await,
            1
        );
    }
}
