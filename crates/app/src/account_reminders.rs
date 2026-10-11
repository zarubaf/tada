//! The reminder settings of the own account: the opt-in and the time zone (ADR 0076).
//!
//! Both belong to the user, not to a membership: a user in two organizations has one setting for both.
//! A user opts in only with a confirmed Telegram link, because the reminders come through Telegram.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::events::EventTimeZone;

use crate::audit::{AuditAction, AuditEvent};
use crate::caller::MemberCaller;
use crate::clock::Clock;
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::store::StoreError;

/// The reminder settings of a user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderSettings {
    /// The time of the opt-in. `None` means that the user gets no reminders.
    pub opted_in_at: Option<Timestamp>,
    /// The time zone of the quiet hours of the user.
    pub time_zone: EventTimeZone,
    /// True if the user has a confirmed Telegram link. An opt-in needs it.
    pub telegram_linked: bool,
}

/// The change of `ReminderSettingsStore::set`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderChange {
    /// The opt-in time for an opt-in, or `None` for an opt-out. A user who opted in already keeps the first time.
    pub opt_in_at: Option<Timestamp>,
    pub time_zone: EventTimeZone,
}

/// The repository port of the reminder settings.
#[async_trait]
pub trait ReminderSettingsStore: Debug + Send + Sync {
    /// The settings of the caller's account.
    async fn get(&self, caller: &MemberCaller) -> Result<ReminderSettings, StoreError>;

    /// Changes the settings of the caller's account and records `audit`, in one transaction.
    /// Returns `None` and changes nothing if the change opts in and the account has no confirmed Telegram link.
    async fn set(
        &self,
        caller: &MemberCaller,
        change: &ReminderChange,
        audit: &AuditEvent,
    ) -> Result<Option<ReminderSettings>, StoreError>;
}

/// The field code of an opt-in without a confirmed Telegram link.
pub const TELEGRAM_NOT_LINKED: &str = "telegram-not-linked";

#[derive(Debug, thiserror::Error)]
pub enum ReminderSettingsError {
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ReminderSettingsError {
    /// All codes of the reminder settings, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for ReminderSettingsError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Invalid(_) => ProblemCode::ValidationFailed,
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

/// The reminder settings of the caller's own account.
pub async fn get_reminder_settings(
    caller: &MemberCaller,
    store: &dyn ReminderSettingsStore,
) -> Result<ReminderSettings, ReminderSettingsError> {
    Ok(store.get(caller).await?)
}

/// Changes the reminder settings of the caller's own account. An opt-in needs a confirmed Telegram link.
/// The audit event names the account and holds no values.
pub async fn set_reminder_settings(
    caller: &MemberCaller,
    opt_in: bool,
    time_zone: String,
    store: &dyn ReminderSettingsStore,
    clock: &dyn Clock,
) -> Result<ReminderSettings, ReminderSettingsError> {
    let time_zone = EventTimeZone::parse(&time_zone).map_err(|_| {
        ReminderSettingsError::Invalid(vec![FieldError::new("time_zone", "invalid-time-zone")])
    })?;
    let change = ReminderChange {
        opt_in_at: opt_in.then(|| clock.now()),
        time_zone,
    };
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::AccountSetReminders,
        Some(caller.user_id().as_uuid()),
        // The settings belong to the user in each organization, so the event names no organization.
        None,
    );
    store.set(caller, &change, &audit).await?.ok_or_else(|| {
        ReminderSettingsError::Invalid(vec![FieldError::new("opt_in", TELEGRAM_NOT_LINKED)])
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tada_domain::identity::OrganizationRole;
    use tada_domain::ids::{OrganizationId, UserId};
    use uuid::Uuid;

    use super::*;

    #[derive(Debug)]
    struct FixedClock(Timestamp);

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            self.0
        }
    }

    /// The settings of one account in memory.
    #[derive(Debug)]
    struct Memory {
        settings: Mutex<ReminderSettings>,
        writes: Mutex<u32>,
    }

    impl Memory {
        fn new(telegram_linked: bool) -> Self {
            Self {
                settings: Mutex::new(ReminderSettings {
                    opted_in_at: None,
                    time_zone: EventTimeZone::default_zone(),
                    telegram_linked,
                }),
                writes: Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl ReminderSettingsStore for Memory {
        async fn get(&self, _: &MemberCaller) -> Result<ReminderSettings, StoreError> {
            Ok(self.settings.lock().unwrap().clone())
        }

        async fn set(
            &self,
            _: &MemberCaller,
            change: &ReminderChange,
            _: &AuditEvent,
        ) -> Result<Option<ReminderSettings>, StoreError> {
            let mut settings = self.settings.lock().unwrap();
            if change.opt_in_at.is_some() && !settings.telegram_linked {
                return Ok(None);
            }
            settings.opted_in_at = change
                .opt_in_at
                .map(|at| settings.opted_in_at.unwrap_or(at));
            settings.time_zone = change.time_zone.clone();
            *self.writes.lock().unwrap() += 1;
            Ok(Some(settings.clone()))
        }
    }

    fn caller() -> MemberCaller {
        MemberCaller::new(
            UserId::from_uuid(Uuid::from_u128(1)),
            OrganizationId::from_uuid(Uuid::from_u128(2)),
            OrganizationRole::Member,
        )
    }

    fn clock() -> FixedClock {
        FixedClock("2026-10-12T06:30:00Z".parse().unwrap())
    }

    fn codes(error: &ReminderSettingsError) -> Vec<(&str, &str)> {
        error
            .field_errors()
            .iter()
            .map(|error| (error.field.as_ref(), error.code))
            .collect()
    }

    #[tokio::test]
    async fn opt_in_needs_telegram_link() {
        let store = Memory::new(false);
        let error =
            set_reminder_settings(&caller(), true, "Europe/Zurich".into(), &store, &clock())
                .await
                .unwrap_err();
        assert_eq!(codes(&error), [("opt_in", TELEGRAM_NOT_LINKED)]);
        assert_eq!(*store.writes.lock().unwrap(), 0);
        // An opt-out and a new time zone need no link.
        let settings = set_reminder_settings(
            &caller(),
            false,
            "America/New_York".into(),
            &store,
            &clock(),
        )
        .await
        .unwrap();
        assert_eq!(settings.time_zone.as_str(), "America/New_York");

        let linked = Memory::new(true);
        let settings =
            set_reminder_settings(&caller(), true, "Europe/Zurich".into(), &linked, &clock())
                .await
                .unwrap();
        assert_eq!(settings.opted_in_at, Some(clock().now()));
    }

    #[tokio::test]
    async fn invalid_time_zone_is_a_field_error() {
        let store = Memory::new(true);
        let error = set_reminder_settings(&caller(), true, "UTC+2".into(), &store, &clock())
            .await
            .unwrap_err();
        assert_eq!(codes(&error), [("time_zone", "invalid-time-zone")]);
        assert_eq!(error.code(), ProblemCode::ValidationFailed);
        assert_eq!(*store.writes.lock().unwrap(), 0);
    }
}
