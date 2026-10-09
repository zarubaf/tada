//! Callers (ADR 0039): the typed values that commands and queries take for authorization.

use std::marker::PhantomData;

pub use tada_domain::identity::OrganizationRole;
use tada_domain::ids::{ApiTokenId, OrganizationId, UserId};
use uuid::Uuid;

use crate::tokens::TokenScope;

/// The ID of one request (ADR 0035). The HTTP server gives it to each driving adapter in the request
/// extensions, also to an adapter that runs its handlers in other tasks, for example the MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestId(Uuid);

impl RequestId {
    pub fn new(id: Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

/// The way a request reached tada (ADR 0039).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Web,
    Telegram,
    Job,
    ApiToken,
    Cli,
}

impl Channel {
    /// The name that audit records and the database use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Telegram => "telegram",
            Self::Job => "job",
            Self::ApiToken => "api-token",
            Self::Cli => "cli",
        }
    }

    /// The channel of a name of `as_str`.
    pub fn parse(name: &str) -> Option<Self> {
        [
            Self::Web,
            Self::Telegram,
            Self::Job,
            Self::ApiToken,
            Self::Cli,
        ]
        .into_iter()
        .find(|channel| channel.as_str() == name)
    }
}

/// The kind of party that acts (ADR 0039).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    Member,
    Service,
    Ai,
}

impl ActorKind {
    /// The name that audit records and the database use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::Service => "service",
            Self::Ai => "ai",
        }
    }

    /// The kind of a name of `as_str`.
    pub fn parse(name: &str) -> Option<Self> {
        [Self::Member, Self::Service, Self::Ai]
            .into_iter()
            .find(|kind| kind.as_str() == name)
    }
}

/// Who did something, for which member and through which channel (ADR 0039).
/// A caller gives its actor, so a record author or an audit event cannot name someone else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Actor {
    kind: ActorKind,
    id: Uuid,
    principal: Option<Uuid>,
    channel: Channel,
    request_id: Option<Uuid>,
}

impl Actor {
    /// A member who acts for themselves.
    pub(crate) fn member(user_id: UserId, channel: Channel, request_id: Option<Uuid>) -> Self {
        Self {
            kind: ActorKind::Member,
            id: user_id.as_uuid(),
            principal: None,
            channel,
            request_id,
        }
    }

    /// For store adapters only: restores the author of a stored record.
    /// Other code gets an actor from a caller.
    #[doc(hidden)]
    pub fn restore(
        kind: ActorKind,
        id: Uuid,
        principal: Option<Uuid>,
        channel: Channel,
        request_id: Option<Uuid>,
    ) -> Self {
        Self {
            kind,
            id,
            principal,
            channel,
            request_id,
        }
    }

    pub fn kind(&self) -> ActorKind {
        self.kind
    }

    /// The user ID of a member or an AI client, or the fixed ID of a service identity.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// The member for whom the actor acts, if the actor is not that member.
    pub fn principal(&self) -> Option<Uuid> {
        self.principal
    }

    pub fn channel(&self) -> Channel {
        self.channel
    }

    pub fn request_id(&self) -> Option<Uuid> {
        self.request_id
    }
}

/// A signed-in member. Only an `Authenticator` creates it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberCaller {
    user_id: UserId,
    organization_id: OrganizationId,
    role: OrganizationRole,
    channel: Channel,
    request_id: Option<Uuid>,
}

impl MemberCaller {
    /// For tests of other crates only. Production code gets a caller from an authenticator.
    #[cfg(any(test, feature = "testing"))]
    pub fn new(user_id: UserId, organization_id: OrganizationId, role: OrganizationRole) -> Self {
        Self::create(user_id, organization_id, role)
    }

    /// For the authenticators of this crate (ADR 0062). Other code gets a caller from an authenticator.
    pub(crate) fn create(
        user_id: UserId,
        organization_id: OrganizationId,
        role: OrganizationRole,
    ) -> Self {
        Self::build(user_id, organization_id, role, Channel::Web)
    }

    /// The one place that builds a member caller from its parts. Only this module can call it.
    fn build(
        user_id: UserId,
        organization_id: OrganizationId,
        role: OrganizationRole,
        channel: Channel,
    ) -> Self {
        Self {
            user_id,
            organization_id,
            role,
            channel,
            request_id: None,
        }
    }

    /// Names the request of this caller, for the actor of audit records.
    /// The channel stays the one of the authenticator, so an adapter cannot change it (ADR 0039).
    #[must_use]
    pub fn with_request(self, request_id: Option<Uuid>) -> Self {
        Self { request_id, ..self }
    }

