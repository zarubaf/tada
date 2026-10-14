//! Risks (ADR 0081): what can go wrong in an event, rated by likelihood and impact, with their direct commands.
//! They follow the rules of actions: a contributor or a manager creates one, and its owner, the lead of its
//! workstream or an event manager changes it.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::RecordVersion;
use tada_domain::ids::{EventId, LocalIdKind, RiskId, UserId, WorkstreamId};
use tada_domain::records::{Level, RiskRating, RiskStatus, RiskTitle};
use tada_domain::redaction::Redactable;
use tada_domain::work::RecordDescription;
use uuid::Uuid;

use super::checks::{
    Assignment, WorkRecord, changed, check_assignment, parse_description, parse_optional_text,
    require_any_change, require_change, require_create, show, show_created, show_one,
    writable_access,
};
use super::{WorkError, WorkFilter, WorkPorts, WorkQuery};
use crate::access::{self, Principal};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{MemberCaller, OrgScope};
use crate::paging::Page;
use crate::records::text_error_code;
use crate::records::{Changed, Checker, Created, RecordRef, Shown, audit, page_by, record_id};
use crate::store::StoreError;

/// A risk of an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskView {
    pub id: RiskId,
    /// The number in `RSK-001`, local to the event (ADR 0038).
    pub local_number: u64,
    pub event_id: EventId,
    pub fields: RiskFields,
    pub version: RecordVersion,
}

impl RiskView {
    /// The readable ID, for example `RSK-001`.
    pub fn local_id(&self) -> String {
        LocalIdKind::Risk.readable_id(self.local_number)
    }
}

/// The values of a risk that a change replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskFields {
    /// A legal redaction removes the title, the description and the mitigation (ADR 0072).
    pub title: Redactable<RiskTitle>,
    pub description: Option<RecordDescription>,
    pub rating: RiskRating,
    pub mitigation: Option<RecordDescription>,
    /// A member of the event with the contributor or manager role.
    pub owner: UserId,
    pub workstream_id: Option<WorkstreamId>,
    pub status: RiskStatus,
}

/// A new risk, checked, before the store gives it its number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRiskRecord {
    pub id: RiskId,
    pub event_id: EventId,
    pub fields: RiskFields,
}

/// The position after the last risk of a page: the key that the risk had when the page was made (its rating and
/// its number), not a pointer to the risk. A page starts after this key in the order of the list, so a rating
/// change of the risk of the cursor, or of any other risk, never makes a list skip a risk. A risk that changes its
/// rating between two pages can appear twice or move, but another risk's change never hides it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiskCursor {
    pub rating: RiskRating,
    pub number: u64,
}

impl RiskCursor {
    /// The cursor after `risk`, with the rating it has now.
    pub fn after(risk: &RiskView) -> Self {
        Self {
            rating: risk.fields.rating,
            number: risk.local_number,
        }
    }

    /// The key of the order of the list: the worst rating first, then the number.
    fn key(self) -> impl Ord {
        (self.rating.sort_key(), self.number)
    }

    /// True if `risk` comes after this cursor in the order of the list (a page after the cursor shows it).
    pub fn comes_before(self, risk: &RiskView) -> bool {
        Self::after(risk).key() > self.key()
    }
}

/// The repository port for risks. Each method stays inside `scope`.
/// A create or a change records its audit event in the same transaction.
#[async_trait]
pub trait RiskStore: Debug + Send + Sync {
    /// Inserts a risk with the version 1 and gives it the next number of its event (ADR 0038).
    async fn create_risk(
        &self,
        scope: OrgScope,
        risk: &NewRiskRecord,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<RiskView>, StoreError>;

    /// Replaces the values of a risk if its version is `expected`, and counts the version up.
    #[expect(
        clippy::too_many_arguments,
        reason = "a write port takes the scope, the record, the values, the version, the time and the audit event"
    )]
    async fn change_risk(
        &self,
        scope: OrgScope,
        event: EventId,
        id: RiskId,
        fields: &RiskFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<RiskView>, StoreError>;

    async fn risk(
        &self,
        scope: OrgScope,
        event: EventId,
        id: RiskId,
    ) -> Result<Option<RiskView>, StoreError>;

    /// At most `filter.limit` risks of the event, the worst first (`RiskRating::sort_key`), then by number.
    /// A page starts after the key of `filter.after` (`RiskCursor`).
    async fn risks(
        &self,
        scope: OrgScope,
        event: EventId,
        filter: &WorkFilter<RiskStatus, RiskCursor>,
    ) -> Result<Vec<RiskView>, StoreError>;
}

