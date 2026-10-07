//! The `serve` process role: the HTTP API and the web client (ADR 0025).

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::Context;
use tada_adapters::clock::SystemClock;
use tada_adapters::storage::{S3Config, S3Storage};
use tada_api::ApiState;
use tada_app::auth::Authenticator;
use tada_store_pg::Database;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::settings::ServeSettings;
use crate::shutdown;

pub async fn run((database, http, storage): ServeSettings) -> anyhow::Result<()> {
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
            authenticator: authenticator(&db).await?,
            events: Arc::new(db.clone()),
            telegram: Arc::new(db.clone()),
            clock: Arc::new(SystemClock),
            trusted_proxies: http.trusted_proxies,
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

/// The development authenticator in debug builds (ADR 0053).
#[cfg(debug_assertions)]
async fn authenticator(db: &Database) -> anyhow::Result<Arc<dyn Authenticator>> {
    db.ensure_dev_organization()
        .await
        .context("cannot create the development organization; did `tada migrate` run?")?;
    tracing::warn!("debug build: each request acts as the development owner (ADR 0053)");
    Ok(Arc::new(tada_store_pg::dev::DevAuthenticator))
}

/// Sign-in comes in Slice 1. Until then, a release build rejects each request (ADR 0053).
#[cfg(not(debug_assertions))]
async fn authenticator(_db: &Database) -> anyhow::Result<Arc<dyn Authenticator>> {
    Ok(Arc::new(NoSignIn))
}

#[cfg(not(debug_assertions))]
#[derive(Debug)]
struct NoSignIn;

#[cfg(not(debug_assertions))]
#[async_trait::async_trait]
impl Authenticator for NoSignIn {
    async fn authenticate(
        &self,
        _credential: Option<tada_app::auth::Credential<'_>>,
    ) -> Result<tada_app::caller::MemberCaller, tada_app::auth::AuthenticationError> {
        Err(tada_app::auth::AuthenticationError::Unauthenticated)
    }
}
