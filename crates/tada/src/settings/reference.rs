//! The settings reference that `tada settings` prints. `doc/settings.md` is this output (ADR 0036).

use super::{
    BootstrapSettings, Logging, MigrateSettings, Section, ServeSettings, Setting, TelegramSettings,
    WorkerSettings,
};

/// The commands that read settings, and their settings.
fn commands() -> Vec<(&'static str, Vec<&'static Setting>)> {
    vec![
        ("serve", <(Logging, ServeSettings)>::settings()),
        ("worker", <(Logging, WorkerSettings)>::settings()),
        ("telegram", <(Logging, TelegramSettings)>::settings()),
        ("migrate", <(Logging, MigrateSettings)>::settings()),
        ("bootstrap", <(Logging, BootstrapSettings)>::settings()),
    ]
}

/// The reference as a Markdown document, in the layout that mdformat keeps.
pub fn reference() -> String {
    let commands = commands();
    let mut settings: Vec<&Setting> = Vec::new();
    for (_, used) in &commands {
        for setting in used {
            if !settings.iter().any(|known| known.name == setting.name) {
                settings.push(setting);
            }
        }
    }

    let header = [
        "Variable",
        "Value",
        "Default",
        "Secret",
        "Commands",
        "Description",
    ]
    .map(String::from);
    let rows: Vec<[String; 6]> = settings
        .iter()
        .map(|setting| {
            let used_by: Vec<_> = commands
                .iter()
                .filter(|(_, used)| used.iter().any(|known| known.name == setting.name))
                .map(|(command, _)| format!("`{command}`"))
                .collect();
            [
                format!("`{}`", setting.name),
                setting.kind.to_owned(),
                match setting.default {
                    None => "none".to_owned(),
                    Some("") => "empty".to_owned(),
                    Some(default) => format!("`{default}`"),
                },
                if setting.secret { "yes" } else { "no" }.to_owned(),
                used_by.join(", "),
                setting.description.to_owned(),
            ]
        })
        .collect();

    let mut out = String::from(
        "# Settings\n\n\
         The command `tada settings` writes this file. Do not change it by hand.\n\
         Run `mise run gen` after a change of the settings code.\n\n\
         Settings come from environment variables (ADR 0025).\n\
         A variable that ends in `_FILE` gives the path of a file that contains a secret (ADR 0036).\n\n",
    );
    out.push_str(&table(&header, &rows));
    out
}

/// A Markdown table with padded columns.
fn table(header: &[String; 6], rows: &[[String; 6]]) -> String {
    let mut widths = header.each_ref().map(|cell| cell.chars().count().max(3));
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let line = |cells: &[String; 6]| {
        let padded: Vec<String> = cells
            .iter()
            .zip(widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect();
        format!("| {} |\n", padded.join(" | "))
    };
    let mut out = line(header);
    let rules: Vec<String> = widths.iter().map(|width| "-".repeat(*width)).collect();
    out.push_str(&format!("| {} |\n", rules.join(" | ")));
    for row in rows {
        out.push_str(&line(row));
    }
    out
}
