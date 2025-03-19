use std::path::PathBuf;

use clap::{command, Parser};

use crate::keytool::KeyToolCommand;

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
        #[arg(long)]
        keystore_path: Option<PathBuf>,
        /// Return command outputs in json format
        #[arg(long, global = true)]
        json: bool,
        /// Subcommands.
        #[command(subcommand)]
        cmd: KeyToolCommand,
    },
}

impl Command {
    /// Runs the command.
    pub fn run(self) -> Result<(), anyhow::Error> {
        match self {
            Command::SayMeow => {
                println!("Meow!");
                Ok(())
            }
            Command::KeyTool {
                keystore_path,
                json,
                cmd,
            } => {
                let keystore_path = keystore_path.unwrap_or(meow_keystore_path()?);
                let mut keystore = Keystore::file_based(&keystore_path)?;

                cmd.run(&mut keystore)
            }
        }
    }
}
