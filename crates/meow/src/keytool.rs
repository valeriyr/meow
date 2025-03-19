use bip32::DerivationPath;
use clap::{command, Parser};
use meow_types::{
    keypair::{mnemonic::MnemonicType, signature_scheme::SignatureScheme, KeyPair},
    keystore::Keystore,
};

/// The keytool commands.
#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum KeyToolCommand {
    /// Generate a new key pair.
    Generate {
        /// The scheme to use for a generated key pair.
        scheme: SignatureScheme,
        /// The derivation path to use for a generated key pair.
        derivation_path: Option<DerivationPath>,
        /// The word length to use for a generated key pair.
        word_length: Option<MnemonicType>,
    },
}

impl KeyToolCommand {
    /// Runs the command.
    pub fn run(self, keystore: &mut Keystore) -> Result<(), anyhow::Error> {
        match self {
            KeyToolCommand::Generate {
                scheme,
                derivation_path,
                word_length,
            } => {
                let keypair = KeyPair::generate(scheme, derivation_path, word_length)?;
                keystore.add_key(keypair)?;
                Ok(())
            }
        }
    }
}
