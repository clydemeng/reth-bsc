use alloy_primitives::bytes::BytesMut;
use alloy_rlp::Decodable;
use futures::{Stream, StreamExt};
use std::{pin::Pin, task::{Context, Poll}};
use reth_eth_wire::multiplex::ProtocolConnection;

use crate::node::network::votes::{VotesPacket, BscCapPacket, handle_votes_broadcast};
use super::protocol::proto::BscProtoMessageId;

/// Stream that handles incoming BSC protocol messages (currently only Votes).
pub struct BscVotesConnection {
    conn: ProtocolConnection,
}

impl BscVotesConnection {
    pub fn new(conn: ProtocolConnection) -> Self { Self { conn } }
}

impl Stream for BscVotesConnection {
    type Item = BytesMut;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let Some(raw) = futures::ready!(this.conn.poll_next_unpin(cx)) else { return Poll::Ready(None) };
        let slice = raw.as_ref();
        if slice.is_empty() { return Poll::Pending }
        match slice[0] {
            x if x == BscProtoMessageId::Votes as u8 => {
                if let Ok(packet) = VotesPacket::decode(&mut &slice[..]) {
                    handle_votes_broadcast(packet);
                }
            }
            x if x == BscProtoMessageId::Capability as u8 => {
                // Decode and ignore capability for v1
                let _ = BscCapPacket::decode(&mut &slice[..]);
            }
            _ => {
                // Unknown message id; ignore.
            }
        }
        // This protocol does not proactively send responses; keep the connection open.
        Poll::Pending
    }
}


