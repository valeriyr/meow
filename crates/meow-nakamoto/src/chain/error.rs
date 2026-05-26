//! Error type for the chain module.

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum ChainError {
    #[error("block is already known")]
    AlreadyKnown,
    #[error("block has no transactions")]
    EmptyBlock,
    #[error("block contains duplicate transaction")]
    DuplicateTransaction,
    #[error("block transaction execution failed")]
    ExecutionFailed,
    #[error("block height {got} is invalid, expected {expected}")]
    InvalidHeight { expected: u64, got: u64 },
    #[error("reward transaction and reward transaction result must both be present or both absent")]
    InconsistentReward,
    #[error("block reward transaction is invalid")]
    InvalidReward,
    #[error("transaction has invalid signature")]
    InvalidSignature,
    #[error("block does not meet PoW difficulty")]
    PowCheckFailed,
    #[error("block results count does not match transaction count")]
    ResultsCountMismatch,
    #[error("re-executed results do not match block")]
    ResultsMismatch,
    #[error("reward root does not match reward transaction")]
    RewardRootMismatch,
    #[error("snapshot height {snap_height} does not exceed local head height {head_height}")]
    SnapshotNotAdvancing { snap_height: u64, head_height: u64 },
    #[error("state root does not match re-executed store")]
    StateRootMismatch,
    #[error("block timestamp does not advance past parent")]
    TimestampNotAdvancing,
    #[error("block timestamp is too far in the future")]
    TimestampTooFarInFuture,
    #[error("transactions root mismatch")]
    TransactionsRootMismatch,
    #[error("block parent is not in the chain")]
    UnknownParent,
}
