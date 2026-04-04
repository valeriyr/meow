pub mod error;

use std::time::Duration;

use futures::StreamExt;
use libp2p::{
    Multiaddr, SwarmBuilder,
    gossipsub::{self, IdentTopic, MessageAuthenticity},
    mdns,
    swarm::{NetworkBehaviour, SwarmEvent},
};
use meow_gossip_types::{
    config::GossipNetworkConfig, event::NetworkEvent, message_id::MessageId, peer_id::PeerId,
};
use tracing::debug;

use crate::error::NetworkError;

/// The result type related to gossip network operations.
pub type Result<T> = std::result::Result<T, NetworkError>;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "BehaviourEvent")]
struct Behaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum BehaviourEvent {
    Gossipsub(gossipsub::Event),
    Mdns(mdns::Event),
}

impl From<gossipsub::Event> for BehaviourEvent {
    fn from(value: gossipsub::Event) -> Self {
        Self::Gossipsub(value)
    }
}

impl From<mdns::Event> for BehaviourEvent {
    fn from(value: mdns::Event) -> Self {
        Self::Mdns(value)
    }
}

/// The gossip network handle.
pub struct GossipNetwork {
    swarm: libp2p::Swarm<Behaviour>,
}

impl GossipNetwork {
    /// Creates a new node with a freshly generated identity keypair.
    pub async fn new(config: GossipNetworkConfig) -> Result<Self> {
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
            .with_behaviour(|key| Behaviour {
                gossipsub: gossipsub::Behaviour::new(
                    MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )
                .expect("gossipsub config must be valid"),
                mdns: mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    key.public().to_peer_id(),
                )
                .expect("mDNS config must be valid"),
            })
            .expect("infallible error")
            .build();

        swarm.listen_on(config.listen_address.into())?;

        for addr in config.bootstrap_peers {
            swarm.dial(Into::<libp2p::Multiaddr>::into(addr))?;
        }

        Ok(Self { swarm })
    }

    /// Subscribe to a topic. Must be called before messages on that topic are delivered.
    pub fn subscribe(&mut self, topic: &str) -> Result<bool> {
        Ok(self
            .swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&IdentTopic::new(topic))?)
    }

    /// Publish a raw byte payload to a topic.
    pub fn publish(&mut self, topic: &str, data: Vec<u8>) -> Result<MessageId> {
        Ok(self
            .swarm
            .behaviour_mut()
            .gossipsub
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
                SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    message,
                    message_id,
                    ..
                })) => {
                    return Some(NetworkEvent::Message {
                        id: message_id.into(),
                        topic: message.topic.to_string(),
                        data: message.data,
                        from: message.source.map(PeerId::from),
                    });
                }
                SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(peers))) => {
                    for (peer_id, _addr) in peers {
                        self.swarm
                            .behaviour_mut()
                            .gossipsub
                            .add_explicit_peer(&peer_id);
                    }
                }
                SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
                    for (peer_id, _addr) in peers {
                        self.swarm
                            .behaviour_mut()
                            .gossipsub
                            .remove_explicit_peer(&peer_id);
                    }
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    return Some(NetworkEvent::PeerConnected(PeerId::from(peer_id)));
                }
                SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    return Some(NetworkEvent::PeerDisconnected(PeerId::from(peer_id)));
                }
                other => {
                    debug!("unhandled swarm event: {:?}", other);
                }
            }
        }
    }
}
