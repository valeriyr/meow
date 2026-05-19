//! Post-execution side-effect collection and result assembly.
//!
//! Inspects the execution context after the VM call returns — transferred objects
//! and destroyed IDs — and assembles the final `ExecutionResult`.

use std::collections::{BTreeSet, HashMap};

use meow_types::{
    address::Address,
    digest::Digest,
    object::{
        Object, object_conversion, object_owner::ObjectOwner, object_type::ObjectType,
        object_version::ObjectVersion,
    },
    system_framework::meow_object,
};

use crate::{context::Context, executor::versioning};

/// `(created, changed, destroyed)` object lists from a single execution.
type ObjectEffects = (Vec<Object>, Vec<Object>, Vec<Object>);

/// Build the created/changed/destroyed object lists from execution results.
///
/// Each output object's version is bumped independently from its own input
/// version. Newly created objects (from `meow_vm_fresh_id`) start at version 1.
pub fn collect_object_effects(
    ctx: &Context,
    object_args: &[&Object],
    tx_digest: &Digest,
) -> std::result::Result<ObjectEffects, String> {
    let mut created_objects: Vec<Object> = Vec::new();
    let mut changed_objects: Vec<Object> = Vec::new();
    let mut destroyed_objects: Vec<Object> = Vec::new();

    // Build a lookup from address → input object for version resolution.
    let input_by_addr: HashMap<Address, &Object> = object_args
        .iter()
        .map(|obj| (*obj.address(), *obj))
        .collect();

    // Invariant: object_args must only contain address-owned non-module objects.
    object_args.iter().for_each(|obj| {
        assert!(
            obj.owner().is_address_owned(),
            "only address-owned objects can be used as call arguments"
        );
        assert!(
            obj.type_() != &ObjectType::Module,
            "module objects cannot be used as call arguments"
        );
        assert!(
            obj.version() != &ObjectVersion::MAX,
            "objects at max version cannot be used as call arguments"
        );
    });

    let transferred_ids: BTreeSet<Address> = ctx
        .transfers()
        .iter()
        .filter_map(|(v, _)| meow_object::object_address(v))
        .collect();
    let destroyed_ids: BTreeSet<Address> = ctx.destroyed().iter().copied().collect();
    let fresh_ids: BTreeSet<Address> = ctx.fresh_ids().iter().copied().collect();

    // Validate that all fresh IDs were accounted for (transferred or destroyed).
    for fresh_id in ctx.fresh_ids() {
        let consumed = transferred_ids.contains(fresh_id) || destroyed_ids.contains(fresh_id);
        if !consumed {
            return Err(format!("created object not consumed: {fresh_id}"));
        }
    }

    // Validate that all input objects are accounted for (transferred or destroyed).
    // The compiler guarantees struct params are consumed; this is a runtime defence check.
    for input_obj in object_args {
        let id = input_obj.address();
        if !transferred_ids.contains(id) && !destroyed_ids.contains(id) {
            return Err(format!("input object not accounted for: {id}"));
        }
    }

    // Collect transferred objects.
    for (obj_val, owner) in ctx.transfers() {
        let id: Address =
            meow_object::object_address(obj_val).expect("transferred object must have an address");

        let is_fresh = fresh_ids.contains(&id);
        let version = if is_fresh {
            ObjectVersion::ONE
        } else {
            let original = input_by_addr
                .get(&id)
                .expect("all transferred objects must be found in inputs");
            versioning::bump_version(original)
        };

        let obj = object_conversion::vm_object_value_to_object(
            obj_val,
            ObjectOwner::Address(*owner),
            *tx_digest,
            version,
        )
        .expect("failed to convert VM object value to object");

        if is_fresh {
            created_objects.push(obj);
        } else {
            changed_objects.push(obj);
        }
    }

    // Collect destroyed objects.
    for id in ctx.destroyed() {
        if !fresh_ids.contains(id) {
            let original = input_by_addr
                .get(id)
                .expect("destroyed object must be found in inputs");
            let version = versioning::bump_version(original);

            destroyed_objects.push(Object::new(
                *original.address(),
                *original.owner(),
                *tx_digest,
                version,
                original.type_().clone(),
                original.content().to_vec(),
            ));
        }
    }

    Ok((created_objects, changed_objects, destroyed_objects))
}
