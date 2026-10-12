//! Decisions: what the club decided, who decided it and its fate (Slice 2d).

use crate::parties::Party;
use crate::work::InvalidTransition;

text_type!(
    /// The wording of a decision: 1 to 2000 characters.
    DecisionWording,
    2000
);

status_enum!(
    /// The status of a decision.
    DecisionStatus { InForce => "in-force", Superseded => "superseded", Revoked => "revoked" }
);

impl DecisionStatus {
    pub const ALL: [Self; 3] = [Self::InForce, Self::Superseded, Self::Revoked];

    /// The one decision of a status change of a decision. `superseded` and `revoked` are final.
    /// A supersede also names the new decision and a revoke gives a reason; the caller checks both.
    pub fn change_to(self, next: Self) -> Result<Self, InvalidTransition> {
        use DecisionStatus::{InForce, Revoked, Superseded};
        let allowed = matches!((self, next), (InForce, Superseded | Revoked));
        if allowed {
            Ok(next)
        } else {
            Err(InvalidTransition)
        }
    }

    /// The statuses that a record in this status can change to.
    pub fn next_statuses(self) -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|next| self.change_to(*next).is_ok())
            .collect()
    }
}

/// A decision needs at least one party that decided it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a decision needs at least one party")]
pub struct EmptyParties;

/// The persons and institutions that decided: at least one, without duplicates, in the given order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionParties(Vec<Party>);

impl DecisionParties {
    /// Drops the duplicates and keeps the first of each in its place.
    pub fn new(parties: Vec<Party>) -> Result<Self, EmptyParties> {
        let mut unique: Vec<Party> = Vec::with_capacity(parties.len());
        for party in parties {
            if !unique.contains(&party) {
                unique.push(party);
            }
        }
        if unique.is_empty() {
            return Err(EmptyParties);
        }
        Ok(Self(unique))
    }

    pub fn as_slice(&self) -> &[Party] {
        &self.0
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

    #[test]
    fn decision_transitions_are_final() {
        let allowed = [
            (DecisionStatus::InForce, DecisionStatus::Superseded),
            (DecisionStatus::InForce, DecisionStatus::Revoked),
        ];
        for from in DecisionStatus::ALL {
            for to in DecisionStatus::ALL {
                assert_eq!(
                    from.change_to(to).is_ok(),
                    allowed.contains(&(from, to)),
                    "{from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn same_status_is_invalid() {
        for status in DecisionStatus::ALL {
            assert_eq!(status.change_to(status), Err(InvalidTransition));
        }
    }

    #[test]
    fn statuses_use_kebab_case() {
        assert_eq!(DecisionStatus::InForce.as_str(), "in-force");
        for status in DecisionStatus::ALL {
            assert_eq!(DecisionStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(
            DecisionStatus::InForce.next_statuses(),
            vec![DecisionStatus::Superseded, DecisionStatus::Revoked]
        );
    }

    #[test]
    fn decision_parties_need_one_and_drop_duplicates() {
        assert_eq!(DecisionParties::new(vec![]), Err(EmptyParties));
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
    fn wording_limit() {
        assert!(DecisionWording::parse(&"a".repeat(2000)).is_ok());
        assert_eq!(
            DecisionWording::parse(&"a".repeat(2001)),
            Err(TextError::TooLong)
        );
    }
}
