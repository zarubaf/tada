//! The counters of the rate limits (ADR 0008, ADR 0056).
//!
//! The counters are infrastructure rows without an organization (ADR 0039).
//! The key of a counter is an HMAC-SHA-256 of its subject with a secret key, so the table holds no
//! address. A plain hash of an IPv4 address is easy to reverse (ADR 0056).

use std::fmt;

use hmac::{Hmac, KeyInit, Mac};
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use secrecy::{ExposeSecret, SecretString};
use sha2::Sha256;
use sqlx::PgConnection;
use tada_app::rate_limit::{RateDecision, RateLimit, RateSubject, RateWindow, WINDOW};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::store_error;

/// Counts requests in fixed windows. Each process with the same key counts in the same rows.
pub struct PgRateLimiter {
    /// `TADA_RATE_LIMIT_KEY_FILE`.
    key: SecretString,
}

impl fmt::Debug for PgRateLimiter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PgRateLimiter(redacted)")
    }
}

impl PgRateLimiter {
    pub fn new(key: SecretString) -> Self {
        Self { key }
    }

    /// Deletes the counters of the windows that ended (`delete_ended_counters`) and counts one
    /// request against each of `limits` in the window of `now`. The caller commits the transaction
    /// of `conn`.
    pub(crate) async fn hit(
        &self,
        conn: &mut PgConnection,
        limits: &[RateLimit<'_>],
        now: Timestamp,
    ) -> Result<RateDecision, StoreError> {
        delete_ended_counters(&mut *conn, now).await?;
        let window = RateWindow::containing(now);
        let mut decision = RateDecision::Allowed;
        for limit in limits {
            let count = sqlx::query_scalar!(
                "INSERT INTO rate_limit_counter (key, window_start, count) VALUES ($1, $2, 1)
                 ON CONFLICT (key, window_start) DO UPDATE SET count = rate_limit_counter.count + 1
                 RETURNING count",
                &self.key_of(limit.subject)?[..],
                window.start.to_sqlx() as _,
            )
            .fetch_one(&mut *conn)
            .await
            .map_err(store_error)?;
            let count = u32::try_from(count).unwrap_or(u32::MAX);
            decision = decision.and(RateDecision::of(count, limit.limit, now));
        }
        Ok(decision)
    }

    /// The key of a counter. A prefix for each kind of subject keeps the kinds apart.
    fn key_of(&self, subject: RateSubject<'_>) -> Result<[u8; 32], StoreError> {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.key.expose_secret().as_bytes())
            .map_err(|error| StoreError::Internal(Box::new(error)))?;
        match subject {
            RateSubject::Email(email) => {
                mac.update(b"email\0");
                mac.update(email.as_str().as_bytes());
            }
            RateSubject::Ip(network) => {
                mac.update(b"ip\0");
                mac.update(network.address().to_string().as_bytes());
            }
        }
        Ok(mac.finalize().into_bytes().into())
    }
}

impl Database {
    /// Deletes the counters of the windows that ended, as each sign-in request does. The worker calls it
    /// once in each window, so a counter stays at most two windows also without a later sign-in (ADR 0065).
    /// Returns the number of deleted counters.
    pub async fn delete_ended_rate_limit_counters(
        &self,
        now: Timestamp,
    ) -> Result<u64, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        delete_ended_counters(&mut conn, now).await
    }
}

/// Deletes the counters of the windows that ended more than one window before the window of `now`.
///
/// The cleanup keeps the previous window, so a process whose clock is up to one window behind never
/// writes a counter that another process deletes. It skips locked rows, so two processes at a window
/// boundary never wait for each other, and cannot deadlock (ADR 0025, ADR 0065).
async fn delete_ended_counters(conn: &mut PgConnection, now: Timestamp) -> Result<u64, StoreError> {
    let ended = RateWindow::containing(now)
        .start
        .saturating_sub(WINDOW)
        .unwrap_or(Timestamp::MIN);
    sqlx::query!(
        "DELETE FROM rate_limit_counter
             WHERE (key, window_start) IN (
                 SELECT key, window_start FROM rate_limit_counter
                 WHERE window_start < $1
                 FOR UPDATE SKIP LOCKED
             )",
        ended.to_sqlx() as _,
    )
    .execute(conn)
    .await
    .map(|done| done.rows_affected())
    .map_err(store_error)
}

#[cfg(test)]
mod tests {
    use jiff::SignedDuration;
    use tada_app::domain::identity::Email;
    use tada_app::rate_limit::sign_in_limits;

    use super::*;
    use crate::testing::TestDatabase;

    const IP: &str = "203.0.113.7";

    fn now() -> Timestamp {
        "2030-05-18T08:45:00Z".parse().unwrap()
    }

    fn limiter() -> PgRateLimiter {
        PgRateLimiter::new(SecretString::from("test rate limit key"))
    }

