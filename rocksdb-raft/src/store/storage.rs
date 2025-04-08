use std::fmt::Debug;
use std::path::Path;
use std::sync::Arc;

use openraft::SnapshotMeta;
use rocksdb::ColumnFamilyDescriptor;
use rocksdb::Options;
use rocksdb::DB;
use serde::Deserialize;
use serde::Serialize;
use crate::network::callback_network_impl::Node;
use crate::NodeId;
use crate::store::chunk_storage::local::LocalChunkStore;
use crate::store::log_storage::LogStore;
use crate::store::state_machine::StateMachineStore;

/// Represents the types of requests that can be made to the Raft cluster.
/// Each variant corresponds to a different operation that can be performed.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RaftRequest {
    /// Set a key-value pair in the store
    Set { key: String, value: String },
    /// Create a short URL from a long URL
    CreateShortUrl { long_url: String },
}

/// Represents the response from a Raft operation.
/// Contains optional values for both regular key-value operations and short URL operations.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RaftResponse {
    /// The value retrieved from a key-value operation
    pub value: Option<String>,
    /// The short URL created from a long URL
    pub short_url: Option<String>,
}

/// Represents a snapshot of the state machine at a particular point in time.
/// Used for state machine recovery and log compaction.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredSnapshot {
    /// Metadata about the snapshot, including the last applied log ID and membership
    pub meta: SnapshotMeta<NodeId, Node>,

    /// The serialized state machine data at the time of this snapshot
    pub data: Vec<u8>,
}

/// Creates a new storage instance with both log storage and state machine storage.
/// 
/// # Arguments
/// * `db_path` - The path where the RocksDB database will be stored
/// 
/// # Returns
/// A tuple containing the log store and state machine store instances
pub async fn new_storage<P: AsRef<Path>>(db_path: P) -> (LogStore, StateMachineStore) {
    let mut db_opts = Options::default();
    db_opts.create_missing_column_families(true);
    db_opts.create_if_missing(true);

    let store = ColumnFamilyDescriptor::new("store", Options::default());
    let new_writes = ColumnFamilyDescriptor::new("new_writes", Options::default());
    let logs = ColumnFamilyDescriptor::new("logs", Options::default());

    let db = DB::open_cf_descriptors(&db_opts, db_path, vec![store, new_writes, logs]).unwrap();
    let db = Arc::new(db);

    let log_store = LogStore { db: db.clone() };
    // TODO(@jcasale): hard-coded path needs to be passed as config
    let chunk_store = LocalChunkStore::new("/chunk_store/chunks".parse().unwrap());
    // let chunk_store = LocalChunkStore::new("/tmp/chunk_store/chunks".parse().unwrap());
    let sm_store = StateMachineStore::new(db, chunk_store).await.unwrap();

    (log_store, sm_store)
}