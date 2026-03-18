use meow_vm_types::types::Value;
use serde::{Deserialize, Serialize};

use crate::{
    address::Address,
    object::{Object, object_decl_ref::ObjectDeclRef, object_type::ObjectType},
    system_framework::utils,
};

/// The meow coin module address is a reserved address where the meow coin module is deployed.
pub const MEOW_COIN_MODULE_ADDRESS: Address = utils::builtin_address(0x1);
/// The meow coin module name.
pub const MEOW_COIN_MODULE_NAME: &str = "meow_coin";
/// The meow coin object name.
pub const MEOW_COIN_OBJECT_NAME: &str = "MeowCoin";

/// The MeowCoin struct represents a coin in the Meow system, with an id and a balance.
#[derive(Serialize, Deserialize)]
pub struct MeowCoin {
    /// The unique on-chain object identifier for this coin.
    id: Address,
    /// The coin balance denominated in the smallest indivisible unit.
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
pub fn is_meow_coin_object(object: &Object) -> bool {
    match object.type_() {
        ObjectType::Object(object_decl_ref) => is_meow_coin_object_decl_ref(object_decl_ref),
        _ => false,
    }
}

/// Returns true if the object declaration reference is a MeowCoin.
pub fn is_meow_coin_object_decl_ref(object_decl_ref: &ObjectDeclRef) -> bool {
    is_meow_coin(object_decl_ref.module(), object_decl_ref.name().as_ref())
}

/// Returns true if the module name and type name define a MeowCoin.
pub fn is_meow_coin(module: &Address, name: &str) -> bool {
    module == &MEOW_COIN_MODULE_ADDRESS && name == MEOW_COIN_OBJECT_NAME
}

/// Read the balance field of a MeowCoin object.
pub fn gas_meow_coin_balance(gas_coin: &Object) -> Option<u64> {
    let fields: Vec<(String, Value)> = bcs::from_bytes(gas_coin.content()).ok()?;
    fields
        .iter()
        .find(|(name, _)| name == "balance")
        .and_then(|(_, val)| val.as_u64())
}

/// Deduct `spent` from the balance field of a gas coin in a single BCS round-trip.
///
/// Returns the updated serialized content with the balance floored at 0 on underflow,
/// or `None` if the content cannot be decoded or re-encoded.
pub fn deduct_gas_coin_balance(gas_coin: &Object, spent: u64) -> Option<Vec<u8>> {
    if !is_meow_coin_object(gas_coin) {
        return None;
    }

    let mut fields: Vec<(String, Value)> = bcs::from_bytes(gas_coin.content()).ok()?;
    for (name, val) in &mut fields {
        if name == "balance" {
            let current = val.as_u64()?;
            *val = Value::U64(current.saturating_sub(spent));
            break;
        }
    }
    bcs::to_bytes(&fields).ok()
}
