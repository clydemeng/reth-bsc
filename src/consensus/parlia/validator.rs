use super::snapshot::{Snapshot, DEFAULT_TURN_LENGTH};
use super::{parse_vote_attestation_from_header, EXTRA_SEAL, EXTRA_VANITY};
use alloy_primitives::{Address, U256};
use reth::consensus::{ConsensusError, HeaderValidator};
use reth_primitives_traits::SealedHeader;
use std::sync::Arc;
use super::vote::{MAX_ATTESTATION_EXTRA_LENGTH, VoteAddress};
use super::constants::{VALIDATOR_BYTES_LEN_BEFORE_LUBAN, VALIDATOR_NUMBER_SIZE, VALIDATOR_BYTES_LEN_AFTER_LUBAN};
use bls_on_arkworks as bls;
use super::gas::validate_gas_limit;
use super::slash_pool;
use reth_db::table::Compress; // for Snapshot::compress

// ---------------------------------------------------------------------------
// Helper: parse epoch update (validator set & turn-length) from a header.
// Returns (validators, vote_addresses (if any), turn_length)
// ---------------------------------------------------------------------------
fn parse_epoch_update<H>(
    header: &H,
    is_luban: bool,
    is_bohr: bool,
) -> (Vec<Address>, Option<Vec<VoteAddress>>, Option<u8>)
where
    H: alloy_consensus::BlockHeader,
{
    let extra = header.extra_data().as_ref();
    if extra.len() <= EXTRA_VANITY + EXTRA_SEAL {
        return (Vec::new(), None, None);
    }

    // Epoch bytes start right after vanity
    let mut cursor = EXTRA_VANITY;

    // Pre-Luban epoch block: validators list only (20-byte each)
    if !is_luban {
        let validator_bytes = &extra[cursor..extra.len() - EXTRA_SEAL];
        let num = validator_bytes.len() / VALIDATOR_BYTES_LEN_BEFORE_LUBAN;
        let mut vals = Vec::with_capacity(num);
        for i in 0..num {
            let start = cursor + i * VALIDATOR_BYTES_LEN_BEFORE_LUBAN;
            let end = start + VALIDATOR_BYTES_LEN_BEFORE_LUBAN;
            vals.push(Address::from_slice(&extra[start..end]));
        }
        return (vals, None, None);
    }

    // Luban & later: 1-byte validator count
    let num_validators = extra[cursor] as usize;
    cursor += VALIDATOR_NUMBER_SIZE;
    
    // Sanity check: ensure we have enough space for all validators + optional turn length
    let required_space = EXTRA_VANITY + VALIDATOR_NUMBER_SIZE + 
                        (num_validators * VALIDATOR_BYTES_LEN_AFTER_LUBAN) + 
                        (if is_bohr { 1 } else { 0 }) + EXTRA_SEAL;
    if extra.len() < required_space {
        // Not enough space for the claimed number of validators
        return (Vec::new(), None, None);
    }

    let mut vals = Vec::with_capacity(num_validators);
    let mut vote_vals = Vec::with_capacity(num_validators);
    for _ in 0..num_validators {
        // Check bounds before accessing consensus address (20 bytes)
        if cursor + 20 > extra.len() - EXTRA_SEAL {
            // Not enough space for validator data
            return (vals, Some(vote_vals), None);
        }
        // 20-byte consensus addr
        vals.push(Address::from_slice(&extra[cursor..cursor + 20]));
        cursor += 20;
        
        // Check bounds before accessing BLS vote address (48 bytes)
        if cursor + 48 > extra.len() - EXTRA_SEAL {
            // Not enough space for vote address data
            return (vals, Some(vote_vals), None);
        }
        // 48-byte BLS vote addr
        vote_vals.push(VoteAddress::from_slice(&extra[cursor..cursor + 48]));
        cursor += 48;
    }

    // Optional turnLength byte in Bohr headers
    let turn_len = if is_bohr {
        // Check if there's space for turn length byte before EXTRA_SEAL
        if cursor + 1 <= extra.len() - EXTRA_SEAL {
            let tl = extra[cursor];
            Some(tl)
        } else {
            // Not enough space for turn length, header might be malformed
            None
        }
    } else {
        None
    };

    (vals, Some(vote_vals), turn_len)
}

