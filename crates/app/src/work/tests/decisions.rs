use tada_domain::records::DecisionWording;
use tada_domain::redaction::Redactable;

use super::*;

/// A wording with several points: a decision keeps its line breaks.
const WORDING: &str =
    "Das Fest findet bei jedem Wetter statt.\nBei Sturm entscheidet das OK am Vortag.";

fn new_decision(owner: u128, workstream: Option<WorkstreamId>) -> NewDecision {
    NewDecision {
        id: None,
        wording: WORDING.to_owned(),
        decided_on: jiff::civil::date(2030, 3, 1),
        decided_by: vec![Party::Person(supplier())],
        owner: user(owner),
        workstream,
    }
}

async fn decision_by(
    memory: &Memory,
    by: u128,
    input: NewDecision,
) -> Result<Shown<DecisionView>, WorkError> {
    create_decision(&caller(by), open_day(), input, memory.ports()).await
}

async fn recorded(memory: &Memory) -> DecisionView {
    decision_by(memory, MANAGER, new_decision(OWNER, Some(ground())))
        .await
        .unwrap()
        .record
}

async fn supersede_by(
    memory: &Memory,
    by: u128,
    old: &DecisionView,
    successor: NewDecision,
) -> Result<Shown<DecisionView>, WorkError> {
    supersede_decision(
        &caller(by),
        open_day(),
        old.id,
        successor,
        old.version,
        memory.ports(),
    )
    .await
}

async fn revoke_by(
    memory: &Memory,
    by: u128,
    decision: &DecisionView,
    reason: &str,
) -> Result<Shown<DecisionView>, WorkError> {
    let revocation = Revocation {
        reason: reason.to_owned(),
        expected_version: decision.version,
    };
    revoke_decision(
        &caller(by),
        open_day(),
        decision.id,
        revocation,
        memory.ports(),
    )
    .await
}

async fn stored(memory: &Memory, id: DecisionId) -> DecisionView {
    get_decision(&caller(VIEWER), open_day(), id, memory.ports())
        .await
        .unwrap()
        .record
}

#[tokio::test]
async fn manager_records_a_decision() {
    let memory = Memory::default();
    let shown = decision_by(&memory, MANAGER, new_decision(OWNER, Some(ground())))
        .await
        .unwrap();
    assert!(shown.can_change && shown.can_end());
    let decision = shown.record;
    assert_eq!(decision.local_id(), "DEC-001");
    assert_eq!(decision.state, DecisionState::InForce);
    assert_eq!(
        decision.wording.as_present().map(DecisionWording::as_str),
        Some(WORDING)
    );
    assert_eq!(decision.fields.owner, user(OWNER));
    assert_eq!(decision.parties.len(), 1);
    assert_eq!(memory.audit(), [AuditAction::DecisionCreate]);

    // The owner is a contributor or a manager, the workstream an active one of the event.
    let result = decision_by(&memory, MANAGER, new_decision(VIEWER, Some(kitchen()))).await;
    assert_eq!(
        fields(result),
        [
            ("owner".to_owned(), "unknown-member"),
            ("workstream".to_owned(), "closed"),
        ]
    );
    let long = NewDecision {
        wording: "a".repeat(2001),
        ..new_decision(OWNER, None)
    };
    let result = decision_by(&memory, MANAGER, long).await;
    assert_eq!(fields(result), field("wording", "too-long"));

    // Each reader sees the decision; only a manager can change or end it.
    let owner = get_decision(&caller(OWNER), open_day(), decision.id, memory.ports())
        .await
        .unwrap();
    assert!(!owner.can_change && !owner.can_end());
    let manager = get_decision(&caller(MANAGER), open_day(), decision.id, memory.ports())
        .await
        .unwrap();
    assert!(manager.can_change && manager.can_end());
}

