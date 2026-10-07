//! The tada binary (ADR 0025).

use std::io::Write;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use tada::bootstrap::BootstrapCommand;
use tada::settings::{
    self, BootstrapSettings, MigrateSettings, ServeSettings, TelegramSettings, WorkerSettings,
};
use tada::{bootstrap, migrate, run, serve, telegram, worker};
use tada_app::domain::identity::{Email, OrganizationName, OrganizationSlug};

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
    /// Create an organization if its slug is free, and invite its first owner if it has none.
    Bootstrap {
        /// The unique key of the organization: 2 to 32 lowercase letters, digits and hyphens.
        #[arg(long, value_parser = OrganizationSlug::parse)]
        organization_slug: OrganizationSlug,
        /// The name of a new organization. An existing organization keeps its name.
        #[arg(long, value_parser = OrganizationName::parse)]
        organization_name: OrganizationName,
        /// The email address of the first owner. The worker sends the invitation to it.
        #[arg(long, value_parser = Email::parse)]
        owner_email: Email,
        /// Also write the invitation link to standard error. It expires after 30 minutes.
        /// The command refuses if standard error is not a terminal.
        #[arg(long)]
        print_link: bool,
    },
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
        Command::Bootstrap {
            organization_slug,
            organization_name,
            owner_email,
            print_link,
        } => run::<BootstrapSettings, _>("bootstrap", |settings| {
            bootstrap::run(
                settings,
                BootstrapCommand {
                    organization_slug,
                    organization_name,
                    owner_email,
                    print_link,
                },
            )
        }),
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
