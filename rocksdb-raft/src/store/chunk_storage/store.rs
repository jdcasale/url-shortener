use std::error::Error;
use async_trait::async_trait;
use crate::store::chunk_storage::local::LocalChunkStore;
use crate::store::chunk_storage::minio::MinioChunkStore;
use crate::store::chunk_storage::tigris::TigrisBlobStore;

/// Trait for storing and fetching chunks by their IDs.
#[async_trait]
pub trait ChunkStore {
    /// Stores a chunk by its id.
    async fn put_chunk(&self, chunk_id: &str, data: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Retrieves a chunk by its id.
    async fn get_chunk(&self, chunk_id: &str) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>>;
    async fn delete_chunk(&self, chunk_id: &str) -> Result<(), Box<dyn Error + Send + Sync>>;
}



#[derive(Debug, Clone)]
pub enum ChunkStores {
    Local(LocalChunkStore),
    Tigris(TigrisBlobStore),
    Minio(MinioChunkStore)
}

impl ChunkStores {
    /// Returns a reference to the inner value as a trait object.
    pub fn as_trait(&self) -> &dyn ChunkStore {
        match self {
            ChunkStores::Local(a) => a,
            ChunkStores::Tigris(b) => b,
            ChunkStores::Minio(c) => c,
        }
    }
}