/// Very light-weight snapshot provider (trait object) so the header validator can fetch the latest snapshot.
pub trait SnapshotProvider: Send + Sync {
    /// Returns the snapshot that is valid for the given `block_number` (usually parent block).
    fn snapshot(&self, block_number: u64) -> Option<Snapshot>;

    /// Inserts (or replaces) the snapshot in the provider.
    fn insert(&self, snapshot: Snapshot);
}

/// Header validator for Parlia consensus.
///
/// The validator currently checks:
/// 1. Miner (beneficiary) must be a validator in the current snapshot.
/// 2. Difficulty must be 2 when the miner is in-turn, 1 otherwise.
/// Further seal and vote checks will be added in later milestones.
#[derive(Debug, Clone)]
pub struct ParliaHeaderValidator<P> {
    provider: Arc<P>,
}

impl<P> ParliaHeaderValidator<P>
where
    P: SnapshotProvider + 'static,
{
    pub fn new(provider: Arc<P>) -> Self {
        // Register global snapshot provider (best‐effort; ignore if already set).
        crate::consensus::parlia::global_snapshot::set(provider.clone());
        Self { provider }
    }
}

// Helper to get expected difficulty.
fn expected_difficulty(inturn: bool) -> u64 { if inturn { 2 } else { 1 } }

