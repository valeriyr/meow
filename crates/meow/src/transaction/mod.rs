//! `meow transaction` commands: build, sign, and submit transactions.

pub mod output;
pub mod signer;

use std::path::PathBuf;

use base64::{Engine, engine::general_purpose};
use clap::Parser;
use meow_node_client::NodeClient;
use meow_types::{
    address::Address,
    config,
    digest::Digest,
    identifier::Identifier,
    keystore::Keystore,
    time,
    transaction::{self, Transaction, call::Call, transaction_type::TransactionType, validator},
};
use meow_vm_adapter::{executor, external_context::ExternalContext, inputs_resolver};

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
        transaction: String,
    },
    /// Simulate a transaction on the node without committing it.
    ///
    /// Accepts an unsigned transaction — no need to sign before simulating.
    /// The node validates object references and executes the transaction against
    /// its current state, returning the execution result.
    ///
    /// Note: if the contract uses meow_vm_rand() or meow_vm_timestamp(), the
    /// result may differ from the actual committed transaction because the block
    /// hash and timestamp are unknown until the block is mined.
    Simulate {
        /// Base64-encoded unsigned transaction (produced by `meow transaction publish` or `meow transaction meow-call`).
        transaction: String,
    },
    /// Execute an unsigned transaction locally by fetching objects from the node
    /// and running the VM in this process. The transaction is not submitted.
    ///
    /// Note: if the contract uses meow_vm_rand() or meow_vm_timestamp(), the
    /// result may differ from the actual committed transaction because the real
    /// block hash and miner timestamp are unknown until the block is mined.
    ExecuteLocally {
        /// Block hash used as the randomness seed (base58).
        /// Defaults to the zero digest. Affects `meow_vm_rand()` results.
        #[arg(long, default_value_t = Digest::ZERO)]
        seed: Digest,
        /// Execution timestamp in milliseconds since Unix epoch.
        /// Defaults to the current system time.
        #[arg(long)]
        timestamp: Option<u64>,
        /// Base64-encoded unsigned transaction (produced by `meow transaction publish` or `meow transaction meow-call`).
        transaction: String,
    },
}

impl TransactionCommand {
    pub async fn run(
        self,
        client: &NodeClient,
        encoder: OutputEncoder,
        with_object_content: bool,
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

                let (module, _) = builder::build_module(client, path).await?;
                let module_bytes = bcs::to_bytes(&module)?;

                let transaction = Transaction::new(
                    sender,
                    gas_coin_ref,
                    TransactionType::MeowModulePublish(module_bytes),
                );

                transaction::validator::validate_transaction(&transaction)?;

                Ok(TransactionCommandOutput::encoded(transaction, encoder)?)
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

                Ok(TransactionCommandOutput::encoded(transaction, encoder)?)
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

                Ok(TransactionCommandOutput::encoded(
                    signed_transaction,
                    encoder,
                )?)
            }
            TransactionCommand::Simulate { transaction } => {
                let bytes = general_purpose::STANDARD.decode(&transaction)?;
                let transaction: Transaction = bcs::from_bytes(&bytes)?;

                validator::validate_transaction(&transaction)?;

                let result = client.simulate_transaction(&transaction).await?;

                Ok(TransactionCommandOutput::simulate(
                    result,
                    with_object_content,
                ))
            }
            TransactionCommand::ExecuteLocally {
                transaction,
                seed,
                timestamp,
            } => {
                let bytes = general_purpose::STANDARD.decode(&transaction)?;
                let transaction: Transaction = bcs::from_bytes(&bytes)?;

                validator::validate_transaction(&transaction)?;

                let timestamp = timestamp.unwrap_or_else(time::current_timestamp);
                let execution_context = ExternalContext::new(seed.into(), timestamp);

                let inputs =
                    inputs_resolver::collect_inputs_async(&transaction, |addr| async move {
                        client.get_object(&addr).await.ok().flatten()
                    })
                    .await;

                let result = executor::execute(&transaction, inputs, &execution_context)?;

                Ok(TransactionCommandOutput::execute_locally(
                    result,
                    with_object_content,
                ))
            }
        }
    }
}
