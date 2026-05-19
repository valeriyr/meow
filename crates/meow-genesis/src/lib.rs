//! Genesis state builder for the Meow blockchain.
//!
//! Publishes the system framework modules and mints the initial coin allocations
//! to produce the set of objects that form block zero.

pub mod error;

use meow_framework::framework_module_objects;
use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{Object, object_ref::ObjectRef, object_version::ObjectVersion},
    system_framework::{
        MEOW_SYSTEM_ADDRESS,
        meow_coin::{self, MEOW_COIN_MINT_FUNCTION_NAME, MEOW_COIN_MODULE_ADDRESS},
    },
    transaction::{
        Transaction, call::Call, execution_result::ExecutionStatus, input::Input,
        transaction_type::TransactionType,
    },
};
use meow_vm_adapter::executor;
use serde::{Deserialize, Serialize};

use crate::error::GenesisError;

/// The result type related to genesis.
pub type Result<T> = std::result::Result<T, GenesisError>;

/// The genesis state of the chain.
#[derive(Serialize, Deserialize, Debug)]
pub struct Genesis {
    /// The genesis objects.
    objects: Vec<Object>,
}

impl Genesis {
    /// Builds the genesis state.
    pub fn build(allocations: &[(Address, u64)]) -> Result<Self> {
        let framework_modules = framework_module_objects();
        let meow_coins = mint_meow_coins(allocations, &framework_modules)?;

        let mut objects = framework_modules;
        objects.extend(meow_coins);

        Ok(Self { objects })
    }

    /// Returns the genesis objects.
    pub fn objects(&self) -> &[Object] {
        &self.objects
    }
}

/// Mints Meow Coins according to the provided minting instructions.
fn mint_meow_coins(
    allocations: &[(Address, u64)],
    framework_modules: &[Object],
) -> Result<Vec<Object>> {
    let function =
        Identifier::new(MEOW_COIN_MINT_FUNCTION_NAME).expect("mint function name is always valid");

    allocations
        .iter()
        .map(|(address, amount)| -> Result<Object> {
            let inputs = vec![
                Input::raw(amount).expect("amount BCS serialization is always valid"),
                Input::raw(address).expect("address BCS serialization is always valid"),
            ];

            let transaction = Transaction::new(
                MEOW_SYSTEM_ADDRESS,
                ObjectRef::new(Address::ZERO, ObjectVersion::ZERO, Digest::ZERO),
                TransactionType::MeowCall(Call::new(
                    MEOW_COIN_MODULE_ADDRESS,
                    function.clone(),
                    inputs,
                )),
            );

            let execution_result =
                executor::execute_genesis_transaction(&transaction, framework_modules.to_vec())?;

            match execution_result.status() {
                ExecutionStatus::Success => {
                    let created_objects_amount = execution_result.created_objects().len();
                    if created_objects_amount != 1 {
                        return Err(GenesisError::MeowCoinMintFailed(format!(
                            "expected 1 created object, but found {created_objects_amount}",
                        )));
                    }

                    let meow_coin = execution_result.created_objects()[0].clone();

                    if meow_coin::is_meow_coin_object(&meow_coin) {
                        Ok(meow_coin)
                    } else {
                        Err(GenesisError::MeowCoinMintFailed(format!(
                            "expected a MEOW coin created, but found an object of type {:?}",
                            meow_coin.type_()
                        )))
                    }
                }
                ExecutionStatus::Failure(message) => {
                    Err(GenesisError::MeowCoinMintFailed(message.clone()))
                }
            }
        })
        .collect()
}
