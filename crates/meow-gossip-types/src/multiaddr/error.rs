/// An error related to multiaddr.
#[derive(Debug, thiserror::Error)]
pub enum MultiaddrError {
    #[error("libp2p multiaddr error: {0}")]
    Libp2pMultiaddrError(#[from] libp2p::multiaddr::Error),
}
