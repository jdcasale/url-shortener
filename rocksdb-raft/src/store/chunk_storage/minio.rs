use crate::store::chunk_storage::store::{ChunkResult, ChunkStore};
use async_trait::async_trait;
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client as S3Client;
use std::error::Error;
use std::sync::Arc;
use aws_config::BehaviorVersion;

/// A `ChunkStore` implementation that stores chunks in a MinIO server via S3 API.
#[derive(Clone, Debug)]
pub struct MinioChunkStore {
    s3_client: S3Client,
    bucket: String,
    bucket_ready: Arc<tokio::sync::Mutex<bool>>,
}


use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::config::{Builder, Credentials};

/// Creates an S3 client that points at the local MinIO server.
pub async fn create_minio_s3_client(host: &str, port: u16, test: bool) -> S3Client {
    // Set up region (MinIO doesn’t care much but SDK requires it)
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");

    let mut builder = aws_config::defaults(BehaviorVersion::latest())
        .region(region_provider);
    if test {
        // if we're running tests, just jam the defaults in
        let creds = Credentials::new("minioadmin", "minioadmin", None, None, "static");
        builder = builder.credentials_provider(creds)
    }
    let base_config = builder
        .load()
        .await;

    // Override the endpoint to point to MinIO

    // Build a custom config
    let config = Builder::from(&base_config)
        .endpoint_url(format!("http://{host}:{port}"))
        .force_path_style(true) // Important! MinIO requires path-style
        .build();

    S3Client::from_conf(config)
}



impl MinioChunkStore {
    /// Creates a new `MinioChunkStore`.
    pub fn new(s3_client: S3Client, bucket: impl Into<String>) -> Self {
        Self {
            s3_client,
            bucket: bucket.into(),
            bucket_ready:     Arc::new(tokio::sync::Mutex::new(false)),

        }
    }

    async fn ensure_bucket_exists(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        tracing::info!(bucket = %self.bucket, "Ensuring that bucket exists");
        let mut ready = self.bucket_ready.lock().await;

        if *ready {
            // Bucket already created
            return Ok(());
        }

        match self.s3_client.head_bucket().bucket(&self.bucket).send().await {
            Ok(_) => {
                *ready = true; // <-- mark ready
                Ok(())
            },
            Err(SdkError::ServiceError(ref service_err)) if service_err.err().is_not_found() => {
                self.s3_client.create_bucket().bucket(&self.bucket).send().await?;
                *ready = true;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
}



#[async_trait]
impl ChunkStore for MinioChunkStore {
    async fn put_chunk(&self, chunk_id: &str, data: &[u8]) -> ChunkResult<()> {
        self.ensure_bucket_exists().await?;
        tracing::info!(chunk_id = %chunk_id, "Uploading chunk");
        self.s3_client
            .put_object()
            .bucket(&self.bucket)
            .key(chunk_id)
            .body(ByteStream::from(data.to_vec()))
            .send()
            .await?;
        Ok(())
    }

    async fn get_chunk(&self, chunk_id: &str) -> ChunkResult<Vec<u8>> {
        self.ensure_bucket_exists().await?;
        tracing::info!(chunk_id = %chunk_id, "Getting chunk");

        let resp = self.s3_client
            .get_object()
            .bucket(&self.bucket)
            .key(chunk_id)
            .send()
            .await?;
        let data = resp.body.collect().await?.into_bytes().to_vec();
        Ok(data)
    }

    async fn delete_chunk(&self, chunk_id: &str) -> ChunkResult<()> {
        self.ensure_bucket_exists().await?;
        tracing::info!(chunk_id = %chunk_id, "Deleting chunk");
        self.s3_client
            .delete_object()
            .bucket(&self.bucket)
            .key(chunk_id)
            .send()
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tokio::runtime::Runtime;
    use uuid::Uuid;
    use testcontainers::{ContainerAsync, GenericImage, ImageExt};
    use testcontainers::runners::{AsyncRunner};
    use testcontainers::core::ContainerPort;

    async fn setup() -> (ContainerAsync<GenericImage>, u16) {
        unsafe {
            env::set_var("AWS_ACCESS_KEY_ID", "min3ioadmin");
            env::set_var("AWS_SECRET_ACCESS_KEY", "3minioadmin")
        }

        // Start MinIO
        let image = GenericImage::new("minio/minio", "latest")
            .with_wait_for(testcontainers::core::WaitFor::message_on_stderr(
                "API:",
            ))
            .with_cmd(vec!["minio", "server", "/data", "--console-address", ":9001"])
            .with_env_var("MINIO_ROOT_USER", "minioadmin")
            .with_env_var("MINIO_ROOT_PASSWORD", "minioadmin")
            .with_mapped_port(9000, ContainerPort::Tcp(9000))
            .with_mapped_port(9001, ContainerPort::Tcp(9001));

        let minio = image.start().await.unwrap();

        let minio_port = minio.get_host_port_ipv4(9000).await.unwrap();

        println!("MinIO accessible at http://localhost:{minio_port}");
        (minio, minio_port)
    }


    #[test]
    fn test_upload_chunk() {

        let string = Uuid::new_v4().to_string();
        let chunk_id = string.as_str();
        let data = b"this is test data";

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let (_minio, port) = setup().await;
            let s3_client = create_minio_s3_client("localhost", port, true).await;
            let uu = Uuid::new_v4().to_string();
            let blob_store = MinioChunkStore::new(s3_client, format!("test-{uu}"));
            blob_store.put_chunk(chunk_id, data).await.expect("upload failed");

            let retrieved = blob_store.get_chunk(chunk_id).await.expect("download failed");
            blob_store.delete_chunk(chunk_id).await.expect("deleted");
            assert_eq!(retrieved, data);
        });
    }

    #[test]
    fn test_upload_chunk_overwrite() {
        let string = Uuid::new_v4().to_string();
        let chunk_id = string.as_str();
        let data1 = b"data one";
        let data2 = b"data two";

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let (_minio, port) = setup().await;
            let s3_client = create_minio_s3_client("localhost", port, true).await;
            let uu = Uuid::new_v4().to_string();
            let blob_store = MinioChunkStore::new(s3_client, format!("test-{uu}"));
            blob_store.put_chunk(chunk_id, data1).await.expect("first upload failed");
            blob_store.put_chunk(chunk_id, data2).await.expect("second upload failed");

            let retrieved = blob_store.get_chunk(chunk_id).await.expect("download failed");
            blob_store.delete_chunk(chunk_id).await.expect("deleted");
            assert_eq!(retrieved, data2);
        });
    }
}
