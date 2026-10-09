use std::collections::HashSet;

use tada_domain::facts::{FactState, Valued};
use tada_domain::ids::FieldDefinitionId;
use tada_domain::proposals::{Proposal, Reason};
use tada_domain::work::CommitmentStatus;
use uuid::Uuid;

use super::checks::Edits;
use super::selection::with_dependencies;
use super::*;

fn id(n: u128) -> ProposalId {
    ProposalId::from_uuid(Uuid::from_u128(n))
}

fn at(time: &str) -> Timestamp {
    time.parse().unwrap()
}

fn record(outcome: ReviewOutcome, time: &str) -> ReviewRecord {
    ReviewRecord {
        proposal_id: id(1),
        outcome,
        created_at: at(time),
    }
}

/// A proposal that only matters for its dependencies.
fn proposal(n: u128, depends_on: &[u128]) -> Proposal {
    Proposal {
        id: id(n),
        operation: Operation::DeprecateField {
            event_id: EventId::from_uuid(Uuid::from_u128(20)),
            field_id: FieldDefinitionId::from_uuid(Uuid::from_u128(30)),
        },
        depends_on: depends_on.iter().map(|n| id(*n)).collect(),
        evidence: Vec::new(),
        reason: Reason::parse("test").unwrap(),
    }
}

fn set(ids: &[u128]) -> HashSet<ProposalId> {
    ids.iter().map(|n| id(*n)).collect()
}

#[test]
fn the_latest_result_gives_the_status() {
    assert_eq!(status(&[]), ProposalStatus::Open);
    let results = [
        record(ReviewOutcome::Conflict, "2030-05-02T00:00:00Z"),
        record(ReviewOutcome::Withdrawn, "2030-05-01T00:00:00Z"),
    ];
    assert_eq!(status(&results), ProposalStatus::Conflict);
    let same_time = [
        record(ReviewOutcome::Rejected, "2030-05-01T00:00:00Z"),
        record(ReviewOutcome::AcceptedWithEdit, "2030-05-01T00:00:00Z"),
    ];
    assert_eq!(status(&same_time), ProposalStatus::AcceptedWithEdit);
}

#[test]
fn an_open_proposal_is_stale_after_fourteen_days() {
    let created = at("2030-05-01T08:00:00Z");
    assert!(!ProposalStatus::Open.is_stale(created, at("2030-05-15T08:00:00Z")));
    assert!(ProposalStatus::Open.is_stale(created, at("2030-05-15T08:00:01Z")));
    assert!(!ProposalStatus::Conflict.is_stale(created, at("2030-06-01T00:00:00Z")));
}

#[test]
fn each_outcome_has_the_name_of_its_status() {
    for (outcome, name) in [
        (ReviewOutcome::Accepted, "accepted"),
        (ReviewOutcome::AcceptedWithEdit, "accepted-with-edit"),
        (ReviewOutcome::Rejected, "rejected"),
        (ReviewOutcome::Conflict, "conflict"),
        (ReviewOutcome::Withdrawn, "withdrawn"),
    ] {
        assert_eq!(outcome.as_str(), name);
        assert_eq!(ReviewOutcome::parse(name), Some(outcome));
    }
    assert_eq!(ReviewOutcome::parse("open"), None);
}

#[test]
fn a_selection_includes_all_dependencies_and_a_rejection_all_dependents() {
    // 3 depends on 2, 2 on 1; 4 is alone.
    let proposals = [
        proposal(1, &[]),
        proposal(2, &[1]),
        proposal(3, &[2]),
        proposal(4, &[]),
    ];
    assert_eq!(with_dependencies(&proposals, &set(&[3])), set(&[1, 2, 3]));
    assert_eq!(with_dependencies(&proposals, &set(&[4])), set(&[4]));
    assert_eq!(
        with_dependents(&proposals, &set(&[1])),
        [id(1), id(2), id(3)]
    );
    assert_eq!(with_dependents(&proposals, &set(&[3])), [id(3)]);
}

