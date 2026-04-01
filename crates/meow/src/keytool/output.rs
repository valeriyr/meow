use meow_types::{address::Address, keypair::KeyPair};
use serde::Serialize;

/// The key information.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KeyOutput {
    /// The MEOW address associated with the key.
    pub address: String,
    /// The Base64-encoded public key.
    pub public_key: String,
    /// The scheme name of the key.
    pub scheme: String,
}

/// The keytool command outputs.
#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum KeyToolCommandOutput {
    /// The generate command output.
    Generate(KeyOutput),
    /// The list command output.
    List(Vec<KeyOutput>),
    /// The remove command output.
    Remove(Option<KeyOutput>),
}

impl From<&KeyPair> for KeyOutput {
    fn from(keypair: &KeyPair) -> Self {
        let public_key = keypair.public();

        KeyOutput {
            address: Address::from(keypair).to_string(),
            public_key: public_key.encode_base64(),
            scheme: public_key.scheme().to_string(),
        }
    }
}
