//! The `telegram` process role: the Telegram gateway (ADRs 0011 and 0076).
//!
//! It receives updates by long polling or by webhook, and serves `/healthz` and `/readyz` in both modes.

use std::sync::Arc;

use anyhow::Context;
use axum::Router;
use secrecy::ExposeSecret;
use tada_adapters::clock::SystemClock;
use tada_app::build_info::Role;
use tada_app::health::DependencyCheck;
use tada_store_pg::Database;
use tada_telegram::{Gateway, UpdateSource};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::heartbeat::{self, Heartbeat};
use crate::settings::TelegramSettings;
use crate::shutdown;

/// The routes of the listener: the health routes over `stores` and the Bot API of `gateway`, and the
/// webhook route in webhook mode.
pub fn routes(
    gateway: Arc<Gateway>,
    source: &UpdateSource,
    mut stores: Vec<Arc<dyn DependencyCheck>>,
) -> Router {
    stores.push(Arc::new(gateway.bot_api_check()));
    tada_api::health::routes(stores).merge(tada_telegram::routes(gateway, source))
}

pub async fn run((database, bot, telegram, storage): TelegramSettings) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    heartbeat::check_schema(&db).await?;
    let gateway = Arc::new(Gateway::new(
        bot.api_url.as_str(),
        bot.bot_token.expose_secret(),
        Arc::new(db.clone()),
        Arc::new(SystemClock),
    ));
    let router = routes(
        gateway.clone(),
        &telegram.source,
        vec![Arc::new(db.clone()), Arc::new(storage.open())],
    );
    let listener = TcpListener::bind(telegram.listen)
        .await
        .context("cannot listen on TADA_TELEGRAM_LISTEN")?;
    tracing::info!("telegram started");
    let heartbeat = Heartbeat::start(&db, Role::Telegram);

    let (stop, stopped) = oneshot::channel::<()>();
    let mut server = tokio::spawn(tada_api::serve(listener, router, async {
        let _ = stopped.await;
    }));
    tokio::select! {
        result = &mut server => {
            heartbeat.stop().await;
            return Ok(result.context("the listener failed")??);
        }
        () = gateway.run(shutdown::signal_received(), &telegram.source) => {}
    }

    tracing::info!("telegram stops");
    heartbeat.stop().await;
    let _ = stop.send(());
    match tokio::time::timeout(shutdown::DRAIN_TIMEOUT, server).await {
        Ok(result) => result.context("the listener failed")??,
        Err(_) => tracing::warn!("open webhook requests did not complete in time"),
    }
    db.close().await;
    Ok(())
}
