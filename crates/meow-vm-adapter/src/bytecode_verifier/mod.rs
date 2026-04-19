pub mod error;

use std::collections::{HashMap, HashSet};

use meow_types::system_framework::meow_object::{
    MEOW_OBJECT_ID_BYTECODE_TYPE_NAME, MEOW_OBJECT_ID_FIELD_NAME,
};
use meow_vm_bytecode_verifier::natives::{NativeSignature, builtin_natives};
use meow_vm_types::address::Address;
use meow_vm_types::{
    bytecode::Instruction,
    module::{Function, Module},
    module_ref::parse_module_ref,
    types::Type,
};

use crate::bytecode_verifier::error::BytecodeVerifierError;

//
// ─── Public API ───
//

/// Run adapter-level bytecode verification on `module`.
///
/// Checks rules that are specific to the chain/adapter convention and are
/// intentionally outside the language-level [`meow_vm_bytecode_verifier`]:
///
/// 1. **Object layout**: every `object` type must have `id: @0x...::Id` as its
///    first field.
/// 2. **ID freshness**: every object construction via `NewStruct` must use a
///    value that originates directly from `meow_vm_fresh_id` in the same
///    function (not from a parameter, local variable seeded elsewhere, or
///    cross-module call).
///
/// The language-level verifier must pass before calling this function.
pub fn verify(
    module: &Module,
    deps: &HashMap<Address, &Module>,
    adapter_natives: &[NativeSignature],
) -> Result<(), Vec<BytecodeVerifierError>> {
    let mut errors = Vec::new();

    check_object_layouts(module, deps, &mut errors);

    let id_type = Type::Struct(MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string());
    let object_types: Vec<&str> = module
        .structs
        .iter()
        .filter(|s| {
            s.fields
                .first()
                .map(|f| f.name == MEOW_OBJECT_ID_FIELD_NAME && f.ty == id_type)
                .unwrap_or(false)
        })
        .map(|s| s.name.as_str())
        .collect();

    if !object_types.is_empty() {
        let builtins = builtin_natives();
        let all_natives: Vec<&NativeSignature> =
            builtins.iter().chain(adapter_natives.iter()).collect();

        for func in &module.functions {
            check_id_freshness(func, module, deps, &all_natives, &object_types, &mut errors);
            check_id_field_mutations(func, module, deps, &all_natives, &object_types, &mut errors);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

//
// ─── Phase 1: Object layout ───
//

fn check_object_layouts(
    module: &Module,
    deps: &HashMap<Address, &Module>,
    errors: &mut Vec<BytecodeVerifierError>,
) {
    let id_type = Type::Struct(MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string());

    // Collect local object-shaped struct names (first field is `id: @0x1::Id`).
    let local_objects: HashSet<&str> = module
        .structs
        .iter()
        .filter(|s| {
            s.fields
                .first()
                .map(|f| f.name == MEOW_OBJECT_ID_FIELD_NAME && f.ty == id_type)
                .unwrap_or(false)
        })
        .map(|s| s.name.as_str())
        .collect();

    for s in &module.structs {
        // Rule 2: no struct field may have an object type.
        for field in &s.fields {
            let Type::Struct(field_type_name) = &field.ty else {
                continue;
            };
            let is_obj = if let Some((dep_addr, type_name)) = parse_module_ref(field_type_name) {
                deps.get(&dep_addr)
                    .and_then(|m| m.get_struct(type_name))
                    .map(|def| {
                        def.fields
                            .first()
                            .map(|f| f.name == MEOW_OBJECT_ID_FIELD_NAME && f.ty == id_type)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false)
            } else {
                local_objects.contains(field_type_name.as_str())
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
// ─── Phase 2: ID freshness ───
//

/// Freshness tag for abstract stack/local values.
#[derive(Clone, PartialEq)]
enum Fresh {
    /// Originated directly from `Call("meow_vm_fresh_id")`.
    Id,
    /// Any other value.
    Other,
}

#[derive(Clone)]
struct State {
    stack: Vec<Fresh>,
    locals: Vec<Fresh>,
    reachable: bool,
}

impl State {
    fn new(local_count: usize) -> Self {
        Self {
            stack: Vec::new(),
            locals: vec![Fresh::Other; local_count],
            reachable: true,
        }
    }

    /// Conservative merge: a slot is `Fresh::Id` only if it is `Fresh::Id` on
    /// both paths.
    fn merge_from(&mut self, other: &State) {
        let stack_len = self.stack.len().min(other.stack.len());
        self.stack.truncate(stack_len);
        for (i, v) in self.stack.iter_mut().enumerate() {
            if other.stack[i] != Fresh::Id {
                *v = Fresh::Other;
            }
        }
        for (i, v) in self.locals.iter_mut().enumerate() {
            if other.locals.get(i) != Some(&Fresh::Id) {
                *v = Fresh::Other;
            }
        }
    }
}

/// Returns `(n_pops, pushes_return)` for a call to `name`.
fn call_effect(
    name: &str,
    module: &Module,
    deps: &HashMap<Address, &Module>,
    natives: &[&NativeSignature],
) -> (usize, bool) {
    // Cross-module call: `@hex::fn`
    if let Some((dep_addr, fn_name)) = parse_module_ref(name) {
        if let Some(func) = deps.get(&dep_addr).and_then(|m| m.get_function(fn_name)) {
            return (func.params.len(), func.return_type.is_some());
        }
        return (0, false);
    }

    // Native function
    if let Some(sig) = natives.iter().find(|s| s.name == name) {
        return (sig.params.len(), sig.return_type.is_some());
    }

    // Local module function
    if let Some(func) = module.get_function(name) {
        return (func.params.len(), func.return_type.is_some());
    }

    (0, false)
}

fn check_id_freshness(
    func: &Function,
    module: &Module,
    deps: &HashMap<Address, &Module>,
    natives: &[&NativeSignature],
    object_types: &[&str],
    errors: &mut Vec<BytecodeVerifierError>,
) {
    let mut current = State::new(func.local_count as usize);
    let mut pending: HashMap<usize, State> = HashMap::new();

    for (pc, instr) in func.code.iter().enumerate() {
        if let Some(incoming) = pending.remove(&pc) {
            if current.reachable {
                current.merge_from(&incoming);
            } else {
                current = incoming;
            }
        }

        if !current.reachable {
            continue;
        }

        match instr {
            Instruction::PushBool(_)
            | Instruction::PushU64(_)
            | Instruction::PushAddress(_)
            | Instruction::PushStr(_) => {
                current.stack.push(Fresh::Other);
            }

            Instruction::Load(s) => {
                let v = current
                    .locals
                    .get(*s as usize)
                    .cloned()
                    .unwrap_or(Fresh::Other);
                current.stack.push(v);
            }

            Instruction::Store(s) => {
                let v = current.stack.pop().unwrap_or(Fresh::Other);
                if let Some(slot) = current.locals.get_mut(*s as usize) {
                    *slot = v;
                }
            }

            Instruction::LoadField(_, _) => {
                current.stack.pop();
                current.stack.push(Fresh::Other);
            }

            Instruction::StoreField(_, _) => {
                current.stack.pop(); // value; slot is not consumed
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
                current.stack.pop();
                current.stack.pop();
                current.stack.push(Fresh::Other);
            }

            Instruction::Not => {
                current.stack.pop();
                current.stack.push(Fresh::Other);
            }

            Instruction::NewStruct {
                type_name,
                field_names,
            } => {
                let n = field_names.len();
                if object_types.contains(&type_name.as_str()) && n > 0 {
                    // `id` is field[0]; fields are pushed in definition order so `id` is
                    // deepest in the N-slot window — at index `stack.len() - N` from bottom.
                    let id_idx = current.stack.len().saturating_sub(n);
                    if current.stack.get(id_idx) != Some(&Fresh::Id) {
                        errors.push(BytecodeVerifierError::ObjectIdNotFresh {
                            function: func.name.clone(),
                            pc,
                            object: type_name.clone(),
                        });
                    }
                }
                for _ in 0..n {
                    current.stack.pop();
                }
                current.stack.push(Fresh::Other);
            }

            Instruction::GetField(_) => {
                current.stack.pop();
                current.stack.push(Fresh::Other);
            }

            Instruction::UnpackStruct { field_names, .. } => {
                current.stack.pop();
                for _ in field_names {
                    current.stack.push(Fresh::Other);
                }
            }

            Instruction::Pop => {
                current.stack.pop();
            }

            Instruction::Dup => {
                let v = current.stack.last().cloned().unwrap_or(Fresh::Other);
                current.stack.push(v);
            }

            Instruction::Jump(offset) => {
                let target = jump_target(pc, *offset);
                enqueue(&mut pending, target, current.clone());
                current.reachable = false;
            }

            Instruction::JumpIf(offset) | Instruction::JumpIfNot(offset) => {
                current.stack.pop(); // bool condition
                let target = jump_target(pc, *offset);
                enqueue(&mut pending, target, current.clone());
                // fall-through continues with current state
            }

            Instruction::Return => {
                current.stack.pop();
                current.reachable = false;
            }

            Instruction::Call(name) => {
                let (n_pops, has_return) = call_effect(name, module, deps, natives);
                for _ in 0..n_pops {
                    current.stack.pop();
                }
                if name == "meow_vm_fresh_id" {
                    current.stack.push(Fresh::Id);
                } else if has_return {
                    current.stack.push(Fresh::Other);
                }
            }

            Instruction::MakeTuple(n) => {
                for _ in 0..*n {
                    current.stack.pop();
                }
                current.stack.push(Fresh::Other);
            }

            Instruction::UnpackTuple(n) => {
                current.stack.pop();
                for _ in 0..*n {
                    current.stack.push(Fresh::Other);
                }
            }
        }
    }
}

fn jump_target(pc: usize, offset: i32) -> usize {
    (pc as isize + offset as isize) as usize
}

fn enqueue(pending: &mut HashMap<usize, State>, target: usize, state: State) {
    pending
        .entry(target)
        .and_modify(|existing| existing.merge_from(&state))
        .or_insert(state);
}

//
// ─── Phase 3: Object id field immutability ───
//

/// Returns the object struct name if `ty` resolves to an object-shaped struct, or `None`.
fn obj_name_for_type<'a>(
    ty: &'a Type,
    module: &'a Module,
    deps: &'a HashMap<Address, &Module>,
    id_type: &Type,
) -> Option<&'a str> {
    let Type::Struct(name) = ty else { return None };
    if let Some((dep_addr, type_name)) = parse_module_ref(name) {
        let def = deps.get(&dep_addr).and_then(|m| m.get_struct(type_name))?;
        if def
            .fields
            .first()
            .map(|f| f.name == MEOW_OBJECT_ID_FIELD_NAME && f.ty == *id_type)
            .unwrap_or(false)
        {
            return Some(type_name);
        }
    } else if let Some(def) = module.get_struct(name)
        && def
            .fields
            .first()
            .map(|f| f.name == MEOW_OBJECT_ID_FIELD_NAME && f.ty == *id_type)
            .unwrap_or(false)
    {
        return Some(name.as_str());
    }

    None
}

fn call_return_obj_name<'a>(
    name: &str,
    module: &'a Module,
    deps: &'a HashMap<Address, &Module>,
    id_type: &Type,
) -> Option<String> {
    let ret_ty = if let Some((dep_addr, fn_name)) = parse_module_ref(name) {
        deps.get(&dep_addr)
            .and_then(|m| m.get_function(fn_name))
            .and_then(|f| f.return_type.clone())
    } else {
        module
            .get_function(name)
            .and_then(|f| f.return_type.clone())
    };
    let Type::Struct(type_name) = ret_ty? else {
        return None;
    };
    // Use obj_name_for_type by reconstructing the type
    let reconstructed = Type::Struct(type_name);
    obj_name_for_type(&reconstructed, module, deps, id_type).map(|s| s.to_string())
}

fn merge_obj_slots(curr: &mut Vec<Option<String>>, incoming: &[Option<String>]) {
    let len = curr.len().min(incoming.len());
    curr.truncate(len);
    for (a, b) in curr.iter_mut().zip(incoming.iter()) {
        if a != b {
            *a = None;
        }
    }
}

#[allow(clippy::type_complexity)]
fn enqueue_obj(
    pending: &mut HashMap<usize, (Vec<Option<String>>, Vec<Option<String>>)>,
    target: usize,
    stack: &[Option<String>],
    locals: &[Option<String>],
) {
    match pending.remove(&target) {
        Some((mut ps, mut pl)) => {
            merge_obj_slots(&mut ps, stack);
            merge_obj_slots(&mut pl, locals);
            pending.insert(target, (ps, pl));
        }
        None => {
            pending.insert(target, (stack.to_vec(), locals.to_vec()));
        }
    }
}

/// Reject any `StoreField(slot, "id")` where `slot` holds an object-shaped struct.
fn check_id_field_mutations(
    func: &Function,
    module: &Module,
    deps: &HashMap<Address, &Module>,
    natives: &[&NativeSignature],
    object_types: &[&str],
    errors: &mut Vec<BytecodeVerifierError>,
) {
    let id_type = Type::Struct(MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string());

    // locals[i] = Some(name) means slot holds object-shaped struct `name`; None = not object.
    let init_locals: Vec<Option<String>> = (0..func.local_count as usize)
        .map(|i| {
            func.params
                .get(i)
                .and_then(|(_, ty)| obj_name_for_type(ty, module, deps, &id_type))
                .map(|s| s.to_string())
        })
        .collect();

    let mut stack: Vec<Option<String>> = Vec::new();
    let mut locals = init_locals;
    let mut reachable = true;
    #[allow(clippy::type_complexity)]
    let mut pending: HashMap<usize, (Vec<Option<String>>, Vec<Option<String>>)> = HashMap::new();

    for (pc, instr) in func.code.iter().enumerate() {
        if let Some((inc_stack, inc_locals)) = pending.remove(&pc) {
            if reachable {
                merge_obj_slots(&mut stack, &inc_stack);
                merge_obj_slots(&mut locals, &inc_locals);
            } else {
                stack = inc_stack;
                locals = inc_locals;
                reachable = true;
            }
        }
        if !reachable {
            continue;
        }

        match instr {
            Instruction::PushBool(_)
            | Instruction::PushU64(_)
            | Instruction::PushAddress(_)
            | Instruction::PushStr(_) => {
                stack.push(None);
            }

            Instruction::Load(s) => {
                stack.push(locals.get(*s as usize).cloned().unwrap_or(None));
            }

            Instruction::Store(s) => {
                let v = stack.pop().unwrap_or(None);
                if let Some(slot) = locals.get_mut(*s as usize) {
                    *slot = v;
                }
            }

            Instruction::LoadField(_, _) => {
                stack.pop();
                stack.push(None); // field types cannot be object-shaped (Rule 2)
            }

            Instruction::StoreField(s, field_path) => {
                stack.pop(); // value being written
                if field_path
                    .first()
                    .map(|f| f == MEOW_OBJECT_ID_FIELD_NAME)
                    .unwrap_or(false)
                    && let Some(Some(obj_name)) = locals.get(*s as usize)
                {
                    errors.push(BytecodeVerifierError::ObjectIdFieldMutated {
                        function: func.name.clone(),
                        pc,
                        object: obj_name.clone(),
                    });
                }
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
                stack.pop();
                stack.pop();
                stack.push(None);
            }

            Instruction::Not => {
                stack.pop();
                stack.push(None);
            }

            Instruction::NewStruct {
                type_name,
                field_names,
            } => {
                for _ in field_names {
                    stack.pop();
                }
                stack.push(if object_types.contains(&type_name.as_str()) {
                    Some(type_name.clone())
                } else {
                    None
                });
            }

            Instruction::GetField(_) => {
                stack.pop();
                stack.push(None);
            }

            Instruction::UnpackStruct { field_names, .. } => {
                stack.pop();
                for _ in field_names {
                    stack.push(None);
                }
            }

            Instruction::Pop => {
                stack.pop();
            }

            Instruction::Dup => {
                let v = stack.last().cloned().unwrap_or(None);
                stack.push(v);
            }

            Instruction::Jump(offset) => {
                let target = jump_target(pc, *offset);
                enqueue_obj(&mut pending, target, &stack, &locals);
                reachable = false;
            }

            Instruction::JumpIf(offset) | Instruction::JumpIfNot(offset) => {
                stack.pop(); // bool condition
                let target = jump_target(pc, *offset);
                enqueue_obj(&mut pending, target, &stack, &locals);
            }

            Instruction::Return => {
                stack.pop();
                reachable = false;
            }

            Instruction::Call(name) => {
                let (n_pops, has_return) = call_effect(name, module, deps, natives);
                for _ in 0..n_pops {
                    stack.pop();
                }
                if has_return {
                    stack.push(call_return_obj_name(name, module, deps, &id_type));
                }
            }

            Instruction::MakeTuple(n) => {
                for _ in 0..*n {
                    stack.pop();
                }
                stack.push(None);
            }

            Instruction::UnpackTuple(n) => {
                stack.pop();
                for _ in 0..*n {
                    stack.push(None);
                }
            }
        }
    }
}
