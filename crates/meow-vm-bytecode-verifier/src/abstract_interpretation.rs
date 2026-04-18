use std::collections::HashMap;

use meow_vm_types::{
    address::Address,
    bytecode::Instruction,
    module::{Function, Module},
    module_ref,
    types::Type,
};

use crate::{
    error::VerificationError,
    natives::{NativeParam, NativeSignature},
};

//
// ─── Abstract types ───
//

#[derive(Debug, Clone, PartialEq, Eq)]
enum AbstractType {
    Bool,
    U64,
    Address,
    Str,
    Struct(String),
    Object(String),
    /// Sentinel pushed by void-returning calls to keep the stack balanced.
    Void,
}

impl AbstractType {
    fn display_name(&self) -> String {
        match self {
            Self::Bool => "bool".to_string(),
            Self::U64 => "u64".to_string(),
            Self::Address => "address".to_string(),
            Self::Str => "str".to_string(),
            Self::Struct(n) => format!("struct({n})"),
            Self::Object(n) => format!("object({n})"),
            Self::Void => "void".to_string(),
        }
    }

    fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }
}

fn from_type(ty: &Type) -> AbstractType {
    match ty {
        Type::Bool => AbstractType::Bool,
        Type::U64 => AbstractType::U64,
        Type::Address => AbstractType::Address,
        Type::Str => AbstractType::Str,
        Type::Struct(n) => AbstractType::Struct(n.clone()),
        Type::Object(n) => AbstractType::Object(n.clone()),
    }
}

/// Like `from_type` but resolves `Type::Struct(name)` to `AbstractType::Object`
/// when the name refers to an `is_object` definition in `module`.
/// This is necessary because the compiler emits `Type::Struct` for all named
/// types in function signatures, regardless of whether they are object types.
fn from_type_resolved(ty: &Type, module: &Module) -> AbstractType {
    match ty {
        Type::Struct(name) => {
            if let Some(def) = module.get_struct(name)
                && def.is_object
            {
                return AbstractType::Object(name.clone());
            }
            AbstractType::Struct(name.clone())
        }
        other => from_type(other),
    }
}

/// Returns `true` if the type name contains `::`, indicating it refers to a
/// type from a dependency module. Field-level structural checks are skipped
/// for cross-module types because the dep-name → address mapping is not
/// stored in the bytecode.
fn is_cross_module_type(name: &str) -> bool {
    name.contains("::")
}

//
// ─── Abstract state ───
//

#[derive(Debug, Clone, PartialEq, Eq)]
enum SlotState {
    Live(AbstractType),
    Dead,
}

#[derive(Debug, Clone)]
struct AbstractState {
    stack: Vec<AbstractType>,
    locals: Vec<SlotState>,
    reachable: bool,
}

impl AbstractState {
    fn new(local_count: usize) -> Self {
        Self {
            stack: Vec::new(),
            locals: vec![SlotState::Dead; local_count],
            reachable: true,
        }
    }

    fn push(&mut self, ty: AbstractType) {
        self.stack.push(ty);
    }

    fn pop(&mut self) -> Option<AbstractType> {
        self.stack.pop()
    }

    fn peek(&self) -> Option<&AbstractType> {
        self.stack.last()
    }
}

//
// ─── State merge ───
//

fn merge(
    current: &AbstractState,
    incoming: &AbstractState,
    join_pc: usize,
    fn_name: &str,
    errors: &mut Vec<VerificationError>,
) -> Option<AbstractState> {
    if !incoming.reachable {
        return Some(current.clone());
    }
    if !current.reachable {
        return Some(incoming.clone());
    }

    if current.stack.len() != incoming.stack.len() {
        errors.push(VerificationError::StackMergeConflict {
            function: fn_name.to_string(),
            join_pc,
        });
        return None;
    }
    for (i, (a, b)) in current.stack.iter().zip(incoming.stack.iter()).enumerate() {
        if a != b {
            let _ = i;
            errors.push(VerificationError::StackMergeConflict {
                function: fn_name.to_string(),
                join_pc,
            });
            return None;
        }
    }
    for (slot, (a, b)) in current
        .locals
        .iter()
        .zip(incoming.locals.iter())
        .enumerate()
    {
        let a_obj = matches!(a, SlotState::Live(AbstractType::Object(_)));
        let b_obj = matches!(b, SlotState::Live(AbstractType::Object(_)));
        if a_obj != b_obj {
            errors.push(VerificationError::LivenessMergeConflict {
                function: fn_name.to_string(),
                join_pc,
                slot: slot as u8,
            });
            return None;
        }
    }

    Some(current.clone())
}

