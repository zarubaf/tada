//! The party store: names that look alike, inside one organization (ADR 0069).

#![allow(clippy::unwrap_used)]

use jiff::Timestamp;
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::clock::Clock;
use tada_app::domain::ids::UserId;
use tada_app::domain::parties::{Party, normalized_name};
use tada_app::parties::{NewInstitution, NewPerson, PartyStore, create_institution, create_person};
use tada_app::problem::{CommandError, FieldError};
use tada_store_pg::testing::TestDatabase;

#[derive(Debug)]
struct Now;

impl Clock for Now {
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }
}

#[tokio::test]
async fn named_like_finds_equal_contained_and_shared_names_of_one_organization() {
    let test = TestDatabase::start().await;
    let database = test.connect();
    let (a, user_a, _) = test.member("testwil", OrganizationRole::Owner).await;
    let (b, user_b, _) = test.member("musterhausen", OrganizationRole::Owner).await;
    let owner_a = MemberCaller::new(user_a, a, OrganizationRole::Owner);
    let owner_b = MemberCaller::new(user_b, b, OrganizationRole::Owner);

    for name in ["Hans Müller", "Anna Beispiel", "Muller"] {
        let input = NewPerson {
            id: None,
            name: name.to_owned(),
            email: None,
            phone: None,
            user_id: None::<UserId>,
        };
        create_person(&owner_a, input, &database, &database, &Now)
            .await
            .unwrap();
    }
    let institution = NewInstitution {
        id: None,
        name: "Müller Generatoren AG".to_owned(),
        kind: "company".to_owned(),
        email: None,
        phone: None,
    };
    create_institution(&owner_a, institution, &database, &database, &Now)
        .await
        .unwrap();
    // Another organization holds a name that would match.
    let other = NewPerson {
        id: None,
        name: "Müller Muster".to_owned(),
        email: None,
        phone: None,
        user_id: None,
    };
    create_person(&owner_b, other, &database, &database, &Now)
        .await
        .unwrap();

    let found = database
        .named_like(owner_a.scope(), &normalized_name("MÜLLER"))
        .await
        .unwrap();
    let ids: Vec<_> = found.iter().map(|party| party.local_id.as_str()).collect();
    assert_eq!(ids, ["PER-001", "PER-003", "INS-001"]);
    assert!(matches!(found[0].party, Party::Person(_)));
    assert!(matches!(found[2].party, Party::Institution(_)));

    let none = database
        .named_like(owner_a.scope(), &normalized_name("Zimmermann"))
        .await
        .unwrap();
    assert!(none.is_empty());
}

/// A client-chosen ID that another record of any organization holds is `taken` (ADR 0038).
#[tokio::test]
async fn a_taken_party_id_is_refused_in_each_organization() {
    let test = TestDatabase::start().await;
    let database = test.connect();
    let (a, user_a, _) = test.member("testwil", OrganizationRole::Owner).await;
    let (b, user_b, _) = test.member("musterhausen", OrganizationRole::Owner).await;
    let owner_a = MemberCaller::new(user_a, a, OrganizationRole::Owner);
    let owner_b = MemberCaller::new(user_b, b, OrganizationRole::Owner);
    let id = uuid::Uuid::now_v7();
    let person = |name: &str| NewPerson {
        id: Some(id),
        name: name.to_owned(),
        email: None,
        phone: None,
        user_id: None,
    };
    let created = create_person(&owner_a, person("Beat Muster"), &database, &database, &Now)
        .await
        .unwrap();
    assert_eq!(created.record.id.as_uuid(), id);
    for caller in [&owner_a, &owner_b] {
        let error = create_person(caller, person("Anna Beispiel"), &database, &database, &Now)
            .await
            .unwrap_err();
        assert_eq!(error.field_errors(), [FieldError::new("id", "taken")]);
    }
}
