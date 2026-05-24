//! Top-level CLI command definitions and dispatch for the meow binary.

use std::path::PathBuf;

use clap::Parser;
use meow_node_client::NodeClient;
use url::Url;

use crate::{
    client::ClientCommand, contract::ContractCommand, genesis::GenesisCommand,
    keytool::KeyToolCommand, output_encoder::OutputEncoder, output_formatter::OutputFormatter,
    transaction::TransactionCommand,
};

use meow_types::{config, keystore::Keystore};

/// The default output formatter for commands that support formatting.
const DEFAULT_OUTPUT_FORMATTER: OutputFormatter = OutputFormatter::Json;
/// The default node RPC URL for client commands.
pub const DEFAULT_NODE_URL: &str = "http://127.0.0.1:8600";

/// The main command line commands.
#[derive(Parser)]
#[command(rename_all = "kebab-case", verbatim_doc_comment)]
pub enum Command {
    /// Say meow!
    SayMeow,
    /// MEOW keytool.
    #[command(name = "keytool")]
    KeyTool {
        /// Path to the keystore file.
        /// If omitted, the default keystore path is used.
        #[arg(long, global = true)]
        keystore_path: Option<PathBuf>,
        /// Output format for command results.
        #[arg(long, global = true, default_value_t = DEFAULT_OUTPUT_FORMATTER)]
        formatter: OutputFormatter,
        /// Subcommands.
        #[command(subcommand)]
        cmd: KeyToolCommand,
    },
    /// MEOW smart contract tools.
    Contract {
        /// MEOW node RPC URL.
        /// Example: http://127.0.0.1:8600
        #[arg(long, global = true, default_value = DEFAULT_NODE_URL)]
        node: Url,
        /// Output format for command results.
        #[arg(long, global = true, default_value_t = DEFAULT_OUTPUT_FORMATTER)]
        formatter: OutputFormatter,
        /// Subcommands.
        #[command(subcommand)]
        cmd: ContractCommand,
    },
    /// MEOW transaction tools.
    Transaction {
        /// MEOW node RPC URL.
        /// Example: http://127.0.0.1:8600
        #[arg(long, global = true, default_value = DEFAULT_NODE_URL)]
        node: Url,
        /// Output format for command results.
        #[arg(long, global = true, default_value_t = DEFAULT_OUTPUT_FORMATTER)]
        formatter: OutputFormatter,
        /// Encoding used when printing transaction payloads.
        #[arg(long, global = true, default_value_t = OutputEncoder::Base64)]
        encoder: OutputEncoder,
        /// Include object field values in the output.
        #[arg(long, global = true)]
        print_object_content: bool,
        /// Subcommands.
        #[command(subcommand)]
        cmd: TransactionCommand,
    },
    /// MEOW genesis tools.
    Genesis {
        /// Output format for command results.
        #[arg(long, global = true, default_value_t = DEFAULT_OUTPUT_FORMATTER)]
        formatter: OutputFormatter,
        /// Include object field values in the output.
        #[arg(long, global = true)]
        print_object_content: bool,
        /// Subcommands.
        #[command(subcommand)]
        cmd: GenesisCommand,
    },
    /// Interact with a running MEOW node.
    Client {
        /// MEOW node RPC URL.
        /// Example: http://127.0.0.1:8600
        #[arg(long, global = true, default_value = DEFAULT_NODE_URL)]
        node: Url,
        /// Output format for command results.
        #[arg(long, global = true, default_value_t = DEFAULT_OUTPUT_FORMATTER)]
        formatter: OutputFormatter,
        /// Include object field values in the output.
        #[arg(long, global = true)]
        print_object_content: bool,
        /// Subcommands.
        #[command(subcommand)]
        cmd: ClientCommand,
    },
}

impl Command {
    /// Runs the command.
    pub async fn run(self) -> Result<(), anyhow::Error> {
        match self {
            Command::SayMeow => {
                println!("Meow!");
            }
            Command::KeyTool {
                keystore_path,
                formatter,
                cmd,
            } => {
                let keystore_path = keystore_path.unwrap_or(config::meow_keystore_path()?);
                let mut keystore = Keystore::file_based(&keystore_path)?;

                let output = cmd.run(&mut keystore)?;

                println!("{}", formatter.format(&output)?);
            }
            Command::Contract {
                node,
                formatter,
                cmd,
            } => {
                let client = NodeClient::with_url(node);

                let output = cmd.run(&client).await?;

                println!("{}", formatter.format(&output)?);
            }
            Command::Transaction {
                node,
                formatter,
                encoder,
                print_object_content,
                cmd,
            } => {
                let client = NodeClient::with_url(node);

                let output = cmd.run(&client, encoder, print_object_content).await?;

                println!("{}", formatter.format(&output)?);
            }
            Command::Genesis {
                formatter,
                print_object_content,
                cmd,
            } => {
                let output = cmd.run(print_object_content)?;

                println!("{}", formatter.format(&output)?);
            }
            Command::Client {
                node,
                formatter,
                print_object_content,
                cmd,
            } => {
                let client = NodeClient::with_url(node);

                let output = cmd.run(&client, print_object_content).await?;

                println!("{}", formatter.format(&output)?);
            }
        }
        Ok(())
    }
}
