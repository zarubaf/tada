//! The cost of the AI work and the monthly cap of an organization (ADRs 0047 and 0077).
//!
//! Each model call records its tokens and its cost from the price table. AI work runs only while the usage of
//! the month is below the cap. An organization without its own cap uses the operator default
//! `TADA_AI_DEFAULT_MONTHLY_CAP_CHF`.

use std::fmt::{self, Debug, Display};
use std::str::FromStr;

use async_trait::async_trait;
use jiff::civil::Date;
use jiff::{Timestamp, ToSpan, tz::TimeZone};
use tada_domain::RecordVersion;
use tada_domain::ids::EventId;
use tada_domain::reminders::PmPolicy;

use crate::access::Principal;
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::model::Usage;
use crate::pm::policy::PmPolicyStore;
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::store::StoreError;

/// An amount in Swiss francs, in millirappen: 1 CHF is 100 000. Text has the form `25`, `25.50` or `0.00125`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Chf(i64);

/// The millirappen of one franc.
const UNITS_PER_CHF: i64 = 100_000;
/// The decimal places of a `Chf`.
const DECIMALS: usize = 5;

impl Chf {
    pub const ZERO: Self = Self(0);

    pub const fn from_millirappen(millirappen: i64) -> Self {
        Self(millirappen)
    }

    pub const fn millirappen(self) -> i64 {
        self.0
    }

    /// The sum, or `None` on an overflow.
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ChfError {
    #[error("expected an amount such as 25 or 0.05, with at most 5 decimal places")]
    Format,
    #[error("the amount is too large")]
    TooLarge,
}

impl FromStr for Chf {
    type Err = ChfError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
        if whole.is_empty()
            || !digits(whole)
            || !digits(fraction)
            || fraction.len() > DECIMALS
            || (text.contains('.') && fraction.is_empty())
        {
            return Err(ChfError::Format);
        }
        let whole: i64 = whole.parse().map_err(|_| ChfError::TooLarge)?;
        let fraction: i64 = format!("{fraction:0<DECIMALS$}")
            .parse()
            .map_err(|_| ChfError::Format)?;
        whole
            .checked_mul(UNITS_PER_CHF)
            .and_then(|units| units.checked_add(fraction))
            .map(Self)
            .ok_or(ChfError::TooLarge)
    }
}

/// At least two decimal places, and more only if they are not zero.
impl Display for Chf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let units = self.0.unsigned_abs();
        let per_chf = UNITS_PER_CHF.unsigned_abs();
        let fraction = format!("{:0DECIMALS$}", units % per_chf);
        let fraction = fraction.trim_end_matches('0');
        write!(
            f,
            "{sign}{}.{fraction:0<2}",
            units / per_chf,
            fraction = fraction
        )
    }
}

/// The price of a model in one period, for one million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Price {
    pub input_per_mtok: Chf,
    pub output_per_mtok: Chf,
}

impl Price {
    /// The cost of one call, rounded up to the next millirappen, so that the usage never stays below the cost.
    pub fn cost(self, usage: Usage) -> Chf {
        let tokens = |count: u32, per_mtok: Chf| i128::from(count) * i128::from(per_mtok.0);
        let total = tokens(usage.input_tokens, self.input_per_mtok)
            + tokens(usage.output_tokens, self.output_per_mtok);
        let millirappen = (total + 999_999) / 1_000_000;
        Chf(i64::try_from(millirappen).unwrap_or(i64::MAX))
    }
}

/// A calendar month in UTC, the period of the cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Month {
    pub start: Timestamp,
    pub end: Timestamp,
}

impl Month {
    pub fn containing(at: Timestamp) -> Self {
        let first = at.to_zoned(TimeZone::UTC).date().first_of_month();
        let start = Self::start_of(first);
        let end = Self::start_of(first.saturating_add(1.month()));
        Self { start, end }
    }

    fn start_of(date: Date) -> Timestamp {
        date.to_zoned(TimeZone::UTC)
            .map(|zoned| zoned.timestamp())
            .unwrap_or(Timestamp::MAX)
    }
}

