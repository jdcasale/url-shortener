use std::io::Cursor;
use std::sync::Arc;
use openraft::{BasicNode, Entry, LogId, Membership, Node, RaftLogId, RaftTypeConfig, TokioRuntime};
use openraft::entry::{FromAppData, RaftEntry, RaftPayload};
use openraft::impls::OneshotResponder;
use serde::{Deserialize, Serialize};
use rocksdb::{DB, Options};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LongUrlEntry {
    hash: String,
    url: String,
    term: u64,
}
impl LongUrlEntry {
    pub fn new(hash: u64, url: String, term: u64) -> Self {
        Self { hash: hash.to_string(), url, term }
    }
}

impl FromAppData<LongUrlEntry> for LongUrlEntry {
    fn from_app_data(t: LongUrlEntry) -> Self {
        t
    }
}

impl RaftPayload<u64, BasicNode> for LongUrlEntry {
    fn is_blank(&self) -> bool {
        todo!()
    }

    fn get_membership(&self) -> Option<&Membership<u64, BasicNode>> {
        todo!()
    }
}

impl RaftLogId<u64> for LongUrlEntry {
    fn get_log_id(&self) -> &LogId<u64> {
        todo!()
    }

    fn set_log_id(&mut self, log_id: &LogId<u64>) {
        todo!()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq, Default, Ord, PartialOrd)]
pub struct TypeConfig;

impl RaftTypeConfig for TypeConfig {
    type D = LongUrlEntry;
    type R = LongUrlEntry;
    type NodeId = u64;
    type Node = BasicNode;
    type Entry = Entry<Self>;
    type SnapshotData = Cursor<Vec<u8>>;
    type AsyncRuntime = TokioRuntime;
    type Responder = OneshotResponder<Self>;
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
    pub fn append_entry(&self, entry: LongUrlEntry) {
        self.db.put(entry.hash, entry.url).expect("Failed to write to RocksDB");
    }

    // Retrieve entry for verification
    pub fn get_entry(&self, index: u64) -> Option<String> {
        self.db.get(index.to_string())
            .unwrap()
            .map(|bytes| String::from_utf8(bytes).expect(""))
    }
}
