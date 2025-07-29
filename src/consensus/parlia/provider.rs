use super::snapshot::Snapshot;
use super::validator::SnapshotProvider;
use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Very simple `SnapshotProvider` that keeps the most recent `max_entries` snapshots in memory.
/// Keys are the **block number** the snapshot is valid for (i.e. the last block of the snapshot’s
/// epoch). For historical sync this is sufficient – we can switch to an MDBX-backed provider later.
#[derive(Clone, Debug)]
pub struct InMemorySnapshotProvider {
    inner: Arc<RwLock<BTreeMap<u64, Snapshot>>>,
    max_entries: usize,
}

impl InMemorySnapshotProvider {
    /// Create a new provider keeping at most `max_entries` snapshots.
    pub fn new(max_entries: usize) -> Self {
        Self { inner: Arc::new(RwLock::new(BTreeMap::new())), max_entries }
    }
}

impl Default for InMemorySnapshotProvider {
    fn default() -> Self { Self::new(2048) }
}

impl SnapshotProvider for InMemorySnapshotProvider {
    fn snapshot(&self, block_number: u64) -> Option<Snapshot> {
        let guard = self.inner.read();
        // Find the greatest key <= block_number.
        if let Some((_, snap)) = guard.range(..=block_number).next_back() {
            return Some(snap.clone());
        }
        None
    }

    fn insert(&self, snapshot: Snapshot) {
        let mut guard = self.inner.write();
        guard.insert(snapshot.block_number, snapshot);
        // clamp size
        while guard.len() > self.max_entries {
            // remove the smallest key
            if let Some(first_key) = guard.keys().next().cloned() {
                guard.remove(&first_key);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MDBX‐backed snapshot provider with LRU front‐cache
// ---------------------------------------------------------------------------

use reth_db::{Database, DatabaseError};
use reth_db::table::{Compress, Decompress};
use reth_db::transaction::{DbTx, DbTxMut};
use reth_db::cursor::DbCursorRO;
use schnellru::{ByLength, LruMap};

/// `DbSnapshotProvider` wraps an MDBX database; it keeps a small in-memory LRU to avoid hitting
/// storage for hot epochs. The DB layer persists snapshots as CBOR blobs via the `ParliaSnapshots`
/// table that is already defined in `db.rs`.
#[derive(Debug)]
pub struct DbSnapshotProvider<DB: Database> {
    db: DB,
    /// Front cache keyed by *block number*.
    cache: RwLock<LruMap<u64, Snapshot, ByLength>>,
}

impl<DB: Database> DbSnapshotProvider<DB> {
    pub fn new(db: DB, capacity: usize) -> Self {
        Self { db, cache: RwLock::new(LruMap::new(ByLength::new(capacity as u32))) }
    }

    fn load_from_db(&self, block_number: u64) -> Option<Snapshot> {
        let tx = self.db.tx().ok()?;
        let mut cursor = tx.cursor_read::<crate::consensus::parlia::db::ParliaSnapshots>()
            .ok()?;
        let mut iter = cursor.walk_range(..=block_number).ok()?;
        let mut last: Option<Snapshot> = None;
        while let Some(Ok((_, raw_blob))) = iter.next() {
            let raw = &raw_blob;
            if let Ok(decoded) = Snapshot::decompress(raw) {
                last = Some(decoded);
            }
        }
        last
    }

    fn persist_to_db(&self, snap: &Snapshot) -> Result<(), DatabaseError> {
        let tx = self.db.tx_mut()?;
        tx.put::<crate::consensus::parlia::db::ParliaSnapshots>(snap.block_number, snap.clone().compress())?;
        tx.commit()?;
        Ok(())
    }
}

impl<DB: Database + 'static> SnapshotProvider for DbSnapshotProvider<DB> {
    fn snapshot(&self, block_number: u64) -> Option<Snapshot> {
        // fast path: cache
        {
            let mut guard = self.cache.write();
            if let Some(snap) = guard.get(&block_number) {
                return Some(snap.clone());
            }
        }

        // slow path: DB scan
        let snap = self.load_from_db(block_number)?;
        self.cache.write().insert(block_number, snap.clone());
        Some(snap)
    }

    fn insert(&self, snapshot: Snapshot) {
        // update cache
        self.cache.write().insert(snapshot.block_number, snapshot.clone());
        // Persist only at checkpoint boundaries to reduce I/O.
        if snapshot.block_number % crate::consensus::parlia::snapshot::CHECKPOINT_INTERVAL == 0 {
            // fire-and-forget DB write; errors are logged but not fatal
            let _ = self.persist_to_db(&snapshot);
        }
    }
} 