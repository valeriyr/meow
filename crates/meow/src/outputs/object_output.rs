use std::collections::BTreeMap;

use meow_types::{object::Object, system_framework};
use serde::Serialize;

/// The brief information of an object.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectOutput {
    pub address: String,
    pub owner: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub version: String,
    pub digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<BTreeMap<String, String>>,
}

impl From<Object> for ObjectOutput {
    fn from(obj: Object) -> Self {
        Self {
            address: obj.address().to_string(),
            owner: obj.owner().to_string(),
            type_: obj.type_().to_string(),
            version: obj.version().to_string(),
            digest: obj.digest().to_string(),
            content: system_framework::extract_human_readable_content(&obj),
        }
    }
}
