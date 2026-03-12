use serde::{Deserialize, Serialize};

use crate::{address::Address, system_framework::utils};

/// The meow coin module address is a reserved address where the meow coin module is deployed.
pub const MEOW_COIN_MODULE_ADDRESS: Address = utils::builtin_address(0x1);
/// The meow coin module name.
pub const MEOW_COIN_MODULE_NAME: &str = "meow_coin";
/// The meow coin object name.
pub const MEOW_COIN_OBJECT_NAME: &str = "MeowCoin";

/// The MeowCoin struct represents a coin in the Meow system, with an id and a balance.
#[derive(Serialize, Deserialize)]
pub struct MeowCoin {
    id: Address,
    balance: u64,
}

impl MeowCoin {
    /// Creates a new MeowCoin with the given id and balance.
    pub fn new(id: Address, balance: u64) -> Self {
        Self { id, balance }
    }

    /// Returns the id of the MeowCoin.
    pub fn id(&self) -> &Address {
        &self.id
    }

    /// Returns the balance of the MeowCoin.
    pub fn balance(&self) -> u64 {
        self.balance
    }
}

/// Returns true if the object is a MeowCoin.
pub fn is_meow_coin(module: &Address, name: &str) -> bool {
    module == &MEOW_COIN_MODULE_ADDRESS && name == MEOW_COIN_OBJECT_NAME
}
