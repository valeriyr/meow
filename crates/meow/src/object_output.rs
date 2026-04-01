use meow_types::object::Object;
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
}

impl From<Object> for ObjectOutput {
    fn from(obj: Object) -> Self {
        Self {
            address: obj.address().to_string(),
            owner: obj.owner().to_string(),
            type_: obj.type_().to_string(),
            version: obj.version().to_string(),
            digest: obj.digest().to_string(),
        }
    }
}
