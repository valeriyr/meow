pub mod output;

use base64::{Engine, engine::general_purpose};
use clap::Parser;
use meow_node_client::NodeClient;
use meow_types::{
    address::Address,
    digest::Digest,
    transaction::{self, SignedTransaction},
};

use crate::client::output::ClientCommandOutput;

/// Commands for interacting with a running meow node.
#[derive(Parser)]
#[command(rename_all = "kebab-case", verbatim_doc_comment)]
pub enum ClientCommand {
    /// Fetch a live object from the node by address.
    GetObject {
        /// On-chain object address (hex, e.g. `0xabcd...`).
        address: Address,
    },
    /// Fetch all the live objects from the node by owner address.
    GetObjects {
        /// Owner address (hex, e.g. `0xabcd...`).
        owner: Address,
    },
    /// Fetch a committed transaction from the node by digest.
    GetTransaction {
        /// Transaction digest (base58).
        digest: Digest,
    },
    /// Fetch the execution result for a committed transaction.
    GetTransactionResult {
        /// Transaction digest (base58).
        digest: Digest,
    },
    /// Submit a signed transaction to the node.
    SubmitTransaction {
        /// Base64-encoded signed transaction (produced by `meow transaction sign`).
        transaction: String,
    },
}

impl ClientCommand {
    pub async fn run(self, client: &NodeClient) -> anyhow::Result<ClientCommandOutput> {
        match self {
            ClientCommand::GetObject { address } => {
                let object = client.get_object(&address).await?;

                Ok(ClientCommandOutput::get_object(object))
            }
            ClientCommand::GetObjects { owner } => {
                let objects = client.get_objects(&owner).await?;

                Ok(ClientCommandOutput::get_objects(objects))
            }
            ClientCommand::GetTransaction { digest } => {
                let transaction = client.get_transaction(&digest).await?;

                Ok(ClientCommandOutput::get_transaction(transaction))
            }
            ClientCommand::GetTransactionResult { digest } => {
                let result = client.get_transaction_result(&digest).await?;

                Ok(ClientCommandOutput::get_transaction_result(result))
            }
            ClientCommand::SubmitTransaction { transaction } => {
                let bytes = general_purpose::STANDARD.decode(&transaction)?;
                let signed_transaction: SignedTransaction = bcs::from_bytes(&bytes)?;

                transaction::validator::validate_signed_transaction(&signed_transaction)?;

                let digest = signed_transaction.transaction().digest();

                client.submit_transaction(&signed_transaction).await?;

                Ok(ClientCommandOutput::submit_transaction(digest))
            }
        }
    }
}
