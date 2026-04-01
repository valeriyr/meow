pub mod output;

use std::path::PathBuf;

use clap::Parser;
use meow_node_client::NodeClient;
use meow_types::identifier::Identifier;
use meow_vm_adapter::{builder, runner};

use crate::{
    call_arg::CallArg, output_encoder::OutputEncoder,
    smart_contract::output::SmartContractCommandOutput,
};

#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum SmartContractCommand {
    /// Build a smart contract module.
    Build {
        /// The path to the smart contract module.
        path: PathBuf,
        /// The module is encoded using this encoder before printing.
        #[arg(long, default_value_t = OutputEncoder::Base64)]
        encoder: OutputEncoder,
    },
    /// Run a smart contract function.
    Run {
        /// The path to the smart contract module.
        path: PathBuf,
        /// The function name.
        function: Identifier,
        /// The arguments to pass to the function.
        ///
        /// Parsing rules (applied in order):
        /// - `true` / `false`    → Raw bool
        /// - all-digit string    → Raw u64
        /// - `@0x<hex>`          → Raw address
        /// - `0x<hex>`           → Address of an object on-chain
        /// - anything else       → Raw string
        args: Vec<CallArg>,
    },
}

impl SmartContractCommand {
    pub fn run(self, client: &NodeClient) -> anyhow::Result<SmartContractCommandOutput> {
        Ok(match self {
            SmartContractCommand::Build { path, encoder } => {
                let module = builder::build_from_file(path)?;

                SmartContractCommandOutput::build(module, encoder)?
            }
            SmartContractCommand::Run {
                path,
                function,
                args,
            } => {
                let module = builder::build_from_file(path)?;
                let args = args
                    .into_iter()
                    .map(|arg| arg.into_value(client))
                    .collect::<Result<Vec<_>, _>>()?;

                let result = runner::run(module, &function, args)?;

                SmartContractCommandOutput::Run(result.into())
            }
        })
    }
}
