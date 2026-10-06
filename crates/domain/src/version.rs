//! The record version for optimistic concurrency (ADR 0006).

/// A number that increases with each change of a record. A new record has version 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecordVersion(i64);

impl RecordVersion {
    pub const FIRST: Self = Self(1);

    /// Returns `None` if `value` is not a valid version.
    pub fn new(value: i64) -> Option<Self> {
        (value >= 1).then_some(Self(value))
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}