/// One model call for the usage table. It holds counts and a cost, no text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRecord {
    pub event_id: Option<EventId>,
    pub task: &'static str,
    pub prompt_version: u16,
    pub model_id: String,
    pub usage: Usage,
    pub cost: Chf,
    pub at: Timestamp,
}

/// The repository port of the prices, the usage and the cap.
#[async_trait]
pub trait AiCostStore: Debug + Send + Sync {
    /// The price of `model_id` that is valid on `on`, or `None` if the table has no price for it.
    /// The table is the same for all organizations; the scope names the organization that pays.
    async fn price(
        &self,
        scope: OrgScope,
        model_id: &str,
        on: Date,
    ) -> Result<Option<Price>, StoreError>;

    /// The sum of the costs of the organization in `month`.
    async fn month_usage(&self, scope: OrgScope, month: Month) -> Result<Chf, StoreError>;

    async fn record(&self, scope: OrgScope, usage: &UsageRecord) -> Result<(), StoreError>;

    /// The cap of the organization, or `None` if no owner or admin set one.
    async fn cap(&self, scope: OrgScope) -> Result<Option<(Chf, RecordVersion)>, StoreError>;

    /// Sets the cap and records `audit` in one transaction, if the cap has the version `expected`.
    /// An organization without a row has version 1. Returns `None` on a version conflict.
    async fn set_cap(
        &self,
        scope: OrgScope,
        monthly: Chf,
        expected: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Option<RecordVersion>, StoreError>;
}

/// The settings of the installation that the AI work depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiSetup {
    /// True if the operator set the API key and the date of the provider check (ADR 0045).
    pub configured: bool,
    /// The cap of an organization that set none, from `TADA_AI_DEFAULT_MONTHLY_CAP_CHF`.
    pub default_cap: Chf,
}

/// Whether AI work can run now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiGate {
    Allowed,
    /// The organization did not switch AI on.
    Off,
    /// The operator did not set the model provider.
    NotConfigured,
    /// The usage of the month reached the cap. Manual planning continues (ADR 0047).
    CapReached,
}

/// AI work runs only if the organization switched it on, the provider is configured and the usage of the month
/// is below the cap (ADR 0077).
pub fn ai_gate(policy: &PmPolicy, configured: bool, used: Chf, cap: Chf) -> AiGate {
    if !policy.ai_enabled {
        AiGate::Off
    } else if !configured {
        AiGate::NotConfigured
    } else if used >= cap {
        AiGate::CapReached
    } else {
        AiGate::Allowed
    }
}

/// The AI state of an organization, for the AI-PM page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiStatus {
    pub enabled: bool,
    pub configured: bool,
    pub cap: Chf,
    /// The usage of the current month.
    pub used: Chf,
    /// The version of the cap. `set_ai_cap` needs it.
    pub version: RecordVersion,
}

#[derive(Debug, thiserror::Error)]
pub enum AiCapError {
    /// Only an owner or an admin reads and changes the cap.
    #[error("the caller cannot do this")]
    Forbidden,
    #[error("the cap changed after the caller read it")]
    VersionConflict,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    /// The session of the caller is older than `session::RECENT_SIGN_IN`. The cap controls spending,
    /// so a stolen session must not raise it.
    #[error("the action needs a recent sign-in")]
    RecentSignInRequired,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl AiCapError {
    /// All codes of the cap commands and queries, for the API contract (ADR 0037).
    /// A change of the cap also answers `recent-sign-in-required`.
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::RecordVersionConflict,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for AiCapError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Forbidden => ProblemCode::Forbidden,
            Self::VersionConflict => ProblemCode::RecordVersionConflict,
            Self::Invalid(_) => ProblemCode::ValidationFailed,
            Self::RecentSignInRequired => ProblemCode::RecentSignInRequired,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }

    fn field_errors(&self) -> &[FieldError] {
        match self {
            Self::Invalid(errors) => errors,
            _ => &[],
        }
    }
}