    /// For tests of other crates only: a member of another channel, for example `telegram`.
    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn with_channel(self, channel: Channel) -> Self {
        Self { channel, ..self }
    }

    pub fn channel(&self) -> Channel {
        self.channel
    }

    pub fn actor(&self) -> Actor {
        Actor::member(self.user_id, self.channel, self.request_id)
    }

    pub fn user_id(&self) -> UserId {
        self.user_id
    }

    /// The organization that all reads and writes of this caller stay in.
    pub fn scope(&self) -> OrgScope {
        OrgScope(self.organization_id)
    }
}

/// Seals the caller traits `Principal` and `MayPropose` (ADR 0039).
/// Only the callers of this module implement `Sealed`, so no other crate can implement the traits.
pub(crate) mod sealed {
    pub trait Sealed {}
}

impl sealed::Sealed for MemberCaller {}

impl sealed::Sealed for AiCaller {}

impl crate::access::Principal for MemberCaller {
    fn user_id(&self) -> UserId {
        self.user_id
    }

    fn scope(&self) -> OrgScope {
        MemberCaller::scope(self)
    }

    fn organization_role(&self) -> OrganizationRole {
        self.role
    }
}

/// An AI client that acts for a member through a personal API token (ADR 0039).
/// Only an `Authenticator` creates it.
///
/// It never gives its member back: code that has an `AiCaller` cannot call a command that takes
/// a `MemberCaller`, for example the review of proposals. It has the rights of its member,
/// limited by the scope of its token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiCaller {
    principal: MemberCaller,
    token_id: ApiTokenId,
    scope: TokenScope,
}

impl AiCaller {
    /// For tests of other crates only. Production code gets a caller from an authenticator.
    #[cfg(any(test, feature = "testing"))]
    pub fn new(principal: MemberCaller, token_id: ApiTokenId, scope: TokenScope) -> Self {
        let MemberCaller {
            user_id,
            organization_id,
            role,
            ..
        } = principal;
        Self::create(user_id, organization_id, role, token_id, scope)
    }

    /// For the token authenticator of this crate (ADR 0062).
    /// It builds the member here, so the token module never holds a `MemberCaller`
    /// that it could give to a command (`scripts/check_no_dev_auth.py`).
    pub(crate) fn create(
        user_id: UserId,
        organization_id: OrganizationId,
        role: OrganizationRole,
        token_id: ApiTokenId,
        scope: TokenScope,
    ) -> Self {
        Self {
            principal: MemberCaller {
                user_id,
                organization_id,
                role,
                channel: Channel::ApiToken,
                request_id: None,
            },
            token_id,
            scope,
        }
    }

    /// Names the request of this caller, for the actor of audit records.
    /// The channel is always `api-token`.
    #[must_use]
    pub fn with_request(self, request_id: Option<Uuid>) -> Self {
        Self {
            principal: self.principal.with_request(request_id),
            ..self
        }
    }

    /// The AI client, for the member that it acts for, through the channel `api-token`.
    pub fn actor(&self) -> Actor {
        Actor {
            kind: ActorKind::Ai,
            id: self.token_id.as_uuid(),
            principal: Some(self.principal.user_id.as_uuid()),
            channel: Channel::ApiToken,
            request_id: self.principal.request_id,
        }
    }

    /// The token that the client showed.
    pub fn token_id(&self) -> ApiTokenId {
        self.token_id
    }

    /// The scope of the token that the client showed.
    pub fn token_scope(&self) -> TokenScope {
        self.scope
    }
}

impl crate::access::Principal for AiCaller {
    fn user_id(&self) -> UserId {
        self.principal.user_id
    }

    fn scope(&self) -> OrgScope {
        self.principal.scope()
    }

    fn organization_role(&self) -> OrganizationRole {
        self.principal.role
    }
}

/// The organization boundary of a repository call (ADR 0006). Only a caller can give one,
/// so a repository method that takes it cannot run without a scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrgScope(OrganizationId);

impl OrgScope {
    /// The scope of the membership check of a credential: a session or an API token (ADR 0056).
    /// The check runs before a caller exists, because it decides if one exists.
    pub(crate) fn for_credential(organization_id: OrganizationId) -> Self {
        Self(organization_id)
    }

    pub fn organization_id(self) -> OrganizationId {
        self.0
    }
}

/// A service identity of tada (ADR 0039). Each identity is its own type, so the set of commands
/// that it can call is fixed in code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceCaller<S>(PhantomData<S>);

