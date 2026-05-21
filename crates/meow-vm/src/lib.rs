//! # meow-vm
//!
//! A stack-based virtual machine with move semantics for objects, native
//! function support, and per-instruction gas metering.
//!
//! ## Types
//! - Primitives: `bool`, `u64`, `address` ([u8; 32]) — freely copyable
//! - `struct` — user-defined, move semantics; must be explicitly consumed, passed on, or returned
//!
//! ## Quick start
//!
//! ```rust
//! use std::collections::HashMap;
//!
//! use meow_vm_compiler::Compiler;
//! use meow_vm::Vm;
//! use meow_vm::gas_meter::GasMeter;
//! use meow_vm::gas_schedule::GasSchedule;
//! use meow_vm_types::{address::Address, config::{CompilerConfig, VmConfig}, types::Value};
//!
//! let source = r#"
//!     mod math;
//!
//!     pub fn add(a: u64, b: u64) -> u64 {
//!         return a + b;
//!     }
//! "#;
//!
//! let module = Compiler::compile(source, &[], &[], CompilerConfig::default()).unwrap();
//! let vm = Vm::new((Address::ZERO, module), vec![], GasSchedule::default(), HashMap::new(), VmConfig::default());
//! let mut gas = GasMeter::new(1_000);
//!
//! let result = vm.call("add", vec![Value::U64(3), Value::U64(4)], &mut gas).unwrap();
//! assert_eq!(result.return_value, Some(Value::U64(7)));
//! println!("gas spent: {}", gas.spent());
//! ```

pub mod error;
pub mod gas_meter;
pub mod gas_schedule;

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use meow_vm_types::module_ref;
use meow_vm_types::natives::{NativeFnEntry, NativeResult};
use meow_vm_types::{address::Address, config::VmConfig, types::Value};

use meow_vm_types::{bytecode::Instruction, module::Module};

use crate::error::VmError;
use crate::gas_meter::GasMeter;
use crate::gas_schedule::GasSchedule;

/// An error that can occur during VM execution.
pub type Result<T> = std::result::Result<T, VmError>;

//
// ─── Execution result ───
//

/// The outcome of a top-level VM function call.
#[derive(Debug, Clone)]
pub struct VmCallResult {
    /// Value returned by the function, if any.
    pub return_value: Option<Value>,
}

//
// ─── Execution frame ───
//

struct Frame {
    /// Local variable slots. `None` means uninitialized or moved-out.
    locals: Vec<Option<Value>>,
    /// Operand stack.
    stack: Vec<Value>,
    /// Program counter (index into `code`).
    pc: usize,
    /// The bytecode being executed.
    code: Vec<Instruction>,
}

impl Frame {
    fn new(code: Vec<Instruction>, args: Vec<Value>, local_count: u8) -> Self {
        let mut locals: Vec<Option<Value>> = (0..local_count as usize).map(|_| None).collect();
        for (i, v) in args.into_iter().enumerate() {
            locals[i] = Some(v);
        }
        Self {
            locals,
            stack: Vec::new(),
            pc: 0,
            code,
        }
    }

    fn push(&mut self, v: Value) {
        self.stack.push(v);
    }

    fn pop(&mut self) -> Result<Value> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    fn peek(&self) -> Result<&Value> {
        self.stack.last().ok_or(VmError::StackUnderflow)
    }
}

//
// ─── VM ───
//

/// The virtual machine executor.
///
/// Create a `Vm` from a [`Module`] and a list of native functions, then call
/// functions by name. For modules with cross-module dependencies, supply dep
/// modules as a `HashMap<Address, Module>` via [`Vm::new`].
pub struct Vm {
    module: Module,
    /// On-chain address of the main module. Used to qualify local struct type names in bytecode.
    module_addr: Address,
    /// Dependency modules indexed by **address** — the module's unique on-chain identifier.
    /// Bytecode encodes cross-module calls as `@<hex_address>::fn_name`, so resolution
    /// is unambiguous even when two dep modules share the same human-readable name.
    deps: HashMap<Address, Module>,
    natives: HashMap<String, NativeFnEntry>,
    gas_schedule: GasSchedule,
    config: VmConfig,
}

