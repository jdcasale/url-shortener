use std::collections::BTreeMap;
use std::sync::Arc;

use openraft::Config;
use tokio::sync::RwLock;

use crate::RaftImpl;
use crate::NodeId;

// Representation of an application state. This struct can be shared around to share
// instances of raft, store and more.
pub struct App {
    pub id: NodeId,
    pub api_addr: String,
    pub raft: RaftImpl,
    pub historical_kvs: Arc<RwLock<BTreeMap<String, String>>>,
    pub new_writes_kvs: Arc<RwLock<BTreeMap<String, String>>>,
    pub config: Arc<Config>,
}