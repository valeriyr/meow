//! `meow keytool` commands: generate, derive, and inspect key pairs and addresses.

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
#[command(rename_all = "kebab-case", verbatim_doc_comment)]
pub enum KeyToolCommand {
    /// Generate a new key.
    Generate {
        /// Key scheme to use (e.g. `ed25519`).
        scheme: SignatureScheme,
        /// BIP-32 derivation path (e.g. `m/44'/784'/0'/0'/0'`).
        #[arg(long)]
        derivation_path: Option<DerivationPath>,
        /// Mnemonic word count (12, 15, 18, 21, or 24).
        #[arg(long)]
        word_length: Option<MnemonicType>,
    },
    /// List all keys in the keystore (address, public key, and scheme).
    List,
    /// Remove a key from the keystore by address.
    Remove {
        /// Address of the key to remove.
        address: Address,
    },
}

impl KeyToolCommand {
    /// Runs the command.
    pub fn run(self, keystore: &mut Keystore) -> Result<KeyToolCommandOutput, anyhow::Error> {
        Ok(match self {
            KeyToolCommand::Generate {
                scheme,
                derivation_path,
                word_length,
            } => {
                let (keypair, phrase) = KeyPair::generate(scheme, derivation_path, word_length)?;

                let key = KeyOutput::from(&keypair);

                keystore.add_key(keypair)?;

                KeyToolCommandOutput::Generate { key, phrase }
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
