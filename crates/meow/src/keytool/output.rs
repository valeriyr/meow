use meow_types::{address::Address, keypair::KeyPair};
use serde::Serialize;

/// The key information.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    /// The MEOW address associated with the key.
    pub address: String,
    /// The Base64-encoded public key.
    pub public_key: String,
    /// The scheme name of the key.
    pub scheme: String,
}

/// The keytool command outputs.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum KeyToolCommandOutput {
    /// The generate command output.
    Generate(Key),
    /// The list command output.
    List(Vec<Key>),
    /// The remove command output.
    Remove(Option<Key>),
}

impl From<&KeyPair> for Key {
    fn from(keypair: &KeyPair) -> Self {
        let public_key = keypair.public();

        Key {
            address: Address::from(keypair).to_string(),
            public_key: public_key.encode_base64(),
            scheme: public_key.scheme().to_string(),
        }
    }
}
