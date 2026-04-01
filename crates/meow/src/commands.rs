use std::path::PathBuf;

use clap::Parser;
use meow_node_client::NodeClient;

use crate::{
    client::ClientCommand, genesis::GenesisCommand, keytool::KeyToolCommand,
    output_encoder::OutputEncoder, output_formatter::OutputFormatter,
    smart_contract::SmartContractCommand, transaction::TransactionCommand,
};

use meow_types::{config::meow_keystore_path, keystore::Keystore};

/// The default output formatter for commands that support formatting.
const DEFAULT_OUTPUT_FORMATTER: OutputFormatter = OutputFormatter::Table;
/// The default node RPC URL for client commands.
const DEFAULT_NODE_URL: &str = "http://127.0.0.1:8080";

/// The main command line commands.
#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum Command {
    /// Say meow!
    SayMeow,
    /// MEOW keytool.
    #[command(name = "keytool")]
    KeyTool {
        /// The path to the keystore file.
        #[arg(long, global = true)]
        keystore_path: Option<PathBuf>,
        /// The command output is formatted using this formatter before printing.
        #[arg(long, global = true, default_value_t = DEFAULT_OUTPUT_FORMATTER)]
        formatter: OutputFormatter,
        /// Subcommands.
        #[command(subcommand)]
        cmd: KeyToolCommand,
    },
    /// MEOW smart contract tools.
    #[command(name = "smart-contract")]
    SmartContract {
        /// The MEOW Node RPC URL.
        #[arg(long, global = true, default_value = DEFAULT_NODE_URL)]
        node: String,
        /// The command output is formatted using this formatter before printing.
        #[arg(long, global = true, default_value_t = DEFAULT_OUTPUT_FORMATTER)]
        formatter: OutputFormatter,
        /// Subcommands.
        #[command(subcommand)]
        cmd: SmartContractCommand,
    },
    /// MEOW transaction tools.
    #[command(name = "transaction")]
    Transaction {
        /// The MEOW Node RPC URL.
        #[arg(long, global = true, default_value = DEFAULT_NODE_URL)]
        node: String,
        /// The path to the keystore file.
        #[arg(long, global = true)]
        keystore_path: Option<PathBuf>,
        /// The command output is formatted using this formatter before printing.
        #[arg(long, global = true, default_value_t = DEFAULT_OUTPUT_FORMATTER)]
        formatter: OutputFormatter,
        /// The transaction is encoded using this encoder before printing.
        #[arg(long, global = true, default_value_t = OutputEncoder::Base64)]
        encoder: OutputEncoder,
        /// Subcommands.
        #[command(subcommand)]
        cmd: TransactionCommand,
    },
    /// Interact with a running MEOW node.
    #[command(name = "genesis")]
    Genesis {
        /// The command output is formatted using this formatter before printing.
        #[arg(long, global = true, default_value_t = DEFAULT_OUTPUT_FORMATTER)]
        formatter: OutputFormatter,
        /// Subcommands.
        #[command(subcommand)]
        cmd: GenesisCommand,
    },
    /// Interact with a running MEOW node.
    #[command(name = "client")]
    Client {
        /// The MEOW Node RPC URL.
        #[arg(long, global = true, default_value = DEFAULT_NODE_URL)]
        node: String,
        /// The command output is formatted using this formatter before printing.
        #[arg(long, global = true, default_value_t = DEFAULT_OUTPUT_FORMATTER)]
        formatter: OutputFormatter,
        /// Subcommands.
        #[command(subcommand)]
        cmd: ClientCommand,
    },
}

impl Command {
    /// Runs the command.
    pub fn run(self) -> Result<(), anyhow::Error> {
        match self {
            Command::SayMeow => {
                println!("Meow!");
            }
            Command::KeyTool {
                keystore_path,
                formatter,
                cmd,
            } => {
                let keystore_path = keystore_path.unwrap_or(meow_keystore_path()?);
                let mut keystore = Keystore::file_based(&keystore_path)?;

                let output = cmd.run(&mut keystore)?;

                println!("{}", formatter.format(&output)?);
            }
            Command::SmartContract {
                node,
                formatter,
                cmd,
            } => {
                let client = NodeClient::new(node);

                let output = cmd.run(&client)?;

                println!("{}", formatter.format(&output)?);
            }
            Command::Transaction {
                node,
                keystore_path,
                formatter,
                encoder,
                cmd,
            } => {
                let client = NodeClient::new(node);

                let keystore_path = keystore_path.unwrap_or(meow_keystore_path()?);
                let keystore = Keystore::file_based(&keystore_path)?;

                let output = cmd.run(&client, &keystore, encoder)?;

                println!("{}", formatter.format(&output)?);
            }
            Command::Genesis { formatter, cmd } => {
                let output = cmd.run()?;

                println!("{}", formatter.format(&output)?);
            }
            Command::Client {
                node,
                formatter,
                cmd,
            } => {
                let client = NodeClient::new(node);

                let output = cmd.run(&client)?;

                println!("{}", formatter.format(&output)?);
            }
        }
        Ok(())
    }
}
