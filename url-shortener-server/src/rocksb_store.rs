use std::fmt::Debug;
use std::future::Future;
use std::io::Cursor;
use std::ops::RangeBounds;
use std::sync::Arc;
use log::kv::Error;
use openraft::{BasicNode, Entry, LogId, LogState, Membership, Node, OptionalSend, RaftLogId, RaftLogReader, RaftTypeConfig, StorageError, TokioRuntime, Vote};
use openraft::entry::{FromAppData, RaftEntry, RaftPayload};
use openraft::impls::OneshotResponder;
use openraft::storage::{LogFlushed, RaftLogStorage};
use serde::{Deserialize, Serialize};
use rocksdb::{DB, Options};
use rocksdb_raft::store::StateMachineStore;
use crate::no_op_network_impl::{EntryBruv, NodeId};
use crate::rocksb_store;

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

impl RaftLogStorage<TypeConfig> for RocksApp {
    type LogReader = Self;

    async fn get_log_state(&mut self) -> Result<LogState<TypeConfig>, StorageError<NodeId>> {
        todo!()
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        todo!()
    }

    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
        todo!()
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        todo!()
    }

    async fn append<I>(&mut self, entries: I, callback: LogFlushed<TypeConfig>) -> Result<(), StorageError<NodeId>>
    where
        I: IntoIterator<Item=<TypeConfig as RaftTypeConfig>::Entry> + OptionalSend,
        I::IntoIter: OptionalSend
    {
        todo!()
    }

    async fn truncate(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        todo!()
    }

    async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>>{
        todo!()
    }
}

impl RocksApp {
    pub fn new(db_path: &str) -> (Self, impl Future<Output =Result<StateMachineStore, openraft::StorageError<u64>>>) {
        let db = Arc::new(DB::open_default(db_path).unwrap());
        let db2 = db.clone();
        (Self { db }, StateMachineStore::new(db2))
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

impl RaftLogReader<TypeConfig> for RocksApp {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(&mut self, range: RB) -> Result<Vec<EntryBruv>, StorageError<NodeId>>{
        todo!()
    }
}
