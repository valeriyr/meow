//! Shared type definitions and utilities for the Meow VM.
//!
//! Kept free of execution logic so both the VM and the adapter layer can depend on this crate.

pub mod address;
pub mod bytecode;
pub mod config;
pub mod convert;
pub mod identifier;
pub mod module;
pub mod module_ref;
pub mod natives;
pub mod types;
