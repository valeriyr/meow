use crate::address::{ADDRESS_LENGTH, Address};

/// An utility function to create a builtin address with the given suffix.
pub const fn builtin_address(suffix: u16) -> Address {
    let mut addr = [0u8; ADDRESS_LENGTH];
    let [hi, lo] = suffix.to_be_bytes();
    addr[ADDRESS_LENGTH - 2] = hi;
    addr[ADDRESS_LENGTH - 1] = lo;
    Address::new(addr)
}
