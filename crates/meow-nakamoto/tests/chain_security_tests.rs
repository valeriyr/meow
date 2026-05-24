use meow_nakamoto::{
    chain::{ChainState, compute_state_root, compute_transactions_root},
    store::Store,
};
use meow_nakamoto_types::{block::Block, block_header::BlockHeader};
use meow_types::{digest::Digest, transaction::execution_result::ExecutionResult};

//
// ─── apply_block ───
//

#[test]
fn block_with_forged_results_is_rejected() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let block = Block {
        header: BlockHeader {
            height: 1,
            parent_hash: chain.head(),
            transactions_root: compute_transactions_root(&[]),
            state_root: compute_state_root(&store),
            timestamp: 0,
            nonce: 0,
        },
        transactions: vec![],
        results: vec![ExecutionResult::failure("forged", Digest::ZERO)],
        reward_transaction: None,
        reward_transaction_result: None,
    };

    assert!(!chain.apply_block(block));
    assert_eq!(chain.head_height(), 0);
    assert!(chain.get_transaction_result(&Digest::ZERO).is_none());
}
