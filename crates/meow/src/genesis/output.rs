use meow_genesis::Genesis;
use serde::Serialize;

use crate::object_brief_info::ObjectBriefInfo;

/// The genesis information.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenesisOutput {
    pub objects: Vec<ObjectBriefInfo>,
}

impl From<Genesis> for GenesisOutput {
    fn from(genesis: Genesis) -> Self {
        GenesisOutput {
            objects: genesis
                .objects()
                .iter()
                .map(|o| ObjectBriefInfo::from(o.clone()))
                .collect(),
        }
    }
}
