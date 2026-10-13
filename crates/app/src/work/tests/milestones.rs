use super::*;

fn new_milestone(owner: u128, workstream: Option<WorkstreamId>) -> NewMilestone {
    NewMilestone {
        id: None,
        title: "Bewilligung liegt vor".to_owned(),
        description: None,
        date: jiff::civil::date(2030, 5, 1),
        owner: user(owner),
        workstream,
    }
}

fn milestone_change(expected_version: RecordVersion) -> MilestoneChange {
    MilestoneChange {
        title: None,
        description: None,
        date: None,
        owner: None,
        workstream: None,
        status: None,
        expected_version,
    }
}

async fn milestone_by(
    memory: &Memory,
    by: u128,
    input: NewMilestone,
) -> Result<MilestoneView, WorkError> {
    create_milestone(&caller(by), open_day(), input, memory.ports())
        .await
        .map(|shown| shown.record)
}

async fn change_milestone_by(
    memory: &Memory,
    by: u128,
    id: MilestoneId,
    change: MilestoneChange,
) -> Result<MilestoneView, WorkError> {
    change_milestone(&caller(by), open_day(), id, change, memory.ports())
        .await
        .map(|shown| shown.record)
}

async fn milestone_status(
    memory: &Memory,
    id: MilestoneId,
    status: MilestoneStatus,
    expected_version: RecordVersion,
) -> Result<MilestoneView, WorkError> {
    let change = MilestoneChange {
        status: Some(status),
        ..milestone_change(expected_version)
    };
    change_milestone_by(memory, OWNER, id, change).await
}

#[tokio::test]
async fn contributor_creates_a_milestone() {
    let memory = Memory::default();
    let milestone = milestone_by(&memory, OTHER, new_milestone(OWNER, Some(ground())))
        .await
        .unwrap();
    assert_eq!(milestone.fields.owner, user(OWNER));
    assert_eq!(milestone.fields.status, MilestoneStatus::Open);
    assert_eq!(milestone.fields.date, jiff::civil::date(2030, 5, 1));
    assert_eq!(milestone.local_id(), "MIL-001");
    assert_eq!(memory.audit(), [AuditAction::MilestoneCreate]);

    // The owner is a contributor or a manager, the workstream an active one of the event.
    let result = milestone_by(&memory, OTHER, new_milestone(VIEWER, Some(kitchen()))).await;
    assert_eq!(
        fields(result),
        [
            ("owner".to_owned(), "unknown-member"),
            ("workstream".to_owned(), "closed"),
        ]
    );
    let long = NewMilestone {
        title: "a".repeat(201),
        ..new_milestone(OWNER, None)
    };
    let result = milestone_by(&memory, OTHER, long).await;
    assert_eq!(fields(result), field("title", "too-long"));
}

#[tokio::test]
async fn viewer_cannot_create_a_milestone() {
    let memory = Memory::default();
    let viewer = milestone_by(&memory, VIEWER, new_milestone(OWNER, None)).await;
    assert!(matches!(viewer, Err(WorkError::Forbidden)), "{viewer:?}");
    let stranger = milestone_by(&memory, STRANGER, new_milestone(OWNER, None)).await;
    assert!(matches!(stranger, Err(WorkError::NotFound)), "{stranger:?}");
    assert!(memory.audit().is_empty());
}

#[tokio::test]
async fn owner_reaches_and_reopens_a_milestone() {
    let memory = Memory::default();
    let milestone = milestone_by(&memory, OWNER, new_milestone(OWNER, Some(ground())))
        .await
        .unwrap();
    let reached = milestone_status(
        &memory,
        milestone.id,
        MilestoneStatus::Reached,
        milestone.version,
    )
    .await
    .unwrap();
    assert_eq!(reached.fields.status, MilestoneStatus::Reached);
    let open = milestone_status(
        &memory,
        milestone.id,
        MilestoneStatus::Open,
        reached.version,
    )
    .await
    .unwrap();
    assert_eq!(open.fields.status, MilestoneStatus::Open);
    assert_eq!(
        memory.audit(),
        [
            AuditAction::MilestoneCreate,
            AuditAction::MilestoneChange,
            AuditAction::MilestoneChange
        ]
    );

    // The lead and a manager change it too; another contributor and a viewer do not.
    let mut version = open.version;
    for by in [LEAD, MANAGER] {
        let change = MilestoneChange {
            date: Some(jiff::civil::date(2030, 5, 2)),
            ..milestone_change(version)
        };
        version = change_milestone_by(&memory, by, milestone.id, change)
            .await
            .unwrap()
            .version;
    }
    for by in [OTHER, VIEWER] {
        let change = MilestoneChange {
            title: Some("X".to_owned()),
            ..milestone_change(version)
        };
        let result = change_milestone_by(&memory, by, milestone.id, change).await;
        assert!(
            matches!(result, Err(WorkError::Forbidden)),
            "{by}: {result:?}"
        );
    }

    // The view tells the owner the next statuses.
    let shown = get_milestone(&caller(OWNER), open_day(), milestone.id, memory.ports())
        .await
        .unwrap();
    assert_eq!(
        shown.next_statuses(),
        [MilestoneStatus::Reached, MilestoneStatus::Canceled]
    );
    let shown = get_milestone(&caller(VIEWER), open_day(), milestone.id, memory.ports())
        .await
        .unwrap();
    assert!(shown.next_statuses().is_empty());
}

