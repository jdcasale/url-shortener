use async_trait::async_trait;
use aws_sdk_s3::{Client as S3Client, Client};
use aws_config;
use std::error::Error;
use aws_config::BehaviorVersion;
use aws_sdk_s3::primitives::ByteStream;
use crate::store::chunk_storage::store::ChunkStore;

/// TigrisBlobStore uses an S3-compatible API to store chunks.
#[derive(Clone, Debug)]
pub struct TigrisBlobStore {
    bucket: String,
    client: S3Client,
}

impl TigrisBlobStore {
    /// Creates a new instance by loading AWS configuration from the environment,
    /// overriding the endpoint and region.
    /// The S3-compatible Tigris endpoint should be provided as a URL (e.g., "https://tigris.example.com").
    pub async fn new(
        // bucket: String,
        // endpoint_url: &str,
        // region: &str,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let shared_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        // Instantiate the S3 client using the loaded configuration
        let s3_client = Client::new(&shared_config);
        Ok(Self { bucket: String::from("url-shortener-23569127845"), client: s3_client })
    }
}

#[async_trait]
impl ChunkStore for TigrisBlobStore {
    async fn put_chunk(&self, chunk_id: &str, data: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.client.put_object()
            .bucket(&self.bucket)
            .key(chunk_id)
            .body(ByteStream::from(data.to_vec()))
            .send()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        Ok(())
    }

    async fn get_chunk(&self, chunk_id: &str) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let resp = self.client.get_object()
            .bucket(&self.bucket)
            .key(chunk_id)
            .send()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        let data = resp.body.collect().await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        Ok(data.into_bytes().to_vec())
    }

    async fn delete_chunk(&self, chunk_id: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let _resp = self.client.delete_object()
            .bucket(&self.bucket)
            .key(chunk_id)
            .send()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;
    use uuid::Uuid;
    // Note: Make sure to set the TEST_BUCKET environment variable
    // and have your AWS credentials and endpoint (if needed) configured
    // in your environment (or via a ~/.aws/credentials file).

    #[test]
    fn test_upload_chunk() {
        let string = Uuid::new_v4().to_string();
        let chunk_id = string.as_str();
        let data = b"this is test data";

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let blob_store = TigrisBlobStore::new().await
                .expect("failed to create TigrisBlobStore");
            blob_store.put_chunk(chunk_id, data).await.expect("upload failed");

            let retrieved = blob_store.get_chunk(chunk_id).await.expect("download failed");
            blob_store.delete_chunk(chunk_id).await.expect("deleted");
            assert_eq!(retrieved, data);
        });
    }

    #[test]
    fn test_upload_chunk_overwrite() {
        // let bucket = env::var("TEST_BUCKET").expect("TEST_BUCKET env var must be set");
        let string = Uuid::new_v4().to_string();
        let chunk_id = string.as_str();
        let data1 = b"data one";
        let data2 = b"data two";

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let blob_store = TigrisBlobStore::new().await
                .expect("failed to create TigrisBlobStore");
            blob_store.put_chunk(chunk_id, data1).await.expect("first upload failed");
            blob_store.put_chunk(chunk_id, data2).await.expect("second upload failed");

            let retrieved = blob_store.get_chunk(chunk_id).await.expect("download failed");
            blob_store.delete_chunk(chunk_id).await.expect("deleted");
            assert_eq!(retrieved, data2);
        });
    }
}