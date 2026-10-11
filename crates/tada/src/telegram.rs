//! The `telegram` process role: the Telegram gateway (ADR 0011). For now it uses long polling.

use std::sync::Arc;

use anyhow::Context;
use secrecy::ExposeSecret;
use tada_adapters::clock::SystemClock;
use tada_app::build_info::Role;
use tada_store_pg::Database;
use tada_telegram::Gateway;

use crate::heartbeat::{self, Heartbeat};
use crate::settings::TelegramSettings;
use crate::shutdown;

pub async fn run((database, telegram): TelegramSettings) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    heartbeat::check_schema(&db).await?;
    let heartbeat = Heartbeat::start(&db, Role::Telegram);
    let gateway = Gateway::new(
        telegram.api_url.as_str(),
        telegram.bot_token.expose_secret(),
        Arc::new(db.clone()),
        Arc::new(SystemClock),
    );
    tracing::info!("telegram started");
    gateway.run(shutdown::signal_received()).await;
    tracing::info!("telegram stops");
    heartbeat.stop().await;
    db.close().await;
    Ok(())
}
