use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;
use std::error::Error;

/// Trait for storing and fetching chunks by their IDs.
#[async_trait]
pub trait ChunkStore {
    /// Stores a chunk by its id.
    async fn put_chunk(&self, chunk_id: &str, data: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Retrieves a chunk by its id.
    async fn get_chunk(&self, chunk_id: &str) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>>;
}

/// A test implementation of `ChunkStore` that writes chunks to a local directory.
#[derive(Clone, Debug)]
pub struct LocalChunkStore {
    directory: PathBuf,
}

impl LocalChunkStore {
    /// Creates a new `LocalChunkStore` targeting the specified directory.
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
}

#[async_trait]
impl ChunkStore for LocalChunkStore {
    async fn put_chunk(&self, chunk_id: &str, data: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut path = self.directory.clone();
        path.push(chunk_id);
        // Write the data to the file asynchronously.
        fs::write(path, data).await?;
        Ok(())
    }

    async fn get_chunk(&self, chunk_id: &str) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let mut path = self.directory.clone();
        path.push(chunk_id);
        // Read the file contents asynchronously.
        let data = fs::read(path).await?;
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_local_chunk_store() {
        // Create a temporary directory.
        let temp_dir = tempdir().unwrap();
        let store = LocalChunkStore::new(temp_dir.path().to_path_buf());

        let chunk_id = "test_chunk";
        let content = b"this is a test chunk";

        // Test writing a chunk.
        store.put_chunk(chunk_id, content).await.unwrap();

        // Test reading the chunk back.
        let retrieved = store.get_chunk(chunk_id).await.unwrap();
        assert_eq!(retrieved, content);
    }
}
