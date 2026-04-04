/// Errors related to the gossip service.
#[derive(Debug, thiserror::Error)]
pub enum GossipServiceError {
    #[error("gossip network error: {0}")]
    GossipNetworkError(#[from] meow_gossip_network::error::NetworkError),
    #[error("miner error: {0}")]
    MinerError(#[from] meow_nakamoto::miner::error::MinerError),
}
