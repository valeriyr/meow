use meow_vm_types::{
    convert::{VmTypeNames, struct_from_rust},
    types::Value,
};
use serde::{Deserialize, Serialize};

use crate::{
    address::Address,
    object::{Object, object_decl_ref::ObjectDeclRef, object_type::ObjectType},
    system_framework::{
        meow_object::{MEOW_OBJECT_ID_BYTECODE_TYPE_NAME, MeowObjectId},
        utils,
    },
};

/// The `meow_coin` module address is a reserved address where the `meow_coin` module is deployed.
pub const MEOW_COIN_MODULE_ADDRESS: Address = utils::builtin_address(0x10);
/// The `meow_coin` module name.
pub const MEOW_COIN_MODULE_NAME: &str = "meow_coin";
/// The `MeowCoin` object name.
pub const MEOW_COIN_OBJECT_NAME: &str = "MeowCoin";
/// The `MeowCoinBalance` struct name.
pub const MEOW_COIN_BALANCE_STRUCT_NAME: &str = "MeowCoinBalance";
/// The `mint` function name.
pub const MEOW_COIN_MINT_FUNCTION_NAME: &str = "mint";

/// Address-qualified type name for `MeowCoin` as it appears in VM values at runtime.
pub const MEOW_COIN_OBJECT_BYTECODE_TYPE_NAME: &str =
    "@0x0000000000000000000000000000000000000000000000000000000000000010::MeowCoin";
/// Address-qualified type name for `MeowCoinBalance` as it appears in VM values at runtime.
pub const MEOW_COIN_BALANCE_BYTECODE_TYPE_NAME: &str =
    "@0x0000000000000000000000000000000000000000000000000000000000000010::MeowCoinBalance";

/// The `MeowCoin` struct represents a coin in the Meow system, with an id and a balance.
#[derive(Serialize, Deserialize)]
pub struct MeowCoin {
    /// The unique on-chain object identifier for this coin.
    id: MeowObjectId,
    /// The coin balance denominated in the smallest indivisible unit.
    balance: u64,
}

impl MeowCoin {
    /// Creates a new `MeowCoin` with the given id and balance.
    pub fn new(id: Address, balance: u64) -> Self {
        Self {
            id: MeowObjectId::new(id),
            balance,
        }
    }

    /// Returns the id of the `MeowCoin`.
    pub fn id(&self) -> &MeowObjectId {
        &self.id
    }

    /// Returns the balance of the `MeowCoin`.
    pub fn balance(&self) -> u64 {
        self.balance
    }
}

impl From<MeowCoin> for Value {
    fn from(coin: MeowCoin) -> Self {
        struct_from_rust(&coin).expect("MeowCoin must convert to Value")
    }
}

impl VmTypeNames for MeowCoin {
    fn type_names() -> &'static [(&'static str, &'static str)] {
        &[
            (stringify!(MeowCoin), MEOW_COIN_OBJECT_BYTECODE_TYPE_NAME),
            (stringify!(MeowObjectId), MEOW_OBJECT_ID_BYTECODE_TYPE_NAME),
        ]
    }
}

/// An unwrapped balance amount used as an intermediate value in coin operations.
#[derive(Serialize, Deserialize)]
pub struct MeowCoinBalance {
    amount: u64,
}

impl MeowCoinBalance {
    /// Creates a new `MeowCoinBalance` with the given amount.
    pub fn new(amount: u64) -> Self {
        Self { amount }
    }

    /// Returns the amount.
    pub fn amount(&self) -> u64 {
        self.amount
    }
}

impl From<MeowCoinBalance> for Value {
    fn from(balance: MeowCoinBalance) -> Self {
        struct_from_rust(&balance).expect("MeowCoinBalance must convert to Value")
    }
}

impl VmTypeNames for MeowCoinBalance {
    fn type_names() -> &'static [(&'static str, &'static str)] {
        &[(
            stringify!(MeowCoinBalance),
            MEOW_COIN_BALANCE_BYTECODE_TYPE_NAME,
        )]
    }
}

/// Returns true if the object is a `MeowCoin`.
pub fn is_meow_coin_object(object: &Object) -> bool {
    match object.type_() {
        ObjectType::Object(object_decl_ref) => is_meow_coin_object_decl_ref(object_decl_ref),
        _ => false,
    }
}

/// Returns true if the object declaration reference is a `MeowCoin`.
pub fn is_meow_coin_object_decl_ref(object_decl_ref: &ObjectDeclRef) -> bool {
    is_meow_coin(object_decl_ref.module(), object_decl_ref.name().as_ref())
}

/// Returns true if the module name and type name define a `MeowCoin`.
pub fn is_meow_coin(module: &Address, name: &str) -> bool {
    module == &MEOW_COIN_MODULE_ADDRESS && name == MEOW_COIN_OBJECT_NAME
}

/// Returns true if the module name and type name define a `MeowCoinBalance`.
pub fn is_meow_coin_balance(module: &Address, name: &str) -> bool {
    module == &MEOW_COIN_MODULE_ADDRESS && name == MEOW_COIN_BALANCE_STRUCT_NAME
}

pub mod meow_coin_object {
    use meow_vm_types::types::Value;

    use crate::object::Object;

    const BALANCE_FIELD_NAME: &str = "balance";

    /// Read the balance field of a `MeowCoin` object.
    pub fn balance_from_object(coin: &Object) -> Option<u64> {
        if !super::is_meow_coin_object(coin) {
            return None;
        }

        let fields: Vec<(String, Value)> = bcs::from_bytes(coin.content()).ok()?;
        fields
            .iter()
            .find(|(name, _)| name == BALANCE_FIELD_NAME)
            .and_then(|(_, val)| val.as_u64())
    }

    /// Read the balance field of a `MeowCoin` struct value.
    pub fn balance_from_value(coin: &Value) -> Option<u64> {
        if coin.type_name() == super::MEOW_COIN_OBJECT_BYTECODE_TYPE_NAME {
            coin.field_u64(BALANCE_FIELD_NAME)
        } else {
            None
        }
    }

    /// Deduct `spent` from the balance field of a gas coin in a single BCS round-trip.
    ///
    /// Returns the updated serialized content with the balance floored at 0 on underflow,
    /// or `None` if the content cannot be decoded or re-encoded.
    pub fn deduct_gas(gas_coin: &Object, spent: u64) -> Option<Vec<u8>> {
        if !super::is_meow_coin_object(gas_coin) {
            return None;
        }

        let mut fields: Vec<(String, Value)> = bcs::from_bytes(gas_coin.content()).ok()?;
        for (name, val) in &mut fields {
            if name == BALANCE_FIELD_NAME {
                let current = val.as_u64()?;
                *val = Value::U64(current.saturating_sub(spent));
                break;
            }
        }
        bcs::to_bytes(&fields).ok()
    }
}

pub mod meow_coin_balance_struct {
    use meow_vm_types::types::Value;

    const AMOUNT_FIELD_NAME: &str = "amount";

    /// Extracts the balance amount from a [`Value`] representing a [`MeowCoinBalance`].
    pub fn amount(v: &Value) -> Option<u64> {
        if v.type_name() == super::MEOW_COIN_BALANCE_BYTECODE_TYPE_NAME {
            v.field_u64(AMOUNT_FIELD_NAME)
        } else {
            None
        }
    }
}