impl WorkRecord for RiskView {
    fn record_ref(&self) -> RecordRef {
        RecordRef::Risk(self.id)
    }

    fn owner(&self) -> UserId {
        self.fields.owner
    }

    fn workstream(&self) -> Option<WorkstreamId> {
        self.fields.workstream_id
    }
}

impl Shown<RiskView> {
    /// The statuses that the caller can set with a change now: none without the right to change the risk.
    pub fn next_statuses(&self) -> Vec<RiskStatus> {
        if self.can_change {
            self.record.fields.status.next_statuses()
        } else {
            Vec::new()
        }
    }
}

/// The input of `create_risk`, as the caller gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRisk {
    /// The ID of the new risk, a UUIDv7 (ADR 0038). Without it, the command chooses one. An ID that a record holds is `taken`, also on a retry.
    pub id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub likelihood: Level,
    pub impact: Level,
    pub mitigation: Option<String>,
    pub owner: UserId,
    pub workstream: Option<WorkstreamId>,
}

/// The input of `change_risk`. A field that is `None` stays as it is; `Some(None)` clears it.
/// The likelihood and the impact change on their own: the one that is `None` stays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskChange {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub likelihood: Option<Level>,
    pub impact: Option<Level>,
    pub mitigation: Option<Option<String>>,
    pub owner: Option<UserId>,
    pub workstream: Option<Option<WorkstreamId>>,
    pub status: Option<RiskStatus>,
    pub expected_version: RecordVersion,
}

fn parse_title(check: &mut Checker, input: &str) -> Option<RiskTitle> {
    check.parse("title", input, RiskTitle::parse, text_error_code)
}

/// Creates a risk. A contributor or a manager of the event can do it.
pub async fn create_risk(
    caller: &MemberCaller,
    event: EventId,
    input: NewRisk,
    ports: WorkPorts<'_>,
) -> Result<Shown<RiskView>, WorkError> {
    let access = require_create(caller, event, ports.identity).await?;
    let scope = caller.scope();
    let id =
        RiskId::from_uuid(record_id(input.id).map_err(|error| WorkError::Invalid(vec![error]))?);
    let mut check = Checker::default();
    let title = parse_title(&mut check, &input.title);
    let description = parse_description(&mut check, input.description.as_deref());
    let mitigation = parse_optional_text(&mut check, "mitigation", input.mitigation.as_deref());
    let assigned = Assignment::new(input.owner, input.workstream);
    check_assignment(&mut check, scope, event, None, assigned, ports).await?;
    let (title, description, mitigation) = check
        .finish(
            title
                .zip(description)
                .zip(mitigation)
                .map(|((t, d), m)| (t, d, m)),
        )
        .map_err(WorkError::Invalid)?;
    let risk = NewRiskRecord {
        id,
        event_id: event,
        fields: RiskFields {
            title: Redactable::Present(title),
            description,
            rating: RiskRating {
                likelihood: input.likelihood,
                impact: input.impact,
            },
            mitigation,
            owner: input.owner,
            workstream_id: input.workstream,
            status: RiskStatus::Open,
        },
    };
    let audit = audit(caller, AuditAction::RiskCreate, id.as_uuid());
    let result = ports
        .risks
        .create_risk(scope, &risk, ports.clock.now(), &audit)
        .await?;
    show_created(caller, access, event, result, ports).await
}

