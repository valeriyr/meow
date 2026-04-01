/// Opaque identifier for a message in the gossip network.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageId(libp2p::gossipsub::MessageId);

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<libp2p::gossipsub::MessageId> for MessageId {
    fn from(message_id: libp2p::gossipsub::MessageId) -> Self {
        MessageId(message_id)
    }
}
