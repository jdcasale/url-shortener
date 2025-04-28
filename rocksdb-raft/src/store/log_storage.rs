use std::fmt::Debug;
use std::ops::RangeBounds;
use std::sync::Arc;
use byteorder::{BigEndian, WriteBytesExt};
use openraft::{AnyError, Entry, ErrorSubject, ErrorVerb, LogId, LogState, OptionalSend, RaftLogReader, StorageError, StorageIOError, Vote};
use openraft::storage::{LogFlushed, RaftLogStorage};
use rocksdb::{ColumnFamily, Direction, DB};
use crate::network::callback_network_impl::NodeId;
use crate::store::types::TypeConfig;

use byteorder::ReadBytesExt;

/// A storage implementation for Raft logs using RocksDB.
/// This struct handles all log-related operations including appending, reading, and purging logs.
#[derive(Debug, Clone)]
pub struct LogStore {
    /// The RocksDB database instance
    pub(crate) db: Arc<DB>,
}

/// A type alias for storage operation results
pub type StorageResult<T> = Result<T, StorageError<NodeId>>;

/// Converts a log ID to a byte vector for storage in RocksDB.
/// Uses big-endian encoding to ensure correct sorting of keys.
fn id_to_bin(id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8);
    buf.write_u64::<BigEndian>(id).unwrap();
    buf
}

/// Converts a byte vector back to a log ID.
fn bin_to_id(buf: &[u8]) -> u64 {
    (&buf[0..8]).read_u64::<BigEndian>().unwrap()
}

impl LogStore {
    /// Returns a handle to the "store" column family
    fn store(&self) -> &ColumnFamily {
        self.db.cf_handle("store").unwrap()
    }

    /// Returns a handle to the "logs" column family
    fn logs(&self) -> &ColumnFamily {
        self.db.cf_handle("logs").unwrap()
    }

    /// Flushes the write-ahead log to disk
    fn flush(&self, subject: ErrorSubject<NodeId>, verb: ErrorVerb) -> Result<(), StorageIOError<NodeId>> {
        self.db.flush_wal(true).map_err(|e| StorageIOError::new(subject, verb, AnyError::new(&e)))?;
        Ok(())
    }

    /// Retrieves the ID of the last purged log entry
    fn get_last_purged_(&self) -> StorageResult<Option<LogId<u64>>> {
        Ok(self
            .db
            .get_cf(self.store(), b"last_purged_log_id")
            .map_err(|e| StorageIOError::read(&e))?
            .and_then(|v| serde_json::from_slice(&v).ok()))
    }

    /// Sets the ID of the last purged log entry
    fn set_last_purged_(&self, log_id: LogId<u64>) -> StorageResult<()> {
        self.db
            .put_cf(
                self.store(),
                b"last_purged_log_id",
                serde_json::to_vec(&log_id).unwrap().as_slice(),
            )
            .map_err(|e| StorageError::IO {
                source: StorageIOError::write(AnyError::new(&e))
            })?;

        self.flush(ErrorSubject::Store, ErrorVerb::Write)?;
        Ok(())
    }

    /// Sets the ID of the last committed log entry
    fn set_committed_(&self, committed: &Option<LogId<NodeId>>) -> Result<(), StorageIOError<NodeId>> {
        let json = serde_json::to_vec(committed).unwrap();

        self.db.put_cf(self.store(), b"committed", json).map_err(|e| StorageIOError::write(AnyError::new(&e)))?;

        self.flush(ErrorSubject::Store, ErrorVerb::Write)?;
        Ok(())
    }

    /// Retrieves the ID of the last committed log entry
    fn get_committed_(&self) -> StorageResult<Option<LogId<NodeId>>> {
        Ok(self
            .db
            .get_cf(self.store(), b"committed")
            .map_err(|e| StorageError::IO {
                source: StorageIOError::read(&e),
            })?
            .and_then(|v| serde_json::from_slice(&v).ok()))
    }

    /// Saves the current vote to storage
    fn set_vote_(&self, vote: &Vote<NodeId>) -> StorageResult<()> {
        self.db
            .put_cf(self.store(), b"vote", serde_json::to_vec(vote).unwrap())
            .map_err(|e| StorageError::IO {
                source: StorageIOError::write_vote(&e),
            })?;

        self.flush(ErrorSubject::Vote, ErrorVerb::Write)?;
        Ok(())
    }

