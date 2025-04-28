use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;
use crate::store::chunk_storage::store::{ChunkResult, ChunkStore};

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
    async fn put_chunk(&self, chunk_id: &str, data: &[u8]) -> ChunkResult<()> {
        let mut path = self.directory.clone();
        path.push(chunk_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        // Write the data to the file asynchronously.
        fs::write(path, data).await?;
        Ok(())
    }

    async fn get_chunk(&self, chunk_id: &str) -> ChunkResult<Vec<u8>> {
        let mut path = self.directory.clone();
        path.push(chunk_id);
        // Read the file contents asynchronously.
        let data = fs::read(path).await?;
        Ok(data)
    }

    async fn delete_chunk(&self, chunk_id: &str) -> ChunkResult<()> {
        let mut path = self.directory.clone();
        path.push(chunk_id);
        fs::remove_file(path).await?;
        Ok(())
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
