//! The `TelegramLinks` adapter (ADR 0011). Codes are 256 random bits; the table holds their SHA-256 hash (ADR 0008).

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use secrecy::ExposeSecret;
use sqlx::types::Uuid;
use tada_app::caller::OrgScope;
use tada_app::domain::identity::Email;
use tada_app::domain::ids::UserId;
use tada_app::store::StoreError;
use tada_app::telegram::{
    Confirmed, LinkRequest, LinkTarget, TelegramLink, TelegramLinks, TelegramName, TelegramUserId,
};

use crate::Database;
use crate::error::{InvalidRow, store_error};
use crate::token::{hash_token, new_token};

#[async_trait]
impl TelegramLinks for Database {
    async fn create_code(
        &self,
        scope: OrgScope,
        user_id: UserId,
        expires_at: Timestamp,
    ) -> Result<String, StoreError> {
        // Telegram accepts this alphabet and length in a `/start` parameter of a deep link.
        let token = new_token("")?;
        sqlx::query!(
            "INSERT INTO telegram_link_code (id, organization_id, user_id, code_hash, expires_at, created_at)
             VALUES ($1, $2, $3, $4, $5, now())",
            Uuid::now_v7(),
            scope.organization_id().as_uuid(),
            user_id.as_uuid(),
            token.hash,
            expires_at.to_sqlx() as _,
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(token.secret.expose_secret().to_owned())
    }

    async fn claim(
        &self,
        code: &str,
        account: TelegramUserId,
        name: &TelegramName,
        now: Timestamp,
    ) -> Result<Option<LinkTarget>, StoreError> {
        let target = sqlx::query!(
            "WITH claimed AS (
                 UPDATE telegram_link_code
                 SET claimed_by = $2, claimed_name = $3, claimed_at = $4
                 WHERE code_hash = $1 AND claimed_by IS NULL AND expires_at > $4
                 RETURNING organization_id, user_id
             )
             SELECT u.display_name AS user_name, o.name AS organization_name, e.email
             FROM claimed c
             JOIN app_user u ON u.id = c.user_id
             JOIN email_identity e ON e.user_id = c.user_id
             JOIN organization o ON o.id = c.organization_id",
            hash_token(code),
            account.0,
            name.0,
            now.to_sqlx() as _,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        target
            .map(|row| {
                Ok(LinkTarget {
                    user_name: row.user_name,
                    organization_name: row.organization_name,
                    email: Email::parse(&row.email)
                        .map_err(|_| InvalidRow("email_identity.email"))?,
                })
            })
            .transpose()
    }

    async fn accept(&self, account: TelegramUserId, now: Timestamp) -> Result<bool, StoreError> {
        // Only the newest claim: the bot named its tada account in the last reply.
        let result = sqlx::query!(
            "UPDATE telegram_link_code SET accepted_at = $2
             WHERE id = (
                 SELECT id FROM telegram_link_code
                 WHERE claimed_by = $1 AND expires_at > $2
                 ORDER BY claimed_at DESC, id DESC
                 LIMIT 1
             ) AND accepted_at IS NULL AND confirmed_at IS NULL",
            account.0,
            now.to_sqlx() as _,
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn requests(
        &self,
        scope: OrgScope,
        user_id: UserId,
        now: Timestamp,
    ) -> Result<Vec<LinkRequest>, StoreError> {
        let rows = sqlx::query!(
            r#"SELECT id, claimed_by AS "claimed_by!", claimed_name AS "claimed_name!",
                      claimed_at AS "claimed_at!: jiff_sqlx::Timestamp"
               FROM telegram_link_code
               WHERE organization_id = $1 AND user_id = $2 AND accepted_at IS NOT NULL
                 AND confirmed_at IS NULL AND expires_at > $3
               ORDER BY claimed_at"#,
            scope.organization_id().as_uuid(),
            user_id.as_uuid(),
            now.to_sqlx() as _,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(rows
            .into_iter()
            .map(|row| LinkRequest {
                id: row.id,
                telegram_user_id: TelegramUserId(row.claimed_by),
                telegram_name: TelegramName(row.claimed_name),
                claimed_at: row.claimed_at.to_jiff(),
            })
            .collect())
    }

    async fn confirm(
        &self,
        scope: OrgScope,
        user_id: UserId,
        request_id: Uuid,
        now: Timestamp,
    ) -> Result<Confirmed, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let claimed = sqlx::query_scalar!(
            r#"UPDATE telegram_link_code SET confirmed_at = $4
               WHERE id = $3 AND organization_id = $1 AND user_id = $2 AND accepted_at IS NOT NULL
                 AND confirmed_at IS NULL AND expires_at > $4
               RETURNING claimed_by AS "claimed_by!""#,
            scope.organization_id().as_uuid(),
            user_id.as_uuid(),
            request_id,
            now.to_sqlx() as _,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        let Some(account) = claimed else {
            return Ok(Confirmed::NotFound);
        };
        let linked = sqlx::query!(
            "INSERT INTO telegram_identity (telegram_user_id, user_id, linked_at) VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
            account,
            user_id.as_uuid(),
            now.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        if linked.rows_affected() == 0 {
            // The rollback also takes back the confirmation of the code.
            return Ok(Confirmed::AlreadyLinked);
        }
        tx.commit().await.map_err(store_error)?;
        Ok(Confirmed::Linked(TelegramUserId(account)))
    }

    async fn user_of(&self, account: TelegramUserId) -> Result<Option<UserId>, StoreError> {
        let user = sqlx::query_scalar!(
            "SELECT user_id FROM telegram_identity WHERE telegram_user_id = $1",
            account.0,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(user.map(UserId::from_uuid))
    }

    async fn link_of(&self, user_id: UserId) -> Result<Option<TelegramLink>, StoreError> {
        let row = sqlx::query!(
            r#"SELECT telegram_user_id, linked_at AS "linked_at: jiff_sqlx::Timestamp"
               FROM telegram_identity WHERE user_id = $1"#,
            user_id.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(row.map(|row| TelegramLink {
            telegram_user_id: TelegramUserId(row.telegram_user_id),
            linked_at: row.linked_at.to_jiff(),
        }))
    }

    async fn unlink_user(&self, user_id: UserId) -> Result<(), StoreError> {
        sqlx::query!(
            "DELETE FROM telegram_identity WHERE user_id = $1",
            user_id.as_uuid()
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(())
    }

    async fn unlink_account(&self, account: TelegramUserId) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        // The claims go first: a confirmation that runs at the same time holds its code row, so the
        // delete waits for it, and the delete of the link then sees the new link.
        sqlx::query!(
            "DELETE FROM telegram_link_code WHERE claimed_by = $1 AND confirmed_at IS NULL",
            account.0
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        sqlx::query!(
            "DELETE FROM telegram_identity WHERE telegram_user_id = $1",
            account.0
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(())
    }

    async fn record_update(&self, update_id: i64) -> Result<bool, StoreError> {
        let result = sqlx::query!(
            "INSERT INTO telegram_update (update_id, received_at) VALUES ($1, now()) ON CONFLICT DO NOTHING",
            update_id,
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(result.rows_affected() == 1)
    }
}

#[cfg(test)]
mod tests {
    use jiff::SignedDuration;
    use tada_app::caller::MemberCaller;
    use tada_app::caller::OrganizationRole::Member;
    use tada_app::domain::identity::{DisplayName, Email};

    use super::*;
    use crate::testing::TestDatabase;

    /// A member with a user row, because the Telegram tables refer to the user.
    async fn member(
        test: &TestDatabase,
        organization: tada_app::domain::ids::OrganizationId,
    ) -> MemberCaller {
        let name = DisplayName::parse("Anna Muster").unwrap();
        let email = Email::parse(&format!("{}@example.org", Uuid::now_v7())).unwrap();
        let user = test.create_user(&name, &email).await;
        test.add_membership(organization, user, Member).await;
        MemberCaller::new(user, organization, Member)
    }

    fn name(text: &str) -> TelegramName {
        TelegramName(text.to_owned())
    }

    #[tokio::test]
    async fn links_an_account_after_the_claim_and_the_confirmation() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let alice = member(&test, organization).await;
        let db = &test.database;
        let now = Timestamp::now();
        let code = db
            .create_code(
                alice.scope(),
                alice.user_id(),
                now + SignedDuration::from_mins(10),
            )
            .await
            .unwrap();
        assert_eq!(code.len(), 43, "256 bits in Base64");

        let target = db
            .claim(&code, TelegramUserId(42), &name("Alice"), now)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            target.user_name, "Anna Muster",
            "the claim names the tada account"
        );
        assert_eq!(target.organization_name, "testwil");
        assert!(
            db.claim(&code, TelegramUserId(43), &name("Mallory"), now)
                .await
                .unwrap()
                .is_none(),
            "a code works once"
        );
        assert!(
            db.requests(alice.scope(), alice.user_id(), now)
                .await
                .unwrap()
                .is_empty(),
            "the Telegram account did not accept yet"
        );
        assert!(!db.accept(TelegramUserId(43), now).await.unwrap());
        assert!(db.accept(TelegramUserId(42), now).await.unwrap());
        assert!(!db.accept(TelegramUserId(42), now).await.unwrap(), "once");

        let requests = db
            .requests(alice.scope(), alice.user_id(), now)
            .await
            .unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            (requests[0].telegram_user_id, &requests[0].telegram_name),
            (TelegramUserId(42), &name("Alice"))
        );

        let confirmed = db
            .confirm(alice.scope(), alice.user_id(), requests[0].id, now)
            .await
            .unwrap();
        assert_eq!(confirmed, Confirmed::Linked(TelegramUserId(42)));
        assert_eq!(
            db.user_of(TelegramUserId(42)).await.unwrap(),
            Some(alice.user_id())
        );
        assert_eq!(db.user_of(TelegramUserId(43)).await.unwrap(), None);
        assert!(
            db.requests(alice.scope(), alice.user_id(), now)
                .await
                .unwrap()
                .is_empty()
        );
        let again = db
            .confirm(alice.scope(), alice.user_id(), requests[0].id, now)
            .await
            .unwrap();
        assert_eq!(again, Confirmed::NotFound);
    }

    /// Only the member who asked for a code sees and confirms the request of that code.
    #[tokio::test]
    async fn another_member_cannot_see_or_confirm_a_request() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let (alice, bob) = (
            member(&test, organization).await,
            member(&test, organization).await,
        );
        let db = &test.database;
        let now = Timestamp::now();
        let code = db
            .create_code(
                alice.scope(),
                alice.user_id(),
                now + SignedDuration::from_mins(10),
            )
            .await
            .unwrap();
        db.claim(&code, TelegramUserId(42), &name("Alice"), now)
            .await
            .unwrap();
        db.accept(TelegramUserId(42), now).await.unwrap();
        let request = db
            .requests(alice.scope(), alice.user_id(), now)
            .await
            .unwrap()
            .remove(0);

        assert!(
            db.requests(bob.scope(), bob.user_id(), now)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            db.confirm(bob.scope(), bob.user_id(), request.id, now)
                .await
                .unwrap(),
            Confirmed::NotFound
        );
    }

    #[tokio::test]
    async fn an_expired_code_cannot_be_claimed_or_confirmed() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let alice = member(&test, organization).await;
        let db = &test.database;
        let now = Timestamp::now();
        let code = db
            .create_code(
                alice.scope(),
                alice.user_id(),
                now + SignedDuration::from_mins(10),
            )
            .await
            .unwrap();
        let later = now + SignedDuration::from_mins(11);
        assert!(
            db.claim(&code, TelegramUserId(42), &name("Alice"), later)
                .await
                .unwrap()
                .is_none()
        );

        let code = db
            .create_code(
                alice.scope(),
                alice.user_id(),
                now + SignedDuration::from_mins(10),
            )
            .await
            .unwrap();
        db.claim(&code, TelegramUserId(42), &name("Alice"), now)
            .await
            .unwrap();
        db.accept(TelegramUserId(42), now).await.unwrap();
        let request = db
            .requests(alice.scope(), alice.user_id(), now)
            .await
            .unwrap()
            .remove(0);
        assert!(
            db.requests(alice.scope(), alice.user_id(), later)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            db.confirm(alice.scope(), alice.user_id(), request.id, later)
                .await
                .unwrap(),
            Confirmed::NotFound
        );
    }

    #[tokio::test]
    async fn an_account_links_to_one_user_only() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let (alice, bob) = (
            member(&test, organization).await,
            member(&test, organization).await,
        );
        let db = &test.database;
        let now = Timestamp::now();
        for caller in [&alice, &bob] {
            let code = db
                .create_code(
                    caller.scope(),
                    caller.user_id(),
                    now + SignedDuration::from_mins(10),
                )
                .await
                .unwrap();
            db.claim(&code, TelegramUserId(42), &name("Alice"), now)
                .await
                .unwrap();
            db.accept(TelegramUserId(42), now).await.unwrap();
        }
        let first = db
            .requests(alice.scope(), alice.user_id(), now)
            .await
            .unwrap()
            .remove(0);
        let second = db
            .requests(bob.scope(), bob.user_id(), now)
            .await
            .unwrap()
            .remove(0);
        assert_eq!(
            db.confirm(alice.scope(), alice.user_id(), first.id, now)
                .await
                .unwrap(),
            Confirmed::Linked(TelegramUserId(42))
        );
        assert_eq!(
            db.confirm(bob.scope(), bob.user_id(), second.id, now)
                .await
                .unwrap(),
            Confirmed::AlreadyLinked
        );
        assert_eq!(
            db.requests(bob.scope(), bob.user_id(), now)
                .await
                .unwrap()
                .len(),
            1,
            "the failed confirmation rolled back"
        );
    }

    /// Both sides can end a link: the member by user, the Telegram account by account. The account
    /// also rejects its open claims.
    #[tokio::test]
    async fn a_link_ends_by_user_or_by_account() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let (alice, bob) = (
            member(&test, organization).await,
            member(&test, organization).await,
        );
        let db = &test.database;
        let now = Timestamp::now();
        for (caller, account) in [(&alice, 42), (&bob, 43)] {
            let code = db
                .create_code(
                    caller.scope(),
                    caller.user_id(),
                    now + SignedDuration::from_mins(10),
                )
                .await
                .unwrap();
            db.claim(&code, TelegramUserId(account), &name("Alice"), now)
                .await
                .unwrap();
            db.accept(TelegramUserId(account), now).await.unwrap();
            let request = db
                .requests(caller.scope(), caller.user_id(), now)
                .await
                .unwrap()
                .remove(0);
            db.confirm(caller.scope(), caller.user_id(), request.id, now)
                .await
                .unwrap();
        }
        let link = db.link_of(alice.user_id()).await.unwrap().unwrap();
        assert_eq!(link.telegram_user_id, TelegramUserId(42));

        db.unlink_user(alice.user_id()).await.unwrap();
        assert_eq!(db.link_of(alice.user_id()).await.unwrap(), None);
        assert_eq!(
            db.user_of(TelegramUserId(43)).await.unwrap(),
            Some(bob.user_id())
        );

        let code = db
            .create_code(
                alice.scope(),
                alice.user_id(),
                now + SignedDuration::from_mins(10),
            )
            .await
            .unwrap();
        db.claim(&code, TelegramUserId(43), &name("Bob"), now)
            .await
            .unwrap();
        db.unlink_account(TelegramUserId(43)).await.unwrap();
        assert_eq!(db.user_of(TelegramUserId(43)).await.unwrap(), None);
        assert!(
            !db.accept(TelegramUserId(43), now).await.unwrap(),
            "the claim is gone"
        );
    }

    /// /trennen that runs at the same time as a confirmation of a claim of the account ends the
    /// new link too.
    #[tokio::test]
    async fn an_unlink_ends_a_link_that_a_confirmation_makes_at_the_same_time() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let alice = member(&test, organization).await;
        let db = &test.database;
        let now = Timestamp::now();
        let code = db
            .create_code(
                alice.scope(),
                alice.user_id(),
                now + SignedDuration::from_mins(10),
            )
            .await
            .unwrap();
        db.claim(&code, TelegramUserId(42), &name("Alice"), now)
            .await
            .unwrap();
        db.accept(TelegramUserId(42), now).await.unwrap();
        // The confirmation of `confirm`, held open.
        let mut confirmation = db.pool.begin().await.unwrap();
        sqlx::query("UPDATE telegram_link_code SET confirmed_at = now() WHERE claimed_by = 42")
            .execute(&mut *confirmation)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO telegram_identity (telegram_user_id, user_id, linked_at) VALUES (42, $1, now())",
        )
        .bind(alice.user_id().as_uuid())
        .execute(&mut *confirmation)
        .await
        .unwrap();

        let unlink = db.unlink_account(TelegramUserId(42));
        let commit = async {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            confirmation.commit().await.unwrap();
        };
        let (unlinked, ()) = tokio::join!(unlink, commit);
        unlinked.unwrap();
        assert_eq!(db.user_of(TelegramUserId(42)).await.unwrap(), None);
    }

    #[tokio::test]
    async fn records_each_update_once() {
        let test = TestDatabase::start().await;
        assert!(test.database.record_update(7).await.unwrap());
        assert!(!test.database.record_update(7).await.unwrap());
    }
}
