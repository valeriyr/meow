//! Adapter-level bytecode verifier that enforces chain-specific object conventions.
//!
//! Runs after the language-level verifier and checks rules that belong to this adapter's
//! object model rather than the language itself — for example, that object IDs are always
//! freshly created within the same function and never reused or aliased.

pub mod error;

use std::collections::HashMap;

use meow_types::system_framework::meow_object::{
    MEOW_OBJECT_ID_BYTECODE_TYPE_NAME, MEOW_OBJECT_ID_FIELD_NAME,
};
use meow_vm_types::address::Address;
use meow_vm_types::natives::{NativeSig, builtin_natives};
use meow_vm_types::{
    bytecode::Instruction,
    module::{Function, Module},
    module_ref::{is_qualified, parse_module_ref},
    types::{StructDef, Type},
};

use meow_types::config;

use crate::bytecode_verifier::error::BytecodeVerifierError;

//
// ─── Public API ───
//

/// Run adapter-level bytecode verification on `module`.
///
/// Checks rules that are specific to the chain/adapter convention and are
/// intentionally outside the language-level [`meow_vm_bytecode_verifier`]:
///
/// 1. **Object layout**: a struct is an object if its first field is
///    `id: @0x...::Id`. An `Id`-typed field is therefore only permitted in
///    first position, and no struct field may itself be an object type (objects
///    cannot be nested).
/// 2. **ID freshness**: every object construction via `NewStruct` must use a
///    value that originates directly from `meow_vm_fresh_id` in the same
///    function (not from a parameter, local variable seeded elsewhere, or
///    cross-module call).
/// 3. **Transfer type**: every `meow_vm_transfer` call must pass an on-chain
///    object struct (a local struct whose first field is `id: meow_object::Id`).
///    The language verifier already guarantees the argument is a *local* struct
///    (dep structs can never reach `meow_vm_transfer`), so this check only needs
///    to reject local structs that are not object-shaped.
///
/// The language-level verifier must pass before calling this function.
pub fn verify(
    module: &Module,
    deps: &HashMap<Address, &Module>,
    adapter_natives: &[NativeSig],
) -> Result<(), Vec<BytecodeVerifierError>> {
    let mut errors = Vec::new();

    let id_type = Type::Struct(MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string());
    let object_types: Vec<&str> = module
        .structs
        .iter()
        .filter(|s| is_object_struct(s, &id_type))
        .map(|s| s.name.as_str())
        .collect();

    check_object_layouts(module, deps, &id_type, &object_types, &mut errors);

    let builtins = builtin_natives();
    let all_natives: Vec<&NativeSig> = builtins.iter().chain(adapter_natives.iter()).collect();

    for func in &module.functions {
        check_function_body(func, module, deps, &all_natives, &object_types, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

//
// ─── Object layout ───
//

/// Enforce the two structural object-layout rules on every local struct:
///
/// 1. **`id` placement**: a field of type `meow_object::Id` is only allowed as
///    the *first* field — that is what designates the struct an object. An
///    `Id`-typed field anywhere else is rejected, so a struct is unambiguously
///    either a well-formed object or carries no object identity at all.
/// 2. **No nested objects**: a field whose type is itself an on-chain object is
///    rejected. Objects have move semantics and a unique identity; embedding one
///    as a field would let it be copied or orphaned with its enclosing struct.
///    Both local object types (matched against `object_types`) and dep object
///    types (resolved through `deps` by their `id: Id` first field) are caught.
fn check_object_layouts(
    module: &Module,
    deps: &HashMap<Address, &Module>,
    id_type: &Type,
    object_types: &[&str],
    errors: &mut Vec<BytecodeVerifierError>,
) {
    for s in &module.structs {
        for (i, field) in s.fields.iter().enumerate() {
            // Rule 1: an `id: meow_object::Id` field must be the first field.
            if field.ty == *id_type && i != 0 {
                errors.push(BytecodeVerifierError::IdFieldNotFirst {
                    struct_name: s.name.clone(),
                    field_name: field.name.clone(),
                });
            }

            // Rule 2: no field may itself be an on-chain object type.
            let Type::Struct(field_type_name) = &field.ty else {
                continue;
            };
            let is_obj = if let Some((dep_addr, type_name)) = parse_module_ref(field_type_name) {
                deps.get(&dep_addr)
                    .and_then(|m| m.get_struct(type_name))
                    .map(|def| is_object_struct(def, id_type))
                    .unwrap_or(false)
            } else {
                object_types.contains(&field_type_name.as_str())
            };
            if is_obj {
                errors.push(BytecodeVerifierError::ObjectAsFieldType {
                    struct_name: s.name.clone(),
                    field_name: field.name.clone(),
                    object_type: field_type_name.clone(),
                });
            }
        }
    }
}

//
// ─── Combined abstract interpretation: freshness + transfer type ───
//
// Two orthogonal properties are checked together by a single forward pass over
// each function body that abstractly tracks, per value, its freshness origin
// ([`Fresh`]) and its struct-type classification ([`StructTag`]) as values flow
// across the operand stack and locals. The two are independent — neither check
// depends on the other; they merely share the traversal and the per-slot
// plumbing. The pass mirrors the VM's own stack discipline so that, at each
// `NewStruct` and `meow_vm_transfer`, the abstract state describes what concrete
// value would sit at that stack slot.
//

/// Freshness origin of a value (drives the ID-freshness check at `NewStruct`).
#[derive(Clone, PartialEq)]
enum Fresh {
    /// Originated directly from `Call(NATIVE_FN_FRESH_ID)`.
    Id,
    /// Any other value.
    Other,
}

/// Struct type classification of a value (drives the transfer-type check at `meow_vm_transfer`).
#[derive(Clone, PartialEq)]
enum StructTag {
    /// Value is known to hold an object-shaped struct. Allowed at `meow_vm_transfer`.
    Object,
    /// Value is known to hold a non-object struct with the given type name.
    /// Rejected at `meow_vm_transfer`.
    NonObject(String),
    /// Object-ness could not be pinned to a single local struct, so the value is
    /// accepted conservatively at `meow_vm_transfer`. Arises for:
    /// - non-struct values (primitives, `GetField`/`LoadField` results, untracked
    ///   tuple elements);
    /// - dep (qualified) struct types — the language verifier guarantees these
    ///   never reach `meow_vm_transfer`, so they are left unresolved here;
    /// - values whose tag diverges across a branch merge (`merge_slot`).
    Other,
}

/// Abstract value held at one stack or local position: the freshness and
/// struct-type tags tracked for whatever concrete value would live there.
///
/// For a tuple value, `tuple` holds the per-element [`StructTag`] in element
/// order (`None` for non-tuples); this lets `UnpackTuple` restore each element's
/// tag instead of discarding it. Freshness is intentionally *not* tracked
/// through tuples — wrapping a value in a tuple resets it to `Fresh::Other`.
#[derive(Clone)]
struct Slot {
    fresh: Fresh,
    tag: StructTag,
    tuple: Option<Vec<StructTag>>,
}

impl Slot {
    fn other() -> Self {
        Self::scalar(Fresh::Other, StructTag::Other)
    }

    /// A non-tuple value carrying the given freshness and struct-type tags.
    fn scalar(fresh: Fresh, tag: StructTag) -> Self {
        Self {
            fresh,
            tag,
            tuple: None,
        }
    }
}

/// Abstract machine state at one program point: a shadow of the VM's operand
/// stack and locals carrying [`Slot`] tags instead of concrete values.
///
/// `reachable` is `false` after an unconditional control-flow exit (`Jump`,
/// `Return`); instructions are skipped until a branch target re-seeds the state.
#[derive(Clone)]
struct VerifyState {
    stack: Vec<Slot>,
    locals: Vec<Slot>,
    reachable: bool,
}

impl VerifyState {
    fn new(local_count: usize) -> Self {
        Self {
            stack: Vec::new(),
            locals: vec![Slot::other(); local_count],
            reachable: true,
        }
    }

    /// Join this state with another path reaching the same program point.
    ///
    /// Conservative: a tag survives only when both paths agree exactly, so a
    /// value is treated as `Fresh::Id` / a specific `StructTag` only if it holds
    /// on *every* path into the merge. The stack is truncated to the shorter of
    /// the two (defensive — the language verifier already balances stack depth).
    fn merge_from(&mut self, other: &VerifyState) {
        let stack_len = self.stack.len().min(other.stack.len());
        self.stack.truncate(stack_len);
        for (i, slot) in self.stack.iter_mut().enumerate() {
            merge_slot(slot, &other.stack[i]);
        }
        for (i, slot) in self.locals.iter_mut().enumerate() {
            match other.locals.get(i) {
                Some(o) => merge_slot(slot, o),
                None => *slot = Slot::other(),
            }
        }
    }

    fn pop(&mut self) -> Slot {
        self.stack.pop().unwrap_or_else(Slot::other)
    }

    fn push(&mut self, slot: Slot) {
        self.stack.push(slot);
    }

    fn push_other(&mut self) {
        self.stack.push(Slot::other());
    }
}

/// Conservatively reconcile `slot` with the same position on another path:
/// a tag (freshness, struct-type, or a tuple element) survives only when both
/// paths agree exactly; any disagreement degrades it to the `Other`/`None` case.
fn merge_slot(slot: &mut Slot, other: &Slot) {
    if other.fresh != Fresh::Id {
        slot.fresh = Fresh::Other;
    }
    if other.tag != slot.tag {
        slot.tag = StructTag::Other;
    }
    match (&mut slot.tuple, &other.tuple) {
        (Some(a), Some(b)) if a.len() == b.len() => {
            for (ta, tb) in a.iter_mut().zip(b.iter()) {
                if ta != tb {
                    *ta = StructTag::Other;
                }
            }
        }
        // One path isn't a tuple, or the lengths differ: drop element tracking.
        (tuple, _) => *tuple = None,
    }
}

/// Classify a *local* struct type name as `Object`, `NonObject`, or `Other`.
///
/// Qualified (dep) names are `Other`: the language verifier guarantees they
/// never reach `meow_vm_transfer`, so the adapter need not classify them.
fn local_struct_tag(type_name: &str, object_types: &[&str]) -> StructTag {
    if is_qualified(type_name) {
        return StructTag::Other;
    }
    // Local unqualified type: all local struct names are known post language-verification.
    if object_types.contains(&type_name) {
        StructTag::Object
    } else {
        StructTag::NonObject(type_name.to_string())
    }
}

/// Build the result [`Slot`] for the return value of a call to `name`.
///
/// Only local functions are tracked; cross-module results are `Other` because
/// their (dep) struct types can never reach `meow_vm_transfer`. A local struct
/// return is classified directly; a tuple return records each element's tag so
/// `UnpackTuple` can recover them. `fresh` is supplied by the caller.
fn return_slot(name: &str, module: &Module, object_types: &[&str], fresh: Fresh) -> Slot {
    if is_qualified(name) {
        return Slot::scalar(fresh, StructTag::Other);
    }
    match module
        .get_function(name)
        .and_then(|f| f.return_type.as_ref())
    {
        Some(Type::Struct(type_name)) => {
            Slot::scalar(fresh, local_struct_tag(type_name, object_types))
        }
        Some(Type::Tuple(types)) => {
            let elems = types
                .iter()
                .map(|t| match t {
                    Type::Struct(n) => local_struct_tag(n, object_types),
                    _ => StructTag::Other,
                })
                .collect();
            Slot {
                fresh,
                tag: StructTag::Other,
                tuple: Some(elems),
            }
        }
        _ => Slot::scalar(fresh, StructTag::Other),
    }
}

/// Returns `(n_pops, pushes_return)` for a call to `name`.
fn call_effect(
    name: &str,
    module: &Module,
    deps: &HashMap<Address, &Module>,
    natives: &[&NativeSig],
) -> (usize, bool) {
    if let Some((dep_addr, fn_name)) = parse_module_ref(name) {
        if let Some(func) = deps.get(&dep_addr).and_then(|m| m.get_function(fn_name)) {
            return (func.params.len(), func.return_type.is_some());
        }
        return (0, false);
    }
    if let Some(sig) = natives.iter().find(|s| s.name == name) {
        return (sig.params.len(), sig.return_type.is_some());
    }
    if let Some(func) = module.get_function(name) {
        return (func.params.len(), func.return_type.is_some());
    }
    (0, false)
}

/// Abstractly interpret `func`'s body, enforcing the ID-freshness and
/// transfer-type rules.
///
/// Walks instructions in index order, maintaining `current` as the abstract
/// state for the instruction about to execute. Forward branches stash a snapshot
/// of the state under their target PC in `pending`; when execution reaches a PC
/// with a stashed snapshot, the snapshot is merged into `current` (or replaces
/// it when arriving from unreachable code). Because branches only ever jump
/// forward (the language verifier forbids backward edges / loops), a single
/// linear pass reaches a fixpoint — no instruction is visited twice.
///
/// Errors are accumulated rather than returned eagerly so a single pass reports
/// every violation in the function.
fn check_function_body(
    func: &Function,
    module: &Module,
    deps: &HashMap<Address, &Module>,
    natives: &[&NativeSig],
    object_types: &[&str],
    errors: &mut Vec<BytecodeVerifierError>,
) {
    let mut current = VerifyState::new(func.local_count as usize);

    // Seed struct-typed param slots up-front (dep-typed params classify as `Other`).
    for (i, (_, ty)) in func.params.iter().enumerate() {
        if let Type::Struct(name) = ty
            && let Some(slot) = current.locals.get_mut(i)
        {
            slot.tag = local_struct_tag(name, object_types);
        }
    }

    let mut pending: HashMap<usize, VerifyState> = HashMap::new();

    for (pc, instr) in func.code.iter().enumerate() {
        // A branch may target this PC: merge its snapshot into the fall-through
        // state, or adopt it outright if the fall-through path is unreachable.
        if let Some(incoming) = pending.remove(&pc) {
            if current.reachable {
                current.merge_from(&incoming);
            } else {
                current = incoming;
            }
        }

        // Dead code after an unconditional exit with no branch landing here.
        if !current.reachable {
            continue;
        }

        // Each arm applies the instruction's abstract stack effect: most ops
        // simply pop their inputs and push an untracked (`Slot::other`) result;
        // the arms that carry tags forward (Load/Store/Dup/NewStruct/UnpackStruct/
        // Call) or perform a check (NewStruct, Call) are commented individually.
        match instr {
            Instruction::PushBool(_)
            | Instruction::PushU64(_)
            | Instruction::PushAddress(_)
            | Instruction::PushStr(_) => {
                current.push_other();
            }

            Instruction::Load(s) => {
                let slot = current
                    .locals
                    .get(*s as usize)
                    .cloned()
                    .unwrap_or_else(Slot::other);
                current.push(slot);
            }

            Instruction::Store(s) => {
                let slot = current.pop();
                if let Some(local) = current.locals.get_mut(*s as usize) {
                    *local = slot;
                }
            }

            Instruction::LoadField(_, _) => {
                current.pop();
                current.push_other();
            }

            Instruction::StoreField(_, _) => {
                current.pop(); // value only; slot is not consumed
            }

            Instruction::Add
            | Instruction::Sub
            | Instruction::Mul
            | Instruction::Div
            | Instruction::Mod
            | Instruction::Eq
            | Instruction::Ne
            | Instruction::Lt
            | Instruction::Le
            | Instruction::Gt
            | Instruction::Ge
            | Instruction::And
            | Instruction::Or => {
                current.pop();
                current.pop();
                current.push_other();
            }

            Instruction::Not => {
                current.pop();
                current.push_other();
            }

            Instruction::NewStruct {
                type_name,
                field_names,
            } => {
                let n = field_names.len();
                // Freshness check: object construction must use a fresh ID at the id field position.
                if object_types.contains(&type_name.as_str()) && n > 0 {
                    let id_idx = current.stack.len().saturating_sub(n);
                    if current.stack.get(id_idx).map(|s| &s.fresh) != Some(&Fresh::Id) {
                        errors.push(BytecodeVerifierError::ObjectIdNotFresh {
                            function: func.name.clone(),
                            pc,
                            object: type_name.clone(),
                        });
                    }
                }
                for _ in 0..n {
                    current.pop();
                }
                // NewStruct type names are always unqualified (language verifier enforces this).
                let tag = local_struct_tag(type_name, object_types);
                current.push(Slot::scalar(Fresh::Other, tag));
            }

            Instruction::GetField(_) => {
                current.pop();
                current.push_other();
            }

            Instruction::UnpackStruct {
                type_name,
                field_names,
            } => {
                current.pop();
                // Recover each unpacked field's struct-type tag from the struct
                // definition, matched by name, so a non-object struct field cannot
                // launder into `meow_vm_transfer` as an untracked (`Other`) value.
                // The language verifier forbids cross-module `UnpackStruct`, so the
                // type is always local.
                let def = module.get_struct(type_name);
                for fname in field_names {
                    let tag = def
                        .and_then(|d| d.fields.iter().find(|f| &f.name == fname))
                        .map(|f| match &f.ty {
                            Type::Struct(n) => local_struct_tag(n, object_types),
                            _ => StructTag::Other,
                        })
                        .unwrap_or(StructTag::Other);
                    current.push(Slot::scalar(Fresh::Other, tag));
                }
            }

            Instruction::Pop => {
                current.pop();
            }

            Instruction::Dup => {
                let slot = current.stack.last().cloned().unwrap_or_else(Slot::other);
                current.push(slot);
            }

            Instruction::Jump(offset) => {
                let target = jump_target(pc, *offset);
                enqueue(&mut pending, target, current.clone());
                current.reachable = false;
            }

            Instruction::JumpIf(offset) | Instruction::JumpIfNot(offset) => {
                current.pop(); // bool condition
                let target = jump_target(pc, *offset);
                enqueue(&mut pending, target, current.clone());
            }

            Instruction::Return => {
                current.pop();
                current.reachable = false;
            }

            Instruction::Call(name) => {
                let (n_pops, has_return) = call_effect(name, module, deps, natives);

                // Transfer-type check: meow_vm_transfer must receive an object-type struct.
                if name == config::NATIVE_FN_TRANSFER {
                    let struct_idx = current.stack.len().saturating_sub(n_pops);
                    if let Some(StructTag::NonObject(struct_name)) =
                        current.stack.get(struct_idx).map(|s| &s.tag)
                    {
                        errors.push(BytecodeVerifierError::TransferNonObjectStruct {
                            function: func.name.clone(),
                            pc,
                            struct_name: struct_name.clone(),
                        });
                    }
                }

                for _ in 0..n_pops {
                    current.pop();
                }

                if has_return {
                    let fresh = if name == config::NATIVE_FN_FRESH_ID {
                        Fresh::Id
                    } else {
                        Fresh::Other
                    };
                    current.push(return_slot(name, module, object_types, fresh));
                }
            }

            Instruction::MakeTuple(n) => {
                // Capture each element's struct-type tag so UnpackTuple can restore
                // it. Elements are popped top-first, so reverse to element order.
                let mut elems: Vec<StructTag> = (0..*n).map(|_| current.pop().tag).collect();
                elems.reverse();
                current.push(Slot {
                    fresh: Fresh::Other,
                    tag: StructTag::Other,
                    tuple: Some(elems),
                });
            }

            Instruction::UnpackTuple(n) => {
                let n = *n as usize;
                let slot = current.pop();
                match slot.tuple {
                    // Restore tracked element tags. The VM pushes element[0] last
                    // (top), so push in reverse element order to match.
                    Some(elems) if elems.len() == n => {
                        for tag in elems.into_iter().rev() {
                            current.push(Slot::scalar(Fresh::Other, tag));
                        }
                    }
                    _ => {
                        for _ in 0..n {
                            current.push_other();
                        }
                    }
                }
            }
        }
    }
}

/// Resolve a PC-relative branch offset to an absolute target PC.
fn jump_target(pc: usize, offset: i32) -> usize {
    (pc as isize + offset as isize) as usize
}

/// Record `state` as reaching `target`, merging it with any snapshot already
/// stashed there (two branches into the same target join conservatively).
fn enqueue(pending: &mut HashMap<usize, VerifyState>, target: usize, state: VerifyState) {
    pending
        .entry(target)
        .and_modify(|existing| existing.merge_from(&state))
        .or_insert(state);
}

/// Returns true if `def` is an on-chain object struct — i.e. its first field is
/// `id: meow_object::Id`. This is the single definition of object-ness the whole
/// verifier keys off (`id_type` is the bytecode-qualified `Id` type).
fn is_object_struct(def: &StructDef, id_type: &Type) -> bool {
    def.fields
        .first()
        .map(|f| f.name == MEOW_OBJECT_ID_FIELD_NAME && f.ty == *id_type)
        .unwrap_or(false)
}
