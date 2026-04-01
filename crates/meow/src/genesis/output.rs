use meow_genesis::Genesis;
use serde::Serialize;

use crate::object_output::ObjectOutput;

/// The genesis information.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenesisOutput {
    pub objects: Vec<ObjectOutput>,
}

impl From<Genesis> for GenesisOutput {
    fn from(genesis: Genesis) -> Self {
        GenesisOutput {
            objects: genesis
                .objects()
                .iter()
                .map(|o| ObjectOutput::from(o.clone()))
                .collect(),
        }
    }
}
