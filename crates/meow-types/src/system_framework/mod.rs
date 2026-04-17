mod utils;

pub mod meow_coin;

use std::collections::BTreeMap;

use crate::{address::Address, object::Object};

/// The system address is a reserved address used for system-level operations and is not owned by any user.
pub const MEOW_SYSTEM_ADDRESS: Address = Address::ZERO;

/// Decode a known system object into a human-readable key-value map for display.
///
/// Returns `None` for unknown or user-defined objects — only system-framework
/// objects with a known schema (e.g. MeowCoin) are decoded. Keys are ordered
/// alphabetically via `BTreeMap` for consistent output.
pub fn extract_human_readable_content(object: &Object) -> Option<BTreeMap<String, String>> {
    if meow_coin::is_meow_coin_object(object) {
        let balance =
            meow_coin::gas_meow_coin_balance(object).expect("expect to be a MeowCoin instance");
        let mut map = BTreeMap::new();
        map.insert("balance".to_string(), balance.to_string());
        Some(map)
    } else {
        None
    }
}
