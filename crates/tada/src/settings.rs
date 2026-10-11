//! The settings loader (ADR 0036).
//!
//! Each command loads only the sections that it needs. The loader collects all errors of these
//! sections and reports them together. Secrets come only from files named by `TADA_<NAME>_FILE`.

mod reference;
mod sections;

use std::fmt::{self, Display};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::str::FromStr;

use secrecy::SecretString;

pub use reference::reference;
pub use sections::{
    Ai, AiPriceSettings, BootstrapSettings, ExportSettings, Logging, MigrateSettings,
    RedactionsSettings, ServeSettings, TelegramSettings, WorkerSettings,
};

/// The description of one setting, for the loader and for the reference.
#[derive(Debug)]
pub struct Setting {
    /// The environment variable, for example `TADA_PORT`.
    pub name: &'static str,
    /// The expected value, in words.
    pub kind: &'static str,
    /// The value if the variable is not set. Secrets never have a default.
    pub default: Option<&'static str>,
    /// If true, the variable gives the path of a file that contains the value.
    pub secret: bool,
    pub description: &'static str,
}

/// A group of settings that one part of the binary needs.
pub trait Section: Sized {
    /// The settings of this section, in reference order.
    fn settings() -> Vec<&'static Setting>;

    /// Reads the section. Returns `None` if a value is missing or invalid; the source then has the error.
    fn read(source: &mut Source<'_>) -> Option<Self>;
}

macro_rules! tuple_section {
    ($($name:ident),+) => {
        impl<$($name: Section),+> Section for ($($name,)+) {
            fn settings() -> Vec<&'static Setting> {
                let mut settings = Vec::new();
                $(settings.extend($name::settings());)+
                settings
            }

            #[allow(non_snake_case)]
            fn read(source: &mut Source<'_>) -> Option<Self> {
                // Read every section before the first `?`, so that the source collects all errors.
                $(let $name = $name::read(source);)+
                Some(($($name?,)+))
            }
        }
    };
}

tuple_section!(A);
tuple_section!(A, B);
tuple_section!(A, B, C);
tuple_section!(A, B, C, D);
tuple_section!(A, B, C, D, E);
tuple_section!(A, B, C, D, E, F);
tuple_section!(A, B, C, D, E, F, G);

/// Reads the section `S` from the environment.
pub fn load<S: Section>(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<Loaded<S>, SettingsErrors> {
    let mut source = Source {
        lookup: &lookup,
        errors: Vec::new(),
        warnings: Vec::new(),
    };
    match S::read(&mut source) {
        Some(settings) if source.errors.is_empty() => Ok(Loaded {
            settings,
            warnings: source.warnings,
        }),
        _ => Err(SettingsErrors(source.errors)),
    }
}

/// The settings of a command, and the warnings to log after the logs start.
#[derive(Debug)]
pub struct Loaded<S> {
    pub settings: S,
    pub warnings: Vec<String>,
}

/// All errors of the loaded sections. The messages never contain a value.
#[derive(Debug)]
pub struct SettingsErrors(pub Vec<String>);

impl Display for SettingsErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "invalid settings:")?;
        for error in &self.0 {
            writeln!(f, "  {error}")?;
        }
        Ok(())
    }
}

