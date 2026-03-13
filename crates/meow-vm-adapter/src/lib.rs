//! Adapter layer between `meow-vm` and `meow-types`.
//!
//! Bridges the stack-based VM and blockchain transaction types, implementing
//! transaction execution via `execute()` and module building via `build()`.

mod context;
pub mod convert;
mod natives;

pub mod builder;
pub mod executor;
pub mod runner;

pub use meow_vm_types::{module::Module, types::Value};
