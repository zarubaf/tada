//! The `CreateEvent` command and the `ListEvents` query.

use std::fmt::Debug;

use async_trait::async_trait;
use tada_domain::RecordVersion;
use tada_domain::events::{
    Event, EventKey, EventKeyError, EventName, EventNameError, EventTimeZone,
};
use tada_domain::ids::{self, EventId};
use uuid::Uuid;

use crate::caller::{MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::paging::{Page, PageLimit};
use crate::problem::{FieldError, ProblemCode};
use crate::store::StoreError;

/// The repository port for events. Each method stays inside `scope`.
#[async_trait]
pub trait EventStore: Debug + Send + Sync {
    /// Inserts a new event. The store checks that its ID and its key are free.
    async fn insert(&self, scope: OrgScope, event: &Event) -> Result<Inserted, StoreError>;

    async fn get(&self, scope: OrgScope, id: EventId) -> Result<Option<Event>, StoreError>;

    /// Returns at most `limit` events in the order of their keys, after `after` if it is given.
    async fn list(
        &self,
        scope: OrgScope,
        after: Option<&EventCursor>,
        limit: u32,
    ) -> Result<Vec<Event>, StoreError>;
}

/// The result of `EventStore::insert`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inserted {
    Inserted,
    /// A record with this ID exists, in this organization or in another one.
    IdTaken,
    /// An event of this organization has this key.
    KeyTaken,
}

/// The input of `CreateEvent`, as the caller gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent {
    /// The ID of the new event. A client that sends it can retry safely (ADR 0038).
    pub id: Option<Uuid>,
    pub key: String,
    pub name: String,
    /// The default is `Europe/Zurich`.
    pub time_zone: Option<String>,
}

/// The result of a successful `CreateEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Created {
    New(Event),
    /// A retry: the event with this ID and the same content exists, and nothing changed.
    Existing(Event),
}

#[derive(Debug, thiserror::Error)]
pub enum CreateEventError {
    #[error("only owners and admins create events")]
    Forbidden,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl CreateEventError {
    /// All codes that this command can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];

    pub fn code(&self) -> ProblemCode {
        match self {
            Self::Forbidden => ProblemCode::Forbidden,
            Self::Invalid(_) => ProblemCode::ValidationFailed,
            Self::Store(error) => store_code(error),
        }
    }
}

/// Creates an event in the caller's organization.
///
/// Only owners and admins create events. They can act as event manager in each event (ADR 0052).
pub async fn create_event(
    caller: &MemberCaller,
    input: NewEvent,
    store: &dyn EventStore,
    clock: &dyn Clock,
) -> Result<Created, CreateEventError> {
    if !caller.is_owner_or_admin() {
        return Err(CreateEventError::Forbidden);
    }
    let scope = caller.scope();
    let event = validate(scope, input, clock)?;

    match store.insert(scope, &event).await? {
        Inserted::Inserted => Ok(Created::New(event)),
        Inserted::KeyTaken => Err(invalid("key", "taken")),
        Inserted::IdTaken => match store.get(scope, event.id).await? {
            Some(existing) if same_content(&existing, &event) => Ok(Created::Existing(existing)),
            _ => Err(invalid("id", "taken")),
        },
    }
}

fn validate(
    scope: OrgScope,
    input: NewEvent,
    clock: &dyn Clock,
) -> Result<Event, CreateEventError> {
    let mut errors = Vec::new();
    let id = match input.id {
        Some(id) if !ids::is_record_id(id) => {
            errors.push(FieldError {
                field: "id",
                code: "not-uuid-v7",
            });
            None
        }
        Some(id) => Some(EventId::from_uuid(id)),
        None => Some(EventId::from_uuid(Uuid::now_v7())),
    };
    let key = EventKey::parse(&input.key)
        .map_err(|error| {
            errors.push(FieldError {
                field: "key",
                code: match error {
                    EventKeyError::Length => "length",
                    EventKeyError::Characters => "characters",
                },
            });
        })
        .ok();
    let name = EventName::parse(&input.name)
        .map_err(|error| {
            errors.push(FieldError {
                field: "name",
                code: match error {
                    EventNameError::Empty => "empty",
                    EventNameError::TooLong => "too-long",
                    EventNameError::ControlCharacter => "control-character",
                },
            });
        })
        .ok();
    let time_zone = match input.time_zone {
        None => Some(EventTimeZone::default_zone()),
        Some(name) => EventTimeZone::parse(&name)
            .map_err(|_| {
                errors.push(FieldError {
                    field: "time_zone",
                    code: "unknown",
                });
            })
            .ok(),
    };

    match (id, key, name, time_zone) {
        (Some(id), Some(key), Some(name), Some(time_zone)) if errors.is_empty() => Ok(Event {
            id,
            organization_id: scope.organization_id(),
            key,
            name,
            time_zone,
            version: RecordVersion::FIRST,
            created_at: clock.now(),
        }),
        _ => Err(CreateEventError::Invalid(errors)),
    }
}

fn same_content(existing: &Event, new: &Event) -> bool {
    existing.key == new.key && existing.name == new.name && existing.time_zone == new.time_zone
}

fn invalid(field: &'static str, code: &'static str) -> CreateEventError {
    CreateEventError::Invalid(vec![FieldError { field, code }])
}

/// The position after the last event of a page: the sort key and the ID (ADR 0044).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventCursor {
    pub key: EventKey,
    pub id: EventId,
}

