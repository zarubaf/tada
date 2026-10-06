//! The settings sections and the sections of each command.

use secrecy::SecretString;
use tracing_subscriber::EnvFilter;
use url::Url;

use super::{Section, Setting, Source};

// Each command also reads `Logging`; see `crate::main`.

/// The settings of `tada serve`.
pub type ServeSettings = (Database, Http, Storage);
/// The settings of `tada worker`.
pub type WorkerSettings = (Database,);
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
}

const PORT: Setting = Setting {
    name: "TADA_PORT",
    kind: "port number",
    default: Some("8080"),
    secret: false,
    description: "The TCP port of the HTTP server. The server does not handle TLS.",
};

impl Section for Http {
    fn settings() -> Vec<&'static Setting> {
        vec![&PORT]
    }

    fn read(source: &mut Source<'_>) -> Option<Self> {
        Some(Self {
            port: source.value(&PORT)?,
        })
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
