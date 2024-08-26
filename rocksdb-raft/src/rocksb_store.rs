use std::fmt::Debug;
use std::future::Future;
use std::io::Cursor;
use std::ops::RangeBounds;
use std::sync::Arc;
use openraft::{BasicNode, Entry, LogId, LogState, Membership, Node, OptionalSend, RaftLogId, RaftLogReader, RaftTypeConfig, StorageError, TokioRuntime, Vote};
use openraft::impls::OneshotResponder;
use openraft::storage::{LogFlushed, RaftLogStorage};
use serde::{Deserialize, Serialize};
use rocksdb::{DB, Options};
use crate::network::no_op_network_impl::{EntryBruv, NodeId};
use crate::store::{Response, StateMachineStore};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LongUrlEntry {
    pub hash: String,
    pub url: String,
    pub term: u64,
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
    type R = Response;
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

