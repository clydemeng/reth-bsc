use reth_network_api::PeerId;
use reth_network::protocol::{ConnectionHandler, OnNotSupported, ProtocolHandler};
use reth_eth_wire::{capability::SharedCapabilities, multiplex::ProtocolConnection, protocol::Protocol};
use std::net::SocketAddr;

use super::proto::BscProtoMessage;
use crate::node::network::bsc_protocol::stream::BscVotesConnection;

#[derive(Clone, Debug, Default)]
pub struct BscProtocolHandler;

pub struct BscConnectionHandler;

impl ProtocolHandler for BscProtocolHandler {
    type ConnectionHandler = BscConnectionHandler;

    fn on_incoming(&self, _socket_addr: SocketAddr) -> Option<Self::ConnectionHandler> {
        Some(BscConnectionHandler)
    }

    fn on_outgoing(&self, _socket_addr: SocketAddr, _peer_id: PeerId) -> Option<Self::ConnectionHandler> {
        Some(BscConnectionHandler)
    }
}

impl ConnectionHandler for BscConnectionHandler {
    type Connection = BscVotesConnection;

    fn protocol(&self) -> Protocol { BscProtoMessage::protocol() }

    fn on_unsupported_by_peer(
        self,
        _supported: &SharedCapabilities,
        _direction: reth_network_api::Direction,
        _peer_id: PeerId,
    ) -> OnNotSupported {
        OnNotSupported::KeepAlive
    }

    fn into_connection(
        self,
        _direction: reth_network_api::Direction,
        _peer_id: PeerId,
        conn: ProtocolConnection,
    ) -> Self::Connection {
        BscVotesConnection::new(conn)
    }
}


