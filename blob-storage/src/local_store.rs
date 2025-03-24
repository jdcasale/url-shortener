use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use crate::store::BlobStore;

pub struct LocalFileBlobStore {
    base_path: String,
}

impl LocalFileBlobStore {
    pub fn new(base_path: String) -> Self {
        Self { base_path }
    }
}

impl BlobStore for LocalFileBlobStore {
    fn upload_chunk(&self, chunk_id: &str, data: &[u8]) -> std::io::Result<()> {
        let file_path = Path::new(&self.base_path).join(chunk_id);
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_path)?;
        // let mut file = File::create(file_path)?;
        file.write_all(data)?;
        Ok(())
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Read;
    use uuid::Uuid;

    fn setup(base_path: &str) {
        // Ensure the base path exists
        fs::create_dir_all(base_path).unwrap();
    }

    fn teardown(base_path: &str, chunk_id: &str) {
        // Clean up
        if Path::new(base_path).exists() {
            fs::remove_file(Path::new(base_path).join(chunk_id)).unwrap();
            fs::remove_dir_all(base_path).unwrap();
        }
    }

    #[test]
    fn test_upload_chunk() {
        let random_bits = Uuid::new_v4();
        let base_path = &format!("test_data_{}", random_bits);
        let blob_store = LocalFileBlobStore::new(base_path.to_string());


        let chunk_id = "test_chunk";
        let data = b"test data";

        // Ensure the base path exists
        setup(base_path);

        // Upload the chunk
        blob_store.upload_chunk(chunk_id, data).unwrap();

        // Verify the file was created and contains the correct data
        let file_path = Path::new(base_path).join(chunk_id);
        let mut file = File::open(file_path).unwrap();
        let mut file_data = Vec::new();
        file.read_to_end(&mut file_data).unwrap();

        assert_eq!(file_data, data);

        // Clean up
        teardown(base_path, chunk_id);
    }

    #[test]
    fn test_upload_chunk_overwrite() {
        let random_bits = Uuid::new_v4();
        let base_path = &format!("test_data_{}", random_bits);
        let blob_store = LocalFileBlobStore::new(base_path.to_string());

        let chunk_id = "test_chunk_{}";
        let data1 = b"test data 1";
        let data2 = b"test data 2";

        // Ensure the base path exists
        setup(base_path);

        // Upload the first chunk
        blob_store.upload_chunk(chunk_id, data1).unwrap();

        // Upload the second chunk with the same ID
        blob_store.upload_chunk(chunk_id, data2).unwrap();

        // Verify the file was overwritten and contains the correct data
        let file_path = Path::new(base_path).join(chunk_id);
        let mut file = File::open(file_path).unwrap();
        let mut file_data = Vec::new();
        file.read_to_end(&mut file_data).unwrap();

        assert_eq!(file_data, data2);

        // Clean up
        teardown(base_path, chunk_id)
    }
}