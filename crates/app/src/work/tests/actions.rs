use super::*;

#[tokio::test]
async fn a_contributor_creates_an_action_for_another_contributor() {
    let memory = Memory::default();
    let action = action_by(&memory, OTHER, new_action(OWNER, Some(ground())))
        .await
        .unwrap();
    assert_eq!(action.fields.owner, user(OWNER));
    assert_eq!(action.fields.status, ActionStatus::Open);
    assert_eq!(action.fields.workstream_id, Some(ground()));
    assert_eq!(action.version, RecordVersion::FIRST);
    assert_eq!(action.local_id(), "ACT-001");
    assert_eq!(memory.audit(), [AuditAction::ActionCreate]);

    // The owner must be a contributor or manager of the event.
    for owner in [VIEWER, STRANGER] {
        let result = action_by(&memory, OTHER, new_action(owner, None)).await;
        assert_eq!(fields(result), field("owner", "unknown-member"), "{owner}");
    }
    // A change of the owner obeys the same rule.
    let change = ActionChange {
        owner: Some(user(VIEWER)),
        ..no_change(action.version)
    };
    let result = change_action_by(&memory, OWNER, action.id, change).await;
    assert_eq!(fields(result), field("owner", "unknown-member"));
}

#[tokio::test]
async fn a_viewer_cannot_create_an_action() {
    let memory = Memory::default();
    let viewer = action_by(&memory, VIEWER, new_action(OWNER, None)).await;
    assert!(matches!(viewer, Err(WorkError::Forbidden)), "{viewer:?}");
    let stranger = action_by(&memory, STRANGER, new_action(OWNER, None)).await;
    assert!(matches!(stranger, Err(WorkError::NotFound)), "{stranger:?}");
    let commitment = create_commitment(
        &caller(VIEWER),
        open_day(),
        new_commitment(None),
        memory.ports(),
    )
    .await;
    assert!(matches!(commitment, Err(WorkError::Forbidden)));
    assert!(memory.audit().is_empty());
}

#[tokio::test]
async fn the_owner_the_lead_and_a_manager_change_an_action_and_nobody_else() {
    let memory = Memory::default();
    let action = action_by(&memory, OWNER, new_action(OWNER, Some(ground())))
        .await
        .unwrap();
    let mut version = action.version;
    for by in [OWNER, LEAD, MANAGER] {
        let change = ActionChange {
            title: Some(format!("Generator bestellen {by}")),
            ..no_change(version)
        };
        version = change_action_by(&memory, by, action.id, change)
            .await
            .unwrap()
            .version;
    }
    assert_eq!(version.get(), 4);
    for by in [OTHER, VIEWER] {
        let change = ActionChange {
            title: Some("X".to_owned()),
            ..no_change(version)
        };
        let result = change_action_by(&memory, by, action.id, change).await;
        assert!(
            matches!(result, Err(WorkError::Forbidden)),
            "{by}: {result:?}"
        );
    }
    let stranger = change_action_by(&memory, STRANGER, action.id, no_change(version)).await;
    assert!(matches!(stranger, Err(WorkError::NotFound)));

    // Without a workstream, the lead is not a lead of the action.
    let free = action_by(&memory, OWNER, new_action(OWNER, None))
        .await
        .unwrap();
    let change = ActionChange {
        title: Some("X".to_owned()),
        ..no_change(free.version)
    };
    let lead = change_action_by(&memory, LEAD, free.id, change).await;
    assert!(matches!(lead, Err(WorkError::Forbidden)));
}

