mod function_arg;

pub mod module_encoder;
pub mod output;

use clap::Parser;
use meow_vm_adapter::{builder, runner};

use crate::smart_contract::{
    function_arg::FunctionArg, module_encoder::ModuleEncoder, output::SmartContractCommandOutput,
};

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
    /// Run a smart contract function.
    Run {
        /// The path to the smart contract module.
        path: String,
        /// The function name.
        function: String,
        /// The arguments to pass to the function.
        ///
        /// Type is inferred automatically: `true`/`false` → bool, `digits only` → u64, `0x<64 hex>` → Address, `anything else` → String.
        args: Vec<FunctionArg>,
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
            SmartContractCommand::Run {
                path,
                function,
                args,
            } => {
                let module = builder::build_from_file(path)?;
                let args = args.into_iter().map(Into::into).collect::<Vec<_>>();

                let result = runner::run(module, &function, args)?;

                SmartContractCommandOutput::Run(result.into())
            }
        })
    }
}
