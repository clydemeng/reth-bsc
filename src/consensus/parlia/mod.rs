//! Skeleton implementation for Parlia (Proof-of-Staked-Authority) consensus.
//!
//! Phase-2: full data-structures ported from the abandoned `reth-bsc-trail` project.
//! Validation & fork-choice logic will follow in subsequent PRs.

// Re-export core sub-modules so that external crates can simply do:
// `use loocapro_reth_bsc::consensus::parlia::{Snapshot, VoteAddress, ...};`
pub mod vote;
pub mod snapshot;
pub mod provider;
pub mod validator;
pub mod validation;
pub mod hertz_patch;
pub mod constants;
pub mod attestation;
pub mod gas;
pub mod hooks;
pub mod slash_pool;
pub mod transaction_splitter;
pub mod consensus;
pub mod util;

pub use snapshot::{Snapshot, ValidatorInfo, CHECKPOINT_INTERVAL};
pub use vote::{VoteAddress, VoteAttestation, VoteData, VoteEnvelope, VoteSignature, ValidatorsBitSet};
pub use provider::InMemorySnapshotProvider;
pub use constants::*;
pub use attestation::parse_vote_attestation_from_header;
pub use validator::{ParliaHeaderValidator, SnapshotProvider};
pub use validation::BscConsensusValidator;
pub use hertz_patch::{HertzPatchManager, StoragePatch};
pub use transaction_splitter::{TransactionSplitter, SplitTransactions, TransactionSplitterError};
pub use consensus::ParliaConsensus;
pub use util::hash_with_chain_id;

/// Epoch length (200 blocks on BSC main-net).
pub const EPOCH: u64 = 200;
// Note: CHECKPOINT_INTERVAL is already defined in snapshot.rs and re-exported

// ============================================================================
// Signer helper (rotation schedule)
// ============================================================================

/// Helper that rotates proposers based on `block.number % epoch`.
#[derive(Debug, Clone)]
pub struct StepSigner {
    epoch: u64,
}

impl StepSigner {
    pub const fn new(epoch: u64) -> Self { Self { epoch } }

    /// Expected proposer index for `number` given a snapshot.
    pub fn proposer_index(&self, number: u64) -> u64 { number % self.epoch }
}

// The real trait impls (HeaderValidator, Consensus, FullConsensus) will be
// added in a later milestone. For now we only ensure the module compiles.

pub mod db;

#[cfg(test)]
mod tests; 