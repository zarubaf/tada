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

impl<V> FactState<V> {
    /// The same state with the value that `f` gives.
    pub fn map<U>(self, f: impl FnOnce(V) -> U) -> FactState<U> {
        match self {
            Self::Accepted(value) => FactState::Accepted(f(value)),
            Self::Assumption(value) => FactState::Assumption(f(value)),
            Self::Unknown => FactState::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_keeps_the_state() {
        assert_eq!(
            FactState::Accepted(2).map(|n| n * 2),
            FactState::Accepted(4)
        );
        assert_eq!(
            FactState::Assumption(2).map(|n| n + 1),
            FactState::Assumption(3)
        );
        assert_eq!(FactState::<i32>::Unknown.map(|n| n + 1), FactState::Unknown);
    }
}
