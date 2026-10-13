use tada_domain::records::Level::{self, High, Low, Medium};
use tada_domain::records::RiskRating;

use super::*;
use crate::paging::Page;

fn rating(likelihood: Level, impact: Level) -> RiskRating {
    RiskRating { likelihood, impact }
}

fn new_risk(owner: u128, workstream: Option<WorkstreamId>) -> NewRisk {
    NewRisk {
        id: None,
        title: "Starkregen am Festtag".to_owned(),
        description: Some("Das Zelt steht auf der Wiese.".to_owned()),
        likelihood: Medium,
        impact: High,
        mitigation: Some("Zweiter Eingang\nund Kies auf dem Weg".to_owned()),
        owner: user(owner),
        workstream,
    }
}

fn risk_change(expected_version: RecordVersion) -> RiskChange {
    RiskChange {
        title: None,
        description: None,
        likelihood: None,
        impact: None,
        mitigation: None,
        owner: None,
        workstream: None,
        status: None,
        expected_version,
    }
}

async fn risk_by(memory: &Memory, by: u128, input: NewRisk) -> Result<RiskView, WorkError> {
    create_risk(&caller(by), open_day(), input, memory.ports())
        .await
        .map(|shown| shown.record)
}

async fn change_risk_by(
    memory: &Memory,
    by: u128,
    id: RiskId,
    change: RiskChange,
) -> Result<RiskView, WorkError> {
    change_risk(&caller(by), open_day(), id, change, memory.ports())
        .await
        .map(|shown| shown.record)
}

async fn risk_status(
    memory: &Memory,
    id: RiskId,
    status: RiskStatus,
    expected_version: RecordVersion,
) -> Result<RiskView, WorkError> {
    let change = RiskChange {
        status: Some(status),
        ..risk_change(expected_version)
    };
    change_risk_by(memory, OWNER, id, change).await
}

#[tokio::test]
async fn contributor_records_a_risk() {
    let memory = Memory::default();
    let risk = risk_by(&memory, OTHER, new_risk(OWNER, Some(ground())))
        .await
        .unwrap();
    assert_eq!(risk.fields.owner, user(OWNER));
    assert_eq!(risk.fields.status, RiskStatus::Open);
    assert_eq!(risk.fields.rating, rating(Medium, High));
    assert_eq!(risk.local_id(), "RSK-001");
    assert_eq!(memory.audit(), [AuditAction::RiskCreate]);

    // The owner is a contributor or a manager, the workstream an active one of the event.
    let result = risk_by(&memory, OTHER, new_risk(VIEWER, Some(kitchen()))).await;
    assert_eq!(
        fields(result),
        [
            ("owner".to_owned(), "unknown-member"),
            ("workstream".to_owned(), "closed"),
        ]
    );
    // The limits of the texts: 200 for the title, 4000 for the description and the mitigation.
    let long = NewRisk {
        title: "a".repeat(201),
        description: Some("b".repeat(4001)),
        mitigation: Some("c".repeat(4001)),
        ..new_risk(OWNER, None)
    };
    let result = risk_by(&memory, OTHER, long).await;
    assert_eq!(
        fields(result),
        [
            ("title".to_owned(), "too-long"),
            ("description".to_owned(), "too-long"),
            ("mitigation".to_owned(), "too-long"),
        ]
    );
    let empty = NewRisk {
        mitigation: Some(String::new()),
        ..new_risk(OWNER, None)
    };
    let result = risk_by(&memory, OTHER, empty).await;
    assert_eq!(fields(result), field("mitigation", "empty"));
}

#[tokio::test]
async fn viewer_cannot_record_a_risk() {
    let memory = Memory::default();
    let viewer = risk_by(&memory, VIEWER, new_risk(OWNER, None)).await;
    assert!(matches!(viewer, Err(WorkError::Forbidden)), "{viewer:?}");
    let stranger = risk_by(&memory, STRANGER, new_risk(OWNER, None)).await;
    assert!(matches!(stranger, Err(WorkError::NotFound)), "{stranger:?}");
    assert!(memory.audit().is_empty());
}

