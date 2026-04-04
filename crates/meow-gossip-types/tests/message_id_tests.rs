use meow_gossip_types::message_id::MessageId;

//
// ─── MessageId display tests ───
//

#[test]
fn from_libp2p_message_id_converts() {
    let id = id_from_bytes(vec![1, 2, 3]);
    assert_eq!(id.to_string(), "010203");
}

#[test]
fn empty_message_id_display() {
    let id = id_from_bytes(vec![]);
    assert_eq!(id.to_string(), "");
}

//
// ─── Utility functions ───
//

fn id_from_bytes(bytes: Vec<u8>) -> MessageId {
    MessageId::from(libp2p::gossipsub::MessageId(bytes))
}
