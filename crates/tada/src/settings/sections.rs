//! The settings sections and the sections of each command.

use std::num::NonZeroU64;
use std::path::PathBuf;
use std::str::FromStr;

use ipnet::IpNet;
use secrecy::{ExposeSecret, SecretString};
use tada_adapters::mail::SmtpTls;
use tada_adapters::storage::{S3Config, S3Storage};
use tada_app::domain::identity::Email;
use tada_app::public_url::PublicUrl;
use tracing_subscriber::EnvFilter;
use url::Url;

use super::{Section, Setting, Source};

// Each command also reads `Logging`; see `crate::main`.

/// The settings of `tada serve`.
pub type ServeSettings = (Database, Http, Storage, PublicUrl, SignIn, Uploads);
/// The settings of `tada worker`.
pub type WorkerSettings = (Database, PublicUrl, Mail, MailSmtp);
/// The settings of `tada telegram`.
pub type TelegramSettings = (Database, Telegram);
/// The settings of `tada bootstrap`.
pub type BootstrapSettings = (Database, PublicUrl);
/// The settings of `tada export` (ADR 0059).
pub type ExportSettings = (Database, Storage);
/// The settings of `tada migrate`.
pub type MigrateSettings = (Database,);

/// The log filter (ADR 0035).
#[derive(Debug)]
pub struct Logging {
    pub filter: String,
}

const LOG: Setting = Setting {
    name: "TADA_LOG",
    kind: "log filter",
    default: Some("info"),
    secret: false,
    description: "The level filter of the logs, in the syntax of `tracing-subscriber`, for example `info,tada_api=debug`.",
};

impl Section for Logging {
    fn settings() -> Vec<&'static Setting> {
        vec![&LOG]
    }

    fn read(source: &mut Source<'_>) -> Option<Self> {
        let filter: String = source.value(&LOG)?;
        if let Err(error) = EnvFilter::try_new(&filter) {
            source.error(&LOG, &format!("is not a valid log filter: {error}"));
            return None;
        }
        Some(Self { filter })
    }
}

/// The PostgreSQL connection (ADRs 0025 and 0036).
#[derive(Debug)]
pub struct Database {
    pub url: String,
    pub password: SecretString,
}

const DATABASE_URL: Setting = Setting {
    name: "TADA_DATABASE_URL",
    kind: "PostgreSQL URL",
    default: None,
    secret: false,
    description: "The database URL without the password, for example `postgres://tada@db:5432/tada`.",
};

const DATABASE_PASSWORD: Setting = Setting {
    name: "TADA_DATABASE_PASSWORD_FILE",
    kind: "file path",
    default: None,
    secret: true,
    description: "The file that contains the database password.",
};

impl Section for Database {
    fn settings() -> Vec<&'static Setting> {
        vec![&DATABASE_URL, &DATABASE_PASSWORD]
    }

    fn read(source: &mut Source<'_>) -> Option<Self> {
        let url: Option<Url> = source.value(&DATABASE_URL);
        let password = source.secret(&DATABASE_PASSWORD);
        let url = url?;
        if !matches!(url.scheme(), "postgres" | "postgresql") {
            source.error(&DATABASE_URL, "must start with postgres://");
            return None;
        }
        if url.password().is_some() {
            source.error(
                &DATABASE_URL,
                "must not contain a password; use TADA_DATABASE_PASSWORD_FILE",
            );
            return None;
        }
        Some(Self {
            url: url.into(),
            password: password?,
        })
    }
}

/// The HTTP server of `tada serve` (ADR 0025).
#[derive(Debug)]
pub struct Http {
    pub port: u16,
    pub trusted_proxies: Vec<IpNet>,
    pub web_root: Option<PathBuf>,
}

const PORT: Setting = Setting {
    name: "TADA_PORT",
    kind: "port number",
    default: Some("8080"),
    secret: false,
    description: "The TCP port of the HTTP server. The server does not handle TLS.",
};

const TRUSTED_PROXIES: Setting = Setting {
    name: "TADA_TRUSTED_PROXIES",
    kind: "list of network ranges",
    default: Some(""),
    secret: false,
    description: "The ranges of the reverse proxies, separated by commas, for example `10.0.0.0/8`. The server accepts `X-Forwarded-For` only from them, and logs their `X-Request-Id` as `proxy_request_id`. A proxy can pass on the value of the client, so this field only helps to find the request in the log of the proxy. It does not identify a request. Set it if a reverse proxy is in front of tada: if it is empty, all clients have the address of the proxy and share one rate limit. The server writes a warning when a request from another peer has `X-Forwarded-For`.",
};

