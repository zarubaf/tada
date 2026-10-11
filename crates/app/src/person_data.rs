//! The data about one person (ADR 0073): where tada mentions the person, and the export of all its data.
//!
//! Only an owner lists the mentions and exports the data. A mention is a text that contains the name, the email
//! address or the phone number of the person, or a record that refers to the person by ID. tada does not guess
//! other spellings.

use std::collections::HashSet;
use std::fmt::{self, Debug};
use std::str::FromStr;

use async_trait::async_trait;
use jiff::Timestamp;
use serde::Serialize;
use tada_domain::ids::{EventId, PersonId, UserId};
use tada_domain::parties::mention_term;
use tada_domain::redaction::Redactable;
use uuid::Uuid;

use crate::access;
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::dispute_notes::DisputeNote;
use crate::parties::{PartyStore, PersonView};
use crate::problem::{CommandError, ProblemCode};
use crate::store::StoreError;

/// The longest excerpt of a mention, in characters.
pub const EXCERPT_MAX_CHARS: usize = 200;

/// The version of the layout of a person export. A reader checks it before it reads the rest.
pub const FORMAT_VERSION: u32 = 1;

/// Where a mention is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MentionKind {
    SourceItem,
    FactVersion,
    Proposal,
    OpenQuestion,
    Action,
    Commitment,
    Workstream,
    DocumentVersion,
    /// A commitment whose promisor is the person: a reference by ID.
    Promisor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this is not a kind of mention")]
pub struct UnknownMentionKind;

impl MentionKind {
    pub const ALL: [Self; 9] = [
        Self::SourceItem,
        Self::FactVersion,
        Self::Proposal,
        Self::OpenQuestion,
        Self::Action,
        Self::Commitment,
        Self::Workstream,
        Self::DocumentVersion,
        Self::Promisor,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceItem => "source-item",
            Self::FactVersion => "fact-version",
            Self::Proposal => "proposal",
            Self::OpenQuestion => "open-question",
            Self::Action => "action",
            Self::Commitment => "commitment",
            Self::Workstream => "workstream",
            Self::DocumentVersion => "document-version",
            Self::Promisor => "promisor",
        }
    }
}

impl FromStr for MentionKind {
    type Err = UnknownMentionKind;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == value)
            .ok_or(UnknownMentionKind)
    }
}

/// A row that the store found, with its full text. A source item can come once for each version that matches.
#[derive(Clone, PartialEq, Eq)]
pub struct FoundMention {
    pub kind: MentionKind,
    pub id: Uuid,
    pub event_id: Option<EventId>,
    /// The text columns of the row. A legal redaction removed them.
    pub text: Redactable<String>,
    /// True if a dispute note is about the row.
    pub disputed: bool,
}

/// The text names a person, so `Debug` leaves it out (ADR 0035).
impl Debug for FoundMention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FoundMention")
            .field("kind", &self.kind)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// One mention as the owner sees it in the list.
#[derive(Clone, PartialEq, Eq)]
pub struct Mention {
    pub kind: MentionKind,
    pub id: Uuid,
    pub event_id: Option<EventId>,
    /// At most `EXCERPT_MAX_CHARS` characters around the first match.
    pub excerpt: Redactable<String>,
    pub disputed: bool,
}

impl Debug for Mention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mention")
            .field("kind", &self.kind)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// The account data of a person who links to an account, for the export. It holds no secret and no hash.
#[derive(Clone, Serialize)]
pub struct AccountData {
    pub user_id: Uuid,
    pub display_name: String,
    pub email: Option<String>,
    /// The role in this organization. It is absent if the membership ended.
    pub organization_role: Option<String>,
    pub event_memberships: Vec<EventMembershipData>,
    pub telegram: Option<TelegramData>,
    pub sessions: Vec<SessionData>,
    pub api_tokens: Vec<ApiTokenData>,
    /// The invitations of this organization to the email address of the account.
    pub invitations: Vec<InvitationData>,
    /// The audit events of this organization with the account as actor or subject.
    pub audit_events: Vec<AuditEventData>,
}

impl Debug for AccountData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountData")
            .field("user_id", &self.user_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EventMembershipData {
    pub event_id: Uuid,
    pub event_role: String,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize)]
pub struct TelegramData {
    pub telegram_user_id: i64,
    pub linked_at: Timestamp,
}

/// A session without its token.
#[derive(Clone, Serialize)]
pub struct SessionData {
    pub created_at: Timestamp,
    pub last_used_at: Timestamp,
    pub user_agent: Option<String>,
}