fn merge_at(
    pending: &mut HashMap<usize, AbstractState>,
    target: usize,
    incoming: AbstractState,
    fn_name: &str,
    errors: &mut Vec<VerificationError>,
) {
    use std::collections::hash_map::Entry;
    match pending.entry(target) {
        Entry::Vacant(e) => {
            e.insert(incoming);
        }
        Entry::Occupied(mut e) => {
            if let Some(merged) = merge(e.get(), &incoming, target, fn_name, errors) {
                e.insert(merged);
            }
        }
    }
}

//
// ─── Field resolution helpers ───
//

/// Resolve the field type for `field_name` on a value of `ty` from `module`.
/// Returns `None` for cross-module types (not resolvable) or unknown fields.
fn resolve_field_type(
    ty: &AbstractType,
    field_name: &str,
    module: &Module,
) -> Option<AbstractType> {
    let type_name = match ty {
        AbstractType::Struct(n) | AbstractType::Object(n) => n.as_str(),
        _ => return None,
    };
    if is_cross_module_type(type_name) {
        return None; // can't resolve without dep-name→address mapping
    }
    let def = module.get_struct(type_name)?;
    let field_def = def.fields.iter().find(|f| f.name == field_name)?;
    Some(from_type(&field_def.ty))
}

/// Check cross-module field read visibility.
/// Returns an error if the type is cross-module and the field is not public.
fn check_field_read_visibility(
    ty: &AbstractType,
    field_name: &str,
    _module: &Module,
    deps: &HashMap<Address, &Module>,
) -> Option<VerificationError> {
    let type_name = match ty {
        AbstractType::Struct(n) | AbstractType::Object(n) => n.as_str(),
        _ => return None,
    };
    if !is_cross_module_type(type_name) {
        return None; // same-module: always allowed
    }
    // Try to resolve via any dep that has this struct name after "::"
    let local_name = type_name.split("::").last()?;
    for dep_mod in deps.values() {
        if let Some(def) = dep_mod.get_struct(local_name) {
            if let Some(field_def) = def.fields.iter().find(|f| f.name == field_name)
                && !field_def.is_public
            {
                return Some(VerificationError::CrossModulePrivateFieldRead {
                    function: String::new(), // filled by caller
                    pc: 0,
                    type_name: type_name.to_string(),
                    field: field_name.to_string(),
                });
            }
            return None; // found struct, field is public or doesn't exist
        }
    }
    None
}

//
// ─── Type matching helpers ───
//

fn types_compatible(a: &AbstractType, b: &AbstractType) -> bool {
    match (a, b) {
        // Exact match
        _ if a == b => true,
        // Void is only compatible with Void
        _ => false,
    }
}

fn native_param_matches(param: &NativeParam, ty: &AbstractType) -> bool {
    match param {
        NativeParam::Concrete(t) => from_type(t) == *ty,
        NativeParam::AnyObject => ty.is_object(),
    }
}

fn native_param_display(param: &NativeParam) -> String {
    match param {
        NativeParam::Concrete(t) => from_type(t).display_name(),
        NativeParam::AnyObject => "any Object".to_string(),
    }
}

//
// ─── Main function checker ───
//