#[tokio::test]
async fn closed_risk_reopens() {
    let memory = Memory::default();
    let risk = risk_by(&memory, OWNER, new_risk(OWNER, Some(ground())))
        .await
        .unwrap();
    // open -> mitigated -> open -> occurred -> closed -> open
    let mut version = risk.version;
    for status in [
        RiskStatus::Mitigated,
        RiskStatus::Open,
        RiskStatus::Occurred,
        RiskStatus::Closed,
        RiskStatus::Open,
    ] {
        let changed = risk_status(&memory, risk.id, status, version)
            .await
            .unwrap();
        assert_eq!(changed.fields.status, status);
        // A status change keeps the rating.
        assert_eq!(changed.fields.rating, rating(Medium, High));
        version = changed.version;
    }
    // occurred does not go back to open or mitigated; the same status is no change.
    let occurred = risk_status(&memory, risk.id, RiskStatus::Occurred, version)
        .await
        .unwrap();
    for status in [
        RiskStatus::Open,
        RiskStatus::Mitigated,
        RiskStatus::Occurred,
    ] {
        let result = risk_status(&memory, risk.id, status, occurred.version).await;
        assert!(
            matches!(result, Err(WorkError::InvalidTransition)),
            "{status:?}: {result:?}"
        );
    }
}

#[tokio::test]
async fn the_owner_sees_the_next_statuses_of_a_risk() {
    let memory = Memory::default();
    let risk = risk_by(&memory, OWNER, new_risk(OWNER, None))
        .await
        .unwrap();
    let shown = get_risk(&caller(OWNER), open_day(), risk.id, memory.ports())
        .await
        .unwrap();
    assert_eq!(
        shown.next_statuses(),
        [
            RiskStatus::Mitigated,
            RiskStatus::Occurred,
            RiskStatus::Closed
        ]
    );
    let shown = get_risk(&caller(VIEWER), open_day(), risk.id, memory.ports())
        .await
        .unwrap();
    assert!(shown.next_statuses().is_empty());
}

async fn risk_by_id(memory: &Memory, id: RiskId) -> RiskView {
    get_risk(&caller(MANAGER), open_day(), id, memory.ports())
        .await
        .unwrap()
        .record
}

#[tokio::test]
async fn owner_lead_and_manager_change_a_risk_and_others_do_not() {
    let memory = Memory::default();
    let risk = risk_by(&memory, OWNER, new_risk(OWNER, Some(ground())))
        .await
        .unwrap();
    let mut version = risk.version;
    for by in [OWNER, LEAD, MANAGER] {
        let before = risk_by_id(&memory, risk.id).await.fields.rating;
        let change = RiskChange {
            likelihood: Some(High),
            ..risk_change(version)
        };
        let changed = change_risk_by(&memory, by, risk.id, change).await.unwrap();
        // The impact stays when only the likelihood changes.
        assert_eq!(changed.fields.rating.impact, before.impact, "{by}");
        assert_eq!(changed.fields.rating.likelihood, High, "{by}");
        version = changed.version;
        let back = RiskChange {
            likelihood: Some(Low),
            impact: Some(Medium),
            ..risk_change(version)
        };
        let changed = change_risk_by(&memory, by, risk.id, back).await.unwrap();
        assert_eq!(changed.fields.rating, rating(Low, Medium), "{by}");
        version = changed.version;
    }
    for by in [OTHER, VIEWER] {
        let change = RiskChange {
            title: Some("X".to_owned()),
            ..risk_change(version)
        };
        let result = change_risk_by(&memory, by, risk.id, change).await;
        assert!(
            matches!(result, Err(WorkError::Forbidden)),
            "{by}: {result:?}"
        );
    }
    assert_eq!(
        memory.audit().first(),
        Some(&AuditAction::RiskCreate),
        "{:?}",
        memory.audit()
    );
    assert!(
        memory.audit()[1..]
            .iter()
            .all(|a| *a == AuditAction::RiskChange)
    );
}

