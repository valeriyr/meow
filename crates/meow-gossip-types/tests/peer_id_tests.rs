//
// ─── PeerId display tests ───
//

#[test]
fn display_is_non_empty() {
    let peer_id = make_deterministic_peer_id();
    assert_eq!(
        peer_id.to_string(),
        "12D3KooWK99VoVxNE7XzyBwXEzW7xhK7Gpv85r9F3V3fyKSUKPH5"
    );
}

//
// ─── Utility functions ───
//

fn make_deterministic_peer_id() -> meow_gossip_types::peer_id::PeerId {
    use libp2p::identity::{Keypair, ed25519};
    let secret = ed25519::SecretKey::try_from_bytes([1u8; 32]).unwrap();
    let keypair = Keypair::from(ed25519::Keypair::from(secret));
    meow_gossip_types::peer_id::PeerId::from(keypair.public().to_peer_id())
}
