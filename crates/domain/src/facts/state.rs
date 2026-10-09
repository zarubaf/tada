//! The state of a fact version (ADR 0049).

/// The state of one fact version.
/// An unknown fact has no value, so tada cannot show or fill in a value for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactState<V> {
    /// The team confirmed the value.
    Accepted(V),
    /// The team uses the value for planning but did not confirm it.
    Assumption(V),
    /// Nobody knows the value yet.
    Unknown,
}
