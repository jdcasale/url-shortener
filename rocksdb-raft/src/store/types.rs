use std::fmt::Debug;
use std::io::Cursor;
use openraft::{BasicNode, Entry, RaftTypeConfig, TokioRuntime};
use openraft::impls::OneshotResponder;
use serde::{Deserialize, Serialize};
use crate::store::store::RaftResponse;

/// Represents a long URL entry in the Raft log.
/// This is the data type that will be stored in the Raft log entries.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LongUrlEntry {
    /// The hash of the URL, used as the key in the key-value store
    pub hash: String,
    /// The actual URL to be stored
    pub url: String,
    /// The Raft term when this entry was created
    pub term: u64,
}

impl LongUrlEntry {
    /// Creates a new LongUrlEntry with the given hash, URL, and term
    pub fn new(hash: u64, url: String, term: u64) -> Self {
        Self { hash: hash.to_string(), url, term }
    }
}

/// Configuration type for the Raft implementation.
/// This type defines all the associated types needed for the Raft protocol.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq, Default, Ord, PartialOrd)]
pub struct TypeConfig;

impl RaftTypeConfig for TypeConfig {
    /// The data type stored in log entries
    type D = LongUrlEntry;
    /// The response type for client requests
    type R = RaftResponse;
    /// The type used for node IDs
    type NodeId = u64;
    /// The type used for node information
    type Node = BasicNode;
    /// The type used for log entries
    type Entry = Entry<Self>;
    /// The type used for snapshot data
    type SnapshotData = Cursor<Vec<u8>>;
    /// The async runtime type
    type AsyncRuntime = TokioRuntime;
    /// The type used for request responders
    type Responder = OneshotResponder<Self>;
}
