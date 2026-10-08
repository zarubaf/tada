//! The author of a record on the wire: who, for which member and through which channel (ADR 0039).
//! The route modules share these types.

use serde::Serialize;
use tada_app::caller::{Actor, ActorKind as AppActorKind, Channel as AppChannel};
use utoipa::ToSchema;
use uuid::Uuid;

/// Who created a record, for which member and through which channel (ADR 0039).
#[derive(Debug, Serialize, ToSchema)]
pub struct Author {
    pub kind: ActorKind,
    /// The user ID of a member or an AI client, or the ID of a service identity.
    pub id: Uuid,
    /// The member for whom the author acts, if the author is not that member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal_id: Option<Uuid>,
    pub channel: Channel,
}

/// The kind of an author. The list of kinds is open.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ActorKind {
    Member,
    Service,
    Ai,
}

/// The way an author reached tada. The list of channels is open.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Channel {
    Web,
    Telegram,
    Job,
    ApiToken,
    Cli,
}

impl From<Actor> for Author {
    fn from(actor: Actor) -> Self {
        Self {
            kind: match actor.kind() {
                AppActorKind::Member => ActorKind::Member,
                AppActorKind::Service => ActorKind::Service,
                AppActorKind::Ai => ActorKind::Ai,
            },
            id: actor.id(),
            principal_id: actor.principal(),
            channel: match actor.channel() {
                AppChannel::Web => Channel::Web,
                AppChannel::Telegram => Channel::Telegram,
                AppChannel::Job => Channel::Job,
                AppChannel::ApiToken => Channel::ApiToken,
                AppChannel::Cli => Channel::Cli,
            },
        }
    }
}
