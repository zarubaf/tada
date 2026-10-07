//! Errors that all repository ports share.

use std::error::Error;

use crate::problem::ProblemCode;

/// A repository call failed for a reason that is not a domain rule.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database did not respond. The client can retry.
    #[error("the store is unavailable")]
    Unavailable(#[source] Box<dyn Error + Send + Sync>),
    /// An unexpected error, for example a constraint that the domain types should have prevented.
    #[error("the store failed")]
    Internal(#[source] Box<dyn Error + Send + Sync>),
}

impl StoreError {
    /// All codes of a store failure, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[ProblemCode::Unavailable, ProblemCode::Internal];

    /// The problem code of the failure (ADR 0037).
    pub fn code(&self) -> ProblemCode {
        match self {
            Self::Unavailable(_) => ProblemCode::Unavailable,
            Self::Internal(_) => ProblemCode::Internal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_error_has_a_code_in_its_list() {
        for error in [
            StoreError::Unavailable("test".into()),
            StoreError::Internal("test".into()),
        ] {
            assert!(StoreError::CODES.contains(&error.code()), "{error:?}");
        }
    }
}