impl Debug for SessionData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionData")
            .field("created_at", &self.created_at)
            .finish_non_exhaustive()
    }
}

/// An API token without its secret and its hash.
#[derive(Debug, Clone, Serialize)]
pub struct ApiTokenData {
    pub id: Uuid,
    pub name: String,
    pub scope: String,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
    pub last_used_at: Option<Timestamp>,
    pub revoked_at: Option<Timestamp>,
}

#[derive(Clone, Serialize)]
pub struct InvitationData {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub created_at: Timestamp,
    pub accepted_at: Option<Timestamp>,
    pub revoked_at: Option<Timestamp>,
}

impl Debug for InvitationData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InvitationData")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// An audit event with its IDs and its action. It has no detail.
#[derive(Debug, Clone, Serialize)]
pub struct AuditEventData {
    pub id: Uuid,
    pub occurred_at: Timestamp,
    pub action: String,
    pub record_kind: String,
    pub record_id: Option<Uuid>,
}

/// What the store reads for an export, in one transaction.
#[derive(Debug, Clone)]
pub struct PersonRecords {
    pub organization_name: String,
    pub mentions: Vec<FoundMention>,
    /// The notes about the mentions.
    pub dispute_notes: Vec<DisputeNote>,
    pub account: Option<AccountData>,
}

/// The repository port of the person data. Each method stays inside `scope`.
#[async_trait]
pub trait PersonDataStore: Debug + Send + Sync {
    /// The rows whose text contains one of `terms`, whatever the case, and the commitments whose promisor is the
    /// person. `%`, `_` and `\` in a term match only themselves. Without terms, only the references by ID come.
    async fn find_mentions(
        &self,
        scope: OrgScope,
        person: PersonId,
        terms: &[String],
    ) -> Result<Vec<FoundMention>, StoreError>;

    /// The data of an export and, with `account`, the data of that account in the organization. It records `audit`
    /// in the same transaction.
    async fn export(
        &self,
        scope: OrgScope,
        person: PersonId,
        account: Option<UserId>,
        terms: &[String],
        audit: &AuditEvent,
    ) -> Result<PersonRecords, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum PersonDataError {
    /// Only an owner reads the data about a person.
    #[error("the caller cannot do this")]
    Forbidden,
    #[error("the person does not exist")]
    NotFound,
    /// The session of the caller is older than `session::RECENT_SIGN_IN`. The export holds all data about a
    /// person, so a stolen session must not take it (ADR 0070).
    #[error("the action needs a recent sign-in")]
    RecentSignInRequired,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl PersonDataError {
    /// All codes of the queries, for the API contract (ADR 0037). The export also answers
    /// `recent-sign-in-required`.
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::NotFound,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for PersonDataError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Forbidden => ProblemCode::Forbidden,
            Self::NotFound => ProblemCode::NotFound,
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
}

/// The person, if the caller is an owner. A caller of another organization does not find the person.
async fn owned_person(
    caller: &MemberCaller,
    id: PersonId,
    parties: &dyn PartyStore,
) -> Result<PersonView, PersonDataError> {
    let person = parties
        .person(caller.scope(), id)
        .await?
        .ok_or(PersonDataError::NotFound)?;
    if !access::is_owner(caller) {
        return Err(PersonDataError::Forbidden);
    }
    Ok(person)
}

/// The search terms of a person: its name, email address and phone number, in NFC, each with at least
/// 3 characters (ADR 0073). A redacted person has none.
pub fn mention_terms(person: &PersonView) -> Vec<String> {
    let name = person.name.as_present().map(|name| name.as_str());
    let email = person.email.as_ref().map(|email| email.as_str());
    let phone = person.phone.as_ref().map(|phone| phone.as_str());
    let mut seen = HashSet::new();
    [name, email, phone]
        .into_iter()
        .flatten()
        .filter_map(mention_term)
        .filter(|term| seen.insert(term.to_lowercase()))
        .collect()
}

/// At most `EXCERPT_MAX_CHARS` characters of `text` around the first match of a term, whatever the case.
/// Without a match, the start of the text.
pub fn excerpt(text: &str, terms: &[String]) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= EXCERPT_MAX_CHARS {
        return text.to_owned();
    }
    let (start, length) = terms
        .iter()
        .filter_map(|term| {
            let term: Vec<char> = term.chars().collect();
            find(&chars, &term).map(|start| (start, term.len()))
        })
        .min()
        .unwrap_or((0, 0));
    let before = EXCERPT_MAX_CHARS.saturating_sub(length) / 2;
    let first = start
        .saturating_sub(before)
        .min(chars.len() - EXCERPT_MAX_CHARS);
    chars[first..first + EXCERPT_MAX_CHARS].iter().collect()
}

/// The first position of `term` in `text`, whatever the case of each character.
fn find(text: &[char], term: &[char]) -> Option<usize> {
    if term.is_empty() || term.len() > text.len() {
        return None;
    }
    let same = |a: char, b: char| a == b || a.to_lowercase().eq(b.to_lowercase());
    (0..=text.len() - term.len())
        .find(|&start| term.iter().zip(&text[start..]).all(|(&a, &b)| same(a, b)))
}

/// The mentions of a person, each record once (ADR 0073). Only an owner can list them.
pub async fn list_mentions(
    caller: &MemberCaller,
    id: PersonId,
    parties: &dyn PartyStore,
    store: &dyn PersonDataStore,
) -> Result<Vec<Mention>, PersonDataError> {
    let person = owned_person(caller, id, parties).await?;
    let terms = mention_terms(&person);
    let mut seen = HashSet::new();
    Ok(store
        .find_mentions(caller.scope(), id, &terms)
        .await?
        .into_iter()
        .filter(|found| seen.insert((found.kind, found.id)))
        .map(|found| Mention {
            kind: found.kind,
            id: found.id,
            event_id: found.event_id,
            excerpt: found.text.map(|text| excerpt(&text, &terms)),
            disputed: found.disputed,
        })
        .collect())
}

/// The export of all data about one person (ADR 0073), as its JSON document holds it.
#[derive(Serialize)]
pub struct PersonExport {
    pub format_version: u32,
    pub generated_at: Timestamp,
    pub organization: ExportedOrganization,
    pub person: ExportedPerson,
    /// The records that refer to the person by ID.
    pub references: Vec<ExportedMention>,
    /// The texts that contain a term of the person, with their full text.
    pub mentions: Vec<ExportedMention>,
    /// The dispute notes about the mentions.
    pub dispute_notes: Vec<ExportedNote>,
    /// The account data, if the person links to an account.
    pub account: Option<AccountData>,
}

impl PersonExport {
    /// The file name of the export, for example `person-PER-001.json`.
    pub fn file_name(&self) -> String {
        format!("person-{}.json", self.person.local_id)
    }
}

impl Debug for PersonExport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PersonExport")
            .field("person", &self.person.id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize)]
