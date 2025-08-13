
use alloy_consensus::Header;
use alloy_primitives::{B256, U256, bytes::BytesMut, keccak256};
use alloy_rlp::Encodable;
use bytes::BufMut;
use super::constants::EXTRA_SEAL;

pub fn hash_with_chain_id(header: &Header, chain_id: u64) -> B256 {
    let mut out = BytesMut::new();
    encode_header_with_chain_id(header, &mut out, chain_id);
    keccak256(&out[..])
}

pub fn encode_header_with_chain_id(header: &Header, out: &mut dyn BufMut, chain_id: u64) {
    rlp_header(header, chain_id).encode(out);
    Encodable::encode(&U256::from(chain_id), out);
    Encodable::encode(&header.parent_hash, out);
    Encodable::encode(&header.ommers_hash, out);
    Encodable::encode(&header.beneficiary, out);
    Encodable::encode(&header.state_root, out);
    Encodable::encode(&header.transactions_root, out);
    Encodable::encode(&header.receipts_root, out);
    Encodable::encode(&header.logs_bloom, out);
    Encodable::encode(&header.difficulty, out);
    Encodable::encode(&U256::from(header.number), out);
    Encodable::encode(&header.gas_limit, out);
    Encodable::encode(&header.gas_used, out);
    Encodable::encode(&header.timestamp, out);
    Encodable::encode(&header.extra_data[..header.extra_data.len() - EXTRA_SEAL], out); // will panic if extra_data is less than EXTRA_SEAL_LEN
    Encodable::encode(&header.mix_hash, out);
    Encodable::encode(&header.nonce, out);

    if header.parent_beacon_block_root.is_some() &&
        header.parent_beacon_block_root.unwrap() == B256::default()
    {
        Encodable::encode(&U256::from(header.base_fee_per_gas.unwrap()), out);
        Encodable::encode(&header.withdrawals_root.unwrap(), out);
        Encodable::encode(&header.blob_gas_used.unwrap(), out);
        Encodable::encode(&header.excess_blob_gas.unwrap(), out);
        Encodable::encode(&header.parent_beacon_block_root.unwrap(), out);
        // https://github.com/bnb-chain/BEPs/blob/master/BEPs/BEP-466.md
        if header.requests_hash.is_some() {
            Encodable::encode(&header.requests_hash.unwrap(), out);
        }
        
    }
}

fn rlp_header(header: &Header, chain_id: u64) -> alloy_rlp::Header {
    let mut rlp_head = alloy_rlp::Header { list: true, payload_length: 0 };

    // add chain_id make more security
    rlp_head.payload_length += U256::from(chain_id).length(); // chain_id
    rlp_head.payload_length += header.parent_hash.length(); // parent_hash
    rlp_head.payload_length += header.ommers_hash.length(); // ommers_hash
    rlp_head.payload_length += header.beneficiary.length(); // beneficiary
    rlp_head.payload_length += header.state_root.length(); // state_root
    rlp_head.payload_length += header.transactions_root.length(); // transactions_root
    rlp_head.payload_length += header.receipts_root.length(); // receipts_root
    rlp_head.payload_length += header.logs_bloom.length(); // logs_bloom
    rlp_head.payload_length += header.difficulty.length(); // difficulty
    rlp_head.payload_length += U256::from(header.number).length(); // block height
    rlp_head.payload_length += header.gas_limit.length(); // gas_limit
    rlp_head.payload_length += header.gas_used.length(); // gas_used
    rlp_head.payload_length += header.timestamp.length(); // timestamp
    rlp_head.payload_length +=
        &header.extra_data[..header.extra_data.len() - EXTRA_SEAL].length(); // extra_data
    rlp_head.payload_length += header.mix_hash.length(); // mix_hash
    rlp_head.payload_length += header.nonce.length(); // nonce

    if header.parent_beacon_block_root.is_some() &&
        header.parent_beacon_block_root.unwrap() == B256::default()
    {
        rlp_head.payload_length += U256::from(header.base_fee_per_gas.unwrap()).length();
        rlp_head.payload_length += header.withdrawals_root.unwrap().length();
        rlp_head.payload_length += header.blob_gas_used.unwrap().length();
        rlp_head.payload_length += header.excess_blob_gas.unwrap().length();
        rlp_head.payload_length += header.parent_beacon_block_root.unwrap().length();
        // https://github.com/bnb-chain/BEPs/blob/master/BEPs/BEP-466.md
        if header.requests_hash.is_some() {
            rlp_head.payload_length += header.requests_hash.unwrap().length();
        }
    }
    rlp_head
}