impl<P, H> HeaderValidator<H> for ParliaHeaderValidator<P>
where
    P: SnapshotProvider + std::fmt::Debug + 'static,
    H: alloy_consensus::BlockHeader + alloy_primitives::Sealable,
{
    fn validate_header(&self, header: &SealedHeader<H>) -> Result<(), ConsensusError> {
        // Genesis header is considered valid.
        if header.number() == 0 {
            return Ok(());
        }

        // Fetch snapshot for parent block.
        let parent_number = header.number() - 1;
        let Some(snap) = self.provider.snapshot(parent_number) else {
            // During initial sync, we may not have snapshots for blocks yet.
            // In this case, we skip validation and trust the network consensus.
            // The full validation will happen when we catch up and have proper snapshots.
            // This is safe because:
            // 1. We're syncing from trusted peers
            // 2. The chain has already been validated by the network
            // 3. We'll validate properly once we have snapshots
            return Ok(());
        };

        let miner: Address = header.beneficiary();

        // Determine fork status for attestation parsing.
        let extra_len = header.header().extra_data().len();
        let is_luban = extra_len > EXTRA_VANITY + EXTRA_SEAL;
        let is_bohr = snap.turn_length.unwrap_or(DEFAULT_TURN_LENGTH) > DEFAULT_TURN_LENGTH;

        // Try parsing vote attestation (may be None).
        let _ = parse_vote_attestation_from_header(
            header.header(),
            snap.epoch_num,
            is_luban,
            is_bohr,
        );

        if !snap.validators.contains(&miner) {
            return Err(ConsensusError::Other("unauthorised validator".to_string()));
        }

        let inturn = snap.inturn_validator() == miner;
        let expected_diff = U256::from(expected_difficulty(inturn));
        if header.difficulty() != expected_diff {
            return Err(ConsensusError::Other("wrong difficulty for proposer turn".to_string()));
        }

        // Milestone-3: proposer over-propose rule
        if snap.sign_recently(miner) {
            return Err(ConsensusError::Other("validator has exceeded proposer quota in recent window".to_string()));
        }
        Ok(())
    }

    fn validate_header_against_parent(
        &self,
        header: &SealedHeader<H>,
        parent: &SealedHeader<H>,
    ) -> Result<(), ConsensusError> {
        // --------------------------------------------------------------------
        // 1. Basic parent/child sanity checks (number & timestamp ordering)
        // --------------------------------------------------------------------
        if header.number() != parent.number() + 1 {
            return Err(ConsensusError::ParentBlockNumberMismatch {
                parent_block_number: parent.number(),
                block_number: header.number(),
            });
        }
        // Maxwell hard-fork relaxation: equal timestamps are allowed.
        if header.timestamp() < parent.timestamp() {
            return Err(ConsensusError::TimestampIsInPast {
                parent_timestamp: parent.timestamp(),
                timestamp: header.timestamp(),
            });
        }

        // --------------------------------------------------------------------
        // 2. Snapshot of the *parent* block (needed for gas-limit & attestation verification)
        // --------------------------------------------------------------------
        let Some(parent_snap) = self.provider.snapshot(parent.number()) else {
            // During initial sync, we may not have snapshots yet.
            // Skip Parlia-specific validation and only do basic checks.
            return Ok(());
        };

        // --------------------------------------------------------------------
        // 2.5 Ramanujan block time validation
        // --------------------------------------------------------------------
        // After Ramanujan fork, enforce stricter timing rules
        if parent.number() >= 13082191 { // Ramanujan activation block on BSC mainnet
            let block_interval = parent_snap.block_interval;
            let validator = header.beneficiary();
            let is_inturn = parent_snap.inturn_validator() == validator;
            
            // Calculate back-off time for out-of-turn validators
            let back_off_time = if is_inturn {
                0
            } else {
                // Out-of-turn validators must wait longer
                let turn_length = parent_snap.turn_length.unwrap_or(1) as u64;
                turn_length * block_interval / 2
            };
            
            let min_timestamp = parent.timestamp() + block_interval + back_off_time;
            if header.timestamp() < min_timestamp {
                return Err(ConsensusError::Other(format!(
                    "Ramanujan block time validation failed: block {} timestamp {} too early (expected >= {})",
                    header.number(),
                    header.timestamp(),
                    min_timestamp
                )));
            }
        }

        // Gas-limit rule verification (Lorentz divisor switch).
        let epoch_len = parent_snap.epoch_num;
        let parent_gas_limit = parent.gas_limit();
        let gas_limit = header.gas_limit();
        if let Err(e) = validate_gas_limit(parent_gas_limit, gas_limit, epoch_len) {
            return Err(ConsensusError::Other(format!("invalid gas limit: {e}")));
        }

        // Use snapshot‐configured block interval to ensure header.timestamp is not too far ahead.
        if header.timestamp() > parent.timestamp() + parent_snap.block_interval {
            return Err(ConsensusError::Other("timestamp exceeds expected block interval".into()));
        }

        // --------------------------------------------------------------------
        // 3. Parse and verify vote attestation (Fast-Finality)
        // --------------------------------------------------------------------
        // Determine fork status for attestation parsing.
        let extra_len = header.header().extra_data().len();
        let is_luban = extra_len > EXTRA_VANITY + EXTRA_SEAL;
        let is_bohr = parent_snap.turn_length.unwrap_or(DEFAULT_TURN_LENGTH) > DEFAULT_TURN_LENGTH;

        let attestation_opt = parse_vote_attestation_from_header(
            header.header(),
            parent_snap.epoch_num,
            is_luban,
            is_bohr,
        );

        if let Some(ref att) = attestation_opt {
            // 3.1 extra bytes length guard
            if att.extra.len() > MAX_ATTESTATION_EXTRA_LENGTH {
                return Err(ConsensusError::Other("attestation extra too long".into()));
            }

            // 3.2 Attestation target MUST be the parent block.
            if att.data.target_number != parent.number() || att.data.target_hash != parent.hash() {
                return Err(ConsensusError::Other("invalid attestation target block".into()));
            }

            // 3.3 Attestation source MUST equal the latest justified checkpoint stored in snapshot.
            if att.data.source_number != parent_snap.vote_data.target_number ||
                att.data.source_hash != parent_snap.vote_data.target_hash
            {
                return Err(ConsensusError::Other("invalid attestation source checkpoint".into()));
            }

            // 3.4 Build list of voter BLS pub-keys from snapshot according to bit-set.
            let total_validators = parent_snap.validators.len();
            let bitset = att.vote_address_set;
            let voted_cnt = bitset.count_ones() as usize;

            if voted_cnt > total_validators {
                return Err(ConsensusError::Other("attestation vote count exceeds validator set".into()));
            }

            // collect vote addresses
            let mut pubkeys: Vec<Vec<u8>> = Vec::with_capacity(voted_cnt);
            for (idx, val_addr) in parent_snap.validators.iter().enumerate() {
                if (bitset & (1u64 << idx)) == 0 {
                    continue;
                }
                let Some(info) = parent_snap.validators_map.get(val_addr) else {
                    return Err(ConsensusError::Other("validator vote address missing".into()));
                };
                // Ensure vote address is non-zero (Bohr upgrade guarantees availability)
                if info.vote_addr.as_slice().iter().all(|b| *b == 0) {
                    return Err(ConsensusError::Other("validator vote address is zero".into()));
                }
                pubkeys.push(info.vote_addr.to_vec());
            }

            // 3.5 quorum check: ≥ 2/3 +1 of total validators
            let min_votes = (total_validators * 2 + 2) / 3; // ceil((2/3) * n)
            if pubkeys.len() < min_votes {
                return Err(ConsensusError::Other("insufficient attestation quorum".into()));
            }

            // 3.6 BLS aggregate signature verification.
            let message_hash = att.data.hash();
            let msg_vec = message_hash.as_slice().to_vec();
            let signature_bytes = att.agg_signature.to_vec();

            let mut msgs = Vec::with_capacity(pubkeys.len());
            msgs.resize(pubkeys.len(), msg_vec.clone());

            const BLS_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";

            let sig_ok = if pubkeys.len() == 1 {
                bls::verify(&pubkeys[0], &msg_vec, &signature_bytes, &BLS_DST.to_vec())
            } else {
                bls::aggregate_verify(pubkeys.clone(), msgs, &signature_bytes, &BLS_DST.to_vec())
            };

            if !sig_ok {
                return Err(ConsensusError::Other("invalid BLS aggregate signature".into()));
            }
        }

        // --------------------------------------------------------------------
        // 4. Advance snapshot once all parent-dependent checks are passed.
        // --------------------------------------------------------------------
        // Detect epoch checkpoint and parse validator set / turnLength if applicable
        let (new_validators, vote_addrs, turn_len) = if header.number() % parent_snap.epoch_num == 0 {
            parse_epoch_update(header.header(), is_luban, is_bohr)
        } else { (Vec::new(), None, None) };

        let new_snap = parent_snap.apply(
            header.beneficiary(),
            header.header(),
            new_validators,
            vote_addrs,
            attestation_opt,
            turn_len,
            is_bohr,
        ).ok_or_else(|| ConsensusError::Other("failed to apply snapshot".into()))?;

        self.provider.insert(new_snap.clone());
        // If this is a checkpoint boundary, enqueue the compressed snapshot so the execution
        // stage can persist it via `ExecutionOutcome`.
        if new_snap.block_number % super::snapshot::CHECKPOINT_INTERVAL == 0 {
            let blob = new_snap.clone().compress();
            crate::snapshot_pool::push((new_snap.block_number, blob));
        }

        // Report slashing evidence if proposer is not in-turn and previous inturn validator hasn't signed recently.
        let inturn_validator_eq_miner = header.beneficiary() == parent_snap.inturn_validator();
        if !inturn_validator_eq_miner {
            let spoiled = parent_snap.inturn_validator();
            if !parent_snap.sign_recently(spoiled) {
                slash_pool::report(spoiled);
            }
        }

        Ok(())
    }
} 