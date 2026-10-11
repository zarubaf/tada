//! The tada binary (ADR 0025).

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use jiff::civil::Date;
use tada::ai_price::SetPrice;
use tada::bootstrap::BootstrapCommand;
use tada::export::ExportCommand;
use tada::settings::{
    self, AiPriceSettings, BootstrapSettings, ExportSettings, MigrateSettings, RedactionsSettings,
    ServeSettings, TelegramSettings, WorkerSettings,
};
use tada::{ai_price, bootstrap, export, migrate, redactions, run, serve, telegram, worker};
use tada_app::ai_cost::{Chf, Price};
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
    /// Write all data of one organization into a new or empty directory (ADR 0059).
    /// The export holds personal data: store it like a backup.
    Export {
        /// The organization to export.
        #[arg(long, value_parser = OrganizationSlug::parse)]
        organization_slug: OrganizationSlug,
        /// The directory for the files of the export. It must be new or empty.
        #[arg(long)]
        output: PathBuf,
    },
    /// Work with the legal redactions (ADR 0072).
    Redactions {
        #[command(subcommand)]
        command: RedactionsCommand,
    },
    /// Work with the price table of the AI models (ADR 0077). AI work runs only for a model with a price.
    AiPrice {
        #[command(subcommand)]
        command: AiPriceCommand,
    },
    /// Print the settings reference as Markdown.
    Settings,
    /// Print the OpenAPI document of the HTTP API.
    Openapi,
    /// Print the catalog of problem codes as Markdown.
    Problems,
}

#[derive(Debug, Subcommand)]
enum RedactionsCommand {
    /// Apply each redaction of the deletion journal that the database does not hold, in time order.
    /// Run it after each restore of the database. It is idempotent.
    Replay,
}

#[derive(Debug, Subcommand)]
enum AiPriceCommand {
    /// Set the price of a model from a date. A price of the same model and date is replaced.
    Set {
        /// The model ID, as in `ai/models.json`, for example `claude-haiku-5-5`.
        #[arg(long)]
        model: String,
        /// The price of one million input tokens in Swiss francs, for example `0.80`.
        #[arg(long)]
        input_chf_per_mtok: Chf,
        /// The price of one million output tokens in Swiss francs, for example `4`.
        #[arg(long)]
        output_chf_per_mtok: Chf,
        /// The first day of the price, for example `2026-10-01`. The days are in UTC.
        #[arg(long)]
        valid_from: Date,
    },
    /// Print the price table as tab-separated text.
    List,
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
        Command::Export {
            organization_slug,
            output,
        } => run::<ExportSettings, _>("export", |settings| {
            export::run(
                settings,
                ExportCommand {
                    organization_slug,
                    output,
                },
            )
        }),
        Command::Redactions {
            command: RedactionsCommand::Replay,
        } => run::<RedactionsSettings, _>("redactions", redactions::run),
        Command::AiPrice {
            command:
                AiPriceCommand::Set {
                    model,
                    input_chf_per_mtok,
                    output_chf_per_mtok,
                    valid_from,
                },
        } => run::<AiPriceSettings, _>("ai-price", |settings| {
            ai_price::set(
                settings,
                SetPrice {
                    model_id: model,
                    valid_from,
                    price: Price {
                        input_per_mtok: input_chf_per_mtok,
                        output_per_mtok: output_chf_per_mtok,
                    },
                },
            )
        }),
        Command::AiPrice {
            command: AiPriceCommand::List,
        } => run::<AiPriceSettings, _>("ai-price", ai_price::list),
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
