pub mod output;

use bip32::DerivationPath;
use clap::Parser;
use meow_types::{
    address::Address,
    keypair::{KeyPair, mnemonic::MnemonicType, signature_scheme::SignatureScheme},
    keystore::Keystore,
};
use output::{KeyOutput, KeyToolCommandOutput};

/// The keytool commands.
#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum KeyToolCommand {
    /// Generate a new key.
    Generate {
        /// The scheme to use for a generated key.
        scheme: SignatureScheme,
        /// The derivation path to use for a generated key.
        derivation_path: Option<DerivationPath>,
        /// The word length to use for a generated key.
        word_length: Option<MnemonicType>,
    },
    /// List all the keys stored in the keystore.
    /// Returns the associated MEOW address, Base64-encoded public key, and key scheme name for each key.
    List,
    /// Remove a key from the keystore.
    Remove {
        /// The address of the key to remove.
        address: Address,
    },
}

/// Runs the command.
impl KeyToolCommand {
    /// Runs the command.
    pub fn run(self, keystore: &mut Keystore) -> Result<KeyToolCommandOutput, anyhow::Error> {
        Ok(match self {
            KeyToolCommand::Generate {
                scheme,
                derivation_path,
                word_length,
            } => {
                let keypair = KeyPair::generate(scheme, derivation_path, word_length)?;

                let key = KeyOutput::from(&keypair);

                keystore.add_key(keypair)?;

                KeyToolCommandOutput::Generate(key)
            }
            KeyToolCommand::List => {
                let keys = keystore
                    .iter()
                    .map(|(_, keypair)| KeyOutput::from(keypair))
                    .collect();

                KeyToolCommandOutput::List(keys)
            }
            KeyToolCommand::Remove { address } => {
                let key = keystore.remove_key(&address)?.as_ref().map(KeyOutput::from);

                KeyToolCommandOutput::Remove(key)
            }
        })
    }
}