impl Vm {
    /// Creates a new VM from a compiled module, native function bindings, a gas schedule, and a config.
    ///
    /// `module` is a `(address, module)` pair — the address is the on-chain address of the module.
    /// Struct values created by `NewStruct` instructions will have their type names qualified as
    /// `@<address>::<name>`, enabling the adapter layer to identify which module each struct belongs to.
    ///
    /// `deps` maps each dep module's on-chain address to its compiled [`Module`].
    /// The address is the key used to resolve `@<address>::fn_name` bytecode references at runtime.
    pub fn new(
        module: (Address, Module),
        natives: Vec<NativeFnEntry>,
        gas_schedule: GasSchedule,
        deps: HashMap<Address, Module>,
        config: VmConfig,
    ) -> Self {
        let mut native_map = HashMap::new();
        for entry in natives {
            match native_map.entry(entry.name.clone()) {
                Entry::Vacant(e) => e.insert(entry),
                Entry::Occupied(_) => {
                    panic!("duplicate native function name: {}", entry.name);
                }
            };
        }
        // If the caller supplied their own meow_vm_abort, validate its signature.
        if let Some(entry) = native_map.get(meow_vm_types::natives::MEOW_VM_ABORT) {
            let expected = meow_vm_types::natives::meow_vm_abort_sig().params;
            assert_eq!(
                entry.params, expected,
                "meow_vm_abort override has wrong parameter types; expected (bool, u64, str)"
            );
            assert!(
                entry.return_type.is_none(),
                "meow_vm_abort override must return void"
            );
        } else {
            // Inject the default meow_vm_abort implementation if the caller didn't supply one.
            native_map.insert(
                meow_vm_types::natives::MEOW_VM_ABORT.to_string(),
                meow_vm_types::natives::meow_vm_abort_entry(),
            );
        }
        let (module_addr, module) = module;
        Self {
            module,
            module_addr,
            deps,
            natives: native_map,
            gas_schedule,
            config,
        }
    }

    /// Call `fn_name` with the given arguments.
    ///
    /// Returns a [`VmCallResult`] with the optional return value.
    pub fn call(
        &self,
        fn_name: &str,
        args: Vec<Value>,
        gas: &mut GasMeter,
    ) -> Result<VmCallResult> {
        let max_dep_modules = self.config.max_dep_modules();
        if self.deps.len() > max_dep_modules {
            return Err(VmError::TooManyDepModules(max_dep_modules));
        }

        // Native built-ins can only be invoked from within contract bytecode, not directly.
        if self.natives.contains_key(fn_name) {
            return Err(VmError::NativeFunctionCallDirect(fn_name.to_string()));
        }

        let func = self
            .module
            .get_function(fn_name)
            .ok_or_else(|| VmError::UndefinedFunction(fn_name.to_string()))?;

        // Only public functions are externally callable.
        if !func.is_public && !self.config.enable_call_private_functions() {
            return Err(VmError::PrivateFunction(fn_name.to_string()));
        }

        for arg in &args {
            assert!(
                check_struct_qualified(arg),
                "struct argument has unqualified type name; all struct values passed to the VM must use @0xHEX::Name format"
            );
        }

        let return_value =
            self.call_inner(fn_name, &self.module, self.module_addr, args, gas, 0)?;

        Ok(VmCallResult { return_value })
    }

