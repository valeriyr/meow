//! Built-in framework module builders.
//!
//! Compiles and exposes the system modules at predefined addresses
//! as ready-to-use [`Module`] values and on-chain [`Object`] wrappers.
//!
//! Used in production by `meow-genesis` to populate the initial object store, and in tests
//! across the workspace wherever framework modules are needed as transaction inputs.

use meow_types::{
    address::Address,
    digest::Digest,
    object::Object,
    system_framework::{
        meow_coin::MEOW_COIN_MODULE_ADDRESS, meow_object::MEOW_OBJECT_MODULE_ADDRESS,
    },
};
use meow_vm_adapter::builder;
use meow_vm_types::module::Module;

pub const MEOW_OBJECT_MODULE_PATH: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/modules/meow_object.meow");

pub const MEOW_COIN_MODULE_PATH: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/modules/meow_coin.meow");

/// Builds and returns the compiled `meow_object` framework module.
pub fn meow_object_module() -> Module {
    builder::build_from_file(MEOW_OBJECT_MODULE_PATH, &[])
        .unwrap_or_else(|_| panic!("{MEOW_OBJECT_MODULE_PATH} must compile"))
}

/// Builds and returns the compiled `meow_coin` framework module.
pub fn meow_coin_module() -> Module {
    build_meow_coin(&meow_object_module())
}

/// Builds the `meow_object` framework module and wraps it as an on-chain module Object.
pub fn meow_object_module_object() -> Object {
    module_object(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module())
}

/// Builds the `meow_coin` framework module and wraps it as an on-chain module Object.
pub fn meow_coin_module_object() -> Object {
    module_object(MEOW_COIN_MODULE_ADDRESS, &meow_coin_module())
}

/// Returns the `meow_object` framework module paired with its on-chain address.
pub fn meow_object_module_entry() -> (Address, Object) {
    let module = meow_object_module();
    (
        MEOW_OBJECT_MODULE_ADDRESS,
        module_object(MEOW_OBJECT_MODULE_ADDRESS, &module),
    )
}

/// Returns the `meow_coin` framework module paired with its on-chain address.
pub fn meow_coin_module_entry() -> (Address, Object) {
    let module = meow_coin_module();
    (
        MEOW_COIN_MODULE_ADDRESS,
        module_object(MEOW_COIN_MODULE_ADDRESS, &module),
    )
}

/// Returns all framework module objects paired with their addresses, in dependency order.
pub fn framework_module_entries() -> Vec<(Address, Object)> {
    // Build meow_object once and reuse it as a dependency for meow_coin.
    let meow_object = meow_object_module();
    let meow_coin = build_meow_coin(&meow_object);

    vec![
        (
            MEOW_OBJECT_MODULE_ADDRESS,
            module_object(MEOW_OBJECT_MODULE_ADDRESS, &meow_object),
        ),
        (
            MEOW_COIN_MODULE_ADDRESS,
            module_object(MEOW_COIN_MODULE_ADDRESS, &meow_coin),
        ),
    ]
}

/// Returns all framework module objects in dependency order (deps before dependents).
pub fn framework_module_objects() -> Vec<Object> {
    framework_module_entries()
        .into_iter()
        .map(|(_, obj)| obj)
        .collect()
}

/// Builds the `meow_coin` module with the given `meow_object` module as a dependency.
fn build_meow_coin(meow_object: &Module) -> Module {
    builder::build_from_file(
        MEOW_COIN_MODULE_PATH,
        &[(MEOW_OBJECT_MODULE_ADDRESS, meow_object)],
    )
    .unwrap_or_else(|_| panic!("{MEOW_COIN_MODULE_PATH} must compile"))
}

/// Wraps a compiled module as an on-chain module Object at the given address.
fn module_object(address: Address, module: &Module) -> Object {
    Object::fresh_module(
        address,
        // Framework modules are published at genesis with no originating transaction.
        Digest::ZERO,
        bcs::to_bytes(module).expect("module must serialize"),
    )
}
