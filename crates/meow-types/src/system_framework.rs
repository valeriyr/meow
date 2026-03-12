use serde::{Deserialize, Serialize};

use crate::address::{ADDRESS_LENGTH, Address};

/// The system address is a reserved address used for system-level operations and is not owned by any user.
pub const MEOW_SYSTEM_ADDRESS_ADDRESS: Address = Address::ZERO;

/// The meow coin module address is a reserved address where the meow coin module is deployed.
pub const MEOW_COIN_MODULE_ADDRESS: Address = builtin_address(0x1);
/// The meow coin module name.
pub const MEOW_COIN_MODULE_NAME: &str = "meow_coin";
/// The meow coin object name.
pub const MEOW_COIN_OBJECT_NAME: &str = "MeowCoin";

/// Returns true if the object is a gas coin.
pub fn is_gas_coin(module: &Address, name: &str) -> bool {
    module == &MEOW_COIN_MODULE_ADDRESS && name == MEOW_COIN_OBJECT_NAME
}

/// An utility function to create a builtin address with the given suffix.
const fn builtin_address(suffix: u16) -> Address {
    let mut addr = [0u8; ADDRESS_LENGTH];
    let [hi, lo] = suffix.to_be_bytes();
    addr[ADDRESS_LENGTH - 2] = hi;
    addr[ADDRESS_LENGTH - 1] = lo;
    Address::new(addr)
}

/// The MeowCoin struct represents a coin in the Meow system, with an id and a balance.
#[derive(Serialize, Deserialize)]
pub struct MeowCoin {
    id: Address,
    balance: u64,
}

impl MeowCoin {
    /// Returns the id of the MeowCoin.
    pub fn id(&self) -> &Address {
        &self.id
    }

    /// Returns the balance of the MeowCoin.
    pub fn balance(&self) -> u64 {
        self.balance
    }
}
