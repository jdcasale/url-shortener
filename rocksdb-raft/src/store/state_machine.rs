use crate::network::callback_network_impl::{Node, NodeId};
use crate::store::chunk_reference::{ChunkReference, SnapshotManifest};
use crate::store::log_storage::StorageResult;
use crate::store::storage::{RaftResponse, StoredSnapshot};
use crate::store::types::{LongUrlEntry, TypeConfig};
use crate::{typ, SnapshotData};
use openraft::storage::{RaftStateMachine, SnapshotSignature};
use openraft::{AnyError, EntryPayload, ErrorSubject, ErrorVerb, LogId, OptionalSend, RaftSnapshotBuilder, RaftTypeConfig, Snapshot, SnapshotMeta, StorageError, StorageIOError, StoredMembership};
use rocksdb::{ColumnFamily, DB};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::store::chunk_storage::store::ChunkStores;
use crate::store::error_bullshit::CustomAnyError;

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
    chunk_store: ChunkStores,
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
        tracing::info!("Building snapshot");
        let last_applied_log = self.data.last_applied_log_id;
        let last_membership = self.data.last_membership.clone();

        // Serialize the entire key-value store as a single blob.
        let kv_json = {
            let kvs = self.data.new_writes_kvs.read().await;
            serde_json::to_vec(&*kvs).map_err(|e| StorageIOError::read_state_machine(&e))?
        };

        // Compute a content-addressable chunk ID.
        let chunk_id = compute_hash(&kv_json);
        let meta = SnapshotMeta {
            last_log_id: self.data.last_applied_log_id,
            last_membership: self.data.last_membership.clone(),
            snapshot_id: chunk_id.clone(), // now a snapshot signature rather than an arbitrary String
        };
        // Upload the blob to the chunk store.
        let result = self.chunk_store.as_trait().put_chunk(&chunk_id, &kv_json).await;
        result
            .map_err(|e| StorageError::IO {
                source: StorageIOError::write_snapshot(Some(meta.signature()), &CustomAnyError::from(e)),
            })?;

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

        let mut chunks: Vec<ChunkReference> = self.data.merged_chunks
            .read()
            .await
            .iter()
            .map(|cid| ChunkReference { id: (*cid.clone()).parse().unwrap() }).collect();
        let new_chunk = ChunkReference { id: chunk_id.clone()};
        chunks.push(new_chunk);
        self.data.merged_chunks.write().await.insert(chunk_id);
        let manifest = SnapshotManifest {chunks};
        // Instead of embedding the full blob, we serialize the chunk ID as our manifest.
        let manifest = serde_json::to_vec(&manifest)
            .map_err(|e| StorageIOError::read_state_machine(&e))?;

        let snapshot = StoredSnapshot {
            meta: meta.clone(),
            data: manifest.clone(),
        };

        // move the keys corresponding to this chunk into our history map
        let mut history = self.data.historical_kvs.write().await;
        for (key, val) in self.data.new_writes_kvs.read().await.iter() {
            history.insert(key.to_string(), val.to_string());
        }

        self.set_current_snapshot_(snapshot)?;
        // nuke the new_writes map after
        self.data.new_writes_kvs.write().await.clear();

        let snapshot = Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(manifest)),
        };
        tracing::info!("Done building snapshot");
        Ok(snapshot)
    }
}

impl StateMachineStore {
    /// Creates a new state machine store instance.
    pub async fn new(
        db: Arc<DB>,
        chunk_store: ChunkStores,
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
        tracing::info!("Updating from snapshot");

        // Deserialize the manifest from snapshot metadata.
        let manifest: SnapshotManifest = serde_json::from_slice(&snapshot.data)
            .map_err(|e| StorageIOError::read_snapshot(Some(snapshot.meta.signature()), &e))?;

        // Collect chunks that need hydration.
        let chunks_to_hydrate: Vec<ChunkReference> = {
            let merged = self.data.merged_chunks.read().await;
            manifest.chunks
                .into_iter()
                .filter(|chunk| !merged.contains(&chunk.id))
                .collect()
        };

        // Now, for each chunk that isn’t merged, hydrate it.
        for chunk in chunks_to_hydrate {
            self.hydrate_chunk(chunk, snapshot.meta.signature()).await?;
        }
        self.data.last_applied_log_id = snapshot.meta.last_log_id;
        self.data.last_membership = snapshot.meta.last_membership.clone();
        tracing::info!("Done updating from snapshot");
        Ok(())
    }

