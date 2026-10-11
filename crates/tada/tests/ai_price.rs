//! The price table commands of the operator (ADR 0077).

#![allow(clippy::unwrap_used)]

use tada::ai_price::{SetPrice, execute_set, table};
use tada_app::ai_cost::{AiCostStore, Chf, Price};
use tada_app::caller::{MemberCaller, TestScope};
use tada_app::domain::identity::OrganizationRole;
use tada_app::domain::ids::UserId;
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

fn chf(text: &str) -> Chf {
    text.parse().unwrap()
}

fn set(model_id: &str, valid_from: &str, input: &str, output: &str) -> SetPrice {
    SetPrice {
        model_id: model_id.to_owned(),
        valid_from: valid_from.parse().unwrap(),
        price: Price {
            input_per_mtok: chf(input),
            output_per_mtok: chf(output),
        },
    }
}

#[tokio::test]
async fn the_operator_sets_and_lists_the_prices() {
    let test = TestDatabase::start().await;
    let db = test.connect();
    let testwil = test.create_organization("testwil").await;
    let scope = MemberCaller::new(
        UserId::from_uuid(Uuid::now_v7()),
        testwil,
        OrganizationRole::Owner,
    )
    .scope();

    execute_set(&db, &set("claude-haiku-5-5", "2026-01-01", "0.80", "4"))
        .await
        .unwrap();
    execute_set(&db, &set("claude-haiku-5-5", "2026-10-01", "1", "5"))
        .await
        .unwrap();
    // The same day again replaces the price.
    execute_set(&db, &set("claude-haiku-5-5", "2026-10-01", "0.75", "3.5"))
        .await
        .unwrap();

    assert_eq!(
        table(&db).await.unwrap(),
        "model\tvalid_from\tinput_chf_per_mtok\toutput_chf_per_mtok\n\
         claude-haiku-5-5\t2026-01-01\t0.80\t4.00\n\
         claude-haiku-5-5\t2026-10-01\t0.75\t3.50\n"
    );
    // The runner reads the price that the command wrote.
    assert_eq!(
        db.price(scope, "claude-haiku-5-5", "2026-10-11".parse().unwrap())
            .await
            .unwrap(),
        Some(Price {
            input_per_mtok: chf("0.75"),
            output_per_mtok: chf("3.5"),
        })
    );
}

#[tokio::test]
async fn a_model_that_no_task_uses_is_refused() {
    let test = TestDatabase::start().await;
    let db = test.connect();

    let error = execute_set(&db, &set("claude-haiku-5", "2026-01-01", "1", "5"))
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("claude-haiku-5-5"),
        "the error names the known models: {error}"
    );
    assert_eq!(
        table(&db).await.unwrap(),
        "model\tvalid_from\tinput_chf_per_mtok\toutput_chf_per_mtok\n"
    );
}
