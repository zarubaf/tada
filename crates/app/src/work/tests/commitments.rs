use super::*;

#[tokio::test]
async fn a_conditional_commitment_stays_conditional_after_a_change() {
    let memory = Memory::default();
    let commitment = create_commitment(
        &caller(OTHER),
        open_day(),
        new_commitment(Some("subject to signed order")),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    assert_eq!(commitment.fields.status, CommitmentStatus::Conditional);
    assert_eq!(commitment.local_id(), "COM-001");
    let change = CommitmentChange {
        text: Some("Generator delivery Saturday 09:00".to_owned()),
        due_date: Some(Some(jiff::civil::date(2030, 6, 1))),
        owner: Some(user(LEAD)),
        ..commitment_change(commitment.version)
    };
    let changed = change_commitment_by(&memory, OWNER, commitment.id, change)
        .await
        .unwrap();
    assert_eq!(changed.fields.status, CommitmentStatus::Conditional);
    assert_eq!(changed.fields.owner, user(LEAD));
    assert_eq!(
        changed.condition.as_ref().map(ConditionText::as_str),
        Some("subject to signed order")
    );
    assert_eq!(changed.fields.firm_reason, None);
}

#[tokio::test]
async fn only_make_firm_makes_a_commitment_firm() {
    let memory = Memory::default();
    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(Some("subject to signed order")),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    let firm = CommitmentChange {
        status: Some(CommitmentStatus::Firm),
        ..commitment_change(commitment.version)
    };
    let refused = change_commitment_by(&memory, OWNER, commitment.id, firm).await;
    assert!(matches!(refused, Err(WorkError::InvalidTransition)));

    let made = firm_by(
        &memory,
        OWNER,
        commitment.id,
        "Order signed",
        commitment.version,
    )
    .await
    .unwrap();
    assert_eq!(made.fields.status, CommitmentStatus::Firm);
    assert_eq!(
        made.fields.firm_reason.as_ref().map(RecordReason::as_str),
        Some("Order signed")
    );
    assert_eq!(
        made.condition.as_ref().map(ConditionText::as_str),
        Some("subject to signed order")
    );
    let again = firm_by(&memory, OWNER, commitment.id, "Again", made.version).await;
    assert!(matches!(again, Err(WorkError::InvalidTransition)));
    assert_eq!(
        memory.audit(),
        [AuditAction::CommitmentCreate, AuditAction::CommitmentFirm]
    );

    // A commitment without a condition starts firm and has no reason.
    let plain = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(None),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    assert_eq!(plain.fields.status, CommitmentStatus::Firm);
    assert_eq!(plain.fields.firm_reason, None);
}

#[tokio::test]
async fn make_firm_needs_a_reason() {
    let memory = Memory::default();
    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(Some("subject to signed order")),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    let empty = firm_by(&memory, OWNER, commitment.id, "  ", commitment.version).await;
    assert_eq!(fields(empty), field("reason", "empty"));
    let long = firm_by(
        &memory,
        OWNER,
        commitment.id,
        &"a".repeat(501),
        commitment.version,
    )
    .await;
    assert_eq!(fields(long), field("reason", "too-long"));
    let other = firm_by(&memory, OTHER, commitment.id, "Signed", commitment.version).await;
    assert!(matches!(other, Err(WorkError::Forbidden)));
    assert_eq!(memory.audit(), [AuditAction::CommitmentCreate]);
}

#[tokio::test]
async fn a_promisor_of_another_organization_is_unknown() {
    let memory = Memory::default();
    for promisor in [
        Party::Person(foreign_person()),
        Party::Person(PersonId::from_uuid(Uuid::now_v7())),
        Party::Institution(InstitutionId::from_uuid(Uuid::now_v7())),
    ] {
        let input = NewCommitment {
            promisor,
            ..new_commitment(None)
        };
        let result = create_commitment(&caller(OWNER), open_day(), input, memory.ports()).await;
        assert_eq!(fields(result), field("promisor", "unknown-record"));
    }
    assert!(memory.audit().is_empty());
}
