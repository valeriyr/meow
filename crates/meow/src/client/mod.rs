pub mod output;

use base64::{Engine, engine::general_purpose};
use clap::Parser;
use meow_node_client::NodeClient;
use meow_types::{address::Address, digest::Digest, transaction::SignedTransaction};

use crate::client::output::ClientCommandOutput;

/// Commands for interacting with a running meow node.
#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum ClientCommand {
    /// Fetch a live object by address.
    GetObject {
        /// On-chain object address (hex, e.g. `0xabcd...`).
        address: Address,
    },
    /// Fetch the execution result for a committed transaction.
    GetTransactionResult {
        /// Transaction digest (base58).
        digest: Digest,
    },
    /// Call a function on a published module.
    SubmitTransaction {
        /// SignedTransaction to submit, as a base64 string.
        transaction: String,
    },
}

impl ClientCommand {
    pub fn run(self, client: &NodeClient) -> anyhow::Result<ClientCommandOutput> {
        match self {
            ClientCommand::GetObject { address } => {
                let object = client.get_object(&address)?;

                Ok(ClientCommandOutput::get_object(object))
            }
            ClientCommand::GetTransactionResult { digest } => {
                let result = client.get_transaction_result(&digest)?;

                Ok(ClientCommandOutput::get_transaction_result(result))
            }
            ClientCommand::SubmitTransaction { transaction } => {
                let bytes = general_purpose::STANDARD.decode(&transaction)?;
                let signed_transaction: SignedTransaction = bcs::from_bytes(&bytes)?;

                signed_transaction.verify()?;

                let digest = signed_transaction.transaction().digest();

                client.submit_transaction(&signed_transaction)?;

                Ok(ClientCommandOutput::submit_transaction(digest))
            }
        }
    }
}
