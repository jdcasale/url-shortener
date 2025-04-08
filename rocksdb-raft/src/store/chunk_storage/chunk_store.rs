// #[async_trait::async_trait]
// pub trait ChunkStore {
//     /// Stores a chunk by its id.
//     async fn put_chunk(&self, chunk_id: &str, data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
//
//     /// Retrieves a chunk by its id.
//     async fn get_chunk(&self, chunk_id: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>;
// }
