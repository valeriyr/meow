/// Opaque identifier for a peer in the network.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PeerId(libp2p::PeerId);

impl std::fmt::Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<libp2p::PeerId> for PeerId {
    fn from(peer_id: libp2p::PeerId) -> Self {
        PeerId(peer_id)
    }
}