#[test]
fn debug_hides_the_value_of_a_step() {
    let value = tada_domain::facts::ShortText::parse("Anna Muster").unwrap();
    let step = ApplyStep {
        proposal_id: id(1),
        operation: Operation::SetFact {
            event_id: EventId::from_uuid(Uuid::from_u128(20)),
            field_id: FieldDefinitionId::from_uuid(Uuid::from_u128(30)),
            state: FactState::Accepted(Valued {
                value: tada_domain::facts::FactValue::Text(value),
                approximate: false,
            }),
            expected_version: None,
        },
        evidence: StepEvidence::Edit,
        firm_reason: None,
    };
    let debug = format!("{step:?}");
    assert!(debug.contains("SetFact"), "{debug}");
    assert!(!debug.contains("Anna"), "{debug}");
}

#[test]
fn a_change_to_firm_without_a_valid_reason_does_not_apply() {
    let mut firm = proposal(1, &[]);
    firm.operation = Operation::ChangeCommitmentStatus {
        event_id: EventId::from_uuid(Uuid::from_u128(20)),
        commitment_id: tada_domain::ids::CommitmentId::from_uuid(Uuid::from_u128(40)),
        status: CommitmentStatus::Firm,
        expected_version: tada_domain::RecordVersion::FIRST,
    };
    firm.reason = Reason::parse(&"a".repeat(FirmReason::MAX_CHARS + 1)).unwrap();
    let result = step(&firm, &Edits::new());
    assert!(matches!(result, Err(ApplyError::Invalid(_))), "{result:?}");
    firm.reason = Reason::parse("Der Auftrag ist unterschrieben.").unwrap();
    let step = step(&firm, &Edits::new()).unwrap();
    assert_eq!(
        step.firm_reason
            .map(|reason| reason.as_str().to_owned())
            .as_deref(),
        Some("Der Auftrag ist unterschrieben.")
    );
}

fn open(n: u128, time: &str) -> InboxChangeset {
    let changeset = OpenChangeset {
        id: ChangesetId::from_uuid(Uuid::from_u128(n)),
        event_id: None,
        author: Actor::restore(
            crate::caller::ActorKind::Member,
            Uuid::from_u128(9),
            None,
            crate::caller::Channel::Web,
            None,
        ),
        created_at: at(time),
        proposals: Vec::new(),
    };
    InboxChangeset {
        changeset,
        in_inbox: 1,
    }
}

#[test]
fn a_page_of_open_changesets_continues_after_its_cursor() {
    let all = || {
        vec![
            open(1, "2030-05-01T00:00:00Z"),
            open(2, "2030-05-01T00:00:00Z"),
            open(3, "2030-05-02T00:00:00Z"),
        ]
    };
    let two = PageLimit::new(2).unwrap();
    let first = page(all(), None, two);
    assert_eq!(first.items, all()[..2]);
    let cursor = first.next.unwrap();
    assert_eq!(cursor.id, ChangesetId::from_uuid(Uuid::from_u128(2)));
    let second = page(all(), Some(cursor), two);
    assert_eq!(second.items, all()[2..]);
    assert_eq!(second.next, None);
}

#[test]
fn each_proposal_applies_after_its_dependencies() {
    // The changeset is in the order of the IDs, so a dependency can come later.
    let proposals = [proposal(1, &[3]), proposal(2, &[]), proposal(3, &[2])];
    let order: Vec<ProposalId> = apply_order(&proposals, &set(&[1, 2, 3]))
        .into_iter()
        .map(|proposal| proposal.id)
        .collect();
    assert_eq!(order, [id(2), id(3), id(1)]);
}

#[test]
fn each_error_gives_a_code_of_its_list() {
    for error in [
        ApplyError::NotFound,
        ApplyError::Forbidden,
        ApplyError::Invalid(Vec::new()),
        ApplyError::InvalidTransition,
        ApplyError::Conflict(Vec::new()),
        ApplyError::Store(StoreError::Internal("test".into())),
        ApplyError::Store(StoreError::Unavailable("test".into())),
    ] {
        assert!(ApplyError::CODES.contains(&error.code()), "{error:?}");
    }
    for error in [
        ReviewQueryError::NotFound,
        ReviewQueryError::Forbidden,
        ReviewQueryError::Store(StoreError::Internal("test".into())),
        ReviewQueryError::Store(StoreError::Unavailable("test".into())),
    ] {
        assert!(ReviewQueryError::CODES.contains(&error.code()), "{error:?}");
    }
}
