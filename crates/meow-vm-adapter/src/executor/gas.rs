use meow_types::{
    digest::Digest, object::Object, system_framework::meow_coin,
    transaction::execution_result::ExecutionResult,
};

use crate::executor::versioning;

/// Deduct gas from the gas coin balance and append it to the changed objects list.
///
/// Called after every execution path — success or failure — so the gas coin is
/// always returned as a changed object.
pub fn apply_gas_spending(
    result: ExecutionResult,
    gas_coin: &Object,
    gas_spent: u64,
    tx_digest: &Digest,
) -> ExecutionResult {
    let updated_content = meow_coin::deduct_gas_coin_balance(gas_coin, gas_spent)
        .expect("gas coin content must be deductible");

    let new_version = versioning::bump_version(gas_coin);

    let updated_gas_coin = Object::new(
        *gas_coin.address(),
        *gas_coin.owner(),
        *tx_digest,
        new_version,
        gas_coin.type_().clone(),
        updated_content,
    );

    let mut changed = result.changed_objects().to_vec();
    changed.push(updated_gas_coin);

    ExecutionResult::new(
        result.status().clone(),
        *result.transaction_digest(),
        result.created_objects().to_vec(),
        changed,
        result.destroyed_objects().to_vec(),
    )
}
