use std::str::FromStr;

use crate::address::Address;

/// Parse a bytecode cross-module reference `@0xHEX::name`.
/// Returns `(Address, local_name)` or `None` for plain local names.
pub fn parse_module_ref(s: &str) -> Option<(Address, &str)> {
    let rest = s.strip_prefix('@')?;
    let (hex_part, name) = rest.split_once("::")?;
    let address = Address::from_str(hex_part).ok()?;
    Some((address, name))
}