    async fn hit(
        test: &TestDatabase,
        limiter: &PgRateLimiter,
        limits: &[RateLimit<'_>],
        now: Timestamp,
    ) -> RateDecision {
        let mut tx = test.database.pool.begin().await.unwrap();
        let decision = limiter.hit(&mut tx, limits, now).await.unwrap();
        tx.commit().await.unwrap();
        decision
    }

    #[tokio::test]
    async fn the_request_after_the_limit_is_limited_until_the_window_ends() {
        let test = TestDatabase::start().await;
        let email = Email::parse("anna@example.org").unwrap();
        let limits = sign_in_limits(&email, IP.parse().unwrap());
        for _ in 0..5 {
            assert_eq!(
                hit(&test, &limiter(), &limits, now()).await,
                RateDecision::Allowed
            );
        }
        // Another limiter with the same key counts in the same rows, as a second process does.
        assert_eq!(
            hit(&test, &limiter(), &limits, now()).await,
            RateDecision::Limited {
                retry_after: SignedDuration::from_mins(15)
            }
        );
        let next_window = now() + SignedDuration::from_mins(15);
        assert_eq!(
            hit(&test, &limiter(), &limits, next_window).await,
            RateDecision::Allowed
        );
    }

    #[tokio::test]
    async fn the_counters_hold_no_address() {
        let test = TestDatabase::start().await;
        let email = Email::parse("anna@example.org").unwrap();
        hit(
            &test,
            &limiter(),
            &sign_in_limits(&email, IP.parse().unwrap()),
            now(),
        )
        .await;
        let rows: i64 = test.scalar("SELECT count(*) FROM rate_limit_counter").await;
        assert_eq!(rows, 2);
        for text in ["anna@example.org", "anna", IP] {
            test.assert_no_plaintext(text).await;
        }
    }

    #[tokio::test]
    async fn another_key_gives_other_counters() {
        let test = TestDatabase::start().await;
        let email = Email::parse("anna@example.org").unwrap();
        let limits = sign_in_limits(&email, IP.parse().unwrap());
        hit(&test, &limiter(), &limits, now()).await;
        let other = PgRateLimiter::new(SecretString::from("another key"));
        hit(&test, &other, &limits, now()).await;
        let rows: i64 = test
            .scalar("SELECT count(*) FROM rate_limit_counter WHERE count = 1")
            .await;
        assert_eq!(rows, 4);
    }

    /// The window starts of all counters, in order.
    async fn windows(test: &TestDatabase) -> Vec<Timestamp> {
        let windows: Vec<jiff_sqlx::Timestamp> =
            sqlx::query_scalar("SELECT window_start FROM rate_limit_counter ORDER BY window_start")
                .fetch_all(&test.database.pool)
                .await
                .unwrap();
        windows.into_iter().map(|window| window.to_jiff()).collect()
    }

    /// A hit deletes the counters of the windows that ended more than one window ago, and keeps the
    /// previous window: a process with a clock up to one window behind still writes there.
    #[tokio::test]
    async fn each_hit_deletes_the_counters_of_windows_that_ended_one_window_ago() {
        let test = TestDatabase::start().await;
        let anna = Email::parse("anna@example.org").unwrap();
        let ben = Email::parse("ben@example.org").unwrap();
        let at = |time: &str| time.parse::<Timestamp>().unwrap();
        let limits = sign_in_limits(&anna, IP.parse().unwrap());
        hit(&test, &limiter(), &limits, at("2030-05-18T07:30:00Z")).await;
        hit(&test, &limiter(), &limits, at("2030-05-18T08:30:00Z")).await;
        hit(
            &test,
            &limiter(),
            &sign_in_limits(&ben, "198.51.100.1".parse().unwrap()),
            at("2030-05-18T09:00:01Z"),
        )
        .await;
        let eight = at("2030-05-18T08:00:00Z");
        let nine = at("2030-05-18T09:00:00Z");
        assert_eq!(windows(&test).await, [eight, eight, nine, nine]);
    }

    /// The worker deletes the ended counters without a sign-in request, so no counter stays longer than
    /// two windows (ADR 0065).
    #[tokio::test]
    async fn the_sweep_deletes_the_counters_of_windows_that_ended_one_window_ago() {
        let test = TestDatabase::start().await;
        let anna = Email::parse("anna@example.org").unwrap();
        let at = |time: &str| time.parse::<Timestamp>().unwrap();
        let limits = sign_in_limits(&anna, IP.parse().unwrap());
        hit(&test, &limiter(), &limits, at("2030-05-18T07:30:00Z")).await;
        hit(&test, &limiter(), &limits, at("2030-05-18T08:30:00Z")).await;

        let deleted = test
            .database
            .delete_ended_rate_limit_counters(at("2030-05-18T09:00:01Z"))
            .await
            .unwrap();
        assert_eq!(deleted, 2);
        let eight = at("2030-05-18T08:00:00Z");
        assert_eq!(windows(&test).await, [eight, eight]);
    }

    /// Two processes at the hour boundary: the one with the later clock does not wait for the
    /// uncommitted counters of the previous window (ADR 0056).
    #[tokio::test]
    async fn a_hit_in_a_new_window_does_not_wait_for_a_hit_in_the_previous_window() {
        let test = TestDatabase::start().await;
        let anna = Email::parse("anna@example.org").unwrap();
        let limits = sign_in_limits(&anna, IP.parse().unwrap());
        // The counters of the previous window exist, so the next hit there locks them.
        hit(
            &test,
            &limiter(),
            &limits,
            "2030-05-18T08:30:00Z".parse().unwrap(),
        )
        .await;
        let mut before = test.database.pool.begin().await.unwrap();
        limiter()
            .hit(
                &mut before,
                &limits,
                "2030-05-18T08:59:59Z".parse().unwrap(),
            )
            .await
            .unwrap();

        let mut after = test.database.pool.begin().await.unwrap();
        let limiter = limiter();
        let later = limiter.hit(&mut after, &limits, "2030-05-18T09:00:01Z".parse().unwrap());
        let decision = tokio::time::timeout(std::time::Duration::from_secs(2), later)
            .await
            .expect("the hit waits for the other transaction")
            .unwrap();
        assert_eq!(decision, RateDecision::Allowed);
        after.commit().await.unwrap();
        before.commit().await.unwrap();
        let counts: Vec<i32> =
            sqlx::query_scalar("SELECT count FROM rate_limit_counter ORDER BY window_start")
                .fetch_all(&test.database.pool)
                .await
                .unwrap();
        assert_eq!(counts, [2, 2, 1, 1]);
    }
}
