//! Shared test helpers for executor tests.

#![allow(dead_code)]

use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{
        Object, object_conversion, object_owner::ObjectOwner, object_type::ObjectType,
        object_version::ObjectVersion,
    },
    system_framework::meow_coin::{MEOW_COIN_MODULE_ADDRESS, MeowCoin},
    transaction::{
        Transaction, call::Call, execution_result::ExecutionResult, input::Input,
        transaction_type::TransactionType,
    },
};
use meow_vm_adapter::{
    Module, Value, builder,
    executor::{self},
    external_context::{DEFAULT_RAND_SEED, ExternalContext, RandSeed},
};

pub const SENDER: Address = Address::suffixed(0xE1);
pub const GAS_ADDR: Address = Address::suffixed(0xF9);
pub const GAS_BALANCE: u64 = 1_000_000;

pub fn make_gas_coin_object() -> Object {
    make_valid_gas_coin_object(SENDER)
}

pub fn make_valid_gas_coin_object(owner: Address) -> Object {
    make_coin_object(GAS_ADDR, owner, GAS_BALANCE)
}

pub fn make_gas_coin_object_at_version(version: ObjectVersion) -> Object {
    make_gas_coin_object_at_version_and_balance(version, GAS_BALANCE)
}

pub fn make_gas_coin_object_at_version_and_balance(version: ObjectVersion, balance: u64) -> Object {
    make_coin_object_at_version(GAS_ADDR, SENDER, balance, version)
}

pub fn make_coin_object(id: Address, owner: Address, balance: u64) -> Object {
    make_coin_object_at_version(id, owner, balance, ObjectVersion::ONE)
}

pub fn make_coin_object_at_version(
    id: Address,
    owner: Address,
    balance: u64,
    version: ObjectVersion,
) -> Object {
    let coin_value: Value = MeowCoin::new(id, balance).into();
    object_conversion::vm_object_value_to_object(
        &coin_value,
        ObjectOwner::Address(owner),
        Digest::ZERO,
        version,
    )
    .expect("MeowCoin must convert to Object")
}

pub fn make_invalid_gas_coin_object() -> Object {
    Object::new(
        GAS_ADDR,
        ObjectOwner::Address(SENDER),
        Digest::ZERO,
        ObjectVersion::ONE,
        ObjectType::Module,
        vec![],
    )
}

pub fn make_module_object(address: Address, content: Vec<u8>) -> Object {
    Object::fresh_module(address, Digest::ZERO, content)
}

pub fn make_module_object_from_compiled(address: Address, module: &Module) -> Object {
    let bytes = bcs::to_bytes(module).expect("module must serialize");
    make_module_object(address, bytes)
}

pub fn make_module_object_from_src(src: &str) -> Object {
    let bytes = compile_to_bytes(src);
    make_module_object(Address::ZERO, bytes)
}

pub fn make_call_transaction(module_addr: Address, fn_name: &str, args: Vec<Input>) -> Transaction {
    let call = Call::new(
        module_addr,
        Identifier::new(fn_name).expect("fn_name must be a valid identifier"),
        args,
    );
    Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowCall(call),
    )
}

pub fn make_meow_call_transaction(fn_name: &str, arguments: Vec<Input>) -> Transaction {
    make_call_transaction(MEOW_COIN_MODULE_ADDRESS, fn_name, arguments)
}

pub fn make_meow_module_publish_transaction(module: Vec<u8>) -> Transaction {
    Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowModulePublish(module),
    )
}

pub fn execute(
    transaction: &Transaction,
    inputs: Vec<Object>,
) -> executor::Result<ExecutionResult> {
    executor::execute(transaction, inputs, &ExternalContext::default())
}

pub fn execute_with_seed(
    transaction: &Transaction,
    inputs: Vec<Object>,
    seed: RandSeed,
) -> executor::Result<ExecutionResult> {
    executor::execute(transaction, inputs, &ExternalContext::new(seed, 0))
}

pub fn execute_with_timestamp(
    transaction: &Transaction,
    inputs: Vec<Object>,
    timestamp: u64,
) -> executor::Result<ExecutionResult> {
    executor::execute(
        transaction,
        inputs,
        &ExternalContext::new(DEFAULT_RAND_SEED, timestamp),
    )
}

pub fn build_module(src: &str, deps: &[(Address, &Module)]) -> Module {
    builder::build(src, deps).expect("must compile")
}

pub fn compile_to_bytes(src: &str) -> Vec<u8> {
    bcs::to_bytes(&build_module(src, &[])).expect("module must serialize")
}

pub fn find_gas_coin(result: &ExecutionResult) -> &Object {
    result
        .changed_objects()
        .iter()
        .find(|o| o.address() == &GAS_ADDR)
        .expect("gas coin must be in changed_objects")
}
