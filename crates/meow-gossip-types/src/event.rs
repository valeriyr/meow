use crate::{message_id::MessageId, peer_id::PeerId};

/// An event produced by the gossip network.
#[derive(Debug)]
pub enum NetworkEvent {
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
}