/// The environment, and the errors and warnings that the reads found.
pub struct Source<'a> {
    lookup: &'a dyn Fn(&str) -> Option<String>,
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl Source<'_> {
    /// Reads and parses a setting that is not secret.
    pub fn value<T>(&mut self, setting: &Setting) -> Option<T>
    where
        T: FromStr,
        T::Err: Display,
    {
        debug_assert!(!setting.secret, "{} is a secret", setting.name);
        let Some(raw) = (self.lookup)(setting.name).or_else(|| setting.default.map(String::from))
        else {
            self.error(setting, "is not set");
            return None;
        };
        match raw.parse() {
            Ok(value) => Some(value),
            Err(error) => {
                self.error(
                    setting,
                    &format!("is not a valid {}: {error}", setting.kind),
                );
                None
            }
        }
    }

    /// Reads a secret from the file that the setting names.
    pub fn secret(&mut self, setting: &Setting) -> Option<SecretString> {
        debug_assert!(setting.secret, "{} is not a secret", setting.name);
        let Some(path) = (self.lookup)(setting.name) else {
            self.error(setting, "is not set");
            return None;
        };
        self.read_secret_file(setting, &path)
    }

    /// True if the setting names a file that exists, false if it is not set. The outer `None` is an error.
    /// It never opens the file: a role that only needs to know whether a secret is set does not hold it.
    pub fn secret_file_exists(&mut self, setting: &Setting) -> Option<bool> {
        debug_assert!(setting.secret, "{} is not a secret", setting.name);
        let Some(path) = (self.lookup)(setting.name) else {
            return Some(false);
        };
        if fs::metadata(path).is_ok() {
            Some(true)
        } else {
            self.error(setting, "names a file that does not exist");
            None
        }
    }

    /// Reads a secret that can be absent. The outer `None` is an error; the inner one is "not set".
    pub fn optional_secret(&mut self, setting: &Setting) -> Option<Option<SecretString>> {
        debug_assert!(setting.secret, "{} is not a secret", setting.name);
        match (self.lookup)(setting.name) {
            None => Some(None),
            Some(path) => self.read_secret_file(setting, &path).map(Some),
        }
    }

    fn read_secret_file(&mut self, setting: &Setting, path: &str) -> Option<SecretString> {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) => {
                self.error(
                    setting,
                    &format!("names a file that cannot be read: {error}"),
                );
                return None;
            }
        };
        // Some runtimes mount secrets readable by all users, so this is a warning only (ADR 0036).
        if fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o004 != 0) {
            self.warnings.push(format!(
                "{} names a file that all users can read",
                setting.name
            ));
        }
        let value = content.trim_end_matches(['\r', '\n']);
        if value.is_empty() {
            self.error(setting, "names an empty file");
            return None;
        }
        Some(SecretString::from(value))
    }

    /// Records an error that a section found after it parsed the value.
    pub fn error(&mut self, setting: &Setting, message: &str) {
        self.errors.push(format!("{} {message}", setting.name));
    }
}

impl fmt::Debug for Source<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Source")
            .field("errors", &self.errors)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Write;

    use secrecy::ExposeSecret;
    use tempfile::NamedTempFile;

    use super::*;
    use tada_adapters::mail::SmtpTls;

    use crate::settings::sections::{Ai, Database, TelegramBot, TelegramGateway, Uploads};
    use tada_app::ai_cost::AiSetup;
    use tada_telegram::UpdateSource;

    fn secret_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    fn load_from<S: Section>(variables: &[(&str, &str)]) -> Result<Loaded<S>, SettingsErrors> {
        let variables: HashMap<String, String> = variables
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        load::<S>(|name| variables.get(name).cloned())
    }

    #[test]
    fn reports_all_errors_of_all_sections_together() {
        let errors = load_from::<(Logging, ServeSettings)>(&[("TADA_PORT", "eighty")]).unwrap_err();
        let names: Vec<&str> = errors
            .0
            .iter()
            .map(|error| error.split(' ').next().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "TADA_DATABASE_URL",
                "TADA_DATABASE_PASSWORD_FILE",
                "TADA_PORT",
                "TADA_S3_ENDPOINT",
                "TADA_S3_REGION",
                "TADA_S3_BUCKET",
                "TADA_S3_ACCESS_KEY_ID_FILE",
                "TADA_S3_SECRET_ACCESS_KEY_FILE",
                "TADA_PUBLIC_URL",
                "TADA_RATE_LIMIT_KEY_FILE",
            ]
        );
    }

    #[test]
    fn reads_a_secret_from_its_file_without_the_final_newline() {
        let password = secret_file("s3cr3t\n");
        let loaded = load_from::<(Database,)>(&[
            ("TADA_DATABASE_URL", "postgres://tada@localhost/tada"),
            (
                "TADA_DATABASE_PASSWORD_FILE",
                password.path().to_str().unwrap(),
            ),
        ])
        .unwrap();
        assert_eq!(loaded.settings.0.password.expose_secret(), "s3cr3t");
    }

    #[test]
    fn rejects_a_password_in_the_database_url() {
        let password = secret_file("s3cr3t");
        let errors = load_from::<(Database,)>(&[
            ("TADA_DATABASE_URL", "postgres://tada:other@localhost/tada"),
            (
                "TADA_DATABASE_PASSWORD_FILE",
                password.path().to_str().unwrap(),
            ),
        ])
        .unwrap_err();
        assert_eq!(errors.0.len(), 1);
        assert!(errors.0[0].starts_with("TADA_DATABASE_URL must not contain a password"));
        assert!(
            !errors.0[0].contains("other"),
            "the error repeats the value"
        );
    }

    #[test]
    fn rejects_an_empty_secret_file() {
        let password = secret_file("\n");
        let errors = load_from::<(Database,)>(&[
            ("TADA_DATABASE_URL", "postgres://tada@localhost/tada"),
            (
                "TADA_DATABASE_PASSWORD_FILE",
                password.path().to_str().unwrap(),
            ),
        ])
        .unwrap_err();
        assert_eq!(
            errors.0,
            ["TADA_DATABASE_PASSWORD_FILE names an empty file"]
        );
    }

    #[test]
    fn uses_the_default_of_a_setting_that_is_not_set() {
        let loaded = load_from::<(Logging,)>(&[]).unwrap();
        assert_eq!(loaded.settings.0.filter, "info");
    }

    #[test]
    fn the_upload_limit_is_100_mb_by_default_and_never_zero() {
        let loaded = load_from::<(Uploads,)>(&[]).unwrap();
        assert_eq!(loaded.settings.0.max_bytes.get(), 100_000_000);
        let errors = load_from::<(Uploads,)>(&[("TADA_UPLOAD_MAX_BYTES", "0")]).unwrap_err();
        assert_eq!(
            errors.0,
            [
                "TADA_UPLOAD_MAX_BYTES is not a valid byte count: number would be zero for non-zero type"
            ]
        );
    }

    #[test]
    fn rejects_an_invalid_log_filter() {
        let errors = load_from::<(Logging,)>(&[("TADA_LOG", "info,[")]).unwrap_err();
        assert!(errors.0[0].starts_with("TADA_LOG is not a valid log filter"));
    }

    const MAIL: [(&str, &str); 3] = [
        ("TADA_PUBLIC_URL", "https://tada.example.org"),
        ("TADA_MAIL_FROM", "tada@example.org"),
        ("TADA_MAIL_SMTP_HOST", "mail.example.org"),
    ];

    fn worker_with(extra: &[(&str, &str)]) -> Result<Loaded<WorkerSettings>, SettingsErrors> {
        let password = secret_file("s3cr3t");
        let path = password.path().to_str().unwrap().to_owned();
        let key = secret_file("key");
        let key_path = key.path().to_str().unwrap().to_owned();
        let mut variables = vec![
            ("TADA_DATABASE_URL", "postgres://tada@localhost/tada"),
            ("TADA_DATABASE_PASSWORD_FILE", path.as_str()),
        ];
        variables.extend(MAIL);
        variables.extend([
            ("TADA_S3_ENDPOINT", "http://garage:3900"),
            ("TADA_S3_REGION", "garage"),
            ("TADA_S3_BUCKET", "tada"),
            ("TADA_S3_ACCESS_KEY_ID_FILE", key_path.as_str()),
            ("TADA_S3_SECRET_ACCESS_KEY_FILE", key_path.as_str()),
            ("TADA_TELEGRAM_BOT_TOKEN_FILE", key_path.as_str()),
        ]);
        variables.extend_from_slice(extra);
        load_from::<WorkerSettings>(&variables)
    }

    #[test]
    fn the_worker_reads_the_mail_settings_with_the_smtp_defaults() {
        let (_, public_url, mail, smtp, storage, _, _) = worker_with(&[]).unwrap().settings;
        assert_eq!(storage.bucket, "tada");
        assert_eq!(public_url.origin(), "https://tada.example.org");
        assert_eq!(mail.from.as_str(), "tada@example.org");
        assert_eq!(smtp.host, "mail.example.org");
        assert_eq!(smtp.port, 465);
        assert_eq!(smtp.tls, SmtpTls::Implicit);
        assert!(smtp.credentials.is_none());
    }

    #[test]
    fn a_missing_mail_setting_is_reported_with_the_other_errors() {
        let errors =
            load_from::<WorkerSettings>(&[("TADA_PUBLIC_URL", "https://tada.example.org")])
                .unwrap_err();
        let names: Vec<&str> = errors
            .0
            .iter()
            .map(|error| error.split(' ').next().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "TADA_DATABASE_URL",
                "TADA_DATABASE_PASSWORD_FILE",
                "TADA_MAIL_FROM",
                "TADA_MAIL_SMTP_HOST",
                "TADA_S3_ENDPOINT",
                "TADA_S3_REGION",
                "TADA_S3_BUCKET",
                "TADA_S3_ACCESS_KEY_ID_FILE",
                "TADA_S3_SECRET_ACCESS_KEY_FILE",
                "TADA_TELEGRAM_BOT_TOKEN_FILE",
            ]
        );
    }

    #[test]
    fn rejects_a_public_url_with_a_path_or_a_query() {
        for bad in [
            "https://tada.example.org/app",
            "https://tada.example.org/?a=1",
            "https://tada.example.org/#x",
            "ftp://tada.example.org",
        ] {
            let errors = worker_with(&[("TADA_PUBLIC_URL", bad)]).unwrap_err();
            assert_eq!(errors.0.len(), 1, "{bad}: {errors}");
            assert!(errors.0[0].starts_with("TADA_PUBLIC_URL "), "{bad}");
        }
    }

    #[test]
    fn rejects_an_invalid_sender_address_without_repeating_it() {
        let errors = worker_with(&[("TADA_MAIL_FROM", "not-an-address")]).unwrap_err();
        assert_eq!(errors.0.len(), 1);
        assert!(errors.0[0].starts_with("TADA_MAIL_FROM "));
        assert!(!errors.0[0].contains("not-an-address"), "{errors}");
    }

    #[cfg(debug_assertions)]
    #[test]
    fn accepts_smtp_without_tls_in_a_debug_build() {
        let (_, _, _, smtp, _, _, _) = worker_with(&[("TADA_MAIL_SMTP_TLS", "none")])
            .unwrap()
            .settings;
        assert_eq!(smtp.tls, SmtpTls::None);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn rejects_smtp_without_tls_in_a_release_build() {
        let errors = worker_with(&[("TADA_MAIL_SMTP_TLS", "none")]).unwrap_err();
        assert!(errors.0[0].starts_with("TADA_MAIL_SMTP_TLS "), "{errors}");
    }

    #[test]
    fn accepts_starttls_and_rejects_an_unknown_tls_mode() {
        let (_, _, _, smtp, _, _, _) = worker_with(&[("TADA_MAIL_SMTP_TLS", "starttls")])
            .unwrap()
            .settings;
        assert_eq!(smtp.tls, SmtpTls::StartTls);
        let errors = worker_with(&[("TADA_MAIL_SMTP_TLS", "maybe")]).unwrap_err();
        assert!(errors.0[0].starts_with("TADA_MAIL_SMTP_TLS "));
        assert!(!errors.0[0].contains("maybe"));
    }

    #[test]
    fn reads_the_smtp_user_and_password_as_a_pair() {
        let password = secret_file("smtp-s3cr3t\n");
        let path = password.path().to_str().unwrap();
        let (_, _, _, smtp, _, _, _) = worker_with(&[
            ("TADA_MAIL_SMTP_USERNAME", "tada"),
            ("TADA_MAIL_SMTP_PASSWORD_FILE", path),
        ])
        .unwrap()
        .settings;
        let (username, secret) = smtp.credentials.unwrap();
        assert_eq!(username, "tada");
        assert_eq!(secret.expose_secret(), "smtp-s3cr3t");

        let errors = worker_with(&[("TADA_MAIL_SMTP_USERNAME", "tada")]).unwrap_err();
        assert_eq!(
            errors.0,
            ["TADA_MAIL_SMTP_PASSWORD_FILE must be set together with TADA_MAIL_SMTP_USERNAME"]
        );
        let errors = worker_with(&[("TADA_MAIL_SMTP_PASSWORD_FILE", path)]).unwrap_err();
        assert_eq!(
            errors.0,
            ["TADA_MAIL_SMTP_USERNAME must be set together with TADA_MAIL_SMTP_PASSWORD_FILE"]
        );
    }

    /// The Telegram sections with a token file, and `extra`. Returns the gateway section.
    fn telegram_with(extra: &[(&str, &str)]) -> Result<TelegramGateway, SettingsErrors> {
        let token = secret_file("123:test-token\n");
        let path = token.path().to_str().unwrap().to_owned();
        let mut variables = vec![("TADA_TELEGRAM_BOT_TOKEN_FILE", path.as_str())];
        variables.extend_from_slice(extra);
        load_from::<(TelegramBot, TelegramGateway)>(&variables).map(|loaded| loaded.settings.1)
    }

    #[test]
    fn the_bot_needs_only_its_token() {
        let token = secret_file("123:test-token\n");
        let path = token.path().to_str().unwrap();
        // A webhook mode without its secret does not stop a role that only sends.
        let bot = load_from::<(TelegramBot,)>(&[
            ("TADA_TELEGRAM_BOT_TOKEN_FILE", path),
            ("TADA_TELEGRAM_MODE", "webhook"),
        ])
        .unwrap()
        .settings
        .0;
        assert_eq!(bot.bot_token.expose_secret(), "123:test-token");
        assert_eq!(bot.api_url.as_str(), "https://api.telegram.org/");
    }

    #[test]
    fn telegram_polls_by_default_and_listens_on_8081() {
        let telegram = telegram_with(&[]).unwrap();
        assert!(matches!(telegram.source, UpdateSource::Polling));
        assert_eq!(telegram.listen.to_string(), "0.0.0.0:8081");
    }

    #[test]
    fn telegram_webhook_mode_needs_a_valid_secret_and_the_public_url() {
        let errors = telegram_with(&[("TADA_TELEGRAM_MODE", "webhook")]).unwrap_err();
        let names: Vec<&str> = errors
            .0
            .iter()
            .map(|error| error.split(' ').next().unwrap())
            .collect();
        assert_eq!(
            names,
            ["TADA_TELEGRAM_WEBHOOK_SECRET_FILE", "TADA_PUBLIC_URL"]
        );

        let bad = secret_file("secret with spaces\n");
        let errors = telegram_with(&[
            ("TADA_TELEGRAM_MODE", "webhook"),
            (
                "TADA_TELEGRAM_WEBHOOK_SECRET_FILE",
                bad.path().to_str().unwrap(),
            ),
            ("TADA_PUBLIC_URL", "https://tada.example.org"),
        ])
        .unwrap_err();
        assert_eq!(errors.0.len(), 1, "{errors}");
        assert!(errors.0[0].starts_with("TADA_TELEGRAM_WEBHOOK_SECRET_FILE "));
        assert!(!errors.0[0].contains("spaces"), "{errors}");

        // Valid characters, but too short to resist a brute force.
        let short = secret_file("test-webhook_Secret-42\n");
        let errors = telegram_with(&[
            ("TADA_TELEGRAM_MODE", "webhook"),
            (
                "TADA_TELEGRAM_WEBHOOK_SECRET_FILE",
                short.path().to_str().unwrap(),
            ),
            ("TADA_PUBLIC_URL", "https://tada.example.org"),
        ])
        .unwrap_err();
        assert_eq!(errors.0.len(), 1, "{errors}");
        assert!(errors.0[0].starts_with("TADA_TELEGRAM_WEBHOOK_SECRET_FILE "));
        assert!(errors.0[0].contains("32 to 256"), "{errors}");
        assert!(!errors.0[0].contains("Secret-42"), "{errors}");

        // The edge: 31 characters fail, 32 pass.
        for (secret, valid) in [("a".repeat(31), false), ("a".repeat(32), true)] {
            let file = secret_file(&format!("{secret}\n"));
            let result = telegram_with(&[
                ("TADA_TELEGRAM_MODE", "webhook"),
                (
                    "TADA_TELEGRAM_WEBHOOK_SECRET_FILE",
                    file.path().to_str().unwrap(),
                ),
                ("TADA_PUBLIC_URL", "https://tada.example.org"),
            ]);
            assert_eq!(result.is_ok(), valid, "{} characters", secret.len());
        }

        let good = secret_file("test-webhook_Secret-42_0123456789ab\n");
        let telegram = telegram_with(&[
            ("TADA_TELEGRAM_MODE", "webhook"),
            (
                "TADA_TELEGRAM_WEBHOOK_SECRET_FILE",
                good.path().to_str().unwrap(),
            ),
            ("TADA_PUBLIC_URL", "https://tada.example.org"),
        ])
        .unwrap();
        let UpdateSource::Webhook { secret, public_url } = telegram.source else {
            panic!("webhook mode gives a webhook source");
        };
        assert_eq!(
            secret.expose_secret(),
            "test-webhook_Secret-42_0123456789ab"
        );
        assert_eq!(public_url.origin(), "https://tada.example.org");

        let errors = telegram_with(&[("TADA_TELEGRAM_MODE", "push")]).unwrap_err();
        assert!(errors.0[0].starts_with("TADA_TELEGRAM_MODE "), "{errors}");
    }

    #[test]
    fn settings_need_check_date_with_key() {
        let key = secret_file("sk-test-not-a-real-key\n");
        let key_path = key.path().to_str().unwrap();

        let errors = load_from::<(Ai,)>(&[("TADA_MODEL_API_KEY_FILE", key_path)]).unwrap_err();
        assert_eq!(
            errors.0,
            [
                "TADA_MODEL_PROVIDER_CHECKED_ON must be set together with TADA_MODEL_API_KEY_FILE (ADR 0045)"
            ]
        );

        let errors = load_from::<(Ai,)>(&[
            ("TADA_MODEL_API_KEY_FILE", key_path),
            ("TADA_MODEL_PROVIDER_CHECKED_ON", "last week"),
        ])
        .unwrap_err();
        assert_eq!(errors.0.len(), 1);
        assert!(errors.0[0].starts_with("TADA_MODEL_PROVIDER_CHECKED_ON is not a valid date"));
        assert!(!errors.0[0].contains("last week"), "{errors}");

        let (ai,) = load_from::<(Ai,)>(&[
            ("TADA_MODEL_API_KEY_FILE", key_path),
            ("TADA_MODEL_PROVIDER_CHECKED_ON", "2026-10-01"),
        ])
        .unwrap()
        .settings;
        let provider = ai.provider.as_ref().unwrap();
        assert_eq!(provider.api_key.expose_secret(), "sk-test-not-a-real-key");
        assert_eq!(provider.api_url.as_str(), "https://api.anthropic.com/");
        assert_eq!(provider.checked_on.to_string(), "2026-10-01");
        assert!(ai.setup().configured);
        assert!(!format!("{ai:?}").contains("sk-test"));

        // Without a key, no AI work runs, and the date alone is allowed.
        let (ai,) = load_from::<(Ai,)>(&[("TADA_MODEL_PROVIDER_CHECKED_ON", "2026-10-01")])
            .unwrap()
            .settings;
        assert!(ai.provider.is_none());
        assert!(!ai.setup().configured);
    }

    #[test]
    fn the_ai_limits_have_the_defaults_and_reject_zero() {
        let (ai,) = load_from::<(Ai,)>(&[]).unwrap().settings;
        assert_eq!(ai.setup().default_cap.to_string(), "25.00");
        assert_eq!(ai.limits.max_turns, 6);
        assert_eq!(ai.limits.max_cost.to_string(), "0.05");
        assert_eq!(ai.limits.run_timeout, jiff::SignedDuration::from_secs(60));
        assert_eq!(ai.limits.call_timeout, jiff::SignedDuration::from_secs(20));
        assert_eq!(ai.limits.call_retries, 2);

        let (ai,) = load_from::<(Ai,)>(&[
            ("TADA_AI_RUN_TIMEOUT", "2m"),
            ("TADA_AI_CALL_TIMEOUT", "1m 30s"),
        ])
        .unwrap()
        .settings;
        assert_eq!(ai.limits.run_timeout, jiff::SignedDuration::from_secs(120));
        assert_eq!(ai.limits.call_timeout, jiff::SignedDuration::from_secs(90));

        let errors = load_from::<(Ai,)>(&[
            ("TADA_AI_DEFAULT_MONTHLY_CAP_CHF", "0"),
            ("TADA_AI_RUN_MAX_TURNS", "0"),
            ("TADA_AI_RUN_MAX_COST_CHF", "free"),
            ("TADA_AI_RUN_TIMEOUT", "0s"),
            ("TADA_AI_CALL_TIMEOUT", "soon"),
        ])
        .unwrap_err();
        let names: Vec<&str> = errors
            .0
            .iter()
            .map(|error| error.split(' ').next().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "TADA_AI_DEFAULT_MONTHLY_CAP_CHF",
                "TADA_AI_RUN_MAX_TURNS",
                "TADA_AI_RUN_MAX_COST_CHF",
                "TADA_AI_RUN_TIMEOUT",
                "TADA_AI_CALL_TIMEOUT",
            ]
        );
        for value in ["free", "soon"] {
            assert!(!errors.to_string().contains(value), "{errors}");
        }

        let errors = load_from::<(Ai,)>(&[("TADA_AI_CALL_TIMEOUT", "61s")]).unwrap_err();
        assert_eq!(
            errors.0,
            ["TADA_AI_CALL_TIMEOUT must not be longer than TADA_AI_RUN_TIMEOUT"]
        );
    }

    #[test]
    fn settings_defaults_listed() {
        let reference = reference();
        for (name, default, serve) in [
            ("TADA_MODEL_API_KEY_FILE", "none", true),
            ("TADA_MODEL_API_URL", "`https://api.anthropic.com`", false),
            ("TADA_MODEL_PROVIDER_CHECKED_ON", "empty", true),
            ("TADA_AI_DEFAULT_MONTHLY_CAP_CHF", "`25`", true),
            ("TADA_AI_RUN_MAX_TURNS", "`6`", false),
            ("TADA_AI_RUN_MAX_COST_CHF", "`0.05`", false),
            ("TADA_AI_RUN_TIMEOUT", "`60s`", false),
            ("TADA_AI_CALL_TIMEOUT", "`20s`", false),
            ("TADA_AI_CALL_RETRIES", "`2`", false),
        ] {
            let row = reference
                .lines()
                .find(|line| line.starts_with(&format!("| `{name}` ")))
                .unwrap_or_else(|| panic!("{name} is not listed"));
            let cells: Vec<&str> = row.split('|').map(str::trim).collect();
            assert_eq!(cells[3], default, "{name}");
            assert!(cells[5].contains("`worker`"), "{name}");
            assert_eq!(cells[5].contains("`serve`"), serve, "{name}");
        }
    }

    #[test]
    fn serve_learns_the_ai_setup_without_reading_the_key() {
        // A key file that nobody can read: `serve` checks only that it exists.
        let key = secret_file("sk-test-not-a-real-key");
        fs::set_permissions(key.path(), fs::Permissions::from_mode(0o000)).unwrap();
        let key_path = key.path().to_str().unwrap();

        let (setup,) = load_from::<(AiSetup,)>(&[
            ("TADA_MODEL_API_KEY_FILE", key_path),
            ("TADA_MODEL_PROVIDER_CHECKED_ON", "2026-10-01"),
        ])
        .unwrap()
        .settings;
        assert!(setup.configured);
        assert_eq!(setup.default_cap.to_string(), "25.00");
        // The load succeeds although the file cannot be read, so `serve` holds no key.
        assert!(!format!("{setup:?}").contains("sk-test"));

        let errors = load_from::<(AiSetup,)>(&[("TADA_MODEL_API_KEY_FILE", key_path)]).unwrap_err();
        assert_eq!(
            errors.0,
            [
                "TADA_MODEL_PROVIDER_CHECKED_ON must be set together with TADA_MODEL_API_KEY_FILE (ADR 0045)"
            ]
        );
        let errors = load_from::<(AiSetup,)>(&[
            ("TADA_MODEL_API_KEY_FILE", "/nonexistent/model-key"),
            ("TADA_MODEL_PROVIDER_CHECKED_ON", "2026-10-01"),
        ])
        .unwrap_err();
        assert_eq!(
            errors.0,
            ["TADA_MODEL_API_KEY_FILE names a file that does not exist"]
        );

        let (setup,) = load_from::<(AiSetup,)>(&[("TADA_MODEL_PROVIDER_CHECKED_ON", "2026-10-01")])
            .unwrap()
            .settings;
        assert!(!setup.configured);
    }
}