pub struct ExportedOrganization {
    pub id: Uuid,
    pub name: String,
}

#[derive(Serialize)]
pub struct ExportedPerson {
    pub id: Uuid,
    pub local_id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub user_id: Option<Uuid>,
    pub redacted: bool,
}

impl Debug for ExportedPerson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExportedPerson")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

#[derive(Serialize)]
pub struct ExportedMention {
    pub kind: &'static str,
    pub id: Uuid,
    pub event_id: Option<Uuid>,
    /// The full text. It is null after a legal redaction.
    pub text: Option<String>,
    pub redacted: bool,
    pub disputed: bool,
}

impl Debug for ExportedMention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExportedMention")
            .field("kind", &self.kind)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl From<FoundMention> for ExportedMention {
    fn from(found: FoundMention) -> Self {
        Self {
            kind: found.kind.as_str(),
            id: found.id,
            event_id: found.event_id.map(EventId::as_uuid),
            redacted: found.text.as_present().is_none(),
            text: found.text.as_present().cloned(),
            disputed: found.disputed,
        }
    }
}

#[derive(Serialize)]
pub struct ExportedNote {
    pub id: Uuid,
    pub target_kind: &'static str,
    pub target_id: Uuid,
    pub text: String,
    pub created_at: Timestamp,
}