    /// Inner recursive call.
    ///
    /// `context_module` — the module that owns the function being called. Used to
    /// resolve unqualified function/struct names within that function's bytecode.
    /// Cross-module calls (`module::fn`) look up the target in `self.deps`.
    fn call_inner(
        &self,
        fn_name: &str,
        context_module: &Module,
        context_module_addr: Address,
        args: Vec<Value>,
        gas: &mut GasMeter,
        depth: usize,
    ) -> Result<Option<Value>> {
        let max_call_depth = self.config.max_call_depth();
        if depth >= max_call_depth {
            return Err(VmError::CallStackOverflow(max_call_depth));
        }

        let func = context_module
            .get_function(fn_name)
            .ok_or_else(|| VmError::UndefinedFunction(fn_name.to_string()))?;

        let mut frame = Frame::new(func.code.clone(), args, func.local_count);

        while let Some(instr) = frame.code.get(frame.pc).cloned() {
            gas.charge(self.gas_schedule.cost_of(&instr))?;
            frame.pc += 1;

            match instr {
                // ── Literals ──────────────────────────────────────────────────
                Instruction::PushBool(v) => frame.push(Value::Bool(v)),
                Instruction::PushU64(v) => frame.push(Value::U64(v)),
                Instruction::PushAddress(a) => frame.push(Value::Address(a)),
                Instruction::PushStr(s) => frame.push(Value::Str(s)),

                // ── Local variable access ─────────────────────────────────────
                Instruction::Load(slot) => {
                    let idx = slot as usize;
                    if idx >= frame.locals.len() {
                        return Err(VmError::UndefinedVariable(slot));
                    }
                    let value = match &frame.locals[idx] {
                        Some(v) if v.uses_move_semantics() => {
                            // Struct: move out of slot (move semantics).
                            frame.locals[idx].take().unwrap()
                        }
                        Some(v) => {
                            // Primitive: copy.
                            v.clone()
                        }
                        None => {
                            return Err(VmError::UseAfterMove(format!(
                                "local slot {slot} has already been moved"
                            )));
                        }
                    };
                    frame.push(value);
                }

                Instruction::Store(slot) => {
                    let v = frame.pop()?;
                    let idx = slot as usize;
                    if idx >= frame.locals.len() {
                        frame.locals.resize(idx + 1, None);
                    }
                    frame.locals[idx] = Some(v);
                }

                Instruction::LoadField(slot, ref path) => {
                    let idx = slot as usize;
                    let root = frame
                        .locals
                        .get(idx)
                        .and_then(|o| o.as_ref())
                        .ok_or_else(|| {
                            VmError::UseAfterMove(format!(
                                "local slot {slot} has already been moved"
                            ))
                        })?;
                    let field_val = read_field_path(root, path)?;
                    frame.push(field_val);
                }

                Instruction::StoreField(slot, ref path) => {
                    let new_val = frame.pop()?;
                    let idx = slot as usize;
                    let root = frame
                        .locals
                        .get_mut(idx)
                        .and_then(|o| o.as_mut())
                        .ok_or_else(|| {
                            VmError::UseAfterMove(format!(
                                "local slot {slot} has already been moved"
                            ))
                        })?;
                    write_field_path(root, path, new_val)?;
                }

                // ── Arithmetic ────────────────────────────────────────────────
                Instruction::Add => {
                    let r = frame.pop()?;
                    let l = frame.pop()?;
                    frame.push(arith_op(l, r, |a, b| a.wrapping_add(b))?);
                }
                Instruction::Sub => {
                    let r = frame.pop()?;
                    let l = frame.pop()?;
                    frame.push(arith_op(l, r, |a, b| a.wrapping_sub(b))?);
                }
                Instruction::Mul => {
                    let r = frame.pop()?;
                    let l = frame.pop()?;
                    frame.push(arith_op(l, r, |a, b| a.wrapping_mul(b))?);
                }
                Instruction::Div => {
                    let r = frame.pop()?;
                    let l = frame.pop()?;
                    if r.as_u64() == Some(0) {
                        return Err(VmError::DivisionByZero);
                    }
                    frame.push(arith_op(l, r, |a, b| a / b)?);
                }
                Instruction::Mod => {
                    let r = frame.pop()?;
                    let l = frame.pop()?;
                    if r.as_u64() == Some(0) {
                        return Err(VmError::DivisionByZero);
                    }
                    frame.push(arith_op(l, r, |a, b| a % b)?);
                }

                // ── Comparison ────────────────────────────────────────────────
                Instruction::Eq => {
                    let r = frame.pop()?;
                    let l = frame.pop()?;
                    frame.push(Value::Bool(l == r));
                }
                Instruction::Ne => {
                    let r = frame.pop()?;
                    let l = frame.pop()?;
                    frame.push(Value::Bool(l != r));
                }
                Instruction::Lt => {
                    let r = frame.pop()?;
                    let l = frame.pop()?;
                    frame.push(Value::Bool(cmp_values(&l, &r)?));
                }
                Instruction::Le => {
                    let r = frame.pop()?;
                    let l = frame.pop()?;
                    frame.push(Value::Bool(!cmp_values(&r, &l)?));
                }
                Instruction::Gt => {
                    let r = frame.pop()?;
                    let l = frame.pop()?;
                    frame.push(Value::Bool(cmp_values(&r, &l)?));
                }
                Instruction::Ge => {
                    let r = frame.pop()?;
                    let l = frame.pop()?;
                    frame.push(Value::Bool(!cmp_values(&l, &r)?));
                }

                // ── Boolean logic ─────────────────────────────────────────────
                Instruction::Not => {
                    let v = frame.pop()?;
                    let b = v.as_bool().ok_or_else(|| {
                        VmError::TypeError(format!("expected bool, got {}", v.type_name()))
                    })?;
                    frame.push(Value::Bool(!b));
                }
                Instruction::And => {
                    let r = frame.pop()?;
                    let l = frame.pop()?;
                    let a = l.as_bool().ok_or_else(|| {
                        VmError::TypeError(format!("expected bool, got {}", l.type_name()))
                    })?;
                    let b = r.as_bool().ok_or_else(|| {
                        VmError::TypeError(format!("expected bool, got {}", r.type_name()))
                    })?;
                    frame.push(Value::Bool(a && b));
                }
                Instruction::Or => {
                    let r = frame.pop()?;
                    let l = frame.pop()?;
                    let a = l.as_bool().ok_or_else(|| {
                        VmError::TypeError(format!("expected bool, got {}", l.type_name()))
                    })?;
                    let b = r.as_bool().ok_or_else(|| {
                        VmError::TypeError(format!("expected bool, got {}", r.type_name()))
                    })?;
                    frame.push(Value::Bool(a || b));
                }

                // ── Struct operations ─────────────────────────────────────────
                Instruction::NewStruct {
                    type_name,
                    field_names,
                } => {
                    // Validate the struct definition exists and qualify the type name.
                    let qualified_type_name = if let Some((dep_addr, struct_name)) =
                        module_ref::parse_module_ref(&type_name)
                    {
                        self.deps
                            .get(&dep_addr)
                            .and_then(|m| m.get_struct(struct_name))
                            .ok_or_else(|| VmError::UndefinedStruct(type_name.clone()))?;
                        type_name // already @addr::name — keep as-is
                    } else {
                        context_module
                            .get_struct(&type_name)
                            .ok_or_else(|| VmError::UndefinedStruct(type_name.clone()))?;
                        module_ref::qualify(&context_module_addr, &type_name)
                    };
                    // Pop values in reverse field order.
                    let mut fields: Vec<(String, Value)> = Vec::with_capacity(field_names.len());
                    for name in field_names.iter().rev() {
                        let v = frame.pop()?;
                        fields.push((name.clone(), v));
                    }
                    fields.reverse();

                    frame.push(Value::Struct {
                        type_name: qualified_type_name,
                        fields,
                    });
                }

                Instruction::GetField(field) => {
                    let v = frame.pop()?;
                    match v {
                        Value::Struct {
                            ref type_name,
                            ref fields,
                        } => {
                            let fv = fields
                                .iter()
                                .find(|(n, _)| *n == field)
                                .map(|(_, v)| v.clone())
                                .ok_or_else(|| VmError::UndefinedField {
                                    type_name: type_name.clone(),
                                    field: field.clone(),
                                })?;
                            frame.push(fv);
                        }
                        other => {
                            return Err(VmError::TypeError(format!(
                                "expected struct, got {}",
                                other.type_name()
                            )));
                        }
                    }
                }

                Instruction::UnpackStruct {
                    ref field_names, ..
                } => {
                    let v = frame.pop()?;
                    match v {
                        Value::Struct { type_name, fields } => {
                            let mut field_map: std::collections::HashMap<String, Value> =
                                fields.into_iter().collect();
                            // Push in reverse so first field ends up on top for first Store
                            for name in field_names.iter().rev() {
                                let fv = field_map.remove(name).ok_or_else(|| {
                                    VmError::UndefinedField {
                                        type_name: type_name.clone(),
                                        field: name.clone(),
                                    }
                                })?;
                                frame.push(fv);
                            }
                        }
                        other => {
                            return Err(VmError::TypeError(format!(
                                "UnpackStruct: expected struct, got {}",
                                other.type_name()
                            )));
                        }
                    }
                }

                // ── Stack manipulation ────────────────────────────────────────
                Instruction::Pop => {
                    frame.pop()?;
                }
                Instruction::Dup => {
                    let v = frame.peek()?.clone();
                    frame.push(v);
                }

                // ── Control flow ──────────────────────────────────────────────
                Instruction::Jump(offset) => {
                    frame.pc = (frame.pc as i64 - 1 + offset as i64).max(0) as usize;
                }
                Instruction::JumpIf(offset) => {
                    let v = frame.pop()?;
                    if v.as_bool() == Some(true) {
                        frame.pc = (frame.pc as i64 - 1 + offset as i64).max(0) as usize;
                    }
                }
                Instruction::JumpIfNot(offset) => {
                    let v = frame.pop()?;
                    if v.as_bool() == Some(false) {
                        frame.pc = (frame.pc as i64 - 1 + offset as i64).max(0) as usize;
                    }
                }

                // ── Function calls ────────────────────────────────────────────
                Instruction::Call(name) => {
                    if let Some((dep_addr, fn_name_in_dep)) = module_ref::parse_module_ref(&name) {
                        // Cross-module call: `@<hex_address>::function_name`.
                        let dep = self
                            .deps
                            .get(&dep_addr)
                            .ok_or_else(|| VmError::UndefinedFunction(name.clone()))?;
                        let arg_count = dep
                            .get_function(fn_name_in_dep)
                            .ok_or_else(|| VmError::UndefinedFunction(name.clone()))?
                            .params
                            .len();
                        let mut args: Vec<Value> = (0..arg_count)
                            .map(|_| frame.pop())
                            .collect::<Result<Vec<_>>>()?;
                        args.reverse();
                        let ret =
                            self.call_inner(fn_name_in_dep, dep, dep_addr, args, gas, depth + 1)?;
                        frame.push(ret.unwrap_or(Value::Void));
                    } else if let Some(callee) = context_module.get_function(&name) {
                        let arg_count = callee.params.len();
                        let mut args: Vec<Value> = (0..arg_count)
                            .map(|_| frame.pop())
                            .collect::<Result<Vec<_>>>()?;
                        args.reverse();

                        let ret = self.call_inner(
                            &name,
                            context_module,
                            context_module_addr,
                            args,
                            gas,
                            depth + 1,
                        )?;
                        frame.push(ret.unwrap_or(Value::Void));
                    } else if let Some(native) = self.natives.get(&name) {
                        let param_count = native.params.len();
                        gas.charge(native.gas_cost)?;

                        let mut args: Vec<Value> = (0..param_count)
                            .map(|_| frame.pop())
                            .collect::<Result<Vec<_>>>()?;
                        args.reverse();

                        match (native.func)(args) {
                            NativeResult::Return(v) => {
                                if let Some(ref val) = v {
                                    assert!(
                                        check_struct_qualified(val),
                                        "native function '{name}' returned a struct with unqualified type name"
                                    );
                                }
                                frame.push(v.unwrap_or(Value::Void));
                            }
                            NativeResult::Abort { code, message } => {
                                return Err(VmError::Aborted { code, message });
                            }
                            NativeResult::Error(msg) => {
                                return Err(VmError::NativeError(msg));
                            }
                        }
                    } else {
                        return Err(VmError::UndefinedFunction(name));
                    }
                }

                // ── Return ────────────────────────────────────────────────────
                Instruction::Return => {
                    return Ok(frame.stack.pop());
                }

                // ── Tuples ────────────────────────────────────────────────────
                Instruction::MakeTuple(n) => {
                    let mut values: Vec<Value> =
                        (0..n).map(|_| frame.pop()).collect::<Result<Vec<_>>>()?;
                    values.reverse(); // collected in reverse; restore original order
                    frame.push(Value::Tuple(values));
                }

                Instruction::UnpackTuple(n) => {
                    let n = n as usize;
                    let v = frame.pop()?;
                    match v {
                        Value::Tuple(values) => {
                            if values.len() != n {
                                return Err(VmError::TypeError(format!(
                                    "UnpackTuple: expected tuple of size {n}, got size {}",
                                    values.len()
                                )));
                            }
                            // Push in reverse so element[0] ends up on top for first Store
                            for val in values.into_iter().rev() {
                                frame.push(val);
                            }
                        }
                        other => {
                            return Err(VmError::TypeError(format!(
                                "UnpackTuple: expected tuple, got {}",
                                other.type_name()
                            )));
                        }
                    }
                }
            }
        }

        Ok(frame.stack.pop())
    }
}

