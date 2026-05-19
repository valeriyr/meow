//! Addresses and type descriptors for the system modules pre-deployed at genesis.
//!
//! The system modules are deployed at fixed, well-known addresses so the executor
//! and VM can refer to them without an on-chain lookup. This module is the single source of
//! truth for those addresses and the type names derived from them.

mod utils;

pub mod meow_coin;
pub mod meow_object;

use crate::address::Address;

/// The system address is a reserved address used for system-level operations and is not owned by any user.
pub const MEOW_SYSTEM_ADDRESS: Address = Address::ZERO;
