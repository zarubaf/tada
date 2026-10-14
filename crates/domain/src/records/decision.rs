//! Decisions: what the club decided, who decided it and its fate (Slice 2d).

use crate::ids::DecisionId;
use crate::parties::Party;
use crate::work::{InvalidTransition, RecordReason};

text_type!(
    /// The wording of a decision: 1 to 2000 characters. It can have line breaks and tabs.
    DecisionWording,
    2000,
    multiline
);

status_enum!(
    /// The status of a decision. Only a `DecisionChange` changes it: no plain status change exists.
    DecisionStatus { InForce => "in-force", Superseded => "superseded", Revoked => "revoked" }
);

/// The end of a decision that is in force (ADR 0081). Each change carries what its status needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionChange {
    /// A newer decision of the same event replaces the decision. The caller checks the successor.
    Supersede { successor: DecisionId },
    /// The decision ends without a successor, for the reason.
    Revoke { reason: RecordReason },
}

impl DecisionChange {
    /// The status that the change gives.
    pub const fn status(&self) -> DecisionStatus {
        match self {
            Self::Supersede { .. } => DecisionStatus::Superseded,
            Self::Revoke { .. } => DecisionStatus::Revoked,
        }
    }
}

impl DecisionStatus {
    /// The one decision of a status change of a decision: only a decision in force ends, and `superseded` and
    /// `revoked` are final.
    pub fn apply(self, change: &DecisionChange) -> Result<Self, InvalidTransition> {
        match self {
            Self::InForce => Ok(change.status()),
            Self::Superseded | Self::Revoked => Err(InvalidTransition),
        }
    }
}

/// The parties of a decision break a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PartiesError {
    #[error("a decision needs at least one party")]
    Empty,
    #[error("a decision has at most {} parties", DecisionParties::MAX)]
    TooMany,
}

/// The persons and institutions that decided: 1 to `MAX`, without duplicates, in the given order.
#[derive(Debug, Clone)]
pub struct DecisionParties {
    parties: Vec<Party>,
    /// The position of each party in the list that it came from, before the duplicates were dropped.
    positions: Vec<usize>,
}

impl PartialEq for DecisionParties {
    fn eq(&self, other: &Self) -> bool {
        self.parties == other.parties
    }
}

impl Eq for DecisionParties {}

impl DecisionParties {
    /// The most parties that one decision names.
    pub const MAX: usize = 50;

    /// Drops the duplicates and keeps the first of each in its place, then checks the count.
    pub fn new(parties: Vec<Party>) -> Result<Self, PartiesError> {
        let mut unique: Vec<Party> = Vec::with_capacity(parties.len());
        let mut positions = Vec::with_capacity(parties.len());
        for (position, party) in parties.into_iter().enumerate() {
            if !unique.contains(&party) {
                unique.push(party);
                positions.push(position);
            }
        }
        match unique.len() {
            0 => Err(PartiesError::Empty),
            count if count > Self::MAX => Err(PartiesError::TooMany),
            _ => Ok(Self {
                parties: unique,
                positions,
            }),
        }
    }

    pub fn as_slice(&self) -> &[Party] {
        &self.parties
    }

    /// Each party with its position in the list that it came from, for error paths.
    pub fn with_input_positions(&self) -> impl Iterator<Item = (usize, &Party)> {
        self.positions.iter().copied().zip(&self.parties)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::TextError;
    use crate::ids::{InstitutionId, PersonId};
    use uuid::Uuid;

    fn person(n: u128) -> Party {
        Party::Person(PersonId::from_uuid(Uuid::from_u128(n)))
    }

    fn successor() -> DecisionChange {
        DecisionChange::Supersede {
            successor: DecisionId::from_uuid(Uuid::from_u128(3)),
        }
    }

    fn revocation() -> DecisionChange {
        DecisionChange::Revoke {
            reason: RecordReason::parse("Der Vorstand hebt den Entscheid auf.").unwrap(),
        }
    }

    #[test]
    fn only_a_decision_in_force_ends() {
        let changes = [successor(), revocation()];
        assert_eq!(
            DecisionStatus::InForce.apply(&changes[0]),
            Ok(DecisionStatus::Superseded)
        );
        assert_eq!(
            DecisionStatus::InForce.apply(&changes[1]),
            Ok(DecisionStatus::Revoked)
        );
        for status in [DecisionStatus::Superseded, DecisionStatus::Revoked] {
            for change in &changes {
                assert_eq!(status.apply(change), Err(InvalidTransition), "{status:?}");
            }
        }
    }

    #[test]
    fn statuses_use_kebab_case() {
        assert_eq!(DecisionStatus::InForce.as_str(), "in-force");
        for status in DecisionStatus::ALL {
            assert_eq!(DecisionStatus::parse(status.as_str()), Some(status));
        }
    }

    #[test]
    fn decision_parties_need_one_and_drop_duplicates() {
        assert_eq!(DecisionParties::new(vec![]), Err(PartiesError::Empty));
        let institution = Party::Institution(InstitutionId::from_uuid(Uuid::from_u128(9)));
        let parties = DecisionParties::new(vec![
            person(2),
            institution,
            person(2),
            person(1),
            institution,
        ])
        .unwrap();
        assert_eq!(parties.as_slice(), &[person(2), institution, person(1)]);
    }

    #[test]
    fn decision_parties_have_a_limit_after_the_duplicates() {
        let most: Vec<Party> = (1..=50).map(person).collect();
        assert!(DecisionParties::new(most.clone()).is_ok());
        let repeated = most.iter().chain(&most).copied().collect();
        assert!(DecisionParties::new(repeated).is_ok());
        let more: Vec<Party> = (1..=51).map(person).collect();
        assert_eq!(DecisionParties::new(more), Err(PartiesError::TooMany));
    }

    #[test]
    fn wording_limit() {
        assert!(DecisionWording::parse(&"a".repeat(2000)).is_ok());
        assert_eq!(
            DecisionWording::parse(&"a".repeat(2001)),
            Err(TextError::TooLong)
        );
    }

    #[test]
    fn wording_keeps_line_breaks_and_tabs() {
        let wording = DecisionWording::parse(" Erstens.\nZweitens:\tGeld.\nDrittens. ").unwrap();
        assert_eq!(wording.as_str(), "Erstens.\nZweitens:\tGeld.\nDrittens.");
        assert_eq!(
            DecisionWording::parse("Erstens.\u{7}"),
            Err(TextError::ControlCharacter)
        );
        assert_eq!(DecisionWording::parse(" \n\t "), Err(TextError::Empty));
        let lines = format!("{}\n{}", "ä".repeat(1000), "ö".repeat(999));
        assert!(DecisionWording::parse(&lines).is_ok());
        assert_eq!(
            DecisionWording::parse(&format!("{lines}ü")),
            Err(TextError::TooLong)
        );
    }
}
