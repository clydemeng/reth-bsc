use once_cell::sync::Lazy;
use std::{collections::HashSet, sync::Mutex};

use alloy_primitives::B256;

use super::vote::VoteEnvelope;

/// Global in-memory pool of incoming Parlia votes.
///
/// This mirrors the simple approach used by the slashing pool: keep votes in
/// memory until they're consumed by another component. Votes are de-duplicated
/// by their RLP hash.
struct VotePool {
    /// Hashes of votes we've already seen in this window.
    seen_hashes: HashSet<B256>,
    /// Collected votes (deduplicated by `seen_hashes`).
    votes: Vec<VoteEnvelope>,
}

impl VotePool {
    fn new() -> Self {
        Self { seen_hashes: HashSet::new(), votes: Vec::new() }
    }

    fn insert(&mut self, vote: VoteEnvelope) {
        let vote_hash = vote.hash();
        if self.seen_hashes.insert(vote_hash) {
            self.votes.push(vote);
        }
    }

    fn drain(&mut self) -> Vec<VoteEnvelope> {
        self.seen_hashes.clear();
        std::mem::take(&mut self.votes)
    }

    fn len(&self) -> usize { self.votes.len() }
}

/// Global singleton pool.
static VOTE_POOL: Lazy<Mutex<VotePool>> = Lazy::new(|| Mutex::new(VotePool::new()));

/// Insert a single vote into the pool (deduplicated by hash).
pub fn put_vote(vote: VoteEnvelope) {
    VOTE_POOL.lock().expect("vote pool poisoned").insert(vote);
}

/// Drain all pending votes.
pub fn drain() -> Vec<VoteEnvelope> {
    VOTE_POOL.lock().expect("vote pool poisoned").drain()
}

/// Current number of queued votes.
pub fn len() -> usize { VOTE_POOL.lock().expect("vote pool poisoned").len() }


