use alloy_primitives::BlockNumber;
use once_cell::sync::Lazy;
use std::sync::Mutex;

/// Global accumulator for Parlia checkpoint snapshots produced by the BSC executor.
/// Each entry is `(block_number, compressed_snapshot_blob)`.
static POOL: Lazy<Mutex<Vec<(BlockNumber, Vec<u8>)>>> = Lazy::new(|| Mutex::new(Vec::new()));

/// Push a snapshot blob so it can later be drained into the [`BscExecutionOutcome`].
pub fn push(snapshot: (BlockNumber, Vec<u8>)) {
    POOL.lock().expect("snapshot pool mutex poisoned").push(snapshot);
}

/// Drain **all** queued snapshots (FIFO order).
pub fn drain() -> Vec<(BlockNumber, Vec<u8>)> {
    POOL.lock().expect("snapshot pool mutex poisoned").drain(..).collect()
} 