const WEB_ROOT: Setting = Setting {
    name: "TADA_WEB_ROOT",
    kind: "folder path",
    default: Some(""),
    secret: false,
    description: "The folder of the built web client. The image sets it. If it is empty, the server delivers the API only.",
};

impl Section for Http {
    fn settings() -> Vec<&'static Setting> {
        vec![&PORT, &TRUSTED_PROXIES, &WEB_ROOT]
    }

    fn read(source: &mut Source<'_>) -> Option<Self> {
        let port = source.value(&PORT);
        let trusted_proxies: Option<NetworkRanges> = source.value(&TRUSTED_PROXIES);
        let web_root: Option<String> = source.value(&WEB_ROOT);
        let web_root = web_root?;
        let web_root = (!web_root.is_empty()).then(|| PathBuf::from(web_root));
        if let Some(root) = &web_root
            && !root.join("index.html").is_file()
        {
            source.error(&WEB_ROOT, "names a folder without index.html");
            return None;
        }
        Some(Self {
            port: port?,
            trusted_proxies: trusted_proxies?.0,
            web_root,
        })
    }
}

/// Network ranges, separated by commas. An empty text is an empty list.
struct NetworkRanges(Vec<IpNet>);

impl FromStr for NetworkRanges {
    type Err = ipnet::AddrParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        text.split(',')
            .map(str::trim)
            .filter(|range| !range.is_empty())
            .map(IpNet::from_str)
            .collect::<Result<_, _>>()
            .map(Self)
    }
}

/// The S3 object storage (ADR 0009).
#[derive(Debug)]
pub struct Storage {
    pub endpoint: Url,
    pub region: String,
    pub bucket: String,
    pub access_key_id: SecretString,
    pub secret_access_key: SecretString,
}

impl Storage {
    /// The S3 adapter of these settings. Each command that reads or writes objects opens it here.
    pub fn open(self) -> S3Storage {
        S3Storage::new(S3Config {
            endpoint: self.endpoint.into(),
            region: self.region,
            bucket: self.bucket,
            access_key_id: self.access_key_id,
            secret_access_key: self.secret_access_key,
        })
    }
}

const S3_ENDPOINT: Setting = Setting {
    name: "TADA_S3_ENDPOINT",
    kind: "URL",
    default: None,
    secret: false,
    description: "The URL of the S3 API, for example `http://garage:3900`.",
};

const S3_REGION: Setting = Setting {
    name: "TADA_S3_REGION",
    kind: "text",
    default: None,
    secret: false,
    description: "The S3 region. Garage uses the `s3_region` of its configuration.",
};

const S3_BUCKET: Setting = Setting {
    name: "TADA_S3_BUCKET",
    kind: "text",
    default: None,
    secret: false,
    description: "The bucket for all objects of this installation.",
};

const S3_ACCESS_KEY_ID: Setting = Setting {
    name: "TADA_S3_ACCESS_KEY_ID_FILE",
    kind: "file path",
    default: None,
    secret: true,
    description: "The file that contains the S3 access key ID.",
};

const S3_SECRET_ACCESS_KEY: Setting = Setting {
    name: "TADA_S3_SECRET_ACCESS_KEY_FILE",
    kind: "file path",
    default: None,
    secret: true,
    description: "The file that contains the S3 secret access key.",
};

impl Section for Storage {
    fn settings() -> Vec<&'static Setting> {
        vec![
            &S3_ENDPOINT,
            &S3_REGION,
            &S3_BUCKET,
            &S3_ACCESS_KEY_ID,
            &S3_SECRET_ACCESS_KEY,
        ]
    }

    fn read(source: &mut Source<'_>) -> Option<Self> {
        let endpoint = source.value(&S3_ENDPOINT);
        let region = source.value(&S3_REGION);
        let bucket = source.value(&S3_BUCKET);
        let access_key_id = source.secret(&S3_ACCESS_KEY_ID);
        let secret_access_key = source.secret(&S3_SECRET_ACCESS_KEY);
        Some(Self {
            endpoint: endpoint?,
            region: region?,
            bucket: bucket?,
            access_key_id: access_key_id?,
            secret_access_key: secret_access_key?,
        })
    }
}

