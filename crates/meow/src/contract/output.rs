//! Output types for the contract subcommand.

use meow_vm_adapter::{Module, runner::RunResult};
use serde::Serialize;

use crate::output_encoder::OutputEncoder;

/// The module information.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModuleOutput {
    /// The name of the module.
    pub name: String,
    /// The content of the module.
    pub content: String,
}

/// The run result.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RunResultOutput {
    /// The return value of the call, if any.
    pub return_value: Option<String>,
    /// Objects transferred out during the call: `(object, new_owner)`.
    pub transfers: Vec<(String, String)>,
    /// Objects destroyed during the call.
    pub destroyed: Vec<String>,
    /// Gas spent during the call.
    pub gas_spent: u64,
}

/// The contract command outputs.
#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum ContractCommandOutput {
    /// The build command output.
    Build(ModuleOutput),
    /// The run command output.
    Run(RunResultOutput),
}

impl ContractCommandOutput {
    /// Builds the Build command output using the specified encoder.
    pub fn build(module: Module, encoder: OutputEncoder) -> anyhow::Result<Self> {
        let encoded_module = encoder.encode(&module)?;

        Ok(ContractCommandOutput::Build(ModuleOutput {
            name: module.name,
            content: encoded_module,
        }))
    }
}

impl From<RunResult> for RunResultOutput {
    fn from(result: RunResult) -> Self {
        let return_value = result.return_value.map(|v| v.to_string());
        let transfers = result
            .transfers
            .into_iter()
            .map(|(obj, new_owner)| (obj.to_string(), new_owner.to_string()))
            .collect();
        let destroyed = result
            .destroyed
            .into_iter()
            .map(|obj| obj.to_string())
            .collect();

        RunResultOutput {
            return_value,
            transfers,
            destroyed,
            gas_spent: result.gas_spent,
        }
    }
}
