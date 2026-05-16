pub mod error;

use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{Object, object_ref::ObjectRef, object_version::ObjectVersion},
    system_framework::{
        MEOW_SYSTEM_ADDRESS,
        meow_coin::{
            self, MEOW_COIN_MINT_FUNCTION_NAME, MEOW_COIN_MODULE_ADDRESS, MEOW_COIN_MODULE_PATH,
        },
        meow_object::{MEOW_OBJECT_MODULE_ADDRESS, MEOW_OBJECT_MODULE_PATH},
    },
    transaction::{
        Transaction, call::Call, execution_result::ExecutionStatus, input::Input,
        transaction_type::TransactionType,
    },
};
use meow_vm_adapter::{Module, builder, executor};
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
        let meow_object_module = build_meow_object_module()?;
        let meow_object_vm_module: Module = bcs::from_bytes(meow_object_module.content())?;

        let meow_coin_module =
            build_meow_coin_module(&[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_vm_module)])?;
        let meow_coins = mint_meow_coins(
            meow_object_module.clone(),
            meow_coin_module.clone(),
            allocations,
        )?;

        let mut objects = vec![meow_object_module, meow_coin_module];
        objects.extend(meow_coins);

        Ok(Self { objects })
    }

    /// Returns the genesis objects.
    pub fn objects(&self) -> &[Object] {
        &self.objects
    }
}

/// Builds the meow_object framework module object (published at 0x01).
fn build_meow_object_module() -> Result<Object> {
    let module = builder::build_from_file(MEOW_OBJECT_MODULE_PATH, &[])?;
    let module_bytes = bcs::to_bytes(&module)?;
    Ok(Object::fresh_module(
        MEOW_OBJECT_MODULE_ADDRESS,
        Digest::ZERO,
        module_bytes,
    ))
}

/// Builds the meow_coin framework module object (published at 0x10).
fn build_meow_coin_module(deps: &[(Address, &Module)]) -> Result<Object> {
    let module = builder::build_from_file(MEOW_COIN_MODULE_PATH, deps)?;
    let module_bytes = bcs::to_bytes(&module)?;
    Ok(Object::fresh_module(
        MEOW_COIN_MODULE_ADDRESS,
        Digest::ZERO,
        module_bytes,
    ))
}

/// Mints Meow Coins according to the provided minting instructions.
fn mint_meow_coins(
    meow_object_module: Object,
    meow_coin_module: Object,
    allocations: &[(Address, u64)],
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

            let execution_result = executor::execute_genesis_transaction(
                &transaction,
                vec![meow_object_module.clone(), meow_coin_module.clone()],
            )?;

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
