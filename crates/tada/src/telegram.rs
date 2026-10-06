//! The `telegram` process role: the Telegram gateway (ADR 0011). For now it uses long polling.

use std::sync::Arc;

use anyhow::Context;
use secrecy::ExposeSecret;
use tada_adapters::clock::SystemClock;
use tada_store_pg::Database;
use tada_telegram::Gateway;

use crate::settings::TelegramSettings;
use crate::shutdown;

pub async fn run((database, telegram): TelegramSettings) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    let gateway = Gateway::new(
        telegram.api_url.as_str(),
        telegram.bot_token.expose_secret(),
        Arc::new(db.clone()),
        Arc::new(SystemClock),
    );
    tracing::info!("telegram started");
    gateway.run(shutdown::signal_received()).await;
    tracing::info!("telegram stops");
    db.close().await;
    Ok(())
}
