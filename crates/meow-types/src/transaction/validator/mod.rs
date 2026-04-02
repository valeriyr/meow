pub mod error;

use std::collections::HashSet;

use meow_vm_types::config::CompilerConfig;

use crate::{
    address::Address,
    config::{
        MAX_BCS_SERIALIZED_MODULE_SIZE, MEOW_CALL_TRANSACTION_BCS_BYTES_MAX_SIZE,
        MEOW_PUBLISH_MODULE_TRANSACTION_BCS_BYTES_MAX_SIZE,
    },
    transaction::{
        Transaction,
        call::{Call, Input},
        transaction_type::TransactionType,
    },
};

pub use error::ValidationError;

/// The result type for transaction validation.
pub type Result<T> = std::result::Result<T, ValidationError>;

/// Validates a [`Transaction`] for structural correctness.
pub fn validate_transaction(transaction: &Transaction, config: &CompilerConfig) -> Result<()> {
    match transaction.type_() {
        TransactionType::MeowCall(call) => {
            validate_transaction_size(transaction, MEOW_CALL_TRANSACTION_BCS_BYTES_MAX_SIZE)?;
            validate_call_args_count(call, config)?;
            validate_gas_coin_not_in_args(call, transaction.gas_coin().address())?;
            validate_no_aliased_args(call)?;
        }
        TransactionType::MeowModulePublish(module) => {
            validate_transaction_size(
                transaction,
                MEOW_PUBLISH_MODULE_TRANSACTION_BCS_BYTES_MAX_SIZE,
            )?;
            validate_module_size(module)?;
        }
    }

    Ok(())
}

/// Validates that the transaction size does not exceed the specified limit.
fn validate_transaction_size(transaction: &Transaction, limit: usize) -> Result<()> {
    let size = bcs::serialized_size(transaction).expect("serialization is infallible");
    if size > limit {
        return Err(ValidationError::TransactionTooLarge { size, limit });
    }
    Ok(())
}

/// Validates that the number of call arguments does not exceed the compiler limit.
fn validate_call_args_count(call: &Call, compiler: &CompilerConfig) -> Result<()> {
    let amount = call.arguments().len();
    let limit = compiler.max_params();
    if amount > limit {
        return Err(ValidationError::TooManyCallArguments { amount, limit });
    }
    Ok(())
}

/// Validates that the gas coin is not used as a call argument.
fn validate_gas_coin_not_in_args(call: &Call, gas_coin_address: &Address) -> Result<()> {
    let used = call.arguments().iter().any(
        |arg| matches!(arg, Input::Object(object_ref) if object_ref.address() == gas_coin_address),
    );
    if used {
        return Err(ValidationError::GasCoinUsedAsCallArgument(
            *gas_coin_address,
        ));
    }
    Ok(())
}

/// Validates that no call argument is duplicated (i.e., aliased).
fn validate_no_aliased_args(call: &Call) -> Result<()> {
    let mut seen = HashSet::new();
    for arg in call.arguments() {
        if let Input::Object(object_ref) = arg
            && !seen.insert(object_ref.address())
        {
            return Err(ValidationError::AliasedCallArgument(*object_ref.address()));
        }
    }
    Ok(())
}

/// Validates that the module size does not exceed the compiler limit.
fn validate_module_size(module: &[u8]) -> Result<()> {
    let size = module.len();
    let limit = MAX_BCS_SERIALIZED_MODULE_SIZE;
    if size > limit {
        return Err(ValidationError::ModuleTooLarge { size, limit });
    }
    Ok(())
}
