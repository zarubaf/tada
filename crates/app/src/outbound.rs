//! Outbound intents and the job that sends them (ADRs 0007, 0008 and 0042).
//!
//! No command sends mail. A command stores an outbound intent and a send job in its own transaction.
//! The worker then creates the token, sends the message and records the outcome.

use async_trait::async_trait;
use jiff::Timestamp;
use secrecy::SecretString;
use tada_domain::identity::Email;
use tada_domain::ids::{InvitationId, OrganizationId, UserId};
use uuid::Uuid;

use crate::caller::OrgScope;
use crate::store::StoreError;

/// The job kind that sends one outbound intent.
/// Payload version 1 is `{"intent_id": "<uuid>"}`: no token and no address (ADR 0042).
pub const SEND_JOB: &str = "send-outbound";

/// Why tada sends a message. This is the only description of an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// A sign-in link for a user. It belongs to the user, not to an organization.
    MagicLink { user_id: UserId },
    /// An invitation to an organization.
    Invitation {
        organization_id: OrganizationId,
        invitation_id: InvitationId,
    },
}

impl Purpose {
    /// The organization of the intent. A magic link has none.
    pub fn organization_id(self) -> Option<OrganizationId> {
        match self {
            Self::MagicLink { .. } => None,
            Self::Invitation {
                organization_id, ..
            } => Some(organization_id),
        }
    }
}

/// An intent that waits for its send, with all that the message needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingIntent {
    pub purpose: Purpose,
    pub to: Email,
    pub locale: String,
    /// The name of the organization of an invitation. A magic link has none.
    pub organization_name: Option<String>,
    /// The left part of the `Message-ID` header. It stays the same for each attempt.
    pub message_id: String,
}

/// The result of a send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Sent,
    /// The mail server rejected the message for good.
    Failed,
    /// The server can have taken the message. Nobody sends it again blindly (ADR 0042).
    Unknown,
}

impl Outcome {
    /// The name that the database uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

/// The repository port of the send job.
#[async_trait]
pub trait OutboundStore: Send + Sync {
    /// The intent, if it still waits for its send.
    /// An infrastructure query without a scope (ADR 0039): it returns the organization of the intent
    /// in its purpose.
    async fn load_pending(&self, intent_id: Uuid) -> Result<Option<PendingIntent>, StoreError>;

    /// Stores the hash of a new magic-link token and returns the token.
    async fn issue_magic_link(
        &self,
        user_id: UserId,
        expires_at: Timestamp,
    ) -> Result<SecretString, StoreError>;

    /// Stores the hash of a new invitation token and returns the token.
    async fn issue_invitation_token(
        &self,
        scope: OrgScope,
        invitation_id: InvitationId,
        expires_at: Timestamp,
    ) -> Result<SecretString, StoreError>;

    /// Records the outcome of a pending intent.
    async fn finish(&self, intent_id: Uuid, outcome: Outcome) -> Result<(), StoreError>;
}
