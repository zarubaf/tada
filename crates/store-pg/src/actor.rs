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

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the review of proposals reads the authors of records"
    )
)]
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
        .with_request(Channel::Telegram, Some(Uuid::now_v7()))
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
    fn rejects_an_unknown_kind_or_channel() {
        let actor = ServiceCaller::<JobRunner>::new().actor();
        for (name, value) in [("kind", "robot"), ("channel", "mail")] {
            let mut json = to_json(&actor);
            json[name] = value.into();
            assert!(from_json(&json).is_err(), "{name}");
        }
    }
}
