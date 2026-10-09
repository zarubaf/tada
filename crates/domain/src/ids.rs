//! Record IDs (ADR 0038). Each kind of record has its own ID type, so the compiler rejects an
//! `EventId` where an `OrganizationId` is expected.

use std::fmt;

use uuid::Uuid;

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

id_type!(
    /// The ID of an organization: the tenant.
    OrganizationId
);
id_type!(
    /// The ID of a user.
    UserId
);
id_type!(
    /// The ID of an event.
    EventId
);
id_type!(
    /// The ID of an invitation to an organization.
    InvitationId
);
id_type!(
    /// The ID of a document.
    DocumentId
);
id_type!(
    /// The ID of one immutable version of a document.
    DocumentVersionId
);
id_type!(
    /// The ID of a field definition.
    FieldDefinitionId
);
id_type!(
    /// The ID of a fact: one field of one event.
    FactId
);
id_type!(
    /// The ID of one immutable version of a fact.
    FactVersionId
);
id_type!(
    /// The ID of a source item.
    SourceItemId
);
id_type!(
    /// The ID of one immutable version of a source item.
    SourceVersionId
);

id_type!(
    /// The ID of a changeset: the proposals of one intake.
    ChangesetId
);
id_type!(
    /// The ID of a proposal.
    ProposalId
);
id_type!(
    /// The ID of an open question.
    OpenQuestionId
);
id_type!(
    /// The ID of a personal API token (ADR 0039).
    ApiTokenId
);

/// Returns true if `uuid` can be the ID of a new record: a UUIDv7 (ADR 0038).
pub fn is_record_id(uuid: Uuid) -> bool {
    uuid.get_version_num() == 7
}

/// The kind of an event-local or organization-local readable ID, for example `QST-001` (ADR 0038).
/// Its prefix is also the kind of its counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalIdKind {
    /// `QST-<n>`, local to the event.
    OpenQuestion,
    /// `DOC-<n>`, local to the organization.
    Document,
}

impl LocalIdKind {
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::OpenQuestion => "QST",
            Self::Document => "DOC",
        }
    }

    /// The readable ID of the number `local_number`, with at least three digits, for example `QST-001`.
    pub fn readable_id(self, local_number: u64) -> String {
        format!("{}-{local_number:03}", self.prefix())
    }
}

#[cfg(test)]
mod local_id_tests {
    use super::LocalIdKind;

    #[test]
    fn a_readable_id_has_its_prefix_and_at_least_three_digits() {
        assert_eq!(LocalIdKind::OpenQuestion.readable_id(1), "QST-001");
        assert_eq!(LocalIdKind::Document.readable_id(42), "DOC-042");
        assert_eq!(LocalIdKind::OpenQuestion.readable_id(1234), "QST-1234");
    }
}