    /// Retrieves the current vote from storage
    fn get_vote_(&self) -> StorageResult<Option<Vote<NodeId>>> {
        Ok(self
            .db
            .get_cf(self.store(), b"vote")
            .map_err(|e| StorageError::IO {
                source: StorageIOError::write_vote(&e),
            })?
            .and_then(|v| serde_json::from_slice(&v).ok()))
    }
}

impl RaftLogReader<TypeConfig> for LogStore {
    /// Retrieves log entries within the specified range
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> StorageResult<Vec<Entry<TypeConfig>>> {
        let start = match range.start_bound() {
            std::ops::Bound::Included(x) => id_to_bin(*x),
            std::ops::Bound::Excluded(x) => id_to_bin(*x + 1),
            std::ops::Bound::Unbounded => id_to_bin(0),
        };

        let mut entries = Vec::new();
        for res in self.db.iterator_cf(self.logs(), rocksdb::IteratorMode::From(&start, Direction::Forward)) {
            let (id, val) = res.map_err(|e| StorageError::IO {
                source: StorageIOError::read_logs(AnyError::new(&e))
            })?;

            let id = bin_to_id(&id);
            if !range.contains(&id) {
                break;
            }

            let entry: Entry<TypeConfig> = serde_json::from_slice(&val).map_err(|e| StorageError::IO {
                source: StorageIOError::read_logs(AnyError::new(&e))
            })?;

            entries.push(entry);
        }

        Ok(entries)
    }
}

impl RaftLogStorage<TypeConfig> for LogStore {
    type LogReader = Self;

    /// Gets the current state of the log storage
    async fn get_log_state(&mut self) -> StorageResult<LogState<TypeConfig>> {
        let last = self.db.iterator_cf(self.logs(), rocksdb::IteratorMode::End).next().and_then(|res| {
            let (_, ent) = res.unwrap();
            Some(serde_json::from_slice::<Entry<TypeConfig>>(&ent).ok()?.log_id)
        });

        let last_purged_log_id = self.get_last_purged_()?;

        let last_log_id = match last {
            None => last_purged_log_id,
            Some(x) => Some(x),
        };
        Ok(LogState {
            last_purged_log_id,
            last_log_id,
        })
    }

    /// Saves the ID of the last committed log entry
    async fn save_committed(&mut self, _committed: Option<LogId<NodeId>>) -> Result<(), StorageError<NodeId>> {
        self.set_committed_(&_committed)?;
        Ok(())
    }

    /// Retrieves the ID of the last committed log entry
    async fn read_committed(&mut self) -> Result<Option<LogId<NodeId>>, StorageError<NodeId>> {
        let c = self.get_committed_()?;
        Ok(c)
    }

