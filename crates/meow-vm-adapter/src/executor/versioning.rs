//! Increments object version numbers during transaction result assembly to maintain the monotone invariant.

use meow_types::object::{Object, object_version::ObjectVersion};

/// Bump an object's version by 1.
pub fn bump_version(obj: &Object) -> ObjectVersion {
    obj.version()
        .next()
        .expect("version must increment; object version should never be MAX")
}

#[cfg(test)]
mod tests {
    use meow_types::{
        address::Address,
        digest::Digest,
        object::{
            Object, object_owner::ObjectOwner, object_type::ObjectType,
            object_version::ObjectVersion,
        },
    };

    use super::bump_version;

    #[test]
    fn bump_from_zero_gives_one() {
        let obj = obj_with_version(ObjectVersion::ZERO);
        assert_eq!(bump_version(&obj), ObjectVersion::ONE);
    }

    #[test]
    fn bump_from_one_increments() {
        let obj = obj_with_version(ObjectVersion::ONE);
        let v2 = bump_version(&obj);
        assert!(v2 > ObjectVersion::ONE);
        assert!(v2 < ObjectVersion::MAX);
    }

    #[test]
    #[should_panic(expected = "version must increment")]
    fn bump_at_max_panics() {
        let obj = obj_with_version(ObjectVersion::MAX);
        bump_version(&obj);
    }

    fn obj_with_version(version: ObjectVersion) -> Object {
        Object::new(
            Address::ZERO,
            ObjectOwner::Immutable,
            Digest::ZERO,
            version,
            ObjectType::Module,
            vec![],
        )
    }
}
