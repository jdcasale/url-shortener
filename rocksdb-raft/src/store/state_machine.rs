use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::sync::Arc;
use openraft::{AnyError, EntryPayload, ErrorSubject, ErrorVerb, LogId, OptionalSend, RaftSnapshotBuilder, RaftTypeConfig, Snapshot, SnapshotMeta, StorageError, StorageIOError, StoredMembership};
use openraft::storage::{RaftStateMachine, SnapshotSignature};
use rocksdb::{ColumnFamily, DB};
use tokio::sync::RwLock;
use crate::network::callback_network_impl::{Node, NodeId};
use crate::store::types::{LongUrlEntry, TypeConfig};
use crate::store::storage::{RaftResponse, StoredSnapshot};
use crate::{typ, SnapshotData};
use crate::store::chunk_reference::{ChunkReference, SnapshotManifest};
use crate::store::chunk_storage::local::{ChunkStore, LocalChunkStore};
use crate::store::log_storage::StorageResult;

// A helper function to compute a SHA-256 hash of the data.
fn compute_hash(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    result.iter().map(|byte| format!("{:02x}", byte)).collect()
}

/// A storage implementation for the Raft state machine using RocksDB.
/// This struct manages the state machine's data and snapshots.
#[derive(Debug, Clone)]
pub struct StateMachineStore {
    /// The current state machine data
    pub data: StateMachineData,

    /// A counter used to generate unique snapshot IDs.
    /// This is not persisted and is only used as a suffix for snapshot IDs.
    /// In practice, using a timestamp in micro-seconds would be sufficient.
    snapshot_idx: u64,

    /// The RocksDB database instance used for storing snapshots
    db: Arc<DB>,

    /// A chunk store for offloading heavy state chunks (e.g. S3 or local directory).
    chunk_store: LocalChunkStore,
}

/// Represents the current state of the state machine.
#[derive(Debug, Clone)]
pub struct StateMachineData {
    /// The ID of the last log entry that was applied to the state machine
    pub last_applied_log_id: Option<LogId<NodeId>>,

    /// The last known cluster membership configuration
    pub last_membership: StoredMembership<NodeId, Node>,

    /// The key-value store that holds the actual state machine data
    pub historical_kvs: Arc<RwLock<BTreeMap<String, String>>>,

    /// The key-value store that holds the actual state machine data
    pub new_writes_kvs: Arc<RwLock<BTreeMap<String, String>>>,

    /// Chunks that are represented locally
    pub merged_chunks: Arc<RwLock<BTreeSet<String>>>,
}

impl RaftSnapshotBuilder<TypeConfig> for StateMachineStore {
    /// Builds a new snapshot of the current state machine state.
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfig>, StorageError<NodeId>> {
        let last_applied_log = self.data.last_applied_log_id;
        let last_membership = self.data.last_membership.clone();

        // Serialize the entire key-value store as a single blob.
        let kv_json = {
            let kvs = self.data.new_writes_kvs.read().await;
            serde_json::to_vec(&*kvs).map_err(|e| StorageIOError::read_state_machine(&e))?
        };

        // Compute a content-addressable chunk ID.
        let chunk_id = compute_hash(&kv_json);
        // let meta = SnapshotMeta {
        //     last_log_id: self.data.last_applied_log_id,
        //     last_membership: self.data.last_membership.clone(),
        //     snapshot_id: chunk_id.clone(), // now a snapshot signature rather than an arbitrary String
        // };
        // Upload the blob to the chunk store.
        self.chunk_store.put_chunk(&chunk_id, &kv_json)
            .await.unwrap();
            // .map_err(|e| StorageError::IO {
            //     source: StorageIOError::write_snapshot(Some(meta.signature()), &e),
            // })?;

        // Build the snapshot metadata.
        let snapshot_id = if let Some(last) = last_applied_log {
            format!("{}-{}-{}", last.leader_id, last.index, self.snapshot_idx)
        } else {
            format!("--{}", self.snapshot_idx)
        };

        let meta = SnapshotMeta {
            last_log_id: last_applied_log,
            last_membership,
            snapshot_id,
        };

        // Instead of embedding the full blob, we serialize the chunk ID as our manifest.
        let manifest = serde_json::to_vec(&chunk_id)
            .map_err(|e| StorageIOError::read_state_machine(&e))?;

        let snapshot = StoredSnapshot {
            meta: meta.clone(),
            data: manifest.clone(),
        };

        self.set_current_snapshot_(snapshot)?;
        let new_writes = self.data.new_writes_kvs.write().await;
        // nuke the new_writes map after
        new_writes.clone().clear();
        let snapshot = Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(manifest)),
        };
        Ok(snapshot)
    }
}

impl StateMachineStore {
    /// Creates a new state machine store instance.
    pub async fn new(
        db: Arc<DB>,
        chunk_store: LocalChunkStore,
    ) -> Result<StateMachineStore, StorageError<NodeId>> {
        let mut sm = Self {
            data: StateMachineData {
                last_applied_log_id: None,
                last_membership: Default::default(),
                historical_kvs: Arc::new(Default::default()),
                new_writes_kvs: Arc::new(Default::default()),
                merged_chunks: Arc::new(Default::default()),
            },
            snapshot_idx: 0,
            db,
            chunk_store,
        };

        let snapshot = sm.get_current_snapshot_()?;
        if let Some(snap) = snapshot {
            sm.update_state_machine_(snap).await?;
        }

        Ok(sm)
    }

