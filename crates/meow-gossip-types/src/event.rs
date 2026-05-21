//! Events emitted by the gossip network layer and consumed by the node to dispatch incoming messages.

use crate::{message_id::MessageId, multiaddr::Multiaddr, peer_id::PeerId};

/// An event produced by the gossip network.
#[derive(Debug)]
pub enum NetworkEvent {
    /// The node is now listening on a bound address.
    Listening { addr: Multiaddr },
    /// A message arrived on a subscribed topic.
    Message {
        id: MessageId,
        topic: String,
        data: Vec<u8>,
        from: Option<PeerId>,
    },
    /// A peer connected.
    PeerConnected(PeerId),
    /// A peer disconnected.
    PeerDisconnected(PeerId),
    /// A peer subscribed to a topic. Safe to send messages to that peer on this topic now.
    PeerSubscribedToTopic { peer: PeerId, topic: String },
}