#[tokio::test]
async fn contributor_cannot_record_a_decision() {
    let memory = Memory::default();
    // Neither the lead of the workstream nor the owner nor a viewer records one. A stranger does not see the event.
    for by in [LEAD, OWNER, VIEWER] {
        let result = decision_by(&memory, by, new_decision(OWNER, Some(ground()))).await;
        assert!(
            matches!(result, Err(WorkError::Forbidden)),
            "{by}: {result:?}"
        );
    }
    let stranger = decision_by(&memory, STRANGER, new_decision(OWNER, None)).await;
    assert!(matches!(stranger, Err(WorkError::NotFound)), "{stranger:?}");
    assert!(memory.audit().is_empty());

    // Nor does the lead or the owner supersede, revoke or change a decision of the workstream.
    let decision = recorded(&memory).await;
    for by in [LEAD, OWNER] {
        let superseded = supersede_by(&memory, by, &decision, new_decision(OWNER, None)).await;
        assert!(
            matches!(superseded, Err(WorkError::Forbidden)),
            "{superseded:?}"
        );
        let revoked = revoke_by(&memory, by, &decision, "Der Vorstand will es.").await;
        assert!(matches!(revoked, Err(WorkError::Forbidden)), "{revoked:?}");
        let update = DecisionUpdate {
            owner: Some(user(LEAD)),
            workstream: None,
            expected_version: decision.version,
        };
        let changed =
            change_decision(&caller(by), open_day(), decision.id, update, memory.ports()).await;
        assert!(matches!(changed, Err(WorkError::Forbidden)), "{changed:?}");
    }
    assert_eq!(stored(&memory, decision.id).await, decision);
    assert_eq!(memory.audit(), [AuditAction::DecisionCreate]);
}

#[tokio::test]
async fn decision_needs_a_party() {
    let memory = Memory::default();
    let nobody = NewDecision {
        decided_by: Vec::new(),
        ..new_decision(OWNER, None)
    };
    let result = decision_by(&memory, MANAGER, nobody).await;
    assert_eq!(fields(result), field("decided_by", "required"));

    // A party of another organization is unknown here.
    let foreign = NewDecision {
        decided_by: vec![Party::Person(supplier()), Party::Person(foreign_person())],
        ..new_decision(OWNER, None)
    };
    let result = decision_by(&memory, MANAGER, foreign).await;
    assert_eq!(fields(result), field("decided_by", "unknown-record"));

    // At most 50 parties; a duplicate counts once.
    let many = NewDecision {
        decided_by: (0..51)
            .map(|n| Party::Person(PersonId::from_uuid(Uuid::from_u128(1000 + n))))
            .collect(),
        ..new_decision(OWNER, None)
    };
    let result = decision_by(&memory, MANAGER, many).await;
    assert_eq!(fields(result), field("decided_by", "too-long"));
    let twice = NewDecision {
        decided_by: vec![Party::Person(supplier()); 60],
        ..new_decision(OWNER, None)
    };
    let decision = decision_by(&memory, MANAGER, twice).await.unwrap().record;
    assert_eq!(decision.parties.len(), 1);
}

