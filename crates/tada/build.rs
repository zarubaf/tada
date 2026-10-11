//! Writes the commit and the build time into the build (ADR 0075).
//!
//! The commit comes from `TADA_BUILD_COMMIT`, else from `git rev-parse --short HEAD`, else it is `unknown`.
//! `SOURCE_DATE_EPOCH` fixes the build time for a reproducible build.

use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// The length of the short commit. The web client uses the same length.
const SHORT: usize = 7;

fn main() {
    println!("cargo:rerun-if-env-changed=TADA_BUILD_COMMIT");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    watch_git();
    println!("cargo:rustc-env=TADA_BUILD_COMMIT={}", commit());
    println!("cargo:rustc-env=TADA_BUILT_AT={}", built_at());
}

/// Reruns the script when a commit, a checkout or a ref update changes `HEAD`.
/// The file `HEAD` holds only the name of the branch, so the file of the branch is watched too.
/// A worktree has its own `HEAD` and shares the refs, so `git rev-parse --git-path` gives the real paths.
/// Without Git nothing is watched, and the script runs again only when its inputs change.
fn watch_git() {
    let Some(head) = git_path("HEAD") else {
        return;
    };
    println!("cargo:rerun-if-changed={head}");
    if let Ok(content) = std::fs::read_to_string(&head)
        && let Some(reference) = content.strip_prefix("ref: ")
        && let Some(reference_path) = git_path(reference.trim())
    {
        println!("cargo:rerun-if-changed={reference_path}");
    }
    // A path that does not exist makes cargo run the script on every build.
    if let Some(packed) = git_path("packed-refs").filter(|path| Path::new(path).exists()) {
        println!("cargo:rerun-if-changed={packed}");
    }
}

fn git_path(name: &str) -> Option<String> {
    git(&["rev-parse", "--git-path", name])
}

fn git(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let text = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn commit() -> String {
    let given = std::env::var("TADA_BUILD_COMMIT").ok();
    given
        .map(|commit| commit.trim().chars().take(SHORT).collect::<String>())
        .filter(|commit| !commit.is_empty())
        .or_else(from_git)
        .unwrap_or_else(|| "unknown".to_owned())
}

fn from_git() -> Option<String> {
    git(&["rev-parse", "--short=7", "HEAD"])
}

/// The build time as `YYYY-MM-DDTHH:MM:SSZ` in UTC.
fn built_at() -> String {
    let seconds = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(0))
        });
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The date of a day count since 1970-01-01 (proleptic Gregorian calendar).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = if month_part < 10 {
        month_part + 3
    } else {
        month_part - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}