//
// ─── Helpers ───
//

fn arith_op(l: Value, r: Value, op: impl Fn(u64, u64) -> u64) -> Result<Value> {
    let a = l
        .as_u64()
        .ok_or_else(|| VmError::TypeError(format!("expected integer, got {}", l.type_name())))?;
    let b = r
        .as_u64()
        .ok_or_else(|| VmError::TypeError(format!("expected integer, got {}", r.type_name())))?;
    Ok(Value::U64(op(a, b)))
}

/// Traverse `path` through nested struct fields and return a clone of the terminal value.
/// Never clones an intermediate struct — only the terminal primitive (or struct if at end).
fn read_field_path(mut current: &Value, path: &[String]) -> Result<Value> {
    for (i, field) in path.iter().enumerate() {
        let fields = match current {
            Value::Struct { fields, .. } => fields,
            other => {
                return Err(VmError::TypeError(format!(
                    "LoadField: expected struct at path step {i}, got {}",
                    other.type_name()
                )));
            }
        };
        current = fields
            .iter()
            .find(|(n, _)| n == field)
            .map(|(_, v)| v)
            .ok_or_else(|| VmError::UndefinedField {
                type_name: current.type_name().to_string(),
                field: field.clone(),
            })?;
    }
    Ok(current.clone())
}

