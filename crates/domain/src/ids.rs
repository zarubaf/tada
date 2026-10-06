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

/// Returns true if `uuid` can be the ID of a new record: a UUIDv7 (ADR 0038).
pub fn is_record_id(uuid: Uuid) -> bool {
    uuid.get_version_num() == 7
}
