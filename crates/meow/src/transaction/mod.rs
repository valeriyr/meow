pub mod output;
pub mod signer;

use std::path::PathBuf;

use base64::{Engine, engine::general_purpose};
use clap::Parser;
use meow_node_client::NodeClient;
use meow_types::{
    address::Address,
    config,
    identifier::Identifier,
    keystore::Keystore,
    transaction::{self, Transaction, call::Call, transaction_type::TransactionType},
};

use crate::{
    builder, call_arg::CallArg, output_encoder::OutputEncoder,
    transaction::output::TransactionCommandOutput,
};

/// Commands for MEOW transactions creation and signing.
#[derive(Parser)]
#[command(rename_all = "kebab-case", verbatim_doc_comment)]
pub enum TransactionCommand {
    /// Compile a `.meow` source file and create a module-publish transaction.
    Publish {
        /// Path to the `.meow` source file.
        path: PathBuf,
        /// Sender address.
        #[arg(long)]
        sender: Address,
        /// Address of the gas coin object to pay fees with.
        #[arg(long)]
        gas_coin: Address,
    },
    /// Create a transaction to call a function on a published module.
    MeowCall {
        /// Address of the module object on-chain.
        #[arg(long)]
        module: Address,
        /// Name of the function to call.
        #[arg(long)]
        function: Identifier,
        /// Sender address.
        #[arg(long)]
        sender: Address,
        /// Address of the gas coin object to pay fees with.
        #[arg(long)]
        gas_coin: Address,
        /// Call argument (repeatable). Auto-detected by format:
        /// - `true` / `false` → bool
        /// - digits only → u64
        /// - `@0x<hex>` → raw address value (not resolved against the node)
        /// - `0x<hex>` → on-chain object (resolved against the node)
        /// - anything else → string
        #[arg(value_name = "VALUE", verbatim_doc_comment)]
        args: Vec<CallArg>,
    },
    /// Sign a transaction produced by `transaction publish` or `transaction meow-call`.
    Sign {
        /// The path to the keystore file.
        #[arg(long)]
        keystore_path: Option<PathBuf>,
        /// Base64-encoded transaction to sign.
        #[arg(long)]
        transaction: String,
    },
}

impl TransactionCommand {
    pub async fn run(
        self,
        client: &NodeClient,
        encoder: OutputEncoder,
    ) -> anyhow::Result<TransactionCommandOutput> {
        match self {
            TransactionCommand::Publish {
                path,
                sender,
                gas_coin,
            } => {
                let gas_coin_ref = client
                    .get_object(&gas_coin)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("gas coin {gas_coin} not found"))?
                    .object_ref();

                let module = builder::build_module(client, path).await?;
                let module_bytes = bcs::to_bytes(&module)?;

                let transaction = Transaction::new(
                    sender,
                    gas_coin_ref,
                    TransactionType::MeowModulePublish(module_bytes),
                );

                transaction::validator::validate_transaction(&transaction)?;

                Ok(TransactionCommandOutput::new(transaction, encoder)?)
            }
            TransactionCommand::MeowCall {
                module,
                function,
                sender,
                gas_coin,
                args,
            } => {
                let gas_coin_ref = client
                    .get_object(&gas_coin)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("gas coin {gas_coin} not found"))?
                    .object_ref();

                let mut inputs = Vec::new();
                for arg in args {
                    inputs.push(arg.into_input(client).await?);
                }

                let call = Call::new(module, function, inputs);
                let transaction =
                    Transaction::new(sender, gas_coin_ref, TransactionType::MeowCall(call));

                transaction::validator::validate_transaction(&transaction)?;

                Ok(TransactionCommandOutput::new(transaction, encoder)?)
            }
            TransactionCommand::Sign {
                keystore_path,
                transaction,
            } => {
                let keystore_path = keystore_path.unwrap_or(config::meow_keystore_path()?);
                let keystore = Keystore::file_based(&keystore_path)?;

                let bytes = general_purpose::STANDARD.decode(&transaction)?;
                let transaction = bcs::from_bytes(&bytes)?;

                transaction::validator::validate_transaction(&transaction)?;

                let signed_transaction = signer::sign_transaction(transaction, &keystore)?;

                Ok(TransactionCommandOutput::new(signed_transaction, encoder)?)
            }
        }
    }
}
