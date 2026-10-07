//! The first owner of an organization (ADR 0036): the `bootstrap` service identity creates an
//! organization and the invitation of its first owner. The worker sends the mail.

use async_trait::async_trait;
use jiff::{SignedDuration, Timestamp};
use secrecy::SecretString;
use tada_domain::identity::{DisplayName, Email, OrganizationName, OrganizationSlug};
use tada_domain::ids::{InvitationId, OrganizationId};

use crate::caller::{Bootstrap, ServiceCaller};
use crate::clock::Clock;
use crate::outbound::OutboundStore;
use crate::public_url::PublicUrl;
use crate::store::StoreError;

/// A link that `tada bootstrap --print-link` prints expires after 30 minutes (ADR 0036).
pub const PRINTED_LINK_LIFETIME: SignedDuration = SignedDuration::from_mins(30);

/// The input of `bootstrap`, as the operator gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapInput {
    pub slug: OrganizationSlug,
    pub name: OrganizationName,
    pub owner_email: Email,
    /// The default is the local part of the email address. The owner can change it later.
    pub owner_display_name: Option<DisplayName>,
}

/// The invitation of the first owner, as the store writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerInvitation {
    pub slug: OrganizationSlug,
    pub name: OrganizationName,
    pub email: Email,
    pub display_name: DisplayName,
    pub now: Timestamp,
}

/// The result of `bootstrap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapOutcome {
    /// The organization has an owner. Nothing changed.
    OwnerExists,
    /// The organization has no owner. A new owner invitation and its mail wait for the worker.
    InvitationQueued {
        invitation_id: InvitationId,
        organization_id: OrganizationId,
    },
}

/// The repository port of `bootstrap`.
#[async_trait]
pub trait BootstrapStore: Send + Sync {
    /// Does all steps of ADR 0036 in one transaction:
    /// it creates the organization if the slug is free, and stops if the organization has an owner.
    /// Otherwise it revokes the pending owner invitations, inserts the new one, queues its mail
    /// and records the audit events.
    ///
    /// The organization comes from an infrastructure query by slug (ADR 0039). The store continues
    /// with the organization ID of its own query, and records the audit events with
    /// `AuditEvent::by_bootstrap`.
    async fn invite_first_owner(
        &self,
        caller: &ServiceCaller<Bootstrap>,
        invitation: &OwnerInvitation,
    ) -> Result<BootstrapOutcome, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    /// The invitation was accepted or revoked before the link was printed.
    #[error("the invitation is no longer pending")]
    NotPending,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Creates the organization of `input` if its slug is free, and invites its first owner
/// if it has none.
pub async fn bootstrap(
    caller: &ServiceCaller<Bootstrap>,
    input: BootstrapInput,
    store: &dyn BootstrapStore,
    clock: &dyn Clock,
) -> Result<BootstrapOutcome, BootstrapError> {
    let display_name = input
        .owner_display_name
        .unwrap_or_else(|| DisplayName::from_email(&input.owner_email));
    let invitation = OwnerInvitation {
        slug: input.slug,
        name: input.name,
        email: input.owner_email,
        display_name,
        now: clock.now(),
    };
    Ok(store.invite_first_owner(caller, &invitation).await?)
}

/// Creates a second token of a queued invitation and returns its link, for the terminal of the
/// operator only. The link expires after `PRINTED_LINK_LIFETIME`.
pub async fn printed_link(
    caller: &ServiceCaller<Bootstrap>,
    organization_id: OrganizationId,
    invitation_id: InvitationId,
    store: &dyn OutboundStore,
    clock: &dyn Clock,
    public_url: &PublicUrl,
) -> Result<SecretString, BootstrapError> {
    let token = store
        .issue_invitation_token(
            caller.scope(organization_id),
            invitation_id,
            clock.now() + PRINTED_LINK_LIFETIME,
        )
        .await?
        .ok_or(BootstrapError::NotPending)?;
    Ok(public_url.invitation_link(&token))
}