/// The view tells each reader whether it can change the record and to which statuses (ADR 0067, ADR 0068),
/// so that a client does not repeat the rules.
#[tokio::test]
async fn the_view_tells_the_reader_its_right_and_the_next_statuses() {
    let memory = Memory::default();
    let action = action_by(&memory, OWNER, new_action(OWNER, Some(ground())))
        .await
        .unwrap();
    for (reader, can_change) in [
        (OWNER, true),
        (LEAD, true),
        (MANAGER, true),
        (OTHER, false),
        (VIEWER, false),
    ] {
        let shown = get_action(&caller(reader), open_day(), action.id, memory.ports())
            .await
            .unwrap();
        assert_eq!(shown.can_change, can_change, "{reader}");
        let next = if can_change {
            vec![
                ActionStatus::InProgress,
                ActionStatus::Blocked,
                ActionStatus::Done,
                ActionStatus::Canceled,
            ]
        } else {
            Vec::new()
        };
        assert_eq!(shown.next_statuses(), next, "{reader}");
    }

    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(Some("subject to signed order")),
        memory.ports(),
    )
    .await
    .unwrap();
    assert!(commitment.can_change);
    // "Make firm" sets `firm`, so a change does not offer it.
    assert_eq!(
        commitment.next_statuses(),
        [
            CommitmentStatus::Fulfilled,
            CommitmentStatus::Broken,
            CommitmentStatus::Withdrawn
        ]
    );
    let by_viewer = get_commitment(
        &caller(VIEWER),
        open_day(),
        commitment.record.id,
        memory.ports(),
    )
    .await
    .unwrap();
    assert!(!by_viewer.can_change);
    assert!(by_viewer.next_statuses().is_empty());
    // Only a conditional commitment can become firm, and only for a caller who can change it.
    assert!(commitment.can_make_firm());
    assert!(!by_viewer.can_make_firm());
    let firm = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(None),
        memory.ports(),
    )
    .await
    .unwrap();
    assert!(firm.can_change);
    assert!(!firm.can_make_firm());
}

#[test]
fn may_change_work_follows_the_permission_table() {
    let (owner, lead, other) = (user(OWNER), user(LEAD), user(OTHER));
    assert!(may_change_work(EventLevel::Contributor, owner, owner, None));
    assert!(may_change_work(
        EventLevel::Contributor,
        lead,
        owner,
        Some(lead)
    ));
    assert!(may_change_work(EventLevel::Manager, other, owner, None));
    assert!(!may_change_work(
        EventLevel::Contributor,
        other,
        owner,
        Some(lead)
    ));
    // A viewer changes nothing, not even a record that it owned before.
    assert!(!may_change_work(
        EventLevel::Viewer,
        owner,
        owner,
        Some(owner)
    ));
}

#[tokio::test]
async fn an_unknown_record_is_not_found_before_its_fields_are_checked() {
    let memory = Memory::default();
    let missing = ActionId::from_uuid(Uuid::now_v7());
    let change = ActionChange {
        title: Some(String::new()),
        ..no_change(RecordVersion::FIRST)
    };
    let result = change_action_by(&memory, MANAGER, missing, change).await;
    assert!(matches!(result, Err(WorkError::NotFound)), "{result:?}");
}

#[tokio::test]
async fn a_change_without_a_field_is_invalid() {
    let memory = Memory::default();
    let action = action_by(&memory, OWNER, new_action(OWNER, None))
        .await
        .unwrap();
    let result = change_action_by(&memory, OWNER, action.id, no_change(action.version)).await;
    assert_eq!(fields(result), []);
    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(None),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    let result = change_commitment_by(
        &memory,
        OWNER,
        commitment.id,
        commitment_change(commitment.version),
    )
    .await;
    assert_eq!(fields(result), []);
    assert_eq!(
        memory.audit(),
        [AuditAction::ActionCreate, AuditAction::CommitmentCreate]
    );
}

#[tokio::test]
async fn an_invalid_status_change_is_refused() {
    let memory = Memory::default();
    let action = action_by(&memory, OWNER, new_action(OWNER, None))
        .await
        .unwrap();
    let status = |status, version| ActionChange {
        status: Some(status),
        ..no_change(version)
    };
    let canceled = change_action_by(
        &memory,
        OWNER,
        action.id,
        status(ActionStatus::Canceled, action.version),
    )
    .await
    .unwrap();
    let reopen = change_action_by(
        &memory,
        OWNER,
        action.id,
        status(ActionStatus::Open, canceled.version),
    )
    .await;
    assert!(
        matches!(reopen, Err(WorkError::InvalidTransition)),
        "{reopen:?}"
    );

    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(None),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    let back = change_commitment_by(
        &memory,
        OWNER,
        commitment.id,
        CommitmentChange {
            status: Some(CommitmentStatus::Conditional),
            ..commitment_change(commitment.version)
        },
    )
    .await;
    assert!(matches!(back, Err(WorkError::InvalidTransition)));
}

