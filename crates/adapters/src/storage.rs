//! Object storage through the S3 API (ADR 0009).

use async_trait::async_trait;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region, RequestChecksumCalculation};
use secrecy::{ExposeSecret, SecretString};
use tada_app::health::{DependencyCheck, DependencyUnavailable};

/// The connection data of one S3 bucket.
#[derive(Debug)]
pub struct S3Config {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key_id: SecretString,
    pub secret_access_key: SecretString,
}

/// An S3-compatible object storage, for example Garage.
#[derive(Debug, Clone)]
pub struct S3Storage {
    client: Client,
    bucket: String,
}

impl S3Storage {
    pub fn new(config: S3Config) -> Self {
        let credentials = Credentials::new(
            config.access_key_id.expose_secret(),
            config.secret_access_key.expose_secret(),
            None,
            None,
            "tada-settings",
        );
        let s3_config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .endpoint_url(config.endpoint)
            .region(Region::new(config.region))
            .credentials_provider(credentials)
            // Garage needs path-style addresses and no checksums on each request (ADR 0009).
            .force_path_style(true)
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .build();
        Self {
            client: Client::from_conf(s3_config),
            bucket: config.bucket,
        }
    }
}

#[async_trait]
impl DependencyCheck for S3Storage {
    fn name(&self) -> &'static str {
        "object-storage"
    }

    async fn check(&self) -> Result<(), DependencyUnavailable> {
        self.client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .map(|_| ())
            .map_err(|error| DependencyUnavailable(Box::new(error)))
    }
}
