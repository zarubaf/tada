//! The `serve` process role: the HTTP API and the web client (ADR 0025).

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::Context;
use tada_adapters::storage::{S3Config, S3Storage};
use tada_api::ApiState;
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
    let router = tada_api::router(ApiState {
        dependencies: vec![Arc::new(db.clone()), Arc::new(storage)],
    });

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
