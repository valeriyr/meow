pub mod module_encoder;
pub mod output;

use clap::Parser;
use meow_vm_adapter::builder;

use crate::smart_contract::{module_encoder::ModuleEncoder, output::SmartContractCommandOutput};

#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum SmartContractCommand {
    /// Build a smart contract module.
    Build {
        /// The path to the smart contract module.
        path: String,
        /// The module is encoded using this encoder before printing.
        #[arg(long)]
        encoding: Option<ModuleEncoder>,
    },
}

/// Runs the command.
impl SmartContractCommand {
    /// Runs the command.
    pub fn run(self) -> Result<SmartContractCommandOutput, anyhow::Error> {
        Ok(match self {
            SmartContractCommand::Build { path, encoding } => {
                let module = builder::build_from_file(path)?;
                let encoder = encoding.unwrap_or(ModuleEncoder::Base64);

                SmartContractCommandOutput::build(module, encoder)?
            }
        })
    }
}
