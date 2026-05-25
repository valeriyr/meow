//! System transactions: construction, validation, and execution.

use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{Object, object_ref::ObjectRef, object_version::ObjectVersion},
    system_framework::{
        meow_coin::{MEOW_COIN_MINT_FUNCTION_NAME, MEOW_COIN_MODULE_ADDRESS},
        meow_object::MEOW_OBJECT_MODULE_ADDRESS,
    },
    transaction::{
        Transaction, call::Call, execution_result::ExecutionResult, input::Input,
        transaction_type::TransactionType,
    },
};
use meow_vm_adapter::executor;

use crate::store::Store;

/// Build the `MeowCall` transaction that mints `amount` coins to `reward_address`.
///
/// `signer` is the transaction sender (must match the signing keypair's address).
/// `reward_address` is the recipient — the miner may direct rewards to any address,
/// for example a cold wallet separate from the signing key.
///
/// `mining_block_hash` makes the transaction unique per block: the miner may mine many blocks, and each
/// reward transaction must produce a distinct digest so their resulting object IDs do not collide.
pub fn make_reward_transaction(
    signer: Address,
    reward_address: Address,
    amount: u64,
    mining_block_hash: Digest,
) -> Transaction {
    Transaction::new(
        signer,
        // The gas-coin `ObjectRef` is a placeholder (system transactions execution bypasses all gas-coin checks);
        // we embed `mining_block_hash` into its digest so the transaction digest varies per block.
        ObjectRef::new(Address::ZERO, ObjectVersion::ZERO, mining_block_hash),
        TransactionType::MeowCall(Call::new(
            MEOW_COIN_MODULE_ADDRESS,
            Identifier::new(MEOW_COIN_MINT_FUNCTION_NAME).expect("mint is a valid identifier"),
            vec![
                Input::raw(&amount).expect("u64 serialization is infallible"),
                Input::raw(&reward_address).expect("address serialization is infallible"),
            ],
        )),
    )
}

/// Return `true` if `reward_transaction` is a valid `meow_coin::mint(amount, address)` call
/// with `expected_amount` as the first argument and a well-formed address as the second.
///
/// Also enforces the gas-coin placeholder convention: `address` must be `Address::ZERO`,
/// `version` must be `ObjectVersion::ZERO`, and `digest` must equal `mining_block_hash`.
pub fn is_valid_reward_transaction(
    reward_transaction: &Transaction,
    expected_amount: u64,
    mining_block_hash: Digest,
) -> bool {
    let TransactionType::MeowCall(call) = reward_transaction.type_() else {
        return false;
    };
    let gas_coin = reward_transaction.gas_coin();
    if *gas_coin.address() != Address::ZERO
        || *gas_coin.version() != ObjectVersion::ZERO
        || *gas_coin.digest() != mining_block_hash
    {
        return false;
    }
    if call.module() != &MEOW_COIN_MODULE_ADDRESS {
        return false;
    }
    if call.function().as_ref() != MEOW_COIN_MINT_FUNCTION_NAME {
        return false;
    }
    if call.arguments().len() != 2 {
        return false;
    }
    let Some(Input::Raw(amount_bytes)) = call.arguments().first() else {
        return false;
    };
    if bcs::from_bytes::<u64>(amount_bytes).ok() != Some(expected_amount) {
        return false;
    }
    let Some(Input::Raw(addr_bytes)) = call.arguments().get(1) else {
        return false;
    };
    bcs::from_bytes::<Address>(addr_bytes).is_ok()
}

/// Collect framework module objects required to execute a reward transaction.
///
/// # Panics
///
/// Panics if either framework module is absent from the store.
/// Both modules are installed at genesis and must always be present.
pub fn collect_inputs_for_reward_transaction(store: &Store) -> Vec<Object> {
    [MEOW_OBJECT_MODULE_ADDRESS, MEOW_COIN_MODULE_ADDRESS]
        .iter()
        .map(|addr| {
            store
                .get_object(addr)
                .cloned()
                .expect("framework module must be present in store")
        })
        .collect()
}

/// Execute the block reward transaction via the privileged system path.
/// Uses `execute_system_transaction`, which bypasses gas-coin validation
/// and grants access to framework-internal functions such as `mint`.
pub fn execute_reward_transaction(
    transaction: &Transaction,
    store: &Store,
) -> executor::Result<ExecutionResult> {
    let inputs = collect_inputs_for_reward_transaction(store);
    executor::execute_system_transaction(transaction, inputs)
}
