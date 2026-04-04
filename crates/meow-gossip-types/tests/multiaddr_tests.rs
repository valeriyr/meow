use meow_gossip_types::multiaddr::Multiaddr;

//
// ─── Multiaddr display tests ───
//

#[test]
fn str_roundtrip_preserves_value() {
    let str = "/ip4/1.2.3.4/tcp/30333";
    let addr: Multiaddr = str.parse().unwrap();
    assert_eq!(addr.to_string(), str);
}