pub(crate) fn check_function(
    func: &Function,
    module: &Module,
    deps: &HashMap<Address, &Module>,
    natives: &[NativeSignature],
) -> Vec<VerificationError> {
    let mut errors = Vec::new();
    let fn_name = &func.name;

    // Initialise state: params fill the first N slots.
    // If local_count < params.len() the structural phase already reported the error;
    // bail early to avoid an out-of-bounds panic here.
    if (func.local_count as usize) < func.params.len() {
        return errors;
    }
    let mut state = AbstractState::new(func.local_count as usize);
    for (i, (_, ty)) in func.params.iter().enumerate() {
        state.locals[i] = SlotState::Live(from_type_resolved(ty, module));
    }

    // pending[pc] = incoming state arriving at instruction pc via a jump
    let mut pending: HashMap<usize, AbstractState> = HashMap::new();

    for (pc, instr) in func.code.iter().enumerate() {
        // ① Apply any pending merge at this pc.
        if let Some(incoming) = pending.remove(&pc) {
            match merge(&state, &incoming, pc, fn_name, &mut errors) {
                Some(merged) => state = merged,
                None => break, // merge error: abort this function
            }
        }

        if !state.reachable {
            continue;
        }

        match instr {
            // ── Literals ──────────────────────────────────────────────────
            Instruction::PushBool(_) => state.push(AbstractType::Bool),
            Instruction::PushU64(_) => state.push(AbstractType::U64),
            Instruction::PushAddress(_) => state.push(AbstractType::Address),
            Instruction::PushStr(_) => state.push(AbstractType::Str),

            // ── Local variable access ─────────────────────────────────────
            Instruction::Load(slot) => {
                let slot = *slot as usize;
                if slot >= state.locals.len() {
                    // structural phase catches this; skip here
                    continue;
                }
                match &state.locals[slot] {
                    SlotState::Dead => {
                        errors.push(VerificationError::UseAfterMove {
                            function: fn_name.clone(),
                            pc,
                            slot: slot as u8,
                        });
                    }
                    SlotState::Live(ty) => {
                        let ty = ty.clone();
                        if ty.is_object() {
                            state.locals[slot] = SlotState::Dead; // move semantics
                        }
                        state.push(ty);
                    }
                }
            }

            Instruction::Store(slot) => {
                let slot = *slot as usize;
                if slot >= state.locals.len() {
                    continue;
                }
                let val = match state.pop() {
                    Some(v) => v,
                    None => {
                        errors.push(VerificationError::StackUnderflow {
                            function: fn_name.clone(),
                            pc,
                            expected: "any".to_string(),
                        });
                        continue;
                    }
                };
                // Overwriting a live Object slot is a resource leak
                if matches!(&state.locals[slot], SlotState::Live(t) if t.is_object()) {
                    errors.push(VerificationError::ObjectSlotOverwrite {
                        function: fn_name.clone(),
                        pc,
                        slot: slot as u8,
                    });
                }
                state.locals[slot] = SlotState::Live(val);
            }

            Instruction::LoadField(slot, field_name) => {
                let slot = *slot as usize;
                if slot >= state.locals.len() {
                    continue;
                }
                let holder_ty = match &state.locals[slot] {
                    SlotState::Dead => {
                        errors.push(VerificationError::UseAfterMove {
                            function: fn_name.clone(),
                            pc,
                            slot: slot as u8,
                        });
                        continue;
                    }
                    SlotState::Live(ty) => ty.clone(),
                };
                match &holder_ty {
                    AbstractType::Struct(_) | AbstractType::Object(_) => {}
                    other => {
                        errors.push(VerificationError::TypeMismatch {
                            function: fn_name.clone(),
                            pc,
                            expected: "struct or object".to_string(),
                            found: other.display_name(),
                        });
                        continue;
                    }
                }
                // Visibility check for cross-module field reads
                if let Some(mut err) =
                    check_field_read_visibility(&holder_ty, field_name, module, deps)
                {
                    if let VerificationError::CrossModulePrivateFieldRead {
                        function,
                        pc: err_pc,
                        ..
                    } = &mut err
                    {
                        *function = fn_name.clone();
                        *err_pc = pc;
                    }
                    errors.push(err);
                }
                // Resolve field type; push Unknown (Address) if unresolvable
                let field_ty = resolve_field_type(&holder_ty, field_name, module)
                    .unwrap_or(AbstractType::Address); // conservative fallback
                state.push(field_ty);
            }

            Instruction::StoreField(slot, field_name) => {
                let slot = *slot as usize;
                if slot >= state.locals.len() {
                    continue;
                }
                let val = match state.pop() {
                    Some(v) => v,
                    None => {
                        errors.push(VerificationError::StackUnderflow {
                            function: fn_name.clone(),
                            pc,
                            expected: "value to store".to_string(),
                        });
                        continue;
                    }
                };
                let holder_ty = match &state.locals[slot] {
                    SlotState::Dead => {
                        errors.push(VerificationError::UseAfterMove {
                            function: fn_name.clone(),
                            pc,
                            slot: slot as u8,
                        });
                        continue;
                    }
                    SlotState::Live(ty) => ty.clone(),
                };
                match &holder_ty {
                    AbstractType::Struct(n) | AbstractType::Object(n) => {
                        if is_cross_module_type(n) {
                            errors.push(VerificationError::CrossModuleFieldWrite {
                                function: fn_name.clone(),
                                pc,
                                type_name: n.clone(),
                                field: field_name.clone(),
                            });
                        } else {
                            // Type-check: field value must match declared type
                            if let Some(expected_ty) =
                                resolve_field_type(&holder_ty, field_name, module)
                                && !types_compatible(&val, &expected_ty)
                            {
                                errors.push(VerificationError::TypeMismatch {
                                    function: fn_name.clone(),
                                    pc,
                                    expected: expected_ty.display_name(),
                                    found: val.display_name(),
                                });
                            }
                        }
                    }
                    other => {
                        errors.push(VerificationError::TypeMismatch {
                            function: fn_name.clone(),
                            pc,
                            expected: "struct or object".to_string(),
                            found: other.display_name(),
                        });
                    }
                }
                // Slot stays live — field updated in place
            }

            // ── Arithmetic ────────────────────────────────────────────────
            Instruction::Add
            | Instruction::Sub
            | Instruction::Mul
            | Instruction::Div
            | Instruction::Mod => {
                for side in ["right operand", "left operand"] {
                    match state.pop() {
                        None => {
                            errors.push(VerificationError::StackUnderflow {
                                function: fn_name.clone(),
                                pc,
                                expected: side.to_string(),
                            });
                        }
                        Some(ty) if ty != AbstractType::U64 => {
                            errors.push(VerificationError::TypeMismatch {
                                function: fn_name.clone(),
                                pc,
                                expected: "u64".to_string(),
                                found: ty.display_name(),
                            });
                        }
                        _ => {}
                    }
                }
                state.push(AbstractType::U64);
            }

            // ── Comparison ────────────────────────────────────────────────
            Instruction::Eq | Instruction::Ne => {
                let r = state.pop();
                let l = state.pop();
                match (l, r) {
                    (None, _) | (_, None) => {
                        errors.push(VerificationError::StackUnderflow {
                            function: fn_name.clone(),
                            pc,
                            expected: "two comparable values".to_string(),
                        });
                    }
                    (Some(lt), Some(rt)) if lt != rt => {
                        errors.push(VerificationError::TypeMismatch {
                            function: fn_name.clone(),
                            pc,
                            expected: lt.display_name(),
                            found: rt.display_name(),
                        });
                    }
                    _ => {}
                }
                state.push(AbstractType::Bool);
            }

            Instruction::Lt | Instruction::Le | Instruction::Gt | Instruction::Ge => {
                for side in ["right operand", "left operand"] {
                    match state.pop() {
                        None => {
                            errors.push(VerificationError::StackUnderflow {
                                function: fn_name.clone(),
                                pc,
                                expected: side.to_string(),
                            });
                        }
                        Some(ty) if ty != AbstractType::U64 => {
                            errors.push(VerificationError::TypeMismatch {
                                function: fn_name.clone(),
                                pc,
                                expected: "u64".to_string(),
                                found: ty.display_name(),
                            });
                        }
                        _ => {}
                    }
                }
                state.push(AbstractType::Bool);
            }

            // ── Boolean logic ─────────────────────────────────────────────
            Instruction::Not => {
                match state.pop() {
                    None => errors.push(VerificationError::StackUnderflow {
                        function: fn_name.clone(),
                        pc,
                        expected: "bool".to_string(),
                    }),
                    Some(ty) if ty != AbstractType::Bool => {
                        errors.push(VerificationError::TypeMismatch {
                            function: fn_name.clone(),
                            pc,
                            expected: "bool".to_string(),
                            found: ty.display_name(),
                        });
                    }
                    _ => {}
                }
                state.push(AbstractType::Bool);
            }

            Instruction::And | Instruction::Or => {
                for side in ["right operand", "left operand"] {
                    match state.pop() {
                        None => {
                            errors.push(VerificationError::StackUnderflow {
                                function: fn_name.clone(),
                                pc,
                                expected: side.to_string(),
                            });
                        }
                        Some(ty) if ty != AbstractType::Bool => {
                            errors.push(VerificationError::TypeMismatch {
                                function: fn_name.clone(),
                                pc,
                                expected: "bool".to_string(),
                                found: ty.display_name(),
                            });
                        }
                        _ => {}
                    }
                }
                state.push(AbstractType::Bool);
            }

            // ── Struct / object construction ──────────────────────────────
            Instruction::NewStruct {
                type_name,
                field_names,
            } => {
                // Cross-module construction already flagged in structural phase;
                // just skip here to avoid cascading errors.
                if module_ref::parse_module_ref(type_name).is_some() {
                    // pop the fields anyway to keep stack consistent
                    for _ in field_names {
                        state.pop();
                    }
                    continue;
                }
                if let Some(def) = module.get_struct(type_name) {
                    // Pop fields in reverse (last field is on top of stack)
                    for field_def in def.fields.iter().rev() {
                        match state.pop() {
                            None => {
                                errors.push(VerificationError::StackUnderflow {
                                    function: fn_name.clone(),
                                    pc,
                                    expected: format!("field '{}'", field_def.name),
                                });
                            }
                            Some(ty) => {
                                let expected = from_type(&field_def.ty);
                                if !types_compatible(&ty, &expected) {
                                    errors.push(VerificationError::TypeMismatch {
                                        function: fn_name.clone(),
                                        pc,
                                        expected: expected.display_name(),
                                        found: ty.display_name(),
                                    });
                                }
                            }
                        }
                    }
                    let result_ty = if def.is_object {
                        AbstractType::Object(type_name.clone())
                    } else {
                        AbstractType::Struct(type_name.clone())
                    };
                    state.push(result_ty);
                } else {
                    // Unknown type — pop placeholders and push Unknown
                    for _ in field_names {
                        state.pop();
                    }
                    state.push(AbstractType::Struct(type_name.clone()));
                }
            }

            Instruction::GetField(field_name) => {
                match state.pop() {
                    None => {
                        errors.push(VerificationError::StackUnderflow {
                            function: fn_name.clone(),
                            pc,
                            expected: "struct or object".to_string(),
                        });
                    }
                    Some(ty) => match &ty {
                        AbstractType::Struct(_) | AbstractType::Object(_) => {
                            let field_ty = resolve_field_type(&ty, field_name, module)
                                .unwrap_or(AbstractType::Address);
                            state.push(field_ty);
                        }
                        other => {
                            errors.push(VerificationError::TypeMismatch {
                                function: fn_name.clone(),
                                pc,
                                expected: "struct or object".to_string(),
                                found: other.display_name(),
                            });
                            state.push(AbstractType::Address); // push placeholder
                        }
                    },
                }
            }

            // ── Stack manipulation ────────────────────────────────────────
            Instruction::Pop => match state.pop() {
                None => errors.push(VerificationError::StackUnderflow {
                    function: fn_name.clone(),
                    pc,
                    expected: "any".to_string(),
                }),
                Some(ty) if ty.is_object() => {
                    errors.push(VerificationError::PopOnObject {
                        function: fn_name.clone(),
                        pc,
                    });
                }
                _ => {}
            },

            Instruction::Dup => match state.peek().cloned() {
                None => errors.push(VerificationError::StackUnderflow {
                    function: fn_name.clone(),
                    pc,
                    expected: "any".to_string(),
                }),
                Some(ty) if ty.is_object() => {
                    errors.push(VerificationError::DupOnObject {
                        function: fn_name.clone(),
                        pc,
                    });
                }
                Some(ty) => state.push(ty),
            },

            // ── Control flow ──────────────────────────────────────────────
            Instruction::Jump(offset) => {
                if *offset > 0 {
                    let target = (pc as i64 + *offset as i64) as usize;
                    merge_at(&mut pending, target, state.clone(), fn_name, &mut errors);
                }
                state.reachable = false;
            }

            Instruction::JumpIf(offset) => {
                match state.pop() {
                    None => errors.push(VerificationError::StackUnderflow {
                        function: fn_name.clone(),
                        pc,
                        expected: "bool".to_string(),
                    }),
                    Some(ty) if ty != AbstractType::Bool => {
                        errors.push(VerificationError::TypeMismatch {
                            function: fn_name.clone(),
                            pc,
                            expected: "bool".to_string(),
                            found: ty.display_name(),
                        });
                    }
                    _ => {}
                }
                if *offset > 0 {
                    let target = (pc as i64 + *offset as i64) as usize;
                    merge_at(&mut pending, target, state.clone(), fn_name, &mut errors);
                }
            }

            Instruction::JumpIfNot(offset) => {
                match state.pop() {
                    None => errors.push(VerificationError::StackUnderflow {
                        function: fn_name.clone(),
                        pc,
                        expected: "bool".to_string(),
                    }),
                    Some(ty) if ty != AbstractType::Bool => {
                        errors.push(VerificationError::TypeMismatch {
                            function: fn_name.clone(),
                            pc,
                            expected: "bool".to_string(),
                            found: ty.display_name(),
                        });
                    }
                    _ => {}
                }
                if *offset > 0 {
                    let target = (pc as i64 + *offset as i64) as usize;
                    merge_at(&mut pending, target, state.clone(), fn_name, &mut errors);
                }
            }

            // ── Function calls ────────────────────────────────────────────
            Instruction::Call(name) => {
                check_call(
                    name,
                    pc,
                    fn_name,
                    module,
                    deps,
                    natives,
                    &mut state,
                    &mut errors,
                );
            }

            // ── Return ────────────────────────────────────────────────────
            Instruction::Return => {
                check_return(func, pc, &state, &mut errors);
                state.reachable = false;
            }
        }
    }

    // If execution falls off the end without a Return — two cases:
    // 1. Fall-through: the last instruction wasn't a Return/Jump.
    // 2. Jump to code_len: a reachable Jump(offset) landed at past-the-end,
    //    storing a state in pending[code_len] that the loop never consumed.
    //    (Compiler-generated dead-code jumps are unreachable so they never
    //    populate pending[code_len]; this only fires for tampered bytecode.)
    let jump_to_end = pending.remove(&func.code.len());
    if state.reachable || jump_to_end.is_some() {
        errors.push(VerificationError::MissingReturn {
            function: fn_name.clone(),
        });
        // Also check for unconsumed objects on the escaped path.
        if let Some(escaped) = jump_to_end {
            for (slot, local) in escaped.locals.iter().enumerate() {
                if matches!(local, SlotState::Live(t) if t.is_object()) {
                    errors.push(VerificationError::UnconsumedObject {
                        function: fn_name.clone(),
                        slot: slot as u8,
                    });
                }
            }
        }
    }

    errors
}

