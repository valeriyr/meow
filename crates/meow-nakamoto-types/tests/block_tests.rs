use meow_nakamoto_types::block::Block;
use meow_types::digest::Digest;

//
// ─── genesis ───
//

#[test]
fn genesis_block_has_expected_fields() {
    let block = Block::genesis();

    assert_eq!(block.header.height, 0);
    assert_eq!(block.header.parent_hash, Digest::ZERO);
    assert!(block.transactions.is_empty());
    assert!(block.results.is_empty());
    assert!(block.reward_transaction.is_none());
    assert!(block.reward_transaction_result.is_none());
}

//
// ─── hash ───
//

/// `Block::hash` is the block's unique identity — two genesis blocks must hash identically.
#[test]
fn block_hash_is_deterministic() {
    assert_eq!(Block::genesis().hash(), Block::genesis().hash());
}

/// `Block::hash` delegates to the header hash, so it must change when the header changes.
#[test]
fn block_hash_changes_when_header_changes() {
    let genesis = Block::genesis();

    let mut other = Block::genesis();
    other.header.height = 1;

    assert_ne!(genesis.hash(), other.hash());
}
