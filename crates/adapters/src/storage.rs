//! Object storage through the S3 API (ADR 0009).

use std::io;

use async_trait::async_trait;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::timeout::TimeoutConfig;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region, RequestChecksumCalculation};
use aws_sdk_s3::primitives::ByteStream as S3Body;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use bytes::{Bytes, BytesMut};
use futures::StreamExt;
use secrecy::{ExposeSecret, SecretString};
use tada_app::blobs::{BlobError, BlobKey, BlobStore, ByteStream};
use tada_app::health::{DependencyCheck, DependencyUnavailable};

#[cfg(any(test, feature = "testing"))]
pub mod testing;

/// The size of one part of a multipart upload. S3 needs at least 5 MiB for each part except the last.
const PART_SIZE: usize = 8 * 1024 * 1024;

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
        Self::with_timeouts(config, None)
    }

    /// Like [`Self::new`] with other timeouts. `None` keeps the SDK defaults. Only tests set them.
    fn with_timeouts(config: S3Config, timeouts: Option<TimeoutConfig>) -> Self {
        let credentials = Credentials::new(
            config.access_key_id.expose_secret(),
            config.secret_access_key.expose_secret(),
            None,
            None,
            "tada-settings",
        );
        let mut s3_config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .endpoint_url(config.endpoint)
            .region(Region::new(config.region))
            .credentials_provider(credentials)
            // Garage needs path-style addresses and no checksums on each request (ADR 0009).
            .force_path_style(true)
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired);
        if let Some(timeouts) = timeouts {
            s3_config = s3_config.timeout_config(timeouts);
        }
        let s3_config = s3_config.build();
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

fn storage_error(error: impl std::error::Error + Send + Sync + 'static) -> BlobError {
    BlobError::Storage(Box::new(error))
}

impl S3Storage {
    /// Uploads the parts of a multipart upload that has started. The caller aborts the upload on an error.
    async fn upload_parts(
        &self,
        key: &BlobKey,
        upload_id: &str,
        first: Bytes,
        body: &mut ByteStream,
        limit: u64,
    ) -> Result<(u64, Vec<CompletedPart>), BlobError> {
        let mut parts = Vec::new();
        let mut size = first.len() as u64;
        let mut buffer = BytesMut::from(first);
        loop {
            let chunk = body.next().await.transpose().map_err(BlobError::Upload)?;
            if let Some(chunk) = &chunk {
                size += chunk.len() as u64;
                if size > limit {
                    return Err(BlobError::TooLarge { limit });
                }
                buffer.extend_from_slice(chunk);
            }
            let last = chunk.is_none();
            if buffer.len() >= PART_SIZE || (last && !buffer.is_empty()) {
                let number = i32::try_from(parts.len() + 1).map_err(storage_error)?;
                let output = self
                    .client
                    .upload_part()
                    .bucket(&self.bucket)
                    .key(key.as_str())
                    .upload_id(upload_id)
                    .part_number(number)
                    .body(S3Body::from(buffer.split().freeze()))
                    .send()
                    .await
                    .map_err(storage_error)?;
                parts.push(
                    CompletedPart::builder()
                        .part_number(number)
                        .set_e_tag(output.e_tag().map(str::to_owned))
                        .build(),
                );
            }
            if last {
                return Ok((size, parts));
            }
        }
    }
}

#[async_trait]
impl BlobStore for S3Storage {
    async fn put(&self, key: &BlobKey, mut body: ByteStream, limit: u64) -> Result<u64, BlobError> {
        // Read up to one part. A small file then needs one request only.
        let mut first = BytesMut::new();
        while first.len() < PART_SIZE {
            match body.next().await.transpose().map_err(BlobError::Upload)? {
                Some(chunk) => {
                    if (first.len() + chunk.len()) as u64 > limit {
                        return Err(BlobError::TooLarge { limit });
                    }
                    first.extend_from_slice(&chunk);
                }
                None => {
                    let size = first.len() as u64;
                    self.client
                        .put_object()
                        .bucket(&self.bucket)
                        .key(key.as_str())
                        .body(S3Body::from(first.freeze()))
                        .send()
                        .await
                        .map_err(storage_error)?;
                    return Ok(size);
                }
            }
        }

        let started = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(key.as_str())
            .send()
            .await
            .map_err(storage_error)?;
        let upload_id = started.upload_id().unwrap_or_default().to_owned();
        let result = match self
            .upload_parts(key, &upload_id, first.freeze(), &mut body, limit)
            .await
        {
            Ok((size, parts)) => self
                .client
                .complete_multipart_upload()
                .bucket(&self.bucket)
                .key(key.as_str())
                .upload_id(&upload_id)
                .multipart_upload(
                    CompletedMultipartUpload::builder()
                        .set_parts(Some(parts))
                        .build(),
                )
                .send()
                .await
                .map(|_| size)
                .map_err(storage_error),
            Err(error) => Err(error),
        };
        if result.is_err() {
            // Without the abort, the parts would stay in the store.
            let aborted = self
                .client
                .abort_multipart_upload()
                .bucket(&self.bucket)
                .key(key.as_str())
                .upload_id(&upload_id)
                .send()
                .await;
            if let Err(error) = aborted {
                tracing::warn!(error = %error, "cannot abort a multipart upload");
            }
        }
        result
    }

