use serde::Serialize;

use crate::smart_contract::module_encoder::ModuleEncoder;

/// The module information.
#[derive(Serialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    /// The name of the module.
    pub name: String,
    /// The content of the module.
    pub content: String,
}

/// The smart contract command outputs.
#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum SmartContractCommandOutput {
    /// The build command output.
    Build(Module),
}

impl SmartContractCommandOutput {
    /// Builds the command output using the specified encoder.
    pub fn build(
        module: meow_vm_adapter::builder::Module,
        encoder: ModuleEncoder,
    ) -> Result<Self, anyhow::Error> {
        let encoded_module = encoder.encode(&module)?;

        Ok(SmartContractCommandOutput::Build(Module {
            name: module.name,
            content: encoded_module,
        }))
    }
}
