//! The `ai-price` commands: the operator sets and lists the price table of the AI models (ADR 0077).
//!
//! AI work runs only for a model with a price, because each call records its cost.

use std::fmt::Write;

use anyhow::{Context, bail};
use jiff::civil::Date;
use tada_app::ai_cost::Price;
use tada_app::model::TaskModels;
use tada_store_pg::Database;

use crate::settings::AiPriceSettings;

/// The arguments of `tada ai-price set`.
#[derive(Debug, Clone)]
pub struct SetPrice {
    pub model_id: String,
    pub valid_from: Date,
    pub price: Price,
}

pub async fn set((database,): AiPriceSettings, command: SetPrice) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    let result = execute_set(&db, &command).await;
    db.close().await;
    result?;
    tracing::info!(
        model_id = command.model_id,
        valid_from = %command.valid_from,
        "the price is set"
    );
    Ok(())
}

pub async fn list((database,): AiPriceSettings) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    let result = table(&db).await;
    db.close().await;
    let table = result?;
    std::io::Write::write_all(&mut std::io::stdout(), table.as_bytes())
        .context("cannot write the price table")
}

/// Sets the price of a model that a task of the build uses. Another model ID is a typing error: its price
/// would never apply.
pub async fn execute_set(db: &Database, command: &SetPrice) -> anyhow::Result<()> {
    let models = TaskModels::of_build();
    let known = models.model_ids();
    if !known.contains(command.model_id.as_str()) {
        bail!(
            "no task of this build uses the model {:?}; the models are: {}",
            command.model_id,
            known.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
    db.set_ai_price(&command.model_id, command.valid_from, command.price)
        .await
        .context("cannot set the price")
}

/// The price table as tab-separated text with a header line, in CHF for one million tokens.
pub async fn table(db: &Database) -> anyhow::Result<String> {
    let rows = db.ai_prices().await.context("cannot read the prices")?;
    let mut text = String::from("model\tvalid_from\tinput_chf_per_mtok\toutput_chf_per_mtok\n");
    for row in rows {
        let _ = writeln!(
            text,
            "{}\t{}\t{}\t{}",
            row.model_id, row.valid_from, row.price.input_per_mtok, row.price.output_per_mtok
        );
    }
    Ok(text)
}
