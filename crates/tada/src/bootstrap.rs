//! The `bootstrap` command: an organization and the invitation of its first owner (ADR 0036).

use std::fmt;
use std::io::{IsTerminal, Write};

use anyhow::Context;
use secrecy::ExposeSecret;
use tada_adapters::clock::SystemClock;
use tada_app::bootstrap::{self, BootstrapInput, BootstrapOutcome, printed_link};
use tada_app::caller::{Bootstrap, ServiceCaller};
use tada_app::clock::Clock;
use tada_app::domain::identity::{Email, OrganizationName, OrganizationSlug};
use tada_app::public_url::PublicUrl;
use tada_store_pg::Database;

use crate::settings::BootstrapSettings;

/// The arguments of `tada bootstrap`.
#[derive(Debug, Clone)]
pub struct BootstrapCommand {
    pub organization_slug: OrganizationSlug,
    pub organization_name: OrganizationName,
    pub owner_email: Email,
    /// Also write the invitation link to standard error.
    pub print_link: bool,
}

/// `--print-link` needs a terminal on standard error: a link in a file or a log stays readable
/// after the operator is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrintLinkRefused;

impl fmt::Display for PrintLinkRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "--print-link writes the link to a terminal only, and standard error is not a terminal; nothing changed",
        )
    }
}

impl std::error::Error for PrintLinkRefused {}

/// Decides if the command writes the link. Returns an error, before any change, if the operator
/// asks for the link and standard error is not a terminal.
pub fn link_output(print_link: bool, is_terminal: bool) -> Result<bool, PrintLinkRefused> {
    match (print_link, is_terminal) {
        (true, false) => Err(PrintLinkRefused),
        (print_link, _) => Ok(print_link),
    }
}

pub async fn run(
    (database, public_url): BootstrapSettings,
    command: BootstrapCommand,
) -> anyhow::Result<()> {
    let mut stderr = std::io::stderr();
    let print = link_output(command.print_link, stderr.is_terminal())?;
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    let link = print.then_some(&mut stderr as &mut dyn Write);
    let result = execute(&db, &public_url, &SystemClock, command, link).await;
    db.close().await;
    match result? {
        BootstrapOutcome::OwnerExists => {
            tracing::info!("the organization has an owner, so nothing changed");
        }
        BootstrapOutcome::InvitationQueued {
            invitation_id,
            organization_id,
        } => {
            tracing::info!(%organization_id, %invitation_id, "the owner invitation is queued; the worker sends it");
        }
    }
    Ok(())
}

/// Runs the command. If `link` is given, writes the link of a new invitation to it.
pub async fn execute(
    db: &Database,
    public_url: &PublicUrl,
    clock: &dyn Clock,
    command: BootstrapCommand,
    link: Option<&mut dyn Write>,
) -> anyhow::Result<BootstrapOutcome> {
    let caller = ServiceCaller::<Bootstrap>::new();
    let input = BootstrapInput {
        slug: command.organization_slug,
        name: command.organization_name,
        owner_email: command.owner_email,
        owner_display_name: None,
    };
    let outcome = bootstrap::bootstrap(&caller, input, db, clock).await?;
    if let (
        Some(out),
        BootstrapOutcome::InvitationQueued {
            invitation_id,
            organization_id,
        },
    ) = (link, outcome)
    {
        // The invitation is committed. A new run revokes it and queues a new one.
        const QUEUED: &str = "the invitation is queued and the worker sends it, but the link \
            could not be printed; a new run is safe: it revokes this invitation and queues a new one";
        let link = printed_link(
            &caller,
            organization_id,
            invitation_id,
            db,
            clock,
            public_url,
        )
        .await
        .context(QUEUED)?;
        writeln!(out, "{}", link.expose_secret()).context(QUEUED)?;
    }
    Ok(outcome)
}
