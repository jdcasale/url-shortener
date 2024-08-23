use std::sync::Arc;
use serde::{Deserialize, Serialize};
use rocksdb::{DB, Options};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LongUrlEntry {
    data: String,
}
impl LongUrlEntry {
    pub fn new(url: String) -> Self {
        Self { data: url }
    }
}

// Simple Raft-like storage
pub struct RocksApp {
    db: Arc<DB>,
}

impl RocksApp {
    pub fn new(db_path: &str) -> Self {
        let db = Arc::new(DB::open_default(db_path).unwrap());
        Self { db }
    }

    // Simulate appending an entry to the log and persisting it
    pub fn append_entry(&self, entry: LongUrlEntry, index: u64) {
        self.db.put(index.to_string(), entry.data).expect("Failed to write to RocksDB");
    }

    // Retrieve entry for verification
    pub fn get_entry(&self, index: u64) -> Option<String> {
        self.db.get(index.to_string())
            .unwrap()
            .map(|bytes| String::from_utf8(bytes).expect(""))
    }
}
