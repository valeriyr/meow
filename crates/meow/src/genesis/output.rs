//! Output type for the genesis subcommand.

use meow_genesis::Genesis;
use serde::Serialize;

use crate::outputs::object_output::ObjectOutput;

/// The genesis information.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenesisOutput {
    pub objects: Vec<ObjectOutput>,
}

impl GenesisOutput {
    pub fn new(genesis: Genesis, with_object_content: bool) -> Self {
        GenesisOutput {
            objects: genesis
                .objects()
                .iter()
                .map(|o| ObjectOutput::new(o.clone(), with_object_content))
                .collect(),
        }
    }
}
