//! Serialisable summary of an on-chain object for CLI output.

use std::collections::BTreeMap;

use meow_types::object::{Object, object_conversion};
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

impl ObjectOutput {
    /// Creates an `ObjectOutput` from an `Object`.
    ///
    /// If `with_object_content` is `true`, the `content` field is populated with
    /// the decoded object field values. If `false`, `content` is `None` and omitted
    /// from the serialized output.
    pub fn new(obj: Object, with_object_content: bool) -> Self {
        Self {
            address: obj.address().to_string(),
            owner: obj.owner().to_string(),
            type_: obj.type_().to_string(),
            version: obj.version().to_string(),
            digest: obj.digest().to_string(),
            content: with_object_content
                .then(|| object_conversion::extract_human_readable_content(&obj))
                .flatten(),
        }
    }
}