//
// ─── Call helper ───
//

#[allow(clippy::too_many_arguments)]
fn check_call(
    name: &str,
    pc: usize,
    fn_name: &str,
    module: &Module,
    deps: &HashMap<Address, &Module>,
    natives: &[NativeSignature],
    state: &mut AbstractState,
    errors: &mut Vec<VerificationError>,
) {
    // Cross-module call: @0xHEX::fn_name
    if let Some((dep_addr, callee_name)) = module_ref::parse_module_ref(name) {
        if let Some(dep_mod) = deps.get(&dep_addr)
            && let Some(callee) = dep_mod.get_function(callee_name)
        {
            pop_and_check_params(&callee.params, name, pc, fn_name, module, state, errors);
            push_return_type(callee.return_type.as_ref(), state);
            return;
        }
        errors.push(VerificationError::UndefinedFunction {
            function: fn_name.to_string(),
            pc,
            callee: name.to_string(),
        });
        return;
    }

    // Same-module call
    // We need to find the function by name. Avoid borrowing `module.functions`
    // through a method to work around borrow conflicts; use `get_function`.
    if let Some(callee) = module.get_function(name) {
        // Clone what we need to avoid borrow of module while mutating state
        let params: Vec<(String, Type)> = callee.params.clone();
        let return_type: Option<Type> = callee.return_type.clone();
        pop_and_check_params(&params, name, pc, fn_name, module, state, errors);
        push_return_type(return_type.as_ref(), state);
        return;
    }

    // Native call
    if let Some(native) = natives.iter().find(|n| n.name == name) {
        let expected = native.params.len();
        // Collect the top N stack items (in reverse, since last pushed = first arg consumed)
        if state.stack.len() < expected {
            errors.push(VerificationError::NativeArgCountMismatch {
                function: fn_name.to_string(),
                pc,
                callee: name.to_string(),
                expected,
                found: state.stack.len(),
            });
            // Pop what's available
            for _ in 0..state.stack.len() {
                state.pop();
            }
        } else {
            // Args on stack in order: leftmost arg was pushed first (bottom),
            // rightmost arg is on top. Pop rightmost first.
            let mut args: Vec<AbstractType> = Vec::with_capacity(expected);
            for _ in 0..expected {
                args.push(state.pop().unwrap());
            }
            args.reverse(); // now args[0] = leftmost

            for (i, (param, ty)) in native.params.iter().zip(args.iter()).enumerate() {
                if !native_param_matches(param, ty) {
                    errors.push(VerificationError::NativeArgTypeMismatch {
                        function: fn_name.to_string(),
                        pc,
                        callee: name.to_string(),
                        arg_index: i,
                        expected: native_param_display(param),
                        found: ty.display_name(),
                    });
                }
            }
        }
        push_return_type(native.return_type.as_ref(), state);
        return;
    }

    errors.push(VerificationError::UndefinedFunction {
        function: fn_name.to_string(),
        pc,
        callee: name.to_string(),
    });
}

