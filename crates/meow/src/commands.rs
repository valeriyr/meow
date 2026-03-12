use std::path::PathBuf;

use clap::Parser;

use crate::{
    keytool::KeyToolCommand, output_formatter::OutputFormatter,
    smart_contract::SmartContractCommand,
};

use meow_types::{config::meow_keystore_path, keystore::Keystore};

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
        #[arg(long, global = true)]
        output_formatter: Option<OutputFormatter>,
        /// Subcommands.
        #[command(subcommand)]
        cmd: KeyToolCommand,
    },
    /// MEOW smart contract tools.
    #[command(name = "smart-contract")]
    SmartContract {
        /// The command output is formatted using this formatter before printing.
        #[arg(long, global = true)]
        output_formatter: Option<OutputFormatter>,
        /// Subcommands.
        #[command(subcommand)]
        cmd: SmartContractCommand,
    },
}

impl Command {
    /// Runs the command.
    pub fn run(self) -> Result<(), anyhow::Error> {
        Ok(match self {
            Command::SayMeow => {
                println!("Meow!");
            }
            Command::KeyTool {
                keystore_path,
                output_formatter,
                cmd,
            } => {
                let keystore_path = keystore_path.unwrap_or(meow_keystore_path()?);
                let mut keystore = Keystore::file_based(&keystore_path)?;

                let output = cmd.run(&mut keystore)?;

                println!(
                    "{}",
                    output_formatter
                        .unwrap_or(OutputFormatter::Table)
                        .format(&output)?
                );
            }
            Command::SmartContract {
                output_formatter,
                cmd,
            } => {
                let output = cmd.run()?;

                println!(
                    "{}",
                    output_formatter
                        .unwrap_or(OutputFormatter::Table)
                        .format(&output)?
                );
            }
        })
    }
}