#[tokio::test]
async fn change_of_the_texts_checks_the_limits_and_the_version() {
    let memory = Memory::default();
    let risk = risk_by(&memory, OWNER, new_risk(OWNER, None))
        .await
        .unwrap();
    let nothing = change_risk_by(&memory, OWNER, risk.id, risk_change(risk.version)).await;
    assert!(matches!(&nothing, Err(WorkError::Invalid(errors)) if errors.is_empty()));
    let stale = RiskChange {
        title: Some("Hagel".to_owned()),
        ..risk_change(next(risk.version))
    };
    let result = change_risk_by(&memory, OWNER, risk.id, stale).await;
    assert!(
        matches!(result, Err(WorkError::VersionConflict)),
        "{result:?}"
    );
    let too_long = RiskChange {
        mitigation: Some(Some("m".repeat(4001))),
        ..risk_change(risk.version)
    };
    let result = change_risk_by(&memory, OWNER, risk.id, too_long).await;
    assert_eq!(fields(result), field("mitigation", "too-long"));
    // `Some(None)` clears the optional texts.
    let clear = RiskChange {
        description: Some(None),
        mitigation: Some(None),
        ..risk_change(risk.version)
    };
    let cleared = change_risk_by(&memory, OWNER, risk.id, clear)
        .await
        .unwrap();
    assert_eq!(
        (cleared.fields.description, cleared.fields.mitigation),
        (None, None)
    );
}

fn query(after: Option<RiskCursor>, limit: u32) -> WorkQuery<RiskStatus, RiskCursor> {
    WorkQuery {
        owner: None,
        status: None,
        workstream: None,
        after,
        limit: crate::paging::PageLimit::new(limit).unwrap(),
    }
}

#[tokio::test]
async fn risk_pages_follow_the_order_of_the_rating() {
    let memory = Memory::default();
    // RSK-001 low x low (1), RSK-002 high x low (3), RSK-003 medium x medium (4), RSK-004 high x high (9).
    for (likelihood, impact) in [(Low, Low), (High, Low), (Medium, Medium), (High, High)] {
        let input = NewRisk {
            likelihood,
            impact,
            ..new_risk(OWNER, None)
        };
        risk_by(&memory, OWNER, input).await.unwrap();
    }
    let ids = |page: &Page<Shown<RiskView>, RiskCursor>| -> Vec<String> {
        page.items
            .iter()
            .map(|shown| shown.record.local_id())
            .collect()
    };
    let first = list_risks(&caller(VIEWER), open_day(), query(None, 2), memory.ports())
        .await
        .unwrap();
    assert_eq!(ids(&first), ["RSK-004", "RSK-003"]);
    let second = list_risks(
        &caller(VIEWER),
        open_day(),
        query(first.next, 2),
        memory.ports(),
    )
    .await
    .unwrap();
    assert_eq!(ids(&second), ["RSK-002", "RSK-001"]);
    assert!(second.next.is_none());
}

/// The cursor holds the key that the last risk had on the first page, so a rating change of that risk between two
/// pages hides no other risk.
#[tokio::test]
async fn a_rating_change_between_two_pages_hides_no_risk() {
    let memory = Memory::default();
    // RSK-001 high x high, RSK-002 high x medium, RSK-003 medium x medium, RSK-004 low x medium.
    for (likelihood, impact) in [
        (High, High),
        (High, Medium),
        (Medium, Medium),
        (Low, Medium),
    ] {
        let input = NewRisk {
            likelihood,
            impact,
            ..new_risk(OWNER, None)
        };
        risk_by(&memory, OWNER, input).await.unwrap();
    }
    let ids = |page: &Page<Shown<RiskView>, RiskCursor>| -> Vec<String> {
        page.items
            .iter()
            .map(|shown| shown.record.local_id())
            .collect()
    };
    let first = list_risks(&caller(VIEWER), open_day(), query(None, 2), memory.ports())
        .await
        .unwrap();
    assert_eq!(ids(&first), ["RSK-001", "RSK-002"]);
    let second = first.items[1].record.clone();
    let lowered = RiskChange {
        likelihood: Some(Low),
        impact: Some(Low),
        ..risk_change(second.version)
    };
    change_risk_by(&memory, OWNER, second.id, lowered)
        .await
        .unwrap();
    let rest = list_risks(
        &caller(VIEWER),
        open_day(),
        query(first.next, 10),
        memory.ports(),
    )
    .await
    .unwrap();
    assert_eq!(ids(&rest), ["RSK-003", "RSK-004", "RSK-002"]);
}