impl<S> ServiceCaller<S> {
    /// For the process role of the identity only, for example `tada telegram`.
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<S: ServiceIdentity> ServiceCaller<S> {
    pub fn actor(&self) -> Actor {
        Actor {
            kind: ActorKind::Service,
            id: S::ID,
            principal: None,
            channel: S::CHANNEL,
            request_id: None,
        }
    }
}

impl<S> Default for ServiceCaller<S> {
    fn default() -> Self {
        Self::new()
    }
}

/// The name, the fixed ID and the channel of a service identity.
pub trait ServiceIdentity {
    const NAME: &'static str;
    const ID: Uuid;
    /// The channel of each action of the identity (ADR 0039).
    const CHANNEL: Channel;
}

/// The service identity `telegram-gateway`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelegramGateway;

impl ServiceIdentity for TelegramGateway {
    const NAME: &'static str = "telegram-gateway";
    const ID: Uuid = Uuid::from_u128(0x0192_0000_0000_7000_8000_0000_0000_0001);
    const CHANNEL: Channel = Channel::Telegram;
}

impl ServiceCaller<TelegramGateway> {
    /// The linked member for whom the gateway acts, through the channel `telegram` (ADR 0039).
    /// Only `telegram::member_for` calls it, after it read the membership of the user.
    pub(crate) fn member(
        &self,
        user_id: UserId,
        organization_id: OrganizationId,
        role: OrganizationRole,
    ) -> MemberCaller {
        MemberCaller::build(user_id, organization_id, role, TelegramGateway::CHANNEL)
    }
}

/// The service identity `job-runner`: the worker that runs the jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobRunner;

impl ServiceIdentity for JobRunner {
    const NAME: &'static str = "job-runner";
    const ID: Uuid = Uuid::from_u128(0x0192_0000_0000_7000_8000_0000_0000_0002);
    const CHANNEL: Channel = Channel::Job;
}

impl ServiceCaller<JobRunner> {
    /// The scope of the organization that an infrastructure query gave for a job (ADR 0039).
    pub(crate) fn scope(&self, organization_id: OrganizationId) -> OrgScope {
        OrgScope(organization_id)
    }
}

/// The service identity `bootstrap`: the command `tada bootstrap` (ADR 0036).
/// It is the only service identity without an organization (ADR 0039).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bootstrap;

impl ServiceIdentity for Bootstrap {
    const NAME: &'static str = "bootstrap";
    const ID: Uuid = Uuid::from_u128(0x0192_0000_0000_7000_8000_0000_0000_0003);
    const CHANNEL: Channel = Channel::Cli;
}

impl ServiceCaller<Bootstrap> {
    /// The scope of the organization that the slug of the operator names (ADR 0039).
    /// Only this crate calls it. Other crates cannot get a scope from a service caller:
    ///
    /// ```compile_fail
    /// use tada_app::caller::{Bootstrap, ServiceCaller};
    /// use tada_app::domain::ids::OrganizationId;
    ///
    /// let organization = OrganizationId::from_uuid(Default::default());
    /// let _ = ServiceCaller::<Bootstrap>::new().scope(organization);
    /// ```
    pub(crate) fn scope(&self, organization_id: OrganizationId) -> OrgScope {
        OrgScope(organization_id)
    }
}

/// The service identity `exporter`: the command `tada export` (ADR 0059).
/// It reads all data of one organization; it changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exporter;

impl ServiceIdentity for Exporter {
    const NAME: &'static str = "exporter";
    const ID: Uuid = Uuid::from_u128(0x0192_0000_0000_7000_8000_0000_0000_0004);
    const CHANNEL: Channel = Channel::Cli;
}

impl ServiceCaller<Exporter> {
    /// The scope of the organization that the slug of the operator names (ADR 0039).
    /// Only this crate calls it, after the named infrastructure query of the slug.
    pub(crate) fn scope(&self, organization_id: OrganizationId) -> OrgScope {
        OrgScope(organization_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member() -> MemberCaller {
        MemberCaller::new(
            UserId::from_uuid(Uuid::from_u128(1)),
            OrganizationId::from_uuid(Uuid::from_u128(2)),
            OrganizationRole::Member,
        )
    }

    #[test]
    fn a_member_actor_names_the_member_the_channel_and_the_request() {
        let request = Uuid::from_u128(3);
        let actor = member().with_request(Some(request)).actor();
        assert_eq!(actor.kind(), ActorKind::Member);
        assert_eq!(actor.id(), Uuid::from_u128(1));
        assert_eq!(actor.principal(), None);
        assert_eq!(actor.channel(), Channel::Web);
        assert_eq!(actor.request_id(), Some(request));
    }

    #[test]
    fn the_request_keeps_the_channel_of_the_authenticator() {
        let request = Uuid::from_u128(3);
        let linked = ServiceCaller::<TelegramGateway>::new()
            .member(
                UserId::from_uuid(Uuid::from_u128(1)),
                OrganizationId::from_uuid(Uuid::from_u128(2)),
                OrganizationRole::Member,
            )
            .with_request(Some(request));
        assert_eq!(linked.actor().channel(), Channel::Telegram);
        assert_eq!(linked.actor().request_id(), Some(request));
    }

    fn ai(scope: TokenScope) -> AiCaller {
        AiCaller::new(member(), ApiTokenId::from_uuid(Uuid::from_u128(4)), scope)
    }

    #[test]
    fn an_ai_actor_names_the_token_the_member_and_the_api_token_channel() {
        let request = Uuid::from_u128(3);
        let actor = ai(TokenScope::Read).with_request(Some(request)).actor();
        assert_eq!(actor.kind(), ActorKind::Ai);
        assert_eq!(actor.id(), Uuid::from_u128(4));
        assert_eq!(actor.principal(), Some(Uuid::from_u128(1)));
        assert_eq!(actor.channel(), Channel::ApiToken);
        assert_eq!(actor.request_id(), Some(request));
        assert_eq!(ai(TokenScope::Read).actor().request_id(), None);
    }

    #[test]
    fn an_ai_caller_has_the_rights_of_its_member_and_the_scope_of_its_token() {
        use crate::access::Principal;

        let ai = ai(TokenScope::Propose);
        assert_eq!(Principal::user_id(&ai), member().user_id());
        assert_eq!(Principal::scope(&ai), member().scope());
        assert_eq!(ai.organization_role(), OrganizationRole::Member);
        assert_eq!(ai.token_id(), ApiTokenId::from_uuid(Uuid::from_u128(4)));
        assert_eq!(ai.token_scope(), TokenScope::Propose);
    }

    #[test]
    fn a_new_member_caller_uses_the_web_channel() {
        assert_eq!(member().channel(), Channel::Web);
        assert_eq!(member().actor().request_id(), None);
    }

    #[test]
    fn a_service_actor_has_the_fixed_id_of_its_identity() {
        let actor = ServiceCaller::<TelegramGateway>::new().actor();
        assert_eq!(actor.kind(), ActorKind::Service);
        assert_eq!(actor.id(), TelegramGateway::ID);
        assert_eq!(TelegramGateway::NAME, "telegram-gateway");
    }

    #[test]
    fn a_service_actor_has_the_channel_of_its_identity() {
        let telegram = ServiceCaller::<TelegramGateway>::new().actor();
        assert_eq!(telegram.channel(), Channel::Telegram);
        let job = ServiceCaller::<JobRunner>::new().actor();
        assert_eq!(job.channel(), Channel::Job);
        let bootstrap = ServiceCaller::<Bootstrap>::new().actor();
        assert_eq!(bootstrap.channel(), Channel::Cli);
    }

    #[test]
    fn service_identities_have_distinct_ids() {
        let ids = [
            JobRunner::ID,
            TelegramGateway::ID,
            Bootstrap::ID,
            Exporter::ID,
        ];
        assert!(ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id)));
        assert_eq!(JobRunner::NAME, "job-runner");
        assert_eq!(Bootstrap::NAME, "bootstrap");
        assert_eq!(Exporter::NAME, "exporter");
        assert_eq!(Exporter::CHANNEL, Channel::Cli);
    }

    #[test]
    fn channels_have_stable_names() {
        let names = [
            Channel::Web,
            Channel::Telegram,
            Channel::Job,
            Channel::ApiToken,
            Channel::Cli,
        ]
        .map(Channel::as_str);
        assert_eq!(names, ["web", "telegram", "job", "api-token", "cli"]);
        for name in names {
            assert_eq!(Channel::parse(name).map(Channel::as_str), Some(name));
        }
        assert_eq!(Channel::parse("mail"), None);
    }

    #[test]
    fn actor_kinds_have_stable_names() {
        let names = [ActorKind::Member, ActorKind::Service, ActorKind::Ai].map(ActorKind::as_str);
        assert_eq!(names, ["member", "service", "ai"]);
        for name in names {
            assert_eq!(ActorKind::parse(name).map(ActorKind::as_str), Some(name));
        }
        assert_eq!(ActorKind::parse("robot"), None);
    }
}
