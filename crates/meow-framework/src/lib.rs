use meow_types::address::{ADDRESS_LENGTH, Address};

/// The system address is a reserved address used for system-level operations and is not owned by any user.
pub const MEOW_SYSTEM_ADDRESS_ADDRESS: Address = Address::ZERO;

/// The meow coin module address is a reserved address where the meow coin module is deployed.
pub const MEOW_COIN_MODULE_ADDRESS: Address = builtin_address(0x1);
/// The meow coin module name.
pub const MEOW_COIN_MODULE_NAME: &str = "meow_coin";

/// An utility function to create a builtin address with the given suffix.
const fn builtin_address(suffix: u16) -> Address {
    let mut addr = [0u8; ADDRESS_LENGTH];
    let [hi, lo] = suffix.to_be_bytes();
    addr[ADDRESS_LENGTH - 2] = hi;
    addr[ADDRESS_LENGTH - 1] = lo;
    Address::new(addr)
}
