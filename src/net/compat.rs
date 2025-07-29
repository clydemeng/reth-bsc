//! Tolerant decoders for BSC‐style extended RLP payloads.
use alloy_rlp::{Decodable, Header, Error as RlpError};
use bytes::Buf;
use reth_eth_wire_types::{
    BlockHashNumber, EthNetworkPrimitives, NewBlock, NewBlockHashes, NewBlockPayload,
};

pub fn new_block_hashes(bytes: &[u8]) -> Option<NewBlockHashes> {
    let mut buf = bytes;
    let outer = Header::decode(&mut buf).ok()?;
    if !outer.list { return None }

    let mut payload = &buf[..outer.payload_length];
    let mut out = Vec::new();
    while !payload.is_empty() {
        let h = Header::decode(&mut payload).ok()?;
        if !h.list { return None }
        let mut entry = &payload[..h.payload_length];

        let hash = <[u8;32]>::decode(&mut entry).ok()?.into();
        let num  = u64::decode(&mut entry).ok()?;
        out.push(BlockHashNumber{hash, number:num});
        payload.advance(h.payload_length);
    }
    Some(NewBlockHashes(out))
}

pub fn new_block<N: EthNetworkPrimitives>(bytes: &[u8]) -> Option<Box<NewBlock<N>>> {
    let mut buf = bytes;
    let hdr = Header::decode(&mut buf).ok()?;
    if !hdr.list { return None }

    let mut payload = &buf[..hdr.payload_length];
    let block = N::NewBlockPayload::decode(&mut payload).ok()?;
    let td    = alloy_primitives::U128::decode(&mut payload).ok()?;
    Some(Box::new(NewBlock { block, td }))
}