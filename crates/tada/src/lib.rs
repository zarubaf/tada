//! The composition root: it connects the adapters to the `app` crate and runs the process roles (ADR 0025).
//! `main.rs` only parses the command line.

mod logging;
pub mod serve;
pub mod settings;
mod shutdown;
pub mod worker;

use std::future::Future;
use std::io::Write;
use std::process::ExitCode;

use anyhow::Context;
use tada_store_pg::Database;

use crate::settings::{Loaded, Logging, MigrateSettings, Section};

/// The exit code for invalid settings (ADR 0036).
const INVALID_SETTINGS: u8 = 2;

/// Loads the settings of a command, starts the logs and runs the command.
pub fn run<S, F>(process_role: &'static str, command: impl FnOnce(S) -> F) -> ExitCode
where
    S: Section,
    F: Future<Output = anyhow::Result<()>>,
{
    let Loaded {
        settings: (logging, settings),
        warnings,
    } = match settings::load::<(Logging, S)>(|name| std::env::var(name).ok()) {
        Ok(loaded) => loaded,
        Err(errors) => {
            let _ = write!(std::io::stderr(), "{errors}");
            return ExitCode::from(INVALID_SETTINGS);
        }
    };
    logging::init(process_role, &logging.filter);
    for warning in warnings {
        tracing::warn!("{warning}");
    }

    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("cannot start the async runtime")
        .and_then(|runtime| runtime.block_on(command(settings)));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = format!("{error:#}"), "{process_role} failed");
            ExitCode::FAILURE
        }
    }
}

pub async fn migrate((database,): MigrateSettings) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    db.migrate().await?;
    tracing::info!("the migrations are applied");
    db.close().await;
    Ok(())
}
