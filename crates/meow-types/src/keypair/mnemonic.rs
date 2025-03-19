use strum_macros::EnumString;

/// Determines the number of words that will be present in a mnemonic phrase.
#[derive(Clone, Copy, Debug, EnumString, strum_macros::Display, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
pub enum MnemonicType {
    Words12,
    Words15,
    Words18,
    Words21,
    Words24,
}

impl From<MnemonicType> for bip39::MnemonicType {
    fn from(words: MnemonicType) -> Self {
        match words {
            MnemonicType::Words12 => bip39::MnemonicType::Words12,
            MnemonicType::Words15 => bip39::MnemonicType::Words15,
            MnemonicType::Words18 => bip39::MnemonicType::Words18,
            MnemonicType::Words21 => bip39::MnemonicType::Words21,
            MnemonicType::Words24 => bip39::MnemonicType::Words24,
        }
    }
}
