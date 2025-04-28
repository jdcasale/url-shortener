use crate::store::chunk_storage::store::ChunkStore;
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
use aws_sdk_s3::config::Builder;
use tide::log;

/// Creates an S3 client that points at the local MinIO server.
pub async fn create_minio_s3_client() -> S3Client {
    // Set up region (MinIO doesn’t care much but SDK requires it)
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let base_config = aws_config::defaults(BehaviorVersion::latest()).region(region_provider).load().await;

    // Override the endpoint to point to MinIO

    // Build a custom config
    let config = Builder::from(&base_config)
        .endpoint_url("http://minio:9000")
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
        let mut ready = self.bucket_ready.lock().await;

        if *ready {
            // Bucket already created
            return Ok(());
        }

        match self.s3_client.head_bucket().bucket(&self.bucket).send().await {
            Ok(_) => {
                // Bucket exists
                Ok(())
            }
            Err(SdkError::ServiceError(service_err)) => {
                if service_err.err().is_not_found() {
                    // Bucket does not exist, create it
                    self.s3_client
                        .create_bucket()
                        .bucket(&self.bucket)
                        .send()
                        .await?;


                    *ready = true;
                    Ok(())
                } else {
                    Err(SdkError::ServiceError(service_err).into())
                }
            }
            Err(e) => {
                Err(e.into())
            }
        }
        }
    }



#[async_trait]
impl ChunkStore for MinioChunkStore {
    async fn put_chunk(&self, chunk_id: &str, data: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>> {
        log::info!("ENSURING BUCKET");
        self.ensure_bucket_exists().await?;
        log::info!("UPLOADING CHUNK");
        self.s3_client
            .put_object()
            .bucket(&self.bucket)
            .key(chunk_id)
            .body(ByteStream::from(data.to_vec()))
            .send()
            .await?;
        Ok(())
    }

    async fn get_chunk(&self, chunk_id: &str) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        log::info!("ENSURING BUCKET");
        self.ensure_bucket_exists().await?;
        log::info!("GETTING CHUNK");

        let resp = self.s3_client
            .get_object()
            .bucket(&self.bucket)
            .key(chunk_id)
            .send()
            .await?;
        let data = resp.body.collect().await?.into_bytes().to_vec();
        Ok(data)
    }

    async fn delete_chunk(&self, chunk_id: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.ensure_bucket_exists().await?;
        self.s3_client
            .delete_object()
            .bucket(&self.bucket)
            .key(chunk_id)
            .send()
            .await?;
        Ok(())
    }
}