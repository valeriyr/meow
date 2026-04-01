use meow_gossip_network::{
    GossipNetwork, config::NetworkConfig, error::NetworkError, event::NetworkEvent,
};
use tokio::sync::mpsc;

pub const TOPIC_TXS: &str = "txns";
pub const TOPIC_BLOCKS: &str = "blocks";

/// A cheap-to-clone handle to the gossip actor task.
///
/// Internally backed by an `mpsc` channel; cloning just clones the sender.
#[derive(Clone)]
pub struct GossipHandle {
    publish_tx: mpsc::UnboundedSender<(String, Vec<u8>)>,
}

impl GossipHandle {
    pub fn publish(&self, topic: &str, data: Vec<u8>) {
        let _ = self.publish_tx.send((topic.to_owned(), data));
    }
}

/// Starts the gossip actor and returns a handle for publishing plus a receiver
/// for incoming network events.
///
/// The actor owns the `GossipNetwork` and drives it in a background task,
/// allowing `publish` and `next_event` (both `&mut self`) to coexist safely.
pub async fn start(
    config: NetworkConfig,
) -> Result<(GossipHandle, mpsc::UnboundedReceiver<NetworkEvent>), NetworkError> {
    let mut gossip: GossipNetwork = GossipNetwork::new(config).await?;
    gossip.subscribe(TOPIC_TXS)?;
    gossip.subscribe(TOPIC_BLOCKS)?;

    let (publish_tx, mut publish_rx) = mpsc::unbounded_channel::<(String, Vec<u8>)>();
    let (events_tx, events_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = publish_rx.recv() => {
                    match msg {
                        Some((topic, data)) => {
                            if let Err(e) = gossip.publish(&topic, data) {
                                tracing::warn!("gossip publish error: {e}");
                            }
                        }
                        None => break,
                    }
                }
                event = gossip.next_event() => {
                    match event {
                        Some(e) => { let _ = events_tx.send(e); }
                        None => break,
                    }
                }
            }
        }
        tracing::warn!("gossip actor exited");
    });

    Ok((GossipHandle { publish_tx }, events_rx))
}