    async fn hydrate_chunk(&self, chunk: ChunkReference, signature: SnapshotSignature<u64>) -> Result<(), StorageError<<TypeConfig as RaftTypeConfig>::NodeId>> {
        tracing::info!("Hydrating from chunk");
        // The snapshot data now contains the serialized chunk ID.
        let chunk_id: String = chunk.id;
        // Retrieve the chunk from the chunk store.
        let chunk_store = self.chunk_store.as_trait();
        let chunk_data = chunk_store.get_chunk(&chunk_id)
            .await
            .map_err(|e| StorageError::IO {
                source: StorageIOError::read_snapshot(Some(signature.clone()), &CustomAnyError::from(e)),
            })?;
        let chunk_kvs: BTreeMap<String, String> = serde_json::from_slice(&chunk_data)
            .map_err(|e| StorageIOError::read_snapshot(Some(signature), &e))?;

        let mut current_kvs = self.data.historical_kvs.write().await;
        for (key, value) in chunk_kvs {
            current_kvs.insert(key, value);
        }
        let mut chunk_ids = self.data.merged_chunks.write().await;
        // update the chunk state to reflect that we've hydrated this chunk
        chunk_ids.insert(chunk_id);
        tracing::info!("Done hydrating from chunk");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::chunk_reference::SnapshotManifest;
    use openraft::{CommittedLeaderId, Entry, EntryPayload, LogId};
    use rocksdb::{ColumnFamilyDescriptor, Options, DB};
    use std::sync::Arc;
    use tempfile::tempdir;
    use crate::store::chunk_storage::local::LocalChunkStore;
    use crate::store::chunk_storage::store::ChunkStore;

    /// Sets up a temporary RocksDB instance and a LocalChunkStore.
    async fn setup_test_store() -> (StateMachineStore, tempfile::TempDir) {
        // Create a temporary directory for RocksDB and chunks.
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("db");
        std::fs::create_dir_all(&db_path).unwrap();

        // Configure RocksDB with a couple of column families.
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        let cf_store = ColumnFamilyDescriptor::new("store", Options::default());
        let cf_snapshot = ColumnFamilyDescriptor::new("snapshot", Options::default());
        let db = DB::open_cf_descriptors(&db_opts, db_path.clone(), vec![cf_store, cf_snapshot])
            .unwrap();
        let db = Arc::new(db);

        // Create a temporary directory for chunks.
        let chunk_dir = temp_dir.path().join("chunks");
        std::fs::create_dir_all(&chunk_dir).unwrap();

        let chunk_store = LocalChunkStore::new(chunk_dir);

        // Create the StateMachineStore.
        let store = StateMachineStore::new(db, ChunkStores::Local(chunk_store)).await.unwrap();
        (store, temp_dir)
    }

    #[tokio::test]
    async fn test_build_snapshot_clears_new_writes_and_uploads_chunk() {
        let (mut store, _temp_dir) = setup_test_store().await;
        // Insert some new writes.
        {
            let mut new_writes = store.data.new_writes_kvs.write().await;
            new_writes.insert("key1".to_string(), "value1".to_string());
            new_writes.insert("key2".to_string(), "value2".to_string());
        }
        assert!(store.data.merged_chunks.read().await.is_empty());
        // Build a snapshot.
        let snapshot = store.build_snapshot().await.unwrap();

        // After snapshotting, the new_writes_kvs should be empty.
        {
            let new_writes = store.data.new_writes_kvs.read().await;
            assert!(new_writes.is_empty(), "new_writes_kvs was not cleared");
        }

        // Check that the snapshot meta has been set.
        assert!(!snapshot.meta.snapshot_id.is_empty());

        // Verify that the chunk was uploaded in the local chunk store.
        // Recompute the expected chunk id from the new writes.
        let mut expected_map = BTreeMap::new();
        expected_map.insert("key1".to_string(), "value1".to_string());
        expected_map.insert("key2".to_string(), "value2".to_string());
        let expected_json = serde_json::to_vec(&expected_map).unwrap();
        let expected_chunk_id = compute_hash(&expected_json);
        
        let stored_blob = store.chunk_store.as_trait().get_chunk(&expected_chunk_id).await.unwrap();
        assert_eq!(stored_blob, expected_json, "Chunk store does not contain the expected data");

        // The snapshot data is a manifest; deserialize it.
        let manifest: SnapshotManifest = serde_json::from_slice(&snapshot.snapshot.into_inner())
            .expect("Failed to deserialize manifest");
        assert!(!manifest.chunks.is_empty());
    }

    #[tokio::test]
    async fn test_update_state_machine_hydrates_chunk() {
        let (mut store, _temp_dir) = setup_test_store().await;
        // Ensure merged_chunks is empty.
        {
            let merged = store.data.merged_chunks.read().await;
            assert!(merged.is_empty());
        }
        // Insert a new write and build snapshot.
        {
            let mut new_writes = store.data.new_writes_kvs.write().await;
            new_writes.insert("key3".to_string(), "value3".to_string());
        }
        let snapshot = store.build_snapshot().await.unwrap();

        let stored = StoredSnapshot {
            meta: snapshot.meta,
            data: snapshot.snapshot.into_inner(),
        };

        // Update (hydrate) the state machine from the snapshot.
        store.update_state_machine_(stored).await.unwrap();

        // Check that historical_kvs now contains the hydrated data.
        {
            let hist = store.data.historical_kvs.read().await;
            assert_eq!(hist.get("key3").unwrap(), "value3");
        }

        // Compute the expected chunk id.
        let mut expected_map = BTreeMap::new();
        expected_map.insert("key3".to_string(), "value3".to_string());
        let expected_json = serde_json::to_vec(&expected_map).unwrap();
        let expected_chunk_id = compute_hash(&expected_json);
        // Verify that merged_chunks now contains the expected chunk id.
        {
            let merged = store.data.merged_chunks.read().await;
            assert!(merged.contains(&expected_chunk_id), "Merged chunks does not contain the expected chunk id");
        }
    }

    #[tokio::test]
    async fn test_apply_entries_updates_new_writes() {
        let (mut store, _temp_dir) = setup_test_store().await;
        // Create a test log entry.
        let entry = Entry {
            log_id: LogId::new(CommittedLeaderId::new(1, 1), 1),
            payload: EntryPayload::Normal(LongUrlEntry::new(123, "http://example.com".to_string(), 1)),
        };
        let responses = store.apply(vec![entry]).await.unwrap();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].clone().short_url.unwrap(), "123");

        // Verify that new_writes_kvs now contains the applied entry.
        {
            let new_writes = store.data.new_writes_kvs.read().await;
            assert_eq!(new_writes.get("123").unwrap(), "http://example.com");
        }
    }
}

