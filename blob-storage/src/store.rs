pub trait BlobStore {
    fn upload_chunk(&self, chunk_id: &str, data: &[u8]) -> std::io::Result<()>;
}