impl Debug for ExportedNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExportedNote")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Exports all data about one person (ADR 0073) and records `person.export`. Only an owner can do this.
/// The type takes a member only, so an AI client cannot export:
///
/// ```compile_fail
/// use tada_app::caller::AiCaller;
/// use tada_app::clock::Clock;
/// use tada_app::domain::ids::PersonId;
/// use tada_app::parties::PartyStore;
/// use tada_app::person_data::{PersonDataStore, export_person};
///
/// async fn by_ai(caller: &AiCaller, parties: &dyn PartyStore, store: &dyn PersonDataStore, clock: &dyn Clock) {
///     let _ = export_person(caller, PersonId::from_uuid(uuid::Uuid::nil()), parties, store, clock).await;
/// }
/// ```
pub async fn export_person(
    caller: &MemberCaller,
    id: PersonId,
    parties: &dyn PartyStore,
    store: &dyn PersonDataStore,
    clock: &dyn Clock,
) -> Result<PersonExport, PersonDataError> {
    let person = owned_person(caller, id, parties).await?;
    if !caller.signed_in_recently(clock.now()) {
        return Err(PersonDataError::RecentSignInRequired);
    }
    let scope = caller.scope();
    let terms = mention_terms(&person);
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::PersonExport,
        Some(id.as_uuid()),
        Some(scope),
    );
    let records = store
        .export(scope, id, person.user_id, &terms, &audit)
        .await?;
    let (references, mentions): (Vec<FoundMention>, Vec<FoundMention>) = records
        .mentions
        .into_iter()
        .partition(|found| found.kind == MentionKind::Promisor);
    Ok(PersonExport {
        format_version: FORMAT_VERSION,
        generated_at: clock.now(),
        organization: ExportedOrganization {
            id: scope.organization_id().as_uuid(),
            name: records.organization_name,
        },
        person: ExportedPerson {
            id: person.id.as_uuid(),
            local_id: person.local_id(),
            redacted: person.name.as_present().is_none(),
            name: person
                .name
                .as_present()
                .map(|name| name.as_str().to_owned()),
            email: person.email.map(|email| email.as_str().to_owned()),
            phone: person.phone.map(|phone| phone.as_str().to_owned()),
            user_id: person.user_id.map(UserId::as_uuid),
        },
        references: references.into_iter().map(ExportedMention::from).collect(),
        mentions: mentions.into_iter().map(ExportedMention::from).collect(),
        dispute_notes: records
            .dispute_notes
            .into_iter()
            .map(|note| ExportedNote {
                id: note.id.as_uuid(),
                target_kind: note.target.kind.as_str(),
                target_id: note.target.id,
                text: note.text.as_str().to_owned(),
                created_at: note.created_at,
            })
            .collect(),
        account: records.account,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn a_short_text_is_its_own_excerpt() {
        assert_eq!(
            excerpt("Anruf bei Beat", &terms(&["beat"])),
            "Anruf bei Beat"
        );
    }

    #[test]
    fn the_excerpt_of_a_long_text_has_the_match_in_the_middle() {
        let text = format!("{}Beat Müller{}", "ä".repeat(300), "ö".repeat(300));
        let excerpt = excerpt(&text, &terms(&["BEAT MÜLLER"]));
        assert_eq!(excerpt.chars().count(), EXCERPT_MAX_CHARS);
        assert!(excerpt.contains("Beat Müller"), "{excerpt}");
        let before = excerpt.chars().take_while(|c| *c == 'ä').count();
        assert_eq!(before, (EXCERPT_MAX_CHARS - 11) / 2);
    }

    #[test]
    fn the_excerpt_keeps_to_the_ends_of_the_text() {
        let text = format!("Beat {}", "x".repeat(400));
        assert!(excerpt(&text, &terms(&["beat"])).starts_with("Beat "));
        let text = format!("{} Beat", "x".repeat(400));
        assert!(excerpt(&text, &terms(&["beat"])).ends_with(" Beat"));
        // Without a match, the excerpt is the start of the text.
        let text = format!("Anfang {}", "x".repeat(400));
        assert!(excerpt(&text, &terms(&["beat"])).starts_with("Anfang"));
    }

    #[test]
    fn the_first_match_of_any_term_wins() {
        let text = format!("{}079 555 01 02 {} Beat", "x".repeat(300), "y".repeat(300));
        let excerpt = excerpt(&text, &terms(&["Beat", "079 555 01 02"]));
        assert!(excerpt.contains("079 555 01 02"), "{excerpt}");
    }

    #[test]
    fn mention_kinds_round_trip() {
        for kind in MentionKind::ALL {
            assert_eq!(kind.as_str().parse::<MentionKind>(), Ok(kind));
        }
    }

    #[test]
    fn each_error_gives_a_code_of_its_list() {
        for error in [
            PersonDataError::Forbidden,
            PersonDataError::NotFound,
            PersonDataError::Store(StoreError::Unavailable("test".into())),
            PersonDataError::Store(StoreError::Internal("test".into())),
        ] {
            assert!(PersonDataError::CODES.contains(&error.code()), "{error:?}");
        }
    }
}
