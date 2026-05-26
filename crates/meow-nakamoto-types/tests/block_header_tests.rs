use meow_nakamoto_types::block_header::BlockHeader;
use meow_types::digest::Digest;

//
// ─── hash ───
//

/// `hash` must produce different values for headers that differ in any field.
#[test]
fn hash_changes_when_nonce_changes() {
    let base = base_header();
    let bumped = BlockHeader {
        nonce: 1,
        ..base.clone()
    };

    assert_ne!(base.hash(), bumped.hash());
}

/// `hash` must include `state_root` — changing it must change the block identity.
/// `mining_hash` must exclude `state_root` — changing it must not change the PoW target.
#[test]
fn mining_hash_excludes_state_root() {
    let base = base_header();
    let different_state = BlockHeader {
        state_root: Digest::from([0xFF; 32]),
        ..base.clone()
    };

    assert_ne!(
        base.hash(),
        different_state.hash(),
        "hash must include state_root"
    );
    assert_eq!(
        base.mining_hash(),
        different_state.mining_hash(),
        "mining_hash must not depend on state_root"
    );
}

/// `hash` must include `reward_root` — changing it must change the block identity.
/// `mining_hash` must exclude `reward_root` — changing it must not change the PoW target.
#[test]
fn mining_hash_excludes_reward_root() {
    let base = base_header();
    let with_reward = BlockHeader {
        reward_root: Some(Digest::from([0xAB; 32])),
        ..base.clone()
    };

    assert_ne!(
        base.hash(),
        with_reward.hash(),
        "hash must include reward_root"
    );
    assert_eq!(
        base.mining_hash(),
        with_reward.mining_hash(),
        "mining_hash must not depend on reward_root"
    );
}

//
// ─── meets_difficulty ───
//

#[test]
fn meets_difficulty_zero_is_always_satisfied() {
    let header = base_header();
    // Confirm the hash has no leading zeros — proving the test is non-trivial:
    // difficulty 0 passes not because the hash is lucky, but because 0 is always satisfied.
    assert_ne!(header.mining_hash().as_ref()[0], 0);
    assert!(header.meets_difficulty(0));
}

//
// ─── Utility functions ───
//

fn base_header() -> BlockHeader {
    BlockHeader {
        height: 0,
        parent_hash: Digest::ZERO,
        transactions_root: Digest::ZERO,
        reward_root: None,
        state_root: Digest::ZERO,
        timestamp: 0,
        nonce: 0,
    }
}
