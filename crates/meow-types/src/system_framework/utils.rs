use crate::address::{ADDRESS_LENGTH, Address};

/// An utility function to create a builtin address with the given suffix.
pub const fn builtin_address(suffix: u16) -> Address {
    let mut addr = [0u8; ADDRESS_LENGTH];
    let [hi, lo] = suffix.to_be_bytes();
    addr[ADDRESS_LENGTH - 2] = hi;
    addr[ADDRESS_LENGTH - 1] = lo;
    Address::new(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    //
    // ─── builtin_address tests ───
    //

    #[test]
    fn zero_suffix_produces_all_zero_address() {
        let addr = builtin_address(0x0000);
        let bytes: &[u8] = addr.as_ref();
        assert!(bytes.iter().all(|&b| b == 0));
    }

    #[test]
    fn single_byte_suffix_goes_into_last_byte_only() {
        let addr = builtin_address(0x0001);
        let bytes: &[u8] = addr.as_ref();
        assert_eq!(bytes[ADDRESS_LENGTH - 1], 0x01);
        assert_eq!(bytes[ADDRESS_LENGTH - 2], 0x00);
        assert!(bytes[..ADDRESS_LENGTH - 2].iter().all(|&b| b == 0));
    }

    #[test]
    fn two_byte_suffix_splits_across_last_two_bytes() {
        let addr = builtin_address(0x1234);
        let bytes: &[u8] = addr.as_ref();
        assert_eq!(bytes[ADDRESS_LENGTH - 2], 0x12);
        assert_eq!(bytes[ADDRESS_LENGTH - 1], 0x34);
        assert!(bytes[..ADDRESS_LENGTH - 2].iter().all(|&b| b == 0));
    }

    #[test]
    fn max_suffix_fills_last_two_bytes() {
        let addr = builtin_address(0xFFFF);
        let bytes: &[u8] = addr.as_ref();
        assert_eq!(bytes[ADDRESS_LENGTH - 2], 0xFF);
        assert_eq!(bytes[ADDRESS_LENGTH - 1], 0xFF);
        assert!(bytes[..ADDRESS_LENGTH - 2].iter().all(|&b| b == 0));
    }

    #[test]
    fn distinct_suffixes_produce_distinct_addresses() {
        assert_ne!(builtin_address(0x01), builtin_address(0x10));
        assert_ne!(builtin_address(0x00), builtin_address(0x01));
    }
}