    async fn get(&self, key: &BlobKey) -> Result<Option<ByteStream>, BlobError> {
        let output = match self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key.as_str())
            .send()
            .await
        {
            Ok(output) => output,
            Err(error)
                if error
                    .as_service_error()
                    .is_some_and(|error| error.is_no_such_key()) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(storage_error(error)),
        };
        let stream = futures::stream::unfold(output.body, |mut body| async move {
            body.next()
                .await
                .map(|chunk| (chunk.map_err(io::Error::other), body))
        });
        Ok(Some(Box::pin(stream)))
    }

    async fn head(&self, key: &BlobKey) -> Result<Option<u64>, BlobError> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key.as_str())
            .send()
            .await
        {
            Ok(output) => Ok(output
                .content_length()
                .and_then(|size| u64::try_from(size).ok())),
            Err(error)
                if error
                    .as_service_error()
                    .is_some_and(|error| error.is_not_found()) =>
            {
                Ok(None)
            }
            Err(error) => Err(storage_error(error)),
        }
    }

    async fn delete(&self, key: &BlobKey) -> Result<(), BlobError> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key.as_str())
            .send()
            .await
            .map(|_| ())
            .map_err(storage_error)
    }
}

#[cfg(test)]
mod tests {
    use futures::TryStreamExt;
    use tada_app::domain::ids::OrganizationId;
    use uuid::Uuid;

    use super::testing::TestGarage;
    use super::*;

    fn key() -> BlobKey {
        BlobKey::new(OrganizationId::from_uuid(Uuid::now_v7()))
    }

    /// A stream of `size` bytes in chunks of 64 KiB, as an HTTP body arrives.
    fn body(data: &[u8]) -> ByteStream {
        let chunks: Vec<Result<Bytes, io::Error>> = data
            .chunks(64 * 1024)
            .map(|chunk| Ok(Bytes::copy_from_slice(chunk)))
            .collect();
        Box::pin(futures::stream::iter(chunks))
    }

    fn data(size: usize) -> Vec<u8> {
        (0..size).map(|index| (index % 251) as u8).collect()
    }

    async fn read(storage: &S3Storage, key: &BlobKey) -> Option<Vec<u8>> {
        let stream = storage.get(key).await.unwrap()?;
        let chunks: Vec<Bytes> = stream.try_collect().await.unwrap();
        Some(chunks.concat())
    }

    #[tokio::test]
    async fn stores_reads_and_deletes_small_and_large_objects() {
        let garage = TestGarage::start().await;
        let storage = &garage.storage;
        for size in [0, 1000, 2 * PART_SIZE + 12_345] {
            let key = key();
            let content = data(size);
            assert_eq!(
                storage.put(&key, body(&content), u64::MAX).await.unwrap(),
                size as u64
            );
            assert_eq!(storage.head(&key).await.unwrap(), Some(size as u64));
            assert_eq!(
                read(storage, &key).await.as_deref(),
                Some(content.as_slice()),
                "size {size}"
            );
            storage.delete(&key).await.unwrap();
            assert_eq!(storage.head(&key).await.unwrap(), None);
            assert!(read(storage, &key).await.is_none());
        }
    }

    #[tokio::test]
    async fn keeps_nothing_of_an_upload_above_its_limit() {
        let garage = TestGarage::start().await;
        let storage = &garage.storage;
        for (size, limit) in [(1000, 999), (2 * PART_SIZE, PART_SIZE as u64 + 1)] {
            let key = key();
            let result = storage.put(&key, body(&data(size)), limit).await;
            assert!(
                matches!(result, Err(BlobError::TooLarge { .. })),
                "size {size}"
            );
            assert_eq!(storage.head(&key).await.unwrap(), None);
        }
        assert_eq!(
            garage.open_uploads().await,
            0,
            "an aborted multipart upload left parts"
        );
    }

    #[tokio::test]
    async fn keeps_nothing_of_an_upload_whose_stream_fails() {
        let garage = TestGarage::start().await;
        let storage = &garage.storage;
        let key = key();
        let content = data(PART_SIZE + 1000);
        let broken = body(&content).chain(futures::stream::once(async {
            Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "the client disconnected",
            ))
        }));
        let result = storage.put(&key, Box::pin(broken), u64::MAX).await;
        assert!(matches!(result, Err(BlobError::Upload(_))));
        assert_eq!(storage.head(&key).await.unwrap(), None);
        assert_eq!(garage.open_uploads().await, 0);
    }
}
