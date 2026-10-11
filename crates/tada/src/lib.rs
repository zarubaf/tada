//! The composition root: it connects the adapters to the `app` crate and runs the process roles (ADR 0025).
//! `main.rs` only parses the command line.

pub mod ai_price;
pub mod bootstrap;
pub mod export;
pub mod heartbeat;
pub mod logging;
pub mod redactions;
pub mod serve;
pub mod settings;
mod shutdown;
pub mod telegram;
pub mod worker;

use std::future::Future;
use std::io::Write;
use std::process::ExitCode;

use anyhow::Context;
use tada_app::build_info::BuildInfo;
use tada_app::domain::facts::{CORE_CATALOG_VERSION, core_catalog};
use tada_store_pg::Database;

use crate::settings::{Loaded, Logging, MigrateSettings, Section};

/// The build of this binary (ADR 0075). `build.rs` writes the commit, the build time and the build ID.
/// The stamp lives in the binary crate, so a new commit rebuilds only this crate.
pub const BUILD: BuildInfo = BuildInfo {
    version: env!("CARGO_PKG_VERSION"),
    commit: env!("TADA_BUILD_COMMIT"),
    built_at: env!("TADA_BUILT_AT"),
    build_id: env!("TADA_BUILD_ID"),
};

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
    // The Rust catalog is the one authority for the shipped fields (ADR 0049).
    db.sync_catalog(&core_catalog(), CORE_CATALOG_VERSION)
        .await
        .context("cannot sync the shipped field catalog")?;
    tracing::info!("the migrations are applied and the shipped field catalog is in sync");
    db.close().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_knows_its_version_commit_and_time() {
        assert_eq!(BUILD.version, env!("CARGO_PKG_VERSION"));
        assert!(!BUILD.commit.is_empty());
        let built_at: jiff::Timestamp = BUILD.built_at.parse().unwrap();
        assert!(built_at.as_second() > 1_700_000_000);
    }
}
