mod utils;

pub mod meow_coin;

use crate::address::Address;

/// The system address is a reserved address used for system-level operations and is not owned by any user.
pub const MEOW_SYSTEM_ADDRESS: Address = Address::ZERO;
