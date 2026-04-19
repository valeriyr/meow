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
use meow_vm::VmCallResult;

use crate::{context::Context, executor::versioning};

/// `(created, changed, destroyed)` object lists from a single execution.
type ObjectEffects = (Vec<Object>, Vec<Object>, Vec<Object>);

/// Build the created/changed/destroyed object lists from execution results.
///
/// Each output object's version is bumped independently from its own input
/// version. Newly created objects (from `meow_vm_fresh_id`) start at version 1.
pub fn collect_object_effects(
    ctx: &Context,
    call_result: &VmCallResult,
    object_args: &[(usize, &Object)], // (arg_index, input_object)
    module_address: &Address,
    tx_digest: &Digest,
) -> std::result::Result<ObjectEffects, String> {
    let mut created_objects: Vec<Object> = Vec::new();
    let mut changed_objects: Vec<Object> = Vec::new();
    let mut destroyed_objects: Vec<Object> = Vec::new();

    // Build a lookup from address → input object for version resolution.
    let input_by_addr: HashMap<Address, &Object> = object_args
        .iter()
        .map(|(_, obj)| (*obj.address(), *obj))
        .collect();

    // Invariant: object_args must only contain address-owned non-module objects.
    // Modules and immutable objects must be rejected earlier in resolve_arg.
    object_args.iter().for_each(|(_, obj)| {
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
        .filter_map(|(v, _)| meow_object::object_id(v))
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

    // Validate that all input objects are accounted for (transferred, destroyed, or surviving in their original slot).
    for (arg_idx, input_obj) in object_args {
        let id = input_obj.address();
        let surviving = call_result
            .final_args
            .get(*arg_idx)
            .is_some_and(|v| v.is_some());
        let accounted = transferred_ids.contains(id) || destroyed_ids.contains(id) || surviving;
        if !accounted {
            return Err(format!("input object not accounted for: {id}"));
        }
    }

    // Collect transferred objects.
    for (obj_val, owner) in ctx.transfers() {
        let id: Address =
            meow_object::object_id(obj_val).expect("transferred object must have an ID");

        let (version, is_fresh) = if fresh_ids.contains(&id) {
            (ObjectVersion::ONE, true)
        } else {
            let original = input_by_addr
                .get(&id)
                .expect("all transferred objects must be found in inputs");
            (versioning::bump_version(original), false)
        };

        let obj = object_conversion::vm_object_value_to_object(
            obj_val,
            ObjectOwner::Address(*owner),
            *tx_digest,
            version,
            module_address,
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
        // If object was created and destroyed in the same execution session, it should not appear in the destroyed list — skip it.
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

    // Collect surviving input objects (those not consumed by transfer/destroy).
    for (arg_idx, input_obj) in object_args {
        let final_val = match call_result.final_args.get(*arg_idx) {
            Some(Some(v)) => v,
            _ => continue, // consumed (moved to transfer/destroy)
        };

        let id: Address =
            meow_object::object_id(final_val).expect("surviving object must have an ID");

        // Skip if already handled as a transferred input.
        if transferred_ids.contains(&id) || destroyed_ids.contains(&id) {
            continue;
        }

        let version = versioning::bump_version(input_obj);

        changed_objects.push(
            object_conversion::vm_object_value_to_object(
                final_val,
                *input_obj.owner(),
                *tx_digest,
                version,
                module_address,
            )
            .expect("VM object value is expected to be convertible to object"),
        );
    }

    Ok((created_objects, changed_objects, destroyed_objects))
}
