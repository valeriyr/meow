pub mod config;
pub mod error;
pub mod event;
pub mod message_id;
pub mod peer_id;

use std::time::Duration;

use futures::StreamExt;
use libp2p::{
    Multiaddr, SwarmBuilder,
    gossipsub::{self, IdentTopic, MessageAuthenticity},
    swarm::SwarmEvent,
};
use tracing::warn;

use crate::{
    config::NetworkConfig, error::NetworkError, event::NetworkEvent, message_id::MessageId,
    peer_id::PeerId,
};

/// The result type related to gossip network operations.
pub type Result<T> = std::result::Result<T, NetworkError>;

/// The gossip network handle.
pub struct GossipNetwork {
    swarm: libp2p::Swarm<gossipsub::Behaviour>,
}

impl GossipNetwork {
    /// Creates a new node with a freshly generated identity keypair.
    pub async fn new(config: NetworkConfig) -> Result<Self> {
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .build()?;

        let mut swarm = SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )?
            .with_behaviour(|key| {
                gossipsub::Behaviour::new(
                    MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )
                .expect("gossipsub config must be valid")
            })
            .expect("infallible error")
            .build();

        swarm.listen_on(config.listen_addr)?;

        for addr in config.bootstrap_peers {
            swarm.dial(addr)?;
        }

        Ok(Self { swarm })
    }

    /// Subscribe to a topic. Must be called before messages on that topic are delivered.
    pub fn subscribe(&mut self, topic: &str) -> Result<bool> {
        Ok(self
            .swarm
            .behaviour_mut()
            .subscribe(&IdentTopic::new(topic))?)
    }

    /// Publish a raw byte payload to a topic.
    pub fn publish(&mut self, topic: &str, data: Vec<u8>) -> Result<MessageId> {
        Ok(self
            .swarm
            .behaviour_mut()
            .publish(IdentTopic::new(topic), data)?
            .into())
    }

    /// The multiaddresses this node is listening on.
    pub fn listeners(&self) -> impl Iterator<Item = &Multiaddr> {
        self.swarm.listeners()
    }

    /// The local peer id.
    pub fn local_peer_id(&self) -> PeerId {
        PeerId::from(*self.swarm.local_peer_id())
    }

    /// Await the next network event.
    ///
    /// Returns `None` only if the swarm is fully shut down.
    pub async fn next_event(&mut self) -> Option<NetworkEvent> {
        loop {
            match self.swarm.next().await? {
                SwarmEvent::Behaviour(gossipsub::Event::Message {
                    message,
                    message_id,
                    ..
                }) => {
                    return Some(NetworkEvent::Message {
                        id: message_id.into(),
                        topic: message.topic.to_string(),
                        data: message.data,
                        from: message.source.map(PeerId::from),
                    });
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    return Some(NetworkEvent::PeerConnected(PeerId::from(peer_id)));
                }
                SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    return Some(NetworkEvent::PeerDisconnected(PeerId::from(peer_id)));
                }
                other => {
                    warn!("unhandled swarm event: {:?}", other);
                }
            }
        }
    }
}
