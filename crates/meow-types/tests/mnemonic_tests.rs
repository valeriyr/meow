use std::str::FromStr;

use meow_types::keypair::mnemonic::MnemonicType;

//
// ─── Mnemonic type tests ───
//

#[test]
fn mnemonic_type_serialization() {
    assert_eq!(MnemonicType::Words12.to_string(), "words12");
    assert_eq!(MnemonicType::Words15.to_string(), "words15");
    assert_eq!(MnemonicType::Words18.to_string(), "words18");
    assert_eq!(MnemonicType::Words21.to_string(), "words21");
    assert_eq!(MnemonicType::Words24.to_string(), "words24");
}

#[test]
fn mnemonic_type_deserialization() {
    assert_eq!(MnemonicType::from_str("words12"), Ok(MnemonicType::Words12));
    assert_eq!(MnemonicType::from_str("words15"), Ok(MnemonicType::Words15));
    assert_eq!(MnemonicType::from_str("words18"), Ok(MnemonicType::Words18));
    assert_eq!(MnemonicType::from_str("words21"), Ok(MnemonicType::Words21));
    assert_eq!(MnemonicType::from_str("words24"), Ok(MnemonicType::Words24));
}
