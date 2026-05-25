//! Error type for the gossip network.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("behaviour init error: {0}")]
    BehaviourInitError(String),
    #[error("config builder error: {0}")]
    ConfigBuilderError(#[from] libp2p::gossipsub::ConfigBuilderError),
    #[error("dial error: {0}")]
    DialError(#[from] libp2p::swarm::DialError),
    #[error("noise error: {0}")]
    NoiseError(#[from] libp2p::noise::Error),
    #[error("publish error: {0}")]
    PublishError(#[from] libp2p::gossipsub::PublishError),
    #[error("subscribe error: {0}")]
    SubscriptionError(#[from] libp2p::gossipsub::SubscriptionError),
    #[error("transport error: {0}")]
    TransportError(#[from] libp2p::TransportError<std::io::Error>),
}
