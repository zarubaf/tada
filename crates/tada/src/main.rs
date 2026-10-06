//! The tada binary: the composition root and all process roles (ADR 0025).

mod logging;
mod serve;
mod settings;
mod shutdown;
mod worker;

use std::future::Future;
use std::io::Write;
use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, Subcommand};
use tada_store_pg::Database;

use crate::settings::{Loaded, Logging, MigrateSettings, Section, ServeSettings, WorkerSettings};

/// The exit code for invalid settings (ADR 0036).
const INVALID_SETTINGS: u8 = 2;

#[derive(Debug, Parser)]
#[command(version, about = "tada: event planning for clubs")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serve the HTTP API and the web client.
    Serve,
    /// Run jobs and schedules.
    Worker,
    /// Apply the pending database migrations, then stop.
    Migrate,
    /// Print the settings reference as Markdown.
    Settings,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Serve => run::<ServeSettings, _>("serve", serve::run),
        Command::Worker => run::<WorkerSettings, _>("worker", worker::run),
        Command::Migrate => run::<MigrateSettings, _>("migrate", migrate),
        Command::Settings => {
            let _ = std::io::stdout().write_all(settings::reference().as_bytes());
            ExitCode::SUCCESS
        }
    }
}

/// Loads the settings of a command, starts the logs and runs the command.
fn run<S, F>(process_role: &'static str, command: impl FnOnce(S) -> F) -> ExitCode
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

async fn migrate((database,): MigrateSettings) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    db.migrate().await?;
    tracing::info!("the migrations are applied");
    db.close().await;
    Ok(())
}
