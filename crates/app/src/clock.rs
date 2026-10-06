//! The clock port (ADR 0038). The app clock gives domain times, for example the creation time of a record.

use std::fmt::Debug;

use jiff::Timestamp;

pub trait Clock: Debug + Send + Sync {
    /// The current time, with microsecond precision. PostgreSQL stores this precision, so a record
    /// that a command returns is equal to the record that a later query reads.
    fn now(&self) -> Timestamp;
}