#[tokio::test]
async fn wording_cannot_change() {
    let memory = Memory::default();
    let decision = recorded(&memory).await;
    // A manager changes the owner and the workstream; the wording, the date and the parties stay.
    let update = DecisionUpdate {
        owner: Some(user(LEAD)),
        workstream: Some(None),
        expected_version: decision.version,
    };
    let changed = change_decision(
        &caller(MANAGER),
        open_day(),
        decision.id,
        update,
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    assert_eq!(changed.fields.owner, user(LEAD));
    assert_eq!(changed.fields.workstream_id, None);
    assert_eq!(
        (&changed.wording, changed.decided_on, &changed.parties),
        (&decision.wording, decision.decided_on, &decision.parties)
    );
    assert_eq!(changed.version.get(), 2);

    // A change without a field changes nothing; an owner without a role is refused.
    let empty = DecisionUpdate {
        owner: None,
        workstream: None,
        expected_version: changed.version,
    };
    let result = change_decision(
        &caller(MANAGER),
        open_day(),
        decision.id,
        empty,
        memory.ports(),
    )
    .await;
    assert!(fields(result).is_empty());
    let viewer = DecisionUpdate {
        owner: Some(user(VIEWER)),
        workstream: None,
        expected_version: changed.version,
    };
    let result = change_decision(
        &caller(MANAGER),
        open_day(),
        decision.id,
        viewer,
        memory.ports(),
    )
    .await;
    assert_eq!(fields(result), field("owner", "unknown-member"));
    assert_eq!(
        memory.audit(),
        [AuditAction::DecisionCreate, AuditAction::DecisionChange]
    );
}

#[tokio::test]
async fn supersede_creates_the_successor_and_ends_the_old_one() {
    let memory = Memory::default();
    let old = recorded(&memory).await;
    let successor = NewDecision {
        wording: "Das Fest findet nur bei trockenem Wetter statt.".to_owned(),
        ..new_decision(OTHER, None)
    };
    let shown = supersede_by(&memory, MANAGER, &old, successor)
        .await
        .unwrap();
    let new = shown.record;
    assert_eq!(new.local_id(), "DEC-002");
    assert_eq!(new.state, DecisionState::InForce);
    assert_eq!(new.fields.owner, user(OTHER));
    let ended = stored(&memory, old.id).await;
    assert_eq!(ended.state, DecisionState::Superseded { by: new.id });
    assert_eq!(ended.version.get(), 2);
    assert_eq!(
        memory.audit(),
        [
            AuditAction::DecisionCreate,
            AuditAction::DecisionCreate,
            AuditAction::DecisionSupersede,
        ]
    );

    // The old decision ended: a second supersede, with its new version, is no transition.
    let again = supersede_by(&memory, MANAGER, &ended, new_decision(OWNER, None)).await;
    assert!(
        matches!(again, Err(WorkError::InvalidTransition)),
        "{again:?}"
    );
    // A supersede with the version that the caller read before is a conflict and creates nothing.
    let stale = supersede_by(&memory, MANAGER, &old, new_decision(OWNER, None)).await;
    assert!(
        matches!(stale, Err(WorkError::VersionConflict)),
        "{stale:?}"
    );
    assert_eq!(memory.decisions.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn supersede_refuses_self_and_ended_decisions() {
    let memory = Memory::default();
    let decision = recorded(&memory).await;
    // The successor cannot be the decision itself: its ID is taken.
    let itself = NewDecision {
        id: Some(decision.id.as_uuid()),
        ..new_decision(OWNER, None)
    };
    let result = supersede_by(&memory, MANAGER, &decision, itself).await;
    assert_eq!(fields(result), field("id", "taken"));
    assert_eq!(stored(&memory, decision.id).await, decision);
    // A revoked decision has ended: it takes no successor.
    let revoked = revoke_by(&memory, MANAGER, &decision, "Die Gemeinde verbietet es.")
        .await
        .unwrap()
        .record;
    let ended = supersede_by(&memory, MANAGER, &revoked, new_decision(OWNER, None)).await;
    assert!(
        matches!(ended, Err(WorkError::InvalidTransition)),
        "{ended:?}"
    );
    assert_eq!(memory.decisions.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn revoke_needs_a_reason() {
    let memory = Memory::default();
    let decision = recorded(&memory).await;
    let empty = revoke_by(&memory, MANAGER, &decision, "  ").await;
    assert_eq!(fields(empty), field("reason", "empty"));
    let long = revoke_by(&memory, MANAGER, &decision, &"a".repeat(501)).await;
    assert_eq!(fields(long), field("reason", "too-long"));
    assert_eq!(stored(&memory, decision.id).await, decision);

    let revoked = revoke_by(&memory, MANAGER, &decision, "Die Gemeinde verbietet es.")
        .await
        .unwrap();
    assert!(revoked.can_change && !revoked.can_end());
    let DecisionState::Revoked {
        reason: Redactable::Present(reason),
    } = &revoked.record.state
    else {
        panic!("not revoked: {:?}", revoked.record.state);
    };
    assert_eq!(reason.as_str(), "Die Gemeinde verbietet es.");
    // A revoked decision is final.
    let again = revoke_by(&memory, MANAGER, &revoked.record, "Noch einmal.").await;
    assert!(
        matches!(again, Err(WorkError::InvalidTransition)),
        "{again:?}"
    );
    assert_eq!(
        memory.audit(),
        [AuditAction::DecisionCreate, AuditAction::DecisionRevoke]
    );
}
