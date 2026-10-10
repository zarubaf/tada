//! The one codec for record authors (ADR 0039): an `Actor` as `jsonb`.
//!
//! Each record-author column uses it, for example `source_version.author_actor` and `fact_version.accepted_by`.
//! Only `audit_event` keeps separate actor columns.

use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use tada_app::caller::{Actor, ActorKind, Channel};
use tada_app::store::StoreError;

use crate::error::InvalidRow;

/// The stored form of an actor.
#[derive(Serialize, Deserialize)]
struct ActorRecord {
    kind: String,
    id: Uuid,
    principal: Option<Uuid>,
    channel: String,
    request_id: Option<Uuid>,
}

pub(crate) fn to_json(actor: &Actor) -> serde_json::Value {
    let record = ActorRecord {
        kind: actor.kind().as_str().to_owned(),
        id: actor.id(),
        principal: actor.principal(),
        channel: actor.channel().as_str().to_owned(),
        request_id: actor.request_id(),
    };
    serde_json::to_value(record).expect("an actor record is valid JSON")
}

/// The stored form of an actor after a legal redaction: its kind and ID only (ADR 0072).
/// The trigger of `source_version` allows exactly this shape as the tombstone of `author_actor`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RedactedRecord {
    kind: String,
    id: Uuid,
}

/// A stored record author: a whole actor, or the kind and ID that a legal redaction keeps (ADR 0072).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "no read shows the author of a source version yet")
)]
pub(crate) enum StoredActor {
    Actor(Actor),
    Redacted { kind: ActorKind, id: Uuid },
}

/// The tombstone of a stored actor: the same kind and ID, without the principal, the channel and the request.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the redaction store writes it; the SQL of the trigger is the same rule"
    )
)]
pub(crate) fn redacted_json(value: &serde_json::Value) -> Result<serde_json::Value, StoreError> {
    let actor = from_json(value)?;
    let record = RedactedRecord {
        kind: actor.kind().as_str().to_owned(),
        id: actor.id(),
    };
    Ok(serde_json::to_value(record).expect("a redacted actor record is valid JSON"))
}

/// A stored actor in either form: whole, or redacted to its kind and ID.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "no read shows the author of a source version yet")
)]
pub(crate) fn stored_from_json(value: &serde_json::Value) -> Result<StoredActor, StoreError> {
    if let Ok(record) = RedactedRecord::deserialize(value) {
        let kind = ActorKind::parse(&record.kind).ok_or(InvalidRow("actor"))?;
        return Ok(StoredActor::Redacted {
            kind,
            id: record.id,
        });
    }
    from_json(value).map(StoredActor::Actor)
}

pub(crate) fn from_json(value: &serde_json::Value) -> Result<Actor, StoreError> {
    let invalid = || InvalidRow("actor");
    let record = ActorRecord::deserialize(value).map_err(|_| invalid())?;
    Ok(Actor::restore(
        ActorKind::parse(&record.kind).ok_or_else(invalid)?,
        record.id,
        record.principal,
        Channel::parse(&record.channel).ok_or_else(invalid)?,
        record.request_id,
    ))
}

#[cfg(test)]
mod tests {
    use tada_app::caller::{
        ActorKind, Channel, JobRunner, MemberCaller, OrganizationRole, ServiceCaller,
    };
    use tada_app::domain::ids::{OrganizationId, UserId};

    use super::*;

    #[test]
    fn restores_each_kind_of_actor() {
        let member = MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            OrganizationId::from_uuid(Uuid::now_v7()),
            OrganizationRole::Member,
        )
        .with_channel(Channel::Telegram)
        .with_request(Some(Uuid::now_v7()))
        .actor();
        let service = ServiceCaller::<JobRunner>::new().actor();
        let ai = Actor::restore(
            ActorKind::Ai,
            Uuid::now_v7(),
            Some(Uuid::now_v7()),
            Channel::ApiToken,
            Some(Uuid::now_v7()),
        );
        for actor in [member, service, ai] {
            assert_eq!(from_json(&to_json(&actor)).unwrap(), actor);
        }
    }

    #[test]
    fn reads_the_redacted_form_with_the_kind_and_id_only() {
        let actor = MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            OrganizationId::from_uuid(Uuid::now_v7()),
            OrganizationRole::Member,
        )
        .with_request(Some(Uuid::now_v7()))
        .actor();
        let json = to_json(&actor);
        assert_eq!(stored_from_json(&json).unwrap(), StoredActor::Actor(actor));
        let redacted = redacted_json(&json).unwrap();
        assert_eq!(
            redacted,
            serde_json::json!({"kind": "member", "id": actor.id()})
        );
        assert_eq!(
            stored_from_json(&redacted).unwrap(),
            StoredActor::Redacted {
                kind: ActorKind::Member,
                id: actor.id()
            }
        );
        // The whole codec refuses the tombstone, so a read that needs the channel cannot miss a redaction.
        assert!(from_json(&redacted).is_err());
        assert!(stored_from_json(&serde_json::json!({"kind": "robot", "id": actor.id()})).is_err());
    }

    #[test]
    fn rejects_an_unknown_kind_or_channel() {
        let actor = ServiceCaller::<JobRunner>::new().actor();
        for (name, value) in [("kind", "robot"), ("channel", "mail")] {
            let mut json = to_json(&actor);
            json[name] = value.into();
            assert!(from_json(&json).is_err(), "{name}");
        }
    }
}