/// The Telegram gateway (ADR 0011).
#[derive(Debug)]
pub struct Telegram {
    pub bot_token: SecretString,
    pub api_url: Url,
}

const TELEGRAM_BOT_TOKEN: Setting = Setting {
    name: "TADA_TELEGRAM_BOT_TOKEN_FILE",
    kind: "file path",
    default: None,
    secret: true,
    description: "The file that contains the token of the Telegram bot.",
};

const TELEGRAM_API_URL: Setting = Setting {
    name: "TADA_TELEGRAM_API_URL",
    kind: "URL",
    default: Some("https://api.telegram.org"),
    secret: false,
    description: "The base URL of the Telegram Bot API. Tests set a local fake.",
};

impl Section for Telegram {
    fn settings() -> Vec<&'static Setting> {
        vec![&TELEGRAM_BOT_TOKEN, &TELEGRAM_API_URL]
    }

    fn read(source: &mut Source<'_>) -> Option<Self> {
        let bot_token = source.secret(&TELEGRAM_BOT_TOKEN);
        let api_url = source.value(&TELEGRAM_API_URL);
        Some(Self {
            bot_token: bot_token?,
            api_url: api_url?,
        })
    }
}

const PUBLIC_URL: Setting = Setting {
    name: "TADA_PUBLIC_URL",
    kind: "URL",
    default: None,
    secret: false,
    description: "The URL that members use, without a path, for example `https://tada.example.org`. All links in mails start with it.",
};

/// The public URL of this installation (ADRs 0025 and 0042).
/// `PublicUrl::parse` is the only validator, so each process role uses the same normalized URL.
impl Section for PublicUrl {
    fn settings() -> Vec<&'static Setting> {
        vec![&PUBLIC_URL]
    }

    fn read(source: &mut Source<'_>) -> Option<Self> {
        let text: String = source.value(&PUBLIC_URL)?;
        match PublicUrl::parse(&text) {
            Ok(url) => Some(url),
            Err(error) => {
                source.error(&PUBLIC_URL, &format!("is not valid: {error}"));
                None
            }
        }
    }
}

/// The sign-in of `tada serve` (ADR 0056).
#[derive(Debug)]
pub struct SignIn {
    pub rate_limit_key: SecretString,
}

const RATE_LIMIT_KEY: Setting = Setting {
    name: "TADA_RATE_LIMIT_KEY_FILE",
    kind: "file path",
    default: None,
    secret: true,
    description: "The file that contains the key of the rate-limit counters, at least 32 bytes. The counters keep an HMAC of each address with it. All `serve` processes need the same key.",
};

/// The shortest key of the rate-limit counters. A shorter key lets a person with a copy of the
/// counters find the key by brute force, and then each IPv4 address.
const RATE_LIMIT_KEY_MIN_BYTES: usize = 32;

impl Section for SignIn {
    fn settings() -> Vec<&'static Setting> {
        vec![&RATE_LIMIT_KEY]
    }

    fn read(source: &mut Source<'_>) -> Option<Self> {
        let rate_limit_key = source.secret(&RATE_LIMIT_KEY)?;
        // Only the length: the value stays in the secret.
        if rate_limit_key.expose_secret().len() < RATE_LIMIT_KEY_MIN_BYTES {
            source.error(
                &RATE_LIMIT_KEY,
                &format!("must have at least {RATE_LIMIT_KEY_MIN_BYTES} bytes"),
            );
            return None;
        }
        Some(Self { rate_limit_key })
    }
}

/// The uploads of `tada serve` (ADR 0043).
#[derive(Debug)]
pub struct Uploads {
    /// The largest file that one upload can have, in bytes.
    pub max_bytes: NonZeroU64,
}

const UPLOAD_MAX_BYTES: Setting = Setting {
    name: "TADA_UPLOAD_MAX_BYTES",
    kind: "byte count",
    default: Some("100000000"),
    secret: false,
    description: "The largest file that one upload can have, in bytes. Only the upload routes accept a body of this size.",
};

impl Section for Uploads {
    fn settings() -> Vec<&'static Setting> {
        vec![&UPLOAD_MAX_BYTES]
    }

    fn read(source: &mut Source<'_>) -> Option<Self> {
        Some(Self {
            max_bytes: source.value(&UPLOAD_MAX_BYTES)?,
        })
    }
}

/// The settings that all mail adapters share (ADR 0057).
#[derive(Debug)]
pub struct Mail {
    pub from: Email,
}

