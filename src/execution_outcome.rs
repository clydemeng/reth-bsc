// loocapro_reth_bsc specific wrapper around upstream ExecutionOutcome
use alloy_primitives::BlockNumber;
use reth_execution_types::ExecutionOutcome as UpOutcome;

/// BSC-specific execution outcome that carries Parlia snapshots.
#[derive(Debug, Clone, PartialEq)]
pub struct BscExecutionOutcome<T = reth_ethereum_primitives::Receipt> {
    /// Upstream outcome containing state, receipts, requests, etc.
    pub upstream: UpOutcome<T>,
    /// Parlia checkpoint snapshots produced during execution `(block, compressed_blob)`.
    pub snapshots: Vec<(BlockNumber, Vec<u8>)>,
}

impl<T> From<UpOutcome<T>> for BscExecutionOutcome<T> {
    fn from(o: UpOutcome<T>) -> Self {
        Self { upstream: o, snapshots: Vec::new() }
    }
}

impl<T> From<BscExecutionOutcome<T>> for UpOutcome<T> {
    fn from(b: BscExecutionOutcome<T>) -> Self { b.upstream }
}

impl<T> BscExecutionOutcome<T> {
    /// Build a [`BscExecutionOutcome`] from an upstream outcome *and* drain the
    /// global snapshot pool so the snapshots travel with this value.
    pub fn with_snapshots(upstream: UpOutcome<T>) -> Self {
        let snaps = crate::snapshot_pool::drain();
        Self { upstream, snapshots: snaps }
    }
} 