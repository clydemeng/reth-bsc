use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use bytes::{BufMut, Bytes};

use crate::consensus::parlia::{vote::VoteEnvelope, votes};
use crate::node::network::bsc_protocol::protocol::proto::BscProtoMessageId;

/// BSC capability packet: version + extra RLP value (opaque), message id 0x00
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BscCapPacket {
    pub protocol_version: u64,
    pub extra: Bytes,
}

impl Encodable for BscCapPacket {
    fn encode(&self, out: &mut dyn BufMut) {
        (BscProtoMessageId::Capability as u8).encode(out);
        CapPayload { protocol_version: self.protocol_version, extra: self.extra.clone() }
            .encode(out);
    }
}

impl Decodable for BscCapPacket {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let message_id = u8::decode(buf)?;
        if message_id != (BscProtoMessageId::Capability as u8) {
            return Err(alloy_rlp::Error::Custom("Invalid message ID for BscCapPacket"));
        }
        let CapPayload { protocol_version, extra } = CapPayload::decode(buf)?;
        Ok(Self { protocol_version, extra })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
struct CapPayload {
    protocol_version: u64,
    extra: Bytes,
}

/// VotesPacket carries a list of votes (message id 0x01)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VotesPacket(pub Vec<VoteEnvelope>);

impl Encodable for VotesPacket {
    fn encode(&self, out: &mut dyn BufMut) {
        (BscProtoMessageId::Votes as u8).encode(out);
        self.0.encode(out);
    }
}

impl Decodable for VotesPacket {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let message_id = u8::decode(buf)?;
        if message_id != (BscProtoMessageId::Votes as u8) {
            return Err(alloy_rlp::Error::Custom("Invalid message ID for VotesPacket"));
        }
        let votes = Vec::<VoteEnvelope>::decode(buf)?;
        Ok(Self(votes))
    }
}

/// Handle an incoming `VotesPacket` from a peer.
/// To avoid DoS from massive batches, only enqueue the first vote if present,
/// mirroring Geth's logic.
pub fn handle_votes_broadcast(packet: VotesPacket) {
    if let Some(first) = packet.0.into_iter().next() {
        votes::put_vote(first);
    }
}