const MAIL_FROM: Setting = Setting {
    name: "TADA_MAIL_FROM",
    kind: "email address",
    default: None,
    secret: false,
    description: "The sender address of all mails that tada sends.",
};

impl Section for Mail {
    fn settings() -> Vec<&'static Setting> {
        vec![&MAIL_FROM]
    }

    fn read(source: &mut Source<'_>) -> Option<Self> {
        let from: String = source.value(&MAIL_FROM)?;
        match Email::parse(&from) {
            Ok(from) => Some(Self { from }),
            Err(error) => {
                source.error(&MAIL_FROM, &format!("is not a valid address: {error}"));
                None
            }
        }
    }
}

/// The SMTP adapter for outbound mail (ADRs 0042 and 0057).
#[derive(Debug)]
pub struct MailSmtp {
    pub host: String,
    pub port: u16,
    pub tls: SmtpTls,
    pub credentials: Option<(String, SecretString)>,
}

const SMTP_HOST: Setting = Setting {
    name: "TADA_MAIL_SMTP_HOST",
    kind: "host name",
    default: None,
    secret: false,
    description: "The host name of the SMTP server. It must match the certificate of the server.",
};

const SMTP_PORT: Setting = Setting {
    name: "TADA_MAIL_SMTP_PORT",
    kind: "port number",
    default: Some("465"),
    secret: false,
    description: "The TCP port of the SMTP server. Use 465 for `implicit` TLS and 587 for `starttls`.",
};

const SMTP_USERNAME: Setting = Setting {
    name: "TADA_MAIL_SMTP_USERNAME",
    kind: "text",
    default: Some(""),
    secret: false,
    description: "The user name for the SMTP server. If it is empty, tada sends without a login. Set it together with `TADA_MAIL_SMTP_PASSWORD_FILE`.",
};

const SMTP_PASSWORD: Setting = Setting {
    name: "TADA_MAIL_SMTP_PASSWORD_FILE",
    kind: "file path",
    default: None,
    secret: true,
    description: "The file that contains the SMTP password. Optional; set it together with `TADA_MAIL_SMTP_USERNAME`.",
};

const SMTP_TLS: Setting = Setting {
    name: "TADA_MAIL_SMTP_TLS",
    kind: "`implicit`, `starttls` or `none`",
    default: Some("implicit"),
    secret: false,
    description: "How tada secures the connection. `implicit` uses TLS from the start, and `starttls` requires an upgrade. `none` is only for a local development server; release builds reject it.",
};

impl Section for MailSmtp {
    fn settings() -> Vec<&'static Setting> {
        vec![
            &SMTP_HOST,
            &SMTP_PORT,
            &SMTP_USERNAME,
            &SMTP_PASSWORD,
            &SMTP_TLS,
        ]
    }

    fn read(source: &mut Source<'_>) -> Option<Self> {
        let host: Option<String> = source.value(&SMTP_HOST);
        let port = source.value(&SMTP_PORT);
        let username: Option<String> = source.value(&SMTP_USERNAME);
        let password = source.optional_secret(&SMTP_PASSWORD);
        let tls = Self::read_tls(source);
        let credentials = match (username?, password?) {
            (username, None) if username.is_empty() => None,
            (username, Some(password)) if !username.is_empty() => Some((username, password)),
            (username, _) if username.is_empty() => {
                source.error(
                    &SMTP_USERNAME,
                    "must be set together with TADA_MAIL_SMTP_PASSWORD_FILE",
                );
                return None;
            }
            _ => {
                source.error(
                    &SMTP_PASSWORD,
                    "must be set together with TADA_MAIL_SMTP_USERNAME",
                );
                return None;
            }
        };
        Some(Self {
            host: host?,
            port: port?,
            tls: tls?,
            credentials,
        })
    }
}

impl MailSmtp {
    fn read_tls(source: &mut Source<'_>) -> Option<SmtpTls> {
        let mode: String = source.value(&SMTP_TLS)?;
        match mode.as_str() {
            "implicit" => Some(SmtpTls::Implicit),
            "starttls" => Some(SmtpTls::StartTls),
            #[cfg(debug_assertions)]
            "none" => Some(SmtpTls::None),
            #[cfg(not(debug_assertions))]
            "none" => {
                source.error(&SMTP_TLS, "must not be `none` in a release build");
                None
            }
            _ => {
                source.error(&SMTP_TLS, "must be `implicit`, `starttls` or `none`");
                None
            }
        }
    }
}
