//! The tada binary (ADR 0025).

use std::io::Write;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use tada::settings::{self, MigrateSettings, ServeSettings, TelegramSettings, WorkerSettings};
use tada::{migrate, run, serve, telegram, worker};

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
    /// Run the Telegram gateway.
    Telegram,
    /// Apply the pending database migrations, then stop.
    Migrate,
    /// Print the settings reference as Markdown.
    Settings,
    /// Print the OpenAPI document of the HTTP API.
    Openapi,
    /// Print the catalog of problem codes as Markdown.
    Problems,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Serve => run::<ServeSettings, _>("serve", serve::run),
        Command::Worker => run::<WorkerSettings, _>("worker", worker::run),
        Command::Telegram => run::<TelegramSettings, _>("telegram", telegram::run),
        Command::Migrate => run::<MigrateSettings, _>("migrate", migrate),
        Command::Settings => print(&settings::reference()),
        Command::Openapi => match tada_api::openapi().to_pretty_json() {
            Ok(json) => print(&format!("{json}\n")),
            Err(_) => ExitCode::FAILURE,
        },
        Command::Problems => print(&tada_api::problem_catalog()),
    }
}

fn print(text: &str) -> ExitCode {
    match std::io::stdout().write_all(text.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