fn pop_and_check_params(
    params: &[(String, Type)],
    callee: &str,
    pc: usize,
    fn_name: &str,
    module: &Module,
    state: &mut AbstractState,
    errors: &mut Vec<VerificationError>,
) {
    if state.stack.len() < params.len() {
        errors.push(VerificationError::StackUnderflow {
            function: fn_name.to_string(),
            pc,
            expected: format!("{} args for '{}'", params.len(), callee),
        });
        for _ in 0..state.stack.len() {
            state.pop();
        }
        return;
    }
    let mut args: Vec<AbstractType> = Vec::with_capacity(params.len());
    for _ in 0..params.len() {
        args.push(state.pop().unwrap());
    }
    args.reverse();

    for ((_, expected_ty), actual) in params.iter().zip(args.iter()) {
        let expected = from_type_resolved(expected_ty, module);
        if !types_compatible(actual, &expected) {
            errors.push(VerificationError::TypeMismatch {
                function: fn_name.to_string(),
                pc,
                expected: expected.display_name(),
                found: actual.display_name(),
            });
        }
    }
}

fn push_return_type(return_type: Option<&Type>, state: &mut AbstractState) {
    match return_type {
        Some(ty) => state.push(from_type(ty)),
        None => state.push(AbstractType::Void),
    }
}

