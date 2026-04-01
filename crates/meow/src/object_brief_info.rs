use meow_types::{
    address::Address,
    digest::Digest,
    object::{Object, object_owner::ObjectOwner, object_type::ObjectType},
};
use serde::Serialize;

/// The brief information of an object.
#[derive(Debug, Serialize)]
pub struct ObjectBriefInfo {
    pub address: Address,
    pub owner: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub version: String,
    pub digest: Digest,
}

impl From<Object> for ObjectBriefInfo {
    fn from(obj: Object) -> Self {
        let owner = match obj.owner() {
            ObjectOwner::Address(a) => a.to_string(),
            ObjectOwner::Immutable => "immutable".into(),
        };
        let type_ = match obj.type_() {
            ObjectType::Object(decl) => format!("{}::{}", decl.module(), decl.name()),
            ObjectType::Module => "module".into(),
        };
        Self {
            address: *obj.address(),
            owner,
            type_,
            version: obj.version().to_string(),
            digest: obj.digest(),
        }
    }
}
