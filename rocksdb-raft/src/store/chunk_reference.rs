use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct SnapshotManifest {
    /// List of all chunk references.
    pub chunks: Vec<ChunkReference>,
}

#[derive(Serialize, Deserialize)]
pub struct ChunkReference {
    /// A content-addressable ID, e.g., a SHA-256 hash.
    pub id: String,
}