    /// Updates the state machine with data from a snapshot.
    async fn update_state_machine_(&mut self, snapshot: StoredSnapshot) -> Result<(), StorageError<NodeId>> {

        // Deserialize the manifest from snapshot metadata.
        let manifest: SnapshotManifest = serde_json::from_slice(&snapshot.data)
            .map_err(|e| StorageIOError::read_snapshot(Some(snapshot.meta.signature()), &e))?;

        let chunk_ids = self.data.merged_chunks.read().await;
        for chunk in manifest.chunks {
            if !chunk_ids.contains(&chunk.id) {
                self.hydrate_chunk(chunk, snapshot.meta.signature()).await?;
            }
        }
        self.data.last_applied_log_id = snapshot.meta.last_log_id;
        self.data.last_membership = snapshot.meta.last_membership.clone();

        Ok(())
    }

    async fn hydrate_chunk(&self, chunk: ChunkReference, signature: SnapshotSignature<u64>) -> Result<(), StorageError<<TypeConfig as RaftTypeConfig>::NodeId>> {
        // The snapshot data now contains the serialized chunk ID.
        let chunk_id: String = chunk.id;
        // Retrieve the chunk from the chunk store.
        let chunk_data = self.chunk_store.get_chunk(&chunk_id)
            .await.unwrap();
            // .map_err(|e| StorageError::IO {
            //     source: StorageIOError::read_snapshot(Some(signature), &e),
            // })?;
        let chunk_kvs: BTreeMap<String, String> = serde_json::from_slice(&chunk_data)
            .map_err(|e| StorageIOError::read_snapshot(Some(signature), &e))?;

        let mut current_kvs = self.data.historical_kvs.write().await;
        for (key, value) in chunk_kvs {
            current_kvs.insert(key, value);
        }
        let chunk_ids = self.data.merged_chunks.write().await;
        // update the chunk state to reflect that we've hydrated this chunk
        chunk_ids.clone().insert(chunk_id);
        Ok(())
    }

    /// Retrieves the current snapshot from storage.
    fn get_current_snapshot_(&self) -> StorageResult<Option<StoredSnapshot>> {
        Ok(self
            .db
            .get_cf(self.store(), b"snapshot")
            .map_err(|e| StorageError::IO {
                source: StorageIOError::read(&e),
            })?
            .and_then(|v| serde_json::from_slice(&v).ok()))
    }

    /// Saves the current snapshot to storage.
    fn set_current_snapshot_(&self, snap: StoredSnapshot) -> StorageResult<()> {
        self.db
            .put_cf(self.store(), b"snapshot", serde_json::to_vec(&snap).unwrap().as_slice())
            .map_err(|e| StorageError::IO {
                source: StorageIOError::write_snapshot(Some(snap.meta.signature()), &e),
            })?;
        self.flush(ErrorSubject::Snapshot(Some(snap.meta.signature())), ErrorVerb::Write)?;
        Ok(())
    }

    /// Flushes the write-ahead log to disk.
    fn flush(&self, subject: ErrorSubject<NodeId>, verb: ErrorVerb) -> Result<(), StorageIOError<NodeId>> {
        self.db.flush_wal(true).map_err(|e| StorageIOError::new(subject, verb, AnyError::new(&e)))?;
        Ok(())
    }

    /// Returns a handle to the "store" column family.
    fn store(&self) -> &ColumnFamily {
        self.db.cf_handle("store").unwrap()
    }
}

impl RaftStateMachine<TypeConfig> for StateMachineStore {
    type SnapshotBuilder = Self;

    /// Gets the current state of the state machine.
    async fn applied_state(
        &mut self,
    ) -> Result<(Option<LogId<NodeId>>, StoredMembership<NodeId, Node>), StorageError<NodeId>> {
        Ok((self.data.last_applied_log_id, self.data.last_membership.clone()))
    }

    /// Applies a series of log entries to the state machine.
    async fn apply<I>(&mut self, entries: I) -> Result<Vec<RaftResponse>, StorageError<NodeId>>
    where
        I: IntoIterator<Item = typ::Entry> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        let entries = entries.into_iter();
        let mut replies = Vec::with_capacity(entries.size_hint().0);
        let mut st = self.data.new_writes_kvs.write().await;

        for ent in entries {
            self.data.last_applied_log_id = Some(ent.log_id);

            match ent.payload {
                EntryPayload::Blank => {
                    replies.push(RaftResponse { value: None, short_url: None });
                }
                EntryPayload::Normal(req) => {
                    let LongUrlEntry { hash, url, .. } = req;
                    {
                        st.insert(hash.clone(), url);
                        replies.push(RaftResponse {
                            value: None,
                            short_url: Some(hash.to_string())
                        });
                    }
                },
                EntryPayload::Membership(mem) => {
                    self.data.last_membership = StoredMembership::new(Some(ent.log_id), mem);
                    replies.push(RaftResponse { value: None, short_url: None });
                }
            }
        }
        Ok(replies)
    }

    /// Returns a new snapshot builder instance.
    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.snapshot_idx += 1;
        self.clone()
    }

    /// Prepares to receive a new snapshot.
    async fn begin_receiving_snapshot(&mut self) -> Result<Box<Cursor<Vec<u8>>>, StorageError<NodeId>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    /// Installs a new snapshot in the state machine.
    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId, Node>,
        snapshot: Box<SnapshotData>,
    ) -> Result<(), StorageError<NodeId>> {
        let new_snapshot = StoredSnapshot {
            meta: meta.clone(),
            data: snapshot.into_inner(),
        };

        self.update_state_machine_(new_snapshot.clone()).await?;
        self.set_current_snapshot_(new_snapshot)?;

        Ok(())
    }

    /// Retrieves the current snapshot from storage.
    async fn get_current_snapshot(&mut self) -> Result<Option<Snapshot<TypeConfig>>, StorageError<NodeId>> {
        let x = self.get_current_snapshot_()?;
        Ok(x.map(|s| Snapshot {
            meta: s.meta.clone(),
            snapshot: Box::new(Cursor::new(s.data.clone())),
        }))
    }
}
