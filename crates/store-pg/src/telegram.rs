//! The `TelegramLinks` adapter (ADR 0011). Codes are 256 random bits; the table holds their SHA-256 hash (ADR 0008).

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use secrecy::ExposeSecret;
use sqlx::types::Uuid;
use tada_app::caller::OrgScope;
use tada_app::domain::ids::UserId;
use tada_app::store::StoreError;
use tada_app::telegram::{Confirmed, LinkRequest, TelegramLinks, TelegramName, TelegramUserId};

use crate::Database;
use crate::error::store_error;
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
    ) -> Result<bool, StoreError> {
        let result = sqlx::query!(
            "UPDATE telegram_link_code
             SET claimed_by = $2, claimed_name = $3, claimed_at = $4
             WHERE code_hash = $1 AND claimed_by IS NULL AND expires_at > $4",
            hash_token(code),
            account.0,
            name.0,
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
               WHERE organization_id = $1 AND user_id = $2 AND claimed_by IS NOT NULL
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
               WHERE id = $3 AND organization_id = $1 AND user_id = $2 AND claimed_by IS NOT NULL
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

    use super::*;
    use crate::testing::TestDatabase;

    fn member(test_org: tada_app::domain::ids::OrganizationId) -> MemberCaller {
        MemberCaller::new(UserId::from_uuid(Uuid::now_v7()), test_org, Member)
    }

    fn name(text: &str) -> TelegramName {
        TelegramName(text.to_owned())
    }

    #[tokio::test]
    async fn links_an_account_after_the_claim_and_the_confirmation() {
        let test = TestDatabase::start().await;
        let alice = member(test.create_organization("testwil").await);
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

        assert!(
            db.claim(&code, TelegramUserId(42), &name("Alice"), now)
                .await
                .unwrap()
        );
        assert!(
            !db.claim(&code, TelegramUserId(43), &name("Mallory"), now)
                .await
                .unwrap(),
            "a code works once"
        );

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
        let (alice, bob) = (member(organization), member(organization));
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
        let alice = member(test.create_organization("testwil").await);
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
            !db.claim(&code, TelegramUserId(42), &name("Alice"), later)
                .await
                .unwrap()
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
        let (alice, bob) = (member(organization), member(organization));
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

    #[tokio::test]
    async fn records_each_update_once() {
        let test = TestDatabase::start().await;
        assert!(test.database.record_update(7).await.unwrap());
        assert!(!test.database.record_update(7).await.unwrap());
    }
}
