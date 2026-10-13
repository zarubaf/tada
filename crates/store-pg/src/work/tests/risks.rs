use tada_app::domain::ids::RiskId;
use tada_app::domain::records::Level::{High, Low, Medium};
use tada_app::domain::records::{Level, RiskRating, RiskStatus, RiskTitle};
use tada_app::work::{NewRiskRecord, RiskCursor, RiskFields, RiskStore, RiskView};

use super::*;

impl Fixture {
    fn risk_fields(&self, title: &str, likelihood: Level, impact: Level) -> RiskFields {
        RiskFields {
            title: Redactable::Present(RiskTitle::parse(title).unwrap()),
            description: Some(RecordDescription::parse("Das Zelt\nsteht auf der Wiese.").unwrap()),
            rating: RiskRating { likelihood, impact },
            mitigation: None,
            owner: self.owner,
            workstream_id: None,
            status: RiskStatus::Open,
        }
    }

    async fn risk(&self, title: &str, likelihood: Level, impact: Level) -> RiskView {
        let record = NewRiskRecord {
            id: RiskId::from_uuid(Uuid::now_v7()),
            event_id: self.event,
            fields: self.risk_fields(title, likelihood, impact),
        };
        let audit = self.audit(AuditAction::RiskCreate, record.id.as_uuid());
        match self
            .test
            .database
            .create_risk(self.scope, &record, AT.parse().unwrap(), &audit)
            .await
            .unwrap()
        {
            Created::Created(view) => view,
            Created::IdTaken => panic!("id taken"),
        }
    }

    async fn change_risk(
        &self,
        scope: OrgScope,
        risk: &RiskView,
        fields: &RiskFields,
        expected: RecordVersion,
    ) -> Changed<RiskView> {
        self.test
            .database
            .change_risk(
                scope,
                risk.event_id,
                risk.id,
                fields,
                expected,
                CHANGED_AT.parse().unwrap(),
                &self.audit(AuditAction::RiskChange, risk.id.as_uuid()),
            )
            .await
            .unwrap()
    }

    /// The readable IDs of the risks of the page, in the order of the page.
    async fn risk_page(
        &self,
        status: Option<RiskStatus>,
        after: Option<RiskCursor>,
        limit: u32,
    ) -> Vec<String> {
        self.test
            .database
            .risks(
                self.scope,
                self.event,
                &WorkFilter {
                    owner: None,
                    status,
                    workstream: None,
                    after,
                    limit,
                },
            )
            .await
            .unwrap()
            .iter()
            .map(RiskView::local_id)
            .collect()
    }
}

/// The list sorts by likelihood times impact (low 1, medium 2, high 3), then by impact, then by number.
/// The SQL is `ORDER BY score DESC, impact_weight DESC, local_number` with
/// `score = weight(likelihood) * weight(impact)` and `impact_weight = weight(impact)`.
#[tokio::test]
async fn risk_list_sorts_by_score_then_impact() {
    let f = Fixture::start().await;
    // The numbers are the creation order. Scores: 3, 4, 6, 6, 3, 4, 9.
    let first = f.risk("Hoch mal gering", High, Low).await;
    let second = f.risk("Mittel mal mittel", Medium, Medium).await;
    let third = f.risk("Hoch mal mittel", High, Medium).await;
    let fourth = f.risk("Mittel mal hoch", Medium, High).await;
    let fifth = f.risk("Gering mal hoch", Low, High).await;
    let sixth = f.risk("Mittel mal mittel, zuletzt", Medium, Medium).await;
    let seventh = f.risk("Hoch mal hoch", High, High).await;
    let numbers =
        [&first, &second, &third, &fourth, &fifth, &sixth, &seventh].map(|risk| risk.local_number);
    assert_eq!(numbers, [1, 2, 3, 4, 5, 6, 7]);

    // 9; then 6 with the higher impact first (medium x high before high x medium); then 4 by number (medium x medium
    // twice); then 3 with the higher impact first (low x high before high x low): medium x medium (4) comes
    // before high x low (3).
    let order = [
        "RSK-007", "RSK-004", "RSK-003", "RSK-002", "RSK-006", "RSK-005", "RSK-001",
    ];
    assert_eq!(f.risk_page(None, None, 50).await, order);

    // The cursor holds the rating and the number of the last risk of the page, and the pages follow the same order.
    assert_eq!(f.risk_page(None, None, 3).await, order[..3]);
    let after = |risk: &RiskView| Some(RiskCursor::after(risk));
    assert_eq!(f.risk_page(None, after(&third), 3).await, order[3..6]);
    assert_eq!(f.risk_page(None, after(&fifth), 3).await, order[6..]);
    assert!(f.risk_page(None, after(&first), 3).await.is_empty());
    let beyond = RiskCursor {
        number: 99,
        ..RiskCursor::after(&first)
    };
    assert!(f.risk_page(None, Some(beyond), 3).await.is_empty());

    // A filter keeps the order.
    let mitigated = RiskFields {
        status: RiskStatus::Mitigated,
        ..third.fields.clone()
    };
    f.change_risk(f.scope, &third, &mitigated, third.version)
        .await;
    assert_eq!(
        f.risk_page(Some(RiskStatus::Open), None, 50).await,
        [
            "RSK-007", "RSK-004", "RSK-002", "RSK-006", "RSK-005", "RSK-001"
        ]
    );
    assert_eq!(
        f.risk_page(Some(RiskStatus::Mitigated), None, 50).await,
        ["RSK-003"]
    );
}

