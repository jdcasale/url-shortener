use std::fmt::Debug;
use std::io::Cursor;
use openraft::{BasicNode, Entry, RaftTypeConfig, TokioRuntime};
use openraft::impls::OneshotResponder;
use serde::{Deserialize, Serialize};
use crate::store::Response;

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
