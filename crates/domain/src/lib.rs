//! Types, rules and state machines. No I/O.
//!
//! A constructor checks each value, so other code cannot create an invalid value (ADR 0003).

pub mod events;
pub mod facts;
pub mod identity;
pub mod ids;
pub mod name;
pub mod sources;

mod version;

pub use version::RecordVersion;