//
// ─── Return helper ───
//

fn check_return(
    func: &Function,
    _pc: usize,
    state: &AbstractState,
    errors: &mut Vec<VerificationError>,
) {
    let fn_name = &func.name;

    match &func.return_type {
        None => {
            // Void function: stack should be empty or have exactly one Void
            match state.stack.last() {
                None => {}                     // OK
                Some(AbstractType::Void) => {} // OK: result of void call
                Some(other) => {
                    errors.push(VerificationError::ReturnTypeMismatch {
                        function: fn_name.clone(),
                        declared: "void".to_string(),
                        found: other.display_name(),
                    });
                }
            }
        }
        Some(declared) => {
            let expected = from_type(declared);
            match state.stack.last() {
                None => {
                    errors.push(VerificationError::ReturnTypeMismatch {
                        function: fn_name.clone(),
                        declared: expected.display_name(),
                        found: "empty stack".to_string(),
                    });
                }
                Some(actual) if !types_compatible(actual, &expected) => {
                    errors.push(VerificationError::ReturnTypeMismatch {
                        function: fn_name.clone(),
                        declared: expected.display_name(),
                        found: actual.display_name(),
                    });
                }
                _ => {}
            }
        }
    }

    // Unconsumed objects in local slots
    for (slot, local) in state.locals.iter().enumerate() {
        if matches!(local, SlotState::Live(t) if t.is_object()) {
            errors.push(VerificationError::UnconsumedObject {
                function: fn_name.clone(),
                slot: slot as u8,
            });
        }
    }
}
