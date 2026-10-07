//! The `serve` process role: the HTTP API and the web client (ADR 0025).

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::Context;
use tada_adapters::clock::SystemClock;
use tada_adapters::storage::{S3Config, S3Storage};
use tada_api::ApiState;
use tada_app::session::SessionAuthenticator;
use tada_store_pg::rate_limit::PgRateLimiter;
use tada_store_pg::{Database, PgSignInRequestStore};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::settings::ServeSettings;
use crate::shutdown;

pub async fn run(
    (database, http, storage, public_url, sign_in): ServeSettings,
) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    let storage = S3Storage::new(S3Config {
        endpoint: storage.endpoint.into(),
        region: storage.region,
        bucket: storage.bucket,
        access_key_id: storage.access_key_id,
        secret_access_key: storage.secret_access_key,
    });
    let router = tada_api::router(
        ApiState {
            dependencies: vec![Arc::new(db.clone()), Arc::new(storage)],
            authenticator: Arc::new(SessionAuthenticator::new(
                Arc::new(db.clone()),
                Arc::new(db.clone()),
                Arc::new(SystemClock),
            )),
            events: Arc::new(db.clone()),
            telegram: Arc::new(db.clone()),
            identity: Arc::new(db.clone()),
            sessions: Arc::new(db.clone()),
            sign_in: Arc::new(db.clone()),
            sign_in_requests: Arc::new(PgSignInRequestStore::new(
                db.clone(),
                PgRateLimiter::new(sign_in.rate_limit_key),
            )),
            clock: Arc::new(SystemClock),
            trusted_proxies: http.trusted_proxies,
            event_members: Arc::new(db.clone()),
            public_url,
        },
        http.web_root.as_deref(),
    );

    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, http.port));
    let listener = TcpListener::bind(address)
        .await
        .context("cannot listen on TADA_PORT")?;
    tracing::info!(port = http.port, "serve started");

    let (stop, stopped) = oneshot::channel::<()>();
    let mut server = tokio::spawn(tada_api::serve(listener, router, async {
        let _ = stopped.await;
    }));
    tokio::select! {
        result = &mut server => return Ok(result.context("the server failed")??),
        () = shutdown::signal_received() => {}
    }

    tracing::info!("serve stops");
    let _ = stop.send(());
    match tokio::time::timeout(shutdown::DRAIN_TIMEOUT, server).await {
        Ok(result) => result.context("the server failed")??,
        Err(_) => tracing::warn!("open requests did not complete in time"),
    }
    db.close().await;
    Ok(())
}
