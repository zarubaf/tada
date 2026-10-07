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
use tada_app::rate_limit::{RateDecision, RateLimit, RateSubject, RateWindow};
use tada_app::store::StoreError;

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

    /// Deletes the counters of the windows that ended and counts one request against each of
    /// `limits` in the window of `now`. The caller commits the transaction of `conn`.
    pub(crate) async fn hit(
        &self,
        conn: &mut PgConnection,
        limits: &[RateLimit<'_>],
        now: Timestamp,
    ) -> Result<RateDecision, StoreError> {
        let window = RateWindow::containing(now);
        sqlx::query!(
            "DELETE FROM rate_limit_counter WHERE window_start < $1",
            window.start.to_sqlx() as _,
        )
        .execute(&mut *conn)
        .await
        .map_err(store_error)?;
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
            RateSubject::Ip(address) => {
                // An IPv4 client of an IPv6 socket counts as the same IPv4 address.
                mac.update(b"ip\0");
                mac.update(address.to_canonical().to_string().as_bytes());
            }
        }
        Ok(mac.finalize().into_bytes().into())
    }
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

    #[tokio::test]
    async fn each_hit_deletes_the_counters_of_ended_windows() {
        let test = TestDatabase::start().await;
        let anna = Email::parse("anna@example.org").unwrap();
        let ben = Email::parse("ben@example.org").unwrap();
        let earlier = now() - SignedDuration::from_hours(1);
        hit(
            &test,
            &limiter(),
            &sign_in_limits(&anna, IP.parse().unwrap()),
            earlier,
        )
        .await;
        hit(
            &test,
            &limiter(),
            &sign_in_limits(&ben, "198.51.100.1".parse().unwrap()),
            now(),
        )
        .await;
        let windows: Vec<jiff_sqlx::Timestamp> =
            sqlx::query_scalar("SELECT window_start FROM rate_limit_counter")
                .fetch_all(&test.database.pool)
                .await
                .unwrap();
        let start = RateWindow::containing(now()).start;
        assert_eq!(windows.len(), 2);
        assert!(windows.iter().all(|window| window.to_jiff() == start));
    }
}
