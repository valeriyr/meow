//! Adapter layer between `meow-vm` and `meow-types`.
//!
//! Bridges the stack-based VM and blockchain transaction types, implementing
//! transaction execution via `execute()` and module building via `build()`.

mod context;
mod natives;

pub mod builder;
pub mod executor;
pub mod runner;

// Re-export commonly used types and functions for external use.
//
// TODO: These types should be replaced with abstractions to abstract the crates users from VM types.
pub use meow_vm_types::{module::Module, types::Value};
