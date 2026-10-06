//! The tada binary (ADR 0025).

use std::io::Write;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use tada::settings::{self, MigrateSettings, ServeSettings, WorkerSettings};
use tada::{migrate, run, serve, worker};

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