    /// Saves the current vote to storage
    #[tracing::instrument(level = "trace", skip(self))]
    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
        self.set_vote_(vote)
    }

    /// Retrieves the current vote from storage
    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        self.get_vote_()
    }

    /// Appends new log entries to storage
    #[tracing::instrument(level = "trace", skip_all)]
    async fn append<I>(&mut self, entries: I, callback: LogFlushed<TypeConfig>) -> StorageResult<()>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + Send,
        I::IntoIter: Send,
    {
        let mut entries: Vec<_> = entries.into_iter().collect();

        // Sort entries by log index to ensure sequential order
        entries.sort_by_key(|e| e.log_id.index);

        // Check for gaps in the sequence
        if let Some(first) = entries.first() {
            let mut expected_index = first.log_id.index;
            for entry in &entries {
                if entry.log_id.index != expected_index {
                    return Err(StorageError::IO {
                        source: StorageIOError::write_logs(AnyError::error(format!(
                            "Non-sequential log entries: expected index {}, got {}",
                            expected_index,
                            entry.log_id.index
                        )))
                    });
                }
                expected_index += 1;
            }
        }

        // Write entries in sequence
        for entry in entries {
            let id = id_to_bin(entry.log_id.index);
            self.db
                .put_cf(
                    self.logs(),
                    id,
                    serde_json::to_vec(&entry).map_err(|e| StorageError::IO {
                        source: StorageIOError::write_logs(AnyError::new(&e))
                    })?,
                )
                .map_err(|e| StorageError::IO {
                    source: StorageIOError::write_logs(AnyError::new(&e))
                })?;
        }

        callback.log_io_completed(Ok(()));

        Ok(())
    }

    /// Truncates the log at the specified log ID
    #[tracing::instrument(level = "debug", skip(self))]
    async fn truncate(&mut self, log_id: LogId<NodeId>) -> StorageResult<()> {
        tracing::debug!("delete_log: [{:?}, +oo)", log_id);

        let from = id_to_bin(log_id.index);
        let to = id_to_bin(0xff_ff_ff_ff_ff_ff_ff_ff);
        self.db.delete_range_cf(self.logs(), &from, &to).map_err(|e| StorageError::IO {
            source: StorageIOError::write_logs(AnyError::new(&e))
        })
    }

    /// Purges log entries up to the specified log ID
    #[tracing::instrument(level = "debug", skip(self))]
    async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        tracing::debug!("delete_log: [0, {:?}]", log_id);

        self.set_last_purged_(log_id)?;
        let from = id_to_bin(0);
        let to = id_to_bin(log_id.index + 1);
        self.db.delete_range_cf(self.logs(), &from, &to).map_err(|e| StorageError::IO {
            source: StorageIOError::write_logs(AnyError::new(&e))
        })
    }

    /// Returns a reader for accessing log entries
    async fn get_log_reader(&mut self) -> Self::LogReader {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use openraft::CommittedLeaderId;
    use openraft::LogId;
    use openraft::Vote;
    use rocksdb::{ColumnFamilyDescriptor, Options};

    async fn setup_test_db() -> (LogStore, PathBuf) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_path_buf();
        
        let mut db_opts = Options::default();
        db_opts.create_missing_column_families(true);
        db_opts.create_if_missing(true);

        let store = ColumnFamilyDescriptor::new("store", Options::default());
        let logs = ColumnFamilyDescriptor::new("logs", Options::default());

        let db = DB::open_cf_descriptors(&db_opts, &db_path, vec![store, logs]).unwrap();
        let db = Arc::new(db);
        
        (LogStore { db }, db_path)
    }

    #[tokio::test]
    async fn test_log_state() {
        let (mut store, _) = setup_test_db().await;

        // Initially, log state should be empty
        let state = store.get_log_state().await.unwrap();
        assert!(state.last_log_id.is_none());
        assert!(state.last_purged_log_id.is_none());
    }

    #[tokio::test]
    async fn test_vote_storage() {
        let (mut store, _) = setup_test_db().await;

        // Initially, no vote should be stored
        let vote = store.read_vote().await.unwrap();
        assert!(vote.is_none());

        // Save a vote
        let test_vote = Vote::new(1, 1);
        store.save_vote(&test_vote).await.unwrap();

        // Read the vote back
        let vote = store.read_vote().await.unwrap();
        assert_eq!(vote.unwrap(), test_vote);
    }

    #[tokio::test]
    async fn test_committed_log() {
        let (mut store, _) = setup_test_db().await;

        // Initially, no committed log should be stored
        let committed = store.read_committed().await.unwrap();
        assert!(committed.is_none());

        // Save a committed log ID
        let committed_id = LogId::new(CommittedLeaderId::new(1u64, 1), 1);
        store.save_committed(Some(committed_id)).await.unwrap();

        // Read the committed log ID back
        let committed = store.read_committed().await.unwrap();
        assert_eq!(committed.unwrap(), committed_id);
    }

    #[tokio::test]
    async fn test_log_purge() {
        let (mut store, _) = setup_test_db().await;

        // Purge logs up to index 2
        store.purge(LogId::new(CommittedLeaderId::new(1u64, 1), 2)).await.unwrap();

        // Check last purged log ID
        let state = store.get_log_state().await.unwrap();
        assert_eq!(state.last_purged_log_id.unwrap().index, 2);
    }

    #[tokio::test]
    async fn test_log_truncate() {
        let (mut store, _) = setup_test_db().await;

        // Truncate at index 2
        store.truncate(LogId::new(CommittedLeaderId::new(1u64, 1), 2)).await.unwrap();

        // Check that no entries remain
        let entries = store.try_get_log_entries(0..4).await.unwrap();
        assert_eq!(entries.len(), 0);
    }
}
