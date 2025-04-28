use std::error::Error;
use async_trait::async_trait;
use crate::store::chunk_storage::local::LocalChunkStore;
use crate::store::chunk_storage::minio::MinioChunkStore;


pub type ChunkResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

/// Trait for storing and fetching chunks by their IDs.
#[async_trait]
pub trait ChunkStore {
    /// Stores a chunk by its id.
    async fn put_chunk(&self, chunk_id: &str, data: &[u8]) -> ChunkResult<()>;

    /// Retrieves a chunk by its id.
    async fn get_chunk(&self, chunk_id: &str) -> ChunkResult<Vec<u8>>;

    async fn delete_chunk(&self, chunk_id: &str) -> ChunkResult<()>;
}



#[derive(Debug, Clone)]
pub enum ChunkStores {
    Local(LocalChunkStore),
    Minio(MinioChunkStore)
}

impl ChunkStores {
    /// Returns a reference to the inner value as a trait object.
    pub fn as_trait(&self) -> &dyn ChunkStore {
        match self {
            ChunkStores::Local(a) => a,
            ChunkStores::Minio(b) => b,
        }
    }
}