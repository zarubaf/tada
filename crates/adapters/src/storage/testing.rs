//! A Garage container with one bucket, for the tests of this crate and of other crates (ADR 0003).

use aws_sdk_s3::config::timeout::TimeoutConfig;
use secrecy::SecretString;
use testcontainers_modules::testcontainers::core::{ExecCommand, IntoContainerPort};
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, GenericImage, ImageExt};
use uuid::Uuid;

use super::{S3Config, S3Storage};

/// The Garage image of `compose.yaml`.
const GARAGE: (&str, &str) = ("dxflrs/garage", "v2.4.1");
const BUCKET: &str = "tada-test";

/// A one-node Garage with a bucket and a key. The secrets are random for each container.
/// The container stops when this value is dropped.
#[derive(Debug)]
pub struct TestGarage {
    pub storage: S3Storage,
    _container: ContainerAsync<GenericImage>,
}

/// Hex digits from the random part of UUIDv7 values.
fn random_hex(bytes: usize) -> String {
    let mut hex = String::new();
    while hex.len() < bytes * 2 {
        hex.push_str(&Uuid::now_v7().simple().to_string()[16..]);
    }
    hex.truncate(bytes * 2);
    hex
}

#[allow(clippy::unwrap_used)]
async fn garage(container: &ContainerAsync<GenericImage>, args: &[&str]) -> String {
    let mut command = vec!["/garage"];
    command.extend_from_slice(args);
    let mut result = container.exec(ExecCommand::new(command)).await.unwrap();
    let stdout = result.stdout_to_vec().await.unwrap();
    assert_eq!(
        result.exit_code().await.unwrap(),
        Some(0),
        "garage {args:?} failed"
    );
    String::from_utf8(stdout).unwrap()
}

impl TestGarage {
    /// Starts a new container with an empty bucket.
    ///
    /// # Panics
    ///
    /// If Docker is not available or Garage does not start.
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    pub async fn start() -> Self {
        let config = format!(
            "metadata_dir = \"/tmp/meta\"\ndata_dir = \"/tmp/data\"\ndb_engine = \"sqlite\"\n\
             replication_factor = 1\nrpc_bind_addr = \"[::]:3901\"\nrpc_public_addr = \"127.0.0.1:3901\"\n\
             rpc_secret = \"{}\"\n[s3_api]\ns3_region = \"garage\"\napi_bind_addr = \"[::]:3900\"\n",
            random_hex(32)
        );
        let container = GenericImage::new(GARAGE.0, GARAGE.1)
            .with_exposed_port(3900.tcp())
            .with_copy_to("/etc/garage.toml", config.into_bytes())
            .start()
            .await
            .expect("cannot start Garage; is Docker running?");

        // Production fails fast; a container on a loaded machine needs up to a minute to answer.
        let mut node = String::new();
        let mut ready = false;
        for _ in 0..300 {
            let mut result = container
                .exec(ExecCommand::new(["/garage", "node", "id", "--quiet"]))
                .await
                .unwrap();
            node = String::from_utf8(result.stdout_to_vec().await.unwrap()).unwrap();
            if result.exit_code().await.unwrap() == Some(0) {
                ready = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        assert!(ready, "Garage did not answer within 60 s");
        let node = node.trim().split('@').next().unwrap().to_owned();
        garage(
            &container,
            &[
                "layout",
                "assign",
                "--zone",
                "test",
                "--capacity",
                "1G",
                &node,
            ],
        )
        .await;
        garage(&container, &["layout", "apply", "--version", "1"]).await;
        let key_id = format!("GK{}", random_hex(12));
        let secret = random_hex(32);
        garage(
            &container,
            &["key", "import", "--yes", "-n", "test", &key_id, &secret],
        )
        .await;
        garage(&container, &["bucket", "create", BUCKET]).await;
        garage(
            &container,
            &[
                "bucket", "allow", "--read", "--write", "--owner", BUCKET, "--key", &key_id,
            ],
        )
        .await;

        let port = container.get_host_port_ipv4(3900).await.unwrap();
        let timeouts = TimeoutConfig::builder()
            .connect_timeout(std::time::Duration::from_secs(60))
            .operation_attempt_timeout(std::time::Duration::from_secs(120))
            .build();
        let storage = S3Storage::with_timeouts(
            S3Config {
                endpoint: format!("http://127.0.0.1:{port}"),
                region: "garage".to_owned(),
                bucket: BUCKET.to_owned(),
                access_key_id: SecretString::from(key_id),
                secret_access_key: SecretString::from(secret),
            },
            Some(timeouts),
        );
        Self {
            storage,
            _container: container,
        }
    }

    /// The number of multipart uploads that are neither complete nor aborted.
    ///
    /// # Panics
    ///
    /// If the request fails.
    #[allow(clippy::unwrap_used)]
    pub async fn open_uploads(&self) -> usize {
        let output = self
            .storage
            .client
            .list_multipart_uploads()
            .bucket(BUCKET)
            .send()
            .await
            .unwrap();
        output.uploads().len()
    }
}