#[tokio::test]
async fn canceled_milestone_is_final() {
    let memory = Memory::default();
    let milestone = milestone_by(&memory, OWNER, new_milestone(OWNER, None))
        .await
        .unwrap();
    let canceled = milestone_status(
        &memory,
        milestone.id,
        MilestoneStatus::Canceled,
        milestone.version,
    )
    .await
    .unwrap();
    for status in MilestoneStatus::ALL {
        let result = milestone_status(&memory, milestone.id, status, canceled.version).await;
        assert!(
            matches!(result, Err(WorkError::InvalidTransition)),
            "{status:?}: {result:?}"
        );
    }
    let page = list_milestones(
        &caller(VIEWER),
        open_day(),
        WorkQuery {
            owner: None,
            status: Some(MilestoneStatus::Canceled),
            workstream: None,
            after: None,
            limit: crate::paging::PageLimit::LARGEST,
        },
        memory.ports(),
    )
    .await
    .unwrap();
    assert_eq!(page.items.len(), 1);
}

#[tokio::test]
async fn action_start_after_due_is_refused() {
    let memory = Memory::default();
    let (may, june) = (jiff::civil::date(2030, 5, 1), jiff::civil::date(2030, 6, 1));
    let late_start = NewAction {
        start_date: Some(june),
        due_date: Some(may),
        ..new_action(OWNER, None)
    };
    let result = action_by(&memory, OWNER, late_start).await;
    assert_eq!(fields(result), field("start_date", "start-after-due"));

    // The same day is in order. A change checks the dates that the action has after the change.
    let action = action_by(
        &memory,
        OWNER,
        NewAction {
            start_date: Some(may),
            due_date: Some(may),
            ..new_action(OWNER, None)
        },
    )
    .await
    .unwrap();
    assert_eq!(action.fields.start_date, Some(may));
    let earlier_due = ActionChange {
        due_date: Some(Some(jiff::civil::date(2030, 4, 1))),
        ..no_change(action.version)
    };
    let result = change_action_by(&memory, OWNER, action.id, earlier_due).await;
    assert_eq!(fields(result), field("start_date", "start-after-due"));
    let moved = ActionChange {
        start_date: Some(Some(june)),
        due_date: Some(None),
        ..no_change(action.version)
    };
    let moved = change_action_by(&memory, OWNER, action.id, moved)
        .await
        .unwrap();
    assert_eq!(
        (moved.fields.start_date, moved.fields.due_date),
        (Some(june), None)
    );
}

/// A milestone of another event: the store of the event does not find it.
fn foreign_milestone() -> MilestoneId {
    MilestoneId::from_uuid(Uuid::from_u128(300))
}

#[tokio::test]
async fn action_cannot_name_a_milestone_of_another_event() {
    let memory = Memory::default();
    let foreign = NewAction {
        milestone: Some(foreign_milestone()),
        ..new_action(OWNER, None)
    };
    let result = action_by(&memory, OWNER, foreign).await;
    assert_eq!(fields(result), field("milestone", "unknown-record"));

    let milestone = milestone_by(&memory, OWNER, new_milestone(OWNER, None))
        .await
        .unwrap();
    let action = action_by(
        &memory,
        OWNER,
        NewAction {
            milestone: Some(milestone.id),
            ..new_action(OWNER, None)
        },
    )
    .await
    .unwrap();
    assert_eq!(action.fields.milestone_id, Some(milestone.id));
    let change = ActionChange {
        milestone: Some(Some(foreign_milestone())),
        ..no_change(action.version)
    };
    let result = change_action_by(&memory, OWNER, action.id, change).await;
    assert_eq!(fields(result), field("milestone", "unknown-record"));
    let cleared = ActionChange {
        milestone: Some(None),
        ..no_change(action.version)
    };
    let cleared = change_action_by(&memory, OWNER, action.id, cleared)
        .await
        .unwrap();
    assert_eq!(cleared.fields.milestone_id, None);
}

#[tokio::test]
async fn action_cannot_name_a_canceled_milestone() {
    let memory = Memory::default();
    let milestone = milestone_by(&memory, OWNER, new_milestone(OWNER, None))
        .await
        .unwrap();
    let action = action_by(
        &memory,
        OWNER,
        NewAction {
            milestone: Some(milestone.id),
            ..new_action(OWNER, None)
        },
    )
    .await
    .unwrap();
    milestone_status(
        &memory,
        milestone.id,
        MilestoneStatus::Canceled,
        milestone.version,
    )
    .await
    .unwrap();
    let result = action_by(
        &memory,
        OWNER,
        NewAction {
            milestone: Some(milestone.id),
            ..new_action(OWNER, None)
        },
    )
    .await;
    assert_eq!(fields(result), field("milestone", "unknown-record"));
    // The existing link stays, and a change of another field keeps it.
    let change = ActionChange {
        title: Some("Zelt bestellen".to_owned()),
        ..no_change(action.version)
    };
    let changed = change_action_by(&memory, OWNER, action.id, change)
        .await
        .unwrap();
    assert_eq!(changed.fields.milestone_id, Some(milestone.id));
}