/// Traverse `path` through nested struct fields and overwrite the terminal field with `val`.
fn write_field_path(mut current: &mut Value, path: &[String], val: Value) -> Result<()> {
    let (last, prefix) = path
        .split_last()
        .ok_or_else(|| VmError::TypeError("StoreField: path must not be empty".to_string()))?;
    for (i, field) in prefix.iter().enumerate() {
        let type_name = current.type_name().to_string();
        let fields = match current {
            Value::Struct { fields, .. } => fields,
            other => {
                return Err(VmError::TypeError(format!(
                    "StoreField: expected struct at path step {i}, got {}",
                    other.type_name()
                )));
            }
        };
        current = fields
            .iter_mut()
            .find(|(n, _)| n == field)
            .map(|(_, v)| v)
            .ok_or_else(|| VmError::UndefinedField {
                type_name,
                field: field.clone(),
            })?;
    }
    let type_name = current.type_name().to_string();
    let fields = match current {
        Value::Struct { fields, .. } => fields,
        other => {
            return Err(VmError::TypeError(format!(
                "StoreField: expected struct for terminal write, got {}",
                other.type_name()
            )));
        }
    };
    let entry =
        fields
            .iter_mut()
            .find(|(n, _)| n == last)
            .ok_or_else(|| VmError::UndefinedField {
                type_name,
                field: last.clone(),
            })?;
    entry.1 = val;
    Ok(())
}

/// Returns `true` if `l < r` (unsigned comparison).
fn cmp_values(l: &Value, r: &Value) -> Result<bool> {
    let a = l
        .as_u64()
        .ok_or_else(|| VmError::TypeError(format!("expected integer, got {}", l.type_name())))?;
    let b = r
        .as_u64()
        .ok_or_else(|| VmError::TypeError(format!("expected integer, got {}", r.type_name())))?;
    Ok(a < b)
}

/// Returns `true` if `val` and all structs nested within it have qualified type names.
fn check_struct_qualified(val: &Value) -> bool {
    if let Value::Struct { type_name, fields } = val {
        if module_ref::parse_module_ref(type_name).is_none() {
            return false;
        }
        return fields.iter().all(|(_, v)| check_struct_qualified(v));
    }
    true
}
