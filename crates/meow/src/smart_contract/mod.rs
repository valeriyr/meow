pub mod output;

use std::path::PathBuf;

use clap::Parser;
use meow_node_client::NodeClient;
use meow_types::identifier::Identifier;
use meow_vm_adapter::runner;

use crate::{
    builder, call_arg::CallArg, output_encoder::OutputEncoder,
    smart_contract::output::SmartContractCommandOutput,
};

#[derive(Parser)]
#[command(rename_all = "kebab-case", verbatim_doc_comment)]
pub enum SmartContractCommand {
    /// Compile a `.meow` source file into a module (without creating a transaction).
    Build {
        /// Path to the `.meow` source file.
        path: PathBuf,
        /// Encoding for the compiled module output.
        #[arg(long, default_value_t = OutputEncoder::Base64)]
        encoder: OutputEncoder,
    },
    /// Run a smart contract function locally without submitting a transaction.
    Run {
        /// Path to the `.meow` source file.
        path: PathBuf,
        /// Name of the function to call.
        function: Identifier,
        /// Argument to pass to the function (repeatable). Auto-detected by format:
        /// - `true` / `false` → bool
        /// - digits only → u64
        /// - `@0x<hex>` → raw address value (not resolved against the node)
        /// - `0x<hex>` → on-chain object (resolved against the node)
        /// - anything else → string
        #[arg(value_name = "VALUE", verbatim_doc_comment)]
        args: Vec<CallArg>,
    },
}

impl SmartContractCommand {
    pub async fn run(self, client: &NodeClient) -> anyhow::Result<SmartContractCommandOutput> {
        Ok(match self {
            SmartContractCommand::Build { path, encoder } => {
                let module = builder::build_module(client, path).await?;
                SmartContractCommandOutput::build(module, encoder)?
            }
            SmartContractCommand::Run {
                path,
                function,
                args,
            } => {
                let module = builder::build_module(client, path).await?;

                let mut values = Vec::new();
                for arg in args {
                    values.push(arg.into_value(client).await?);
                }

                let result = runner::run(module, &function, values)?;

                SmartContractCommandOutput::Run(result.into())
            }
        })
    }
}
