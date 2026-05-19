//! Adapter layer that connects the stack-based VM to the blockchain transaction model.
//!
//! Provides transaction execution and module building on top of the raw VM,
//! wiring together gas accounting, native functions, and object lifecycle management.

mod bytecode_verifier;
mod context;
mod natives;

pub mod builder;
pub mod executor;
pub mod external_context;
pub mod inputs_resolver;
pub mod runner;

// TODO: These types should be replaced with abstractions that hide the internal VM crate boundary.
pub use meow_vm_types::{module::Module, types::Value};
