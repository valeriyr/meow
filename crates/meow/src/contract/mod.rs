//! `meow contract` commands: compile and run Meow Language contracts against a local VM.

pub mod output;

use std::path::PathBuf;

use clap::Parser;
use meow_node_client::NodeClient;
use meow_types::{address::Address, identifier::Identifier};
use meow_vm_adapter::{external_context::ExternalContext, runner};

use crate::{
    builder, call_arg::CallArg, contract::output::ContractCommandOutput,
    output_encoder::OutputEncoder,
};

#[derive(Parser)]
#[command(rename_all = "kebab-case", verbatim_doc_comment)]
pub enum ContractCommand {
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
    /// Run a privileged smart contract function locally without submitting a transaction.
    RunPrivileged {
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

impl ContractCommand {
    pub async fn run(self, client: &NodeClient) -> anyhow::Result<ContractCommandOutput> {
        Ok(match self {
            ContractCommand::Build { path, encoder } => {
                let (module, _) = builder::build_module(client, path).await?;
                ContractCommandOutput::build(module, encoder)?
            }
            ContractCommand::Run {
                path,
                function,
                args,
            } => {
                let (module, deps) = builder::build_module(client, path).await?;

                let mut values = Vec::new();
                for arg in args {
                    values.push(arg.into_value(client).await?);
                }

                let result = runner::run(
                    (Address::ZERO, module),
                    &function,
                    values,
                    deps,
                    ExternalContext::default(),
                )?;

                ContractCommandOutput::Run(result.into())
            }
            ContractCommand::RunPrivileged {
                path,
                function,
                args,
            } => {
                let (module, deps) = builder::build_module(client, path).await?;

                let mut values = Vec::new();
                for arg in args {
                    values.push(arg.into_value(client).await?);
                }

                let result = runner::run_privileged(
                    (Address::ZERO, module),
                    &function,
                    values,
                    deps,
                    ExternalContext::default(),
                )?;

                ContractCommandOutput::Run(result.into())
            }
        })
    }
}
