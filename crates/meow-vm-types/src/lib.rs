//! # meow-vm-types
//!
//! Shared type definitions, configurations, and value conversion utilities for
//! the meow virtual machine.
//!
//! This crate is intentionally kept free of VM execution logic so it can be
//! depended on by both the VM itself and higher-level adapter layers.

pub mod address;
pub mod bytecode;
pub mod config;
pub mod convert;
pub mod identifier;
pub mod module;
pub mod types;
