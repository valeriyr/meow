//! Utilities for cross-module bytecode references of the form `@0xHEX::name`.

use std::str::FromStr;

use crate::address::Address;

/// Parse a bytecode cross-module reference `@0xHEX::name`.
/// Returns `(Address, name)` on success, or `None` if `s` is not in that format.
pub fn parse_module_ref(s: &str) -> Option<(Address, &str)> {
    let rest = s.strip_prefix('@')?;
    let (hex_part, name) = rest.split_once("::")?;
    let address = Address::from_str(hex_part).ok()?;
    Some((address, name))
}

/// Format an address-qualified type name: `@<address>::<name>`.
pub fn qualify(address: &Address, name: &str) -> String {
    format!("@{address}::{name}")
}

/// Returns `true` if `s` is a name-qualified cross-module type reference — i.e. it contains `::`.
pub fn is_qualified(s: &str) -> bool {
    s.contains("::")
}
