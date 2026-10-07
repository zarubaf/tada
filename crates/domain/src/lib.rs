//! Types, rules and state machines. No I/O.
//!
//! A constructor checks each value, so other code cannot create an invalid value (ADR 0003).

pub mod events;
pub mod identity;
pub mod ids;

mod version;

pub use version::RecordVersion;