#[derive(Debug, thiserror::Error)]
pub enum ListEventsError {
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ListEventsError {
    /// All codes that this query can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[ProblemCode::Unavailable, ProblemCode::Internal];

    pub fn code(&self) -> ProblemCode {
        match self {
            Self::Store(error) => store_code(error),
        }
    }
}

/// Lists the events that the caller can see, in the order of their keys.
///
/// Owners and admins see all events of the organization. Other members see only the events in which
/// they have an event role (ADR 0052). Event roles do not exist yet, so they see none.
pub async fn list_events(
    caller: &MemberCaller,
    after: Option<EventCursor>,
    limit: PageLimit,
    store: &dyn EventStore,
) -> Result<Page<Event, EventCursor>, ListEventsError> {
    if !caller.is_owner_or_admin() {
        return Ok(Page {
            items: Vec::new(),
            next: None,
        });
    }
    // One more than the limit shows if a next page exists.
    let mut items = store
        .list(caller.scope(), after.as_ref(), limit.get() + 1)
        .await?;
    let more = items.len() > limit.get() as usize;
    items.truncate(limit.get() as usize);
    let next = more
        .then(|| items.last())
        .flatten()
        .map(|last| EventCursor {
            key: last.key.clone(),
            id: last.id,
        });
    Ok(Page { items, next })
}

fn store_code(error: &StoreError) -> ProblemCode {
    match error {
        StoreError::Unavailable(_) => ProblemCode::Unavailable,
        StoreError::Internal(_) => ProblemCode::Internal,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use jiff::Timestamp;
    use tada_domain::ids::{OrganizationId, UserId};

    use super::*;
    use crate::caller::OrganizationRole;

    #[derive(Debug, Default)]
    struct MemoryStore(Mutex<Vec<Event>>);

    #[async_trait]
    impl EventStore for MemoryStore {
        async fn insert(&self, scope: OrgScope, event: &Event) -> Result<Inserted, StoreError> {
            assert_eq!(scope.organization_id(), event.organization_id);
            let mut events = self.0.lock().unwrap();
            if events.iter().any(|known| known.id == event.id) {
                return Ok(Inserted::IdTaken);
            }
            if events.iter().any(|known| {
                known.organization_id == event.organization_id && known.key == event.key
            }) {
                return Ok(Inserted::KeyTaken);
            }
            events.push(event.clone());
            Ok(Inserted::Inserted)
        }

        async fn get(&self, scope: OrgScope, id: EventId) -> Result<Option<Event>, StoreError> {
            let events = self.0.lock().unwrap();
            Ok(events
                .iter()
                .find(|event| event.id == id && event.organization_id == scope.organization_id())
                .cloned())
        }

        async fn list(
            &self,
            scope: OrgScope,
            after: Option<&EventCursor>,
            limit: u32,
        ) -> Result<Vec<Event>, StoreError> {
            let mut events: Vec<Event> = self
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|event| event.organization_id == scope.organization_id())
                .filter(|event| {
                    after.is_none_or(|after| (&event.key, event.id) > (&after.key, after.id))
                })
                .cloned()
                .collect();
            events.sort_by(|a, b| (&a.key, a.id).cmp(&(&b.key, b.id)));
            events.truncate(limit as usize);
            Ok(events)
        }
    }

    #[derive(Debug)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            "2030-05-18T08:00:00Z".parse().unwrap()
        }
    }

    fn caller(organization: u128, role: OrganizationRole) -> MemberCaller {
        MemberCaller::new(
            UserId::from_uuid(Uuid::from_u128(1)),
            OrganizationId::from_uuid(Uuid::from_u128(organization)),
            role,
        )
    }

    fn owner() -> MemberCaller {
        caller(100, OrganizationRole::Owner)
    }

    fn new_event(key: &str) -> NewEvent {
        NewEvent {
            id: None,
            key: key.to_owned(),
            name: "Open Day Testwil".to_owned(),
            time_zone: None,
        }
    }

    #[tokio::test]
    async fn creates_an_event_with_version_1_and_the_default_time_zone() {
        let store = MemoryStore::default();
        let Created::New(event) = create_event(&owner(), new_event("TEST30"), &store, &FixedClock)
            .await
            .unwrap()
        else {
            panic!("not a new event");
        };
        assert_eq!(event.key.as_str(), "TEST30");
        assert_eq!(event.time_zone.as_str(), "Europe/Zurich");
        assert_eq!(event.version, RecordVersion::FIRST);
        assert_eq!(event.organization_id, owner().scope().organization_id());
        assert!(ids::is_record_id(event.id.as_uuid()));
    }

    #[tokio::test]
    async fn reports_all_invalid_fields_together() {
        let store = MemoryStore::default();
        let input = NewEvent {
            id: Some(Uuid::from_u128(5)),
            key: "x".to_owned(),
            name: " ".to_owned(),
            time_zone: Some("Mars/Olympus".to_owned()),
        };
        let Err(CreateEventError::Invalid(errors)) =
            create_event(&owner(), input, &store, &FixedClock).await
        else {
            panic!("not invalid");
        };
        let fields: Vec<_> = errors
            .iter()
            .map(|error| (error.field, error.code))
            .collect();
        assert_eq!(
            fields,
            [
                ("id", "not-uuid-v7"),
                ("key", "length"),
                ("name", "empty"),
                ("time_zone", "unknown")
            ]
        );
    }

    #[tokio::test]
    async fn a_retry_with_the_same_id_and_content_returns_the_existing_event() {
        let store = MemoryStore::default();
        let input = NewEvent {
            id: Some(Uuid::now_v7()),
            ..new_event("TEST30")
        };
        let Created::New(first) = create_event(&owner(), input.clone(), &store, &FixedClock)
            .await
            .unwrap()
        else {
            panic!("not a new event");
        };
        let retry = create_event(&owner(), input.clone(), &store, &FixedClock)
            .await
            .unwrap();
        assert_eq!(retry, Created::Existing(first));

        let changed = NewEvent {
            name: "Another name".to_owned(),
            ..input
        };
        let Err(CreateEventError::Invalid(errors)) =
            create_event(&owner(), changed, &store, &FixedClock).await
        else {
            panic!("not invalid");
        };
        assert_eq!(
            errors,
            [FieldError {
                field: "id",
                code: "taken"
            }]
        );
    }

    #[tokio::test]
    async fn rejects_a_key_that_the_organization_uses() {
        let store = MemoryStore::default();
        create_event(&owner(), new_event("TEST30"), &store, &FixedClock)
            .await
            .unwrap();
        let Err(CreateEventError::Invalid(errors)) =
            create_event(&owner(), new_event("TEST30"), &store, &FixedClock).await
        else {
            panic!("not invalid");
        };
        assert_eq!(
            errors,
            [FieldError {
                field: "key",
                code: "taken"
            }]
        );

        let other = caller(200, OrganizationRole::Admin);
        assert!(
            create_event(&other, new_event("TEST30"), &store, &FixedClock)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn members_cannot_create_events() {
        let store = MemoryStore::default();
        let member = caller(100, OrganizationRole::Member);
        let result = create_event(&member, new_event("TEST30"), &store, &FixedClock).await;
        assert!(matches!(result, Err(CreateEventError::Forbidden)));
    }

    #[tokio::test]
    async fn lists_events_in_pages_in_the_order_of_their_keys() {
        let store = MemoryStore::default();
        for key in ["CC", "AA", "BB"] {
            create_event(&owner(), new_event(key), &store, &FixedClock)
                .await
                .unwrap();
        }
        let limit = PageLimit::new(2).unwrap();
        let first = list_events(&owner(), None, limit, &store).await.unwrap();
        let keys: Vec<_> = first.items.iter().map(|event| event.key.as_str()).collect();
        assert_eq!(keys, ["AA", "BB"]);

        let second = list_events(&owner(), first.next, limit, &store)
            .await
            .unwrap();
        let keys: Vec<_> = second
            .items
            .iter()
            .map(|event| event.key.as_str())
            .collect();
        assert_eq!(keys, ["CC"]);
        assert_eq!(second.next, None);
    }

    #[tokio::test]
    async fn lists_no_events_of_another_organization() {
        let store = MemoryStore::default();
        create_event(&owner(), new_event("TEST30"), &store, &FixedClock)
            .await
            .unwrap();
        let other = caller(200, OrganizationRole::Owner);
        let page = list_events(&other, None, PageLimit::DEFAULT, &store)
            .await
            .unwrap();
        assert!(page.items.is_empty());
    }
}