/// The cap of the organization, or the default with version 1.
async fn cap_of(
    store: &dyn AiCostStore,
    scope: OrgScope,
    setup: AiSetup,
) -> Result<(Chf, RecordVersion), StoreError> {
    Ok(store
        .cap(scope)
        .await?
        .unwrap_or((setup.default_cap, RecordVersion::FIRST)))
}

/// The AI state of the caller's organization. An owner or an admin can read it.
/// The policy is the one authority on whether AI is on.
pub async fn get_ai_status(
    caller: &MemberCaller,
    store: &dyn AiCostStore,
    policies: &dyn PmPolicyStore,
    setup: AiSetup,
    clock: &dyn Clock,
) -> Result<AiStatus, AiCapError> {
    if !caller.organization_role().is_owner_or_admin() {
        return Err(AiCapError::Forbidden);
    }
    let scope = caller.scope();
    let (cap, version) = cap_of(store, scope, setup).await?;
    Ok(AiStatus {
        enabled: policies.get(scope).await?.0.ai_enabled,
        configured: setup.configured,
        cap,
        used: store
            .month_usage(scope, Month::containing(clock.now()))
            .await?,
        version,
    })
}

/// Sets the monthly cap of the caller's organization. An owner or an admin can do this (ADR 0047).
/// The cap controls spending, so the session must be younger than `session::RECENT_SIGN_IN`, as for the
/// retention periods. The audit event holds no amount.
pub async fn set_ai_cap(
    caller: &MemberCaller,
    monthly_chf: &str,
    expected_version: RecordVersion,
    store: &dyn AiCostStore,
    policies: &dyn PmPolicyStore,
    setup: AiSetup,
    clock: &dyn Clock,
) -> Result<AiStatus, AiCapError> {
    if !caller.organization_role().is_owner_or_admin() {
        return Err(AiCapError::Forbidden);
    }
    if !caller.signed_in_recently(clock.now()) {
        return Err(AiCapError::RecentSignInRequired);
    }
    let monthly = match monthly_chf.parse::<Chf>() {
        Ok(monthly) if monthly > Chf::ZERO => monthly,
        Ok(_) => {
            return Err(AiCapError::Invalid(vec![FieldError::new(
                "cap_chf",
                "out-of-range",
            )]));
        }
        Err(ChfError::TooLarge) => {
            return Err(AiCapError::Invalid(vec![FieldError::new(
                "cap_chf",
                "out-of-range",
            )]));
        }
        Err(ChfError::Format) => {
            return Err(AiCapError::Invalid(vec![FieldError::new(
                "cap_chf", "pattern",
            )]));
        }
    };
    let scope = caller.scope();
    // The cap has no ID of its own: the record ID is its organization.
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::OrganizationSetAiCap,
        Some(scope.organization_id().as_uuid()),
        Some(scope),
    );
    store
        .set_cap(scope, monthly, expected_version, &audit)
        .await?
        .ok_or(AiCapError::VersionConflict)?;
    get_ai_status(caller, store, policies, setup, clock).await
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tada_domain::identity::OrganizationRole;
    use tada_domain::ids::{OrganizationId, UserId};
    use uuid::Uuid;

    use jiff::SignedDuration;

    use super::*;

    fn chf(text: &str) -> Chf {
        text.parse().unwrap()
    }

    #[test]
    fn an_amount_reads_and_shows_as_decimal_francs() {
        assert_eq!(chf("25"), Chf::from_millirappen(2_500_000));
        assert_eq!(chf("0.05"), Chf::from_millirappen(5_000));
        assert_eq!(chf("0.00001"), Chf::from_millirappen(1));
        assert_eq!(chf("25").to_string(), "25.00");
        assert_eq!(chf("0.05").to_string(), "0.05");
        assert_eq!(chf("0.00125").to_string(), "0.00125");
        assert_eq!(chf("1.5").to_string(), "1.50");
        for bad in ["", ".5", "5.", "-1", "1.000001", "1,5", "abc", "1e3", " 1"] {
            assert_eq!(bad.parse::<Chf>(), Err(ChfError::Format), "{bad:?}");
        }
        assert_eq!(
            "99999999999999999999".parse::<Chf>(),
            Err(ChfError::TooLarge)
        );
    }

    #[test]
    fn cost_from_price_table() {
        let price = Price {
            input_per_mtok: chf("0.80"),
            output_per_mtok: chf("4"),
        };
        // 2000 × 0.80 / 1e6 + 500 × 4 / 1e6 = 0.0016 + 0.002 = 0.0036 CHF.
        let usage = Usage {
            input_tokens: 2_000,
            output_tokens: 500,
        };
        assert_eq!(price.cost(usage), chf("0.0036"));
        // A part of a millirappen rounds up.
        let one = Usage {
            input_tokens: 1,
            output_tokens: 0,
        };
        assert_eq!(price.cost(one), Chf::from_millirappen(1));
        assert_eq!(price.cost(Usage::default()), Chf::ZERO);
    }

    #[test]
    fn a_month_is_the_calendar_month_in_utc() {
        let month = Month::containing("2026-10-11T08:30:00Z".parse().unwrap());
        assert_eq!(month.start, "2026-10-01T00:00:00Z".parse().unwrap());
        assert_eq!(month.end, "2026-11-01T00:00:00Z".parse().unwrap());
        let december = Month::containing("2026-12-31T23:59:59Z".parse().unwrap());
        assert_eq!(december.end, "2027-01-01T00:00:00Z".parse().unwrap());
    }

    #[test]
    fn gate_cases() {
        let on = PmPolicy {
            ai_enabled: true,
            ..PmPolicy::default()
        };
        let off = PmPolicy::default();
        assert_eq!(ai_gate(&off, true, Chf::ZERO, chf("25")), AiGate::Off);
        assert_eq!(
            ai_gate(&on, false, Chf::ZERO, chf("25")),
            AiGate::NotConfigured
        );
        assert_eq!(ai_gate(&on, true, chf("25"), chf("25")), AiGate::CapReached);
        assert_eq!(ai_gate(&on, true, chf("30"), chf("25")), AiGate::CapReached);
        assert_eq!(
            ai_gate(&on, true, chf("24.99999"), chf("25")),
            AiGate::Allowed
        );
    }

    #[derive(Debug, Default)]
    struct FakeStore {
        cap: Mutex<Option<(Chf, RecordVersion)>>,
        audit: Mutex<Vec<AuditEvent>>,
    }

    #[async_trait]
    impl AiCostStore for FakeStore {
        async fn price(&self, _: OrgScope, _: &str, _: Date) -> Result<Option<Price>, StoreError> {
            Ok(None)
        }

        async fn month_usage(&self, _: OrgScope, _: Month) -> Result<Chf, StoreError> {
            Ok(chf("1.25"))
        }

        async fn record(&self, _: OrgScope, _: &UsageRecord) -> Result<(), StoreError> {
            Ok(())
        }

        async fn cap(&self, _: OrgScope) -> Result<Option<(Chf, RecordVersion)>, StoreError> {
            Ok(*self.cap.lock().unwrap())
        }

        async fn set_cap(
            &self,
            _: OrgScope,
            monthly: Chf,
            expected: RecordVersion,
            audit: &AuditEvent,
        ) -> Result<Option<RecordVersion>, StoreError> {
            let mut cap = self.cap.lock().unwrap();
            let current = cap.map_or(RecordVersion::FIRST, |(_, version)| version);
            if current != expected {
                return Ok(None);
            }
            let next = RecordVersion::new(current.get() + 1).unwrap();
            *cap = Some((monthly, next));
            self.audit.lock().unwrap().push(audit.clone());
            Ok(Some(next))
        }
    }

    /// A policy store with AI switched on.
    #[derive(Debug)]
    struct AiOn;

    #[async_trait]
    impl PmPolicyStore for AiOn {
        async fn get(&self, _: OrgScope) -> Result<(PmPolicy, RecordVersion), StoreError> {
            let policy = PmPolicy {
                ai_enabled: true,
                ..PmPolicy::default()
            };
            Ok((policy, RecordVersion::FIRST))
        }

        async fn set(
            &self,
            _: OrgScope,
            _: &PmPolicy,
            _: RecordVersion,
            _: &AuditEvent,
        ) -> Result<Option<RecordVersion>, StoreError> {
            unreachable!()
        }
    }

    #[derive(Debug)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            "2026-10-11T08:30:00Z".parse().unwrap()
        }
    }

    const SETUP: AiSetup = AiSetup {
        configured: true,
        default_cap: Chf::from_millirappen(2_500_000),
    };

    /// A caller who signed in just now.
    fn caller(role: OrganizationRole) -> MemberCaller {
        MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            OrganizationId::from_uuid(Uuid::now_v7()),
            role,
        )
        .with_sign_in(FixedClock.now())
    }

    #[tokio::test]
    async fn an_old_session_cannot_set_the_cap() {
        let store = FakeStore::default();
        let signed_in =
            FixedClock.now() - crate::session::RECENT_SIGN_IN - SignedDuration::from_secs(1);
        let owner = MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            OrganizationId::from_uuid(Uuid::now_v7()),
            OrganizationRole::Owner,
        )
        .with_sign_in(signed_in);
        let result = set_ai_cap(
            &owner,
            "100",
            RecordVersion::FIRST,
            &store,
            &AiOn,
            SETUP,
            &FixedClock,
        )
        .await;
        assert!(
            matches!(result, Err(AiCapError::RecentSignInRequired)),
            "{result:?}"
        );
        assert_eq!(*store.cap.lock().unwrap(), None);
        // Reading the cap needs no recent sign-in.
        assert!(
            get_ai_status(&owner, &store, &AiOn, SETUP, &FixedClock)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn member_cannot_set_cap() {
        let store = FakeStore::default();
        let member = caller(OrganizationRole::Member);
        let result = set_ai_cap(
            &member,
            "100",
            RecordVersion::FIRST,
            &store,
            &AiOn,
            SETUP,
            &FixedClock,
        )
        .await;
        assert!(matches!(result, Err(AiCapError::Forbidden)), "{result:?}");
        assert!(matches!(
            get_ai_status(&member, &store, &AiOn, SETUP, &FixedClock).await,
            Err(AiCapError::Forbidden)
        ));
        assert_eq!(*store.cap.lock().unwrap(), None);
        assert!(store.audit.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_admin_sets_the_cap_over_the_default() {
        let store = FakeStore::default();
        let admin = caller(OrganizationRole::Admin);
        let status = get_ai_status(&admin, &store, &AiOn, SETUP, &FixedClock)
            .await
            .unwrap();
        assert_eq!(
            status,
            AiStatus {
                enabled: true,
                configured: true,
                cap: chf("25"),
                used: chf("1.25"),
                version: RecordVersion::FIRST,
            }
        );

        let status = set_ai_cap(
            &admin,
            "40.50",
            RecordVersion::FIRST,
            &store,
            &AiOn,
            SETUP,
            &FixedClock,
        )
        .await
        .unwrap();
        assert_eq!(status.cap, chf("40.50"));
        assert_eq!(status.version.get(), 2);
        let action = store.audit.lock().unwrap()[0].action();
        assert_eq!(action.as_str(), "organization.set_ai_cap");

        let stale = set_ai_cap(
            &admin,
            "10",
            RecordVersion::FIRST,
            &store,
            &AiOn,
            SETUP,
            &FixedClock,
        )
        .await;
        assert!(matches!(stale, Err(AiCapError::VersionConflict)));
    }

    #[tokio::test]
    async fn a_cap_must_be_a_positive_amount() {
        let store = FakeStore::default();
        let owner = caller(OrganizationRole::Owner);
        for (value, code) in [("0", "out-of-range"), ("-5", "pattern"), ("ten", "pattern")] {
            let error = set_ai_cap(
                &owner,
                value,
                RecordVersion::FIRST,
                &store,
                &AiOn,
                SETUP,
                &FixedClock,
            )
            .await
            .unwrap_err();
            assert_eq!(
                error.field_errors(),
                [FieldError::new("cap_chf", code)],
                "{value}"
            );
        }
    }
}