/// A risk that changes its rating between two pages never hides another risk: the cursor holds the key it was
/// issued with, not the number whose current key the store would look up.
#[tokio::test]
async fn a_rating_change_between_two_pages_skips_no_risk() {
    let f = Fixture::start().await;
    f.risk("A", High, High).await;
    let second = f.risk("B", High, Medium).await;
    f.risk("C", Medium, Medium).await;
    f.risk("D", Low, Medium).await;
    f.risk("E", Low, Low).await;
    assert_eq!(f.risk_page(None, None, 2).await, ["RSK-001", "RSK-002"]);
    let issued = RiskCursor::after(&second);
    let lowered = RiskFields {
        rating: RiskRating {
            likelihood: Low,
            impact: Low,
        },
        ..second.fields.clone()
    };
    f.change_risk(f.scope, &second, &lowered, second.version)
        .await;
    // The cursor of the first page holds the key it had then: high x medium, number 2.
    let rest = f.risk_page(None, Some(issued), 10).await;
    assert_eq!(rest, ["RSK-003", "RSK-004", "RSK-002", "RSK-005"]);
}

#[tokio::test]
async fn creates_and_changes_a_risk_with_numbers_and_audit_events() {
    let f = Fixture::start().await;
    let first = f.risk("Starkregen", Medium, High).await;
    let second = f.risk("Stromausfall", Low, Low).await;
    assert_eq!(
        (first.local_id(), second.local_id()),
        ("RSK-001".into(), "RSK-002".into())
    );
    assert_eq!(
        first.fields.description.as_ref().unwrap().as_str(),
        "Das Zelt\nsteht auf der Wiese."
    );
    let db = &f.test.database;
    assert_eq!(
        db.risk(f.scope, f.event, first.id).await.unwrap(),
        Some(first.clone())
    );

    let fields = RiskFields {
        rating: RiskRating {
            likelihood: High,
            impact: High,
        },
        mitigation: Some(RecordDescription::parse("Zweiter Eingang").unwrap()),
        status: RiskStatus::Mitigated,
        ..first.fields.clone()
    };
    let Changed::Changed(changed) = f
        .change_risk(f.scope, &first, &fields, RecordVersion::FIRST)
        .await
    else {
        panic!("not changed");
    };
    assert_eq!(changed.fields, fields);
    assert_eq!(changed.version.get(), 2);
    let updated_at: jiff_sqlx::Timestamp =
        sqlx::query_scalar("SELECT updated_at FROM risk WHERE id = $1")
            .bind(first.id.as_uuid())
            .fetch_one(&f.test.database.pool)
            .await
            .unwrap();
    assert_eq!(updated_at.to_jiff(), CHANGED_AT.parse().unwrap());
    assert_eq!(
        f.change_risk(f.scope, &first, &fields, RecordVersion::FIRST)
            .await,
        Changed::VersionConflict
    );
    assert_eq!(
        f.audit_actions().await,
        [
            "institution.create",
            "risk.create",
            "risk.create",
            "risk.change"
        ]
    );
}

#[tokio::test]
async fn keeps_risks_inside_their_event_and_organization() {
    let f = Fixture::start().await;
    let db = &f.test.database;
    let risk = f.risk("Starkregen", Medium, High).await;
    let other_event = f
        .test
        .create_event(f.scope.organization_id(), "TEST31")
        .await;
    assert_eq!(db.risk(f.scope, other_event, risk.id).await.unwrap(), None);
    let musterhausen = f.test.create_organization("musterhausen").await;
    let stranger = MemberCaller::new(f.owner, musterhausen, OrganizationRole::Owner).scope();
    assert_eq!(db.risk(stranger, f.event, risk.id).await.unwrap(), None);
    assert_eq!(
        f.change_risk(stranger, &risk, &risk.fields, RecordVersion::FIRST)
            .await,
        Changed::NotFound
    );
    // The counter of readable IDs counts in each event.
    let record = NewRiskRecord {
        id: RiskId::from_uuid(Uuid::now_v7()),
        event_id: other_event,
        fields: f.risk_fields("Hagel", Low, Low),
    };
    let Created::Created(other) = db
        .create_risk(
            f.scope,
            &record,
            AT.parse().unwrap(),
            &f.audit(AuditAction::RiskCreate, record.id.as_uuid()),
        )
        .await
        .unwrap()
    else {
        panic!("not created");
    };
    assert_eq!(other.local_id(), "RSK-001");
    // A taken ID is reported, also by another organization.
    assert_eq!(
        db.create_risk(
            stranger,
            &NewRiskRecord {
                id: risk.id,
                ..record
            },
            AT.parse().unwrap(),
            &f.audit(AuditAction::RiskCreate, risk.id.as_uuid()),
        )
        .await
        .unwrap(),
        Created::IdTaken
    );
}

/// Two changes with one version: exactly one wins.
#[tokio::test]
async fn two_risk_changes_with_one_version_leave_one_winner() {
    let f = Fixture::start().await;
    let risk = f.risk("Starkregen", Medium, High).await;
    let alpha = f.risk_fields("Alpha", Low, Low);
    let beta = f.risk_fields("Beta", High, High);
    let (a, b) = tokio::join!(
        f.change_risk(f.scope, &risk, &alpha, RecordVersion::FIRST),
        f.change_risk(f.scope, &risk, &beta, RecordVersion::FIRST)
    );
    let winners = [&a, &b]
        .iter()
        .filter(|changed| matches!(changed, Changed::Changed(_)))
        .count();
    assert_eq!(winners, 1, "{a:?} {b:?}");
    assert!([&a, &b].contains(&&Changed::VersionConflict));
}