/// Changes a risk: its owner, the lead of its workstream or an event manager can do it.
pub async fn change_risk(
    caller: &MemberCaller,
    event: EventId,
    id: RiskId,
    change: RiskChange,
    ports: WorkPorts<'_>,
) -> Result<Shown<RiskView>, WorkError> {
    let access = writable_access(caller, event, ports.identity).await?;
    let scope = caller.scope();
    let current = ports
        .risks
        .risk(scope, event, id)
        .await?
        .ok_or(WorkError::NotFound)?;
    let old = &current.fields;
    // A redacted risk has no content to change (ADR 0072).
    let Redactable::Present(old_title) = &old.title else {
        return Err(WorkError::InvalidTransition);
    };
    require_change(
        caller,
        access,
        event,
        old.owner,
        old.workstream_id,
        ports.workstreams,
    )
    .await?;
    let RiskChange {
        title,
        description,
        likelihood,
        impact,
        mitigation,
        owner,
        workstream,
        status,
        expected_version,
    } = change;
    let any = title.is_some()
        || description.is_some()
        || likelihood.is_some()
        || impact.is_some()
        || mitigation.is_some()
        || owner.is_some()
        || workstream.is_some()
        || status.is_some();
    require_any_change(any, current.version, expected_version)?;
    let mut check = Checker::default();
    let title = match &title {
        Some(text) => parse_title(&mut check, text),
        None => Some(old_title.clone()),
    };
    let description = match &description {
        Some(text) => parse_description(&mut check, text.as_deref()),
        None => Some(old.description.clone()),
    };
    let mitigation = match &mitigation {
        Some(text) => parse_optional_text(&mut check, "mitigation", text.as_deref()),
        None => Some(old.mitigation.clone()),
    };
    let owner = owner.unwrap_or(old.owner);
    let workstream_id = workstream.unwrap_or(old.workstream_id);
    let assigned = Assignment::new(owner, workstream_id);
    let before = Assignment::new(old.owner, old.workstream_id);
    check_assignment(&mut check, scope, event, Some(before), assigned, ports).await?;
    let (title, description, mitigation) = check
        .finish(
            title
                .zip(description)
                .zip(mitigation)
                .map(|((t, d), m)| (t, d, m)),
        )
        .map_err(WorkError::Invalid)?;
    let status = match status {
        Some(next) => old.status.change_to(next)?,
        None => old.status,
    };
    let fields = RiskFields {
        title: Redactable::Present(title),
        description,
        rating: RiskRating {
            likelihood: likelihood.unwrap_or(old.rating.likelihood),
            impact: impact.unwrap_or(old.rating.impact),
        },
        mitigation,
        owner,
        workstream_id,
        status,
    };
    let audit = audit(caller, AuditAction::RiskChange, id.as_uuid());
    let risk = changed(
        ports
            .risks
            .change_risk(
                scope,
                event,
                id,
                &fields,
                expected_version,
                ports.clock.now(),
                &audit,
            )
            .await?,
    )?;
    Ok(show_one(caller, access, event, risk, ports).await?)
}

/// One risk of the event, with its evidence. Each reader of the event sees it.
pub async fn get_risk(
    caller: &impl Principal,
    event: EventId,
    id: RiskId,
    ports: WorkPorts<'_>,
) -> Result<Shown<RiskView>, WorkError> {
    let access = access::event_access(caller, event, ports.identity).await?;
    let risk = ports
        .risks
        .risk(caller.scope(), event, id)
        .await?
        .ok_or(WorkError::NotFound)?;
    Ok(show_one(caller, access, event, risk, ports).await?)
}

/// The risks of the event that match `query`, the worst first and then in the order of their numbers, with their
/// evidence. The cursor of a page is the key of its last risk (`RiskCursor`): a risk that changes its rating
/// between two pages can appear twice or move, but the change of a risk never makes the list skip another risk.
pub async fn list_risks(
    caller: &impl Principal,
    event: EventId,
    query: WorkQuery<RiskStatus, RiskCursor>,
    ports: WorkPorts<'_>,
) -> Result<Page<Shown<RiskView>, RiskCursor>, WorkError> {
    let access = access::event_access(caller, event, ports.identity).await?;
    let items = ports
        .risks
        .risks(caller.scope(), event, &query.filter())
        .await?;
    let Page { items, next } = page_by(items, query.limit, RiskCursor::after);
    let items = show(caller, access, event, items, ports).await?;
    Ok(Page { items, next })
}