/// A change to the current status is no transition (ADR 0068), as for a proposal: it changes nothing.
#[tokio::test]
async fn a_change_to_the_same_status_is_refused() {
    let memory = Memory::default();
    let action = action_by(&memory, OWNER, new_action(OWNER, None))
        .await
        .unwrap();
    let same = change_action_by(
        &memory,
        OWNER,
        action.id,
        ActionChange {
            status: Some(ActionStatus::Open),
            ..no_change(action.version)
        },
    )
    .await;
    assert!(
        matches!(same, Err(WorkError::InvalidTransition)),
        "{same:?}"
    );

    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(Some("subject to signed order")),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    let same = change_commitment_by(
        &memory,
        OWNER,
        commitment.id,
        CommitmentChange {
            status: Some(CommitmentStatus::Conditional),
            ..commitment_change(commitment.version)
        },
    )
    .await;
    assert!(
        matches!(same, Err(WorkError::InvalidTransition)),
        "{same:?}"
    );
    assert_eq!(
        memory.audit(),
        [AuditAction::ActionCreate, AuditAction::CommitmentCreate]
    );
}

#[tokio::test]
async fn a_closed_workstream_cannot_be_set() {
    let memory = Memory::default();
    let closed = action_by(&memory, OWNER, new_action(OWNER, Some(kitchen()))).await;
    assert_eq!(fields(closed), field("workstream", "closed"));
    let unknown = action_by(
        &memory,
        OWNER,
        new_action(OWNER, Some(WorkstreamId::from_uuid(Uuid::now_v7()))),
    )
    .await;
    assert_eq!(fields(unknown), field("workstream", "unknown-record"));
    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        NewCommitment {
            workstream: Some(kitchen()),
            ..new_commitment(None)
        },
        memory.ports(),
    )
    .await;
    assert_eq!(fields(commitment), field("workstream", "closed"));

    let action = action_by(&memory, OWNER, new_action(OWNER, Some(ground())))
        .await
        .unwrap();
    let move_to_kitchen = ActionChange {
        workstream: Some(Some(kitchen())),
        ..no_change(action.version)
    };
    let result = change_action_by(&memory, OWNER, action.id, move_to_kitchen).await;
    assert_eq!(fields(result), field("workstream", "closed"));

    // A record keeps its workstream after the workstream closes, and other changes still work.
    memory.workstreams.lock().unwrap()[0].status = WorkstreamStatus::Closed;
    let change = ActionChange {
        title: Some("Generator bestellt".to_owned()),
        workstream: Some(Some(ground())),
        ..no_change(action.version)
    };
    let changed = change_action_by(&memory, OWNER, action.id, change)
        .await
        .unwrap();
    assert_eq!(changed.fields.workstream_id, Some(ground()));
}

#[tokio::test]
async fn a_stale_version_conflicts() {
    let memory = Memory::default();
    let action = action_by(&memory, OWNER, new_action(OWNER, None))
        .await
        .unwrap();
    let change = |version| ActionChange {
        due_date: Some(Some(jiff::civil::date(2030, 6, 1))),
        ..no_change(version)
    };
    change_action_by(&memory, OWNER, action.id, change(action.version))
        .await
        .unwrap();
    let stale = change_action_by(&memory, OWNER, action.id, change(action.version)).await;
    assert!(matches!(stale, Err(WorkError::VersionConflict)));
}

#[tokio::test]
async fn a_list_pages_and_filters_by_owner() {
    let memory = Memory::default();
    for owner in [OWNER, LEAD, OWNER] {
        action_by(&memory, OWNER, new_action(owner, None))
            .await
            .unwrap();
    }
    let query = WorkQuery {
        owner: Some(user(OWNER)),
        status: None,
        workstream: None,
        after: None,
        limit: PageLimit::new(1).unwrap(),
    };
    let first = list_actions(&caller(VIEWER), open_day(), query, memory.ports())
        .await
        .unwrap();
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.next, Some(NumberCursor(1)));
    let second = list_actions(
        &caller(VIEWER),
        open_day(),
        WorkQuery {
            after: first.next,
            ..query
        },
        memory.ports(),
    )
    .await
    .unwrap();
    assert_eq!(second.items[0].record.local_number, 3);
    assert_eq!(second.next, None);
}
