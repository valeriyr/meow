//! # meow-vm
//!
//! A stack-based virtual machine with move semantics for objects, native
//! function support, and per-instruction gas metering.
//!
//! ## Types
//! - Primitives: `bool`, `u64`, `address` ([u8; 32]) — freely copyable
//! - `struct` — user-defined, value semantics (freely copyable)
//! - `object` — user-defined, move semantics; must have `id: address` first field
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
//! use meow_vm_types::{config::{CompilerConfig, VmConfig}, types::Value};
//!
//! let source = r#"
//!     module math;
//!
//!     pub fn add(a: u64, b: u64): u64 {
//!         return a + b;
//!     }
//! "#;
//!
//! let module = Compiler::compile(source, &[], CompilerConfig::default()).unwrap();
//! let vm = Vm::new(module, vec![], GasSchedule::default(), HashMap::new(), VmConfig::default());
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
use std::str::FromStr;

use meow_vm_types::{address::Address, config::VmConfig, types::Value};

use meow_vm_types::{bytecode::Instruction, module::Module};

use crate::error::VmError;
use crate::gas_meter::GasMeter;
use crate::gas_schedule::GasSchedule;

/// An error that can occur during VM execution.
pub type Result<T> = std::result::Result<T, VmError>;

//
// ─── Native functions ───
//

/// Result returned by a native function.
pub enum NativeResult {
    /// The function completed normally. `None` means void (a `Void` value will be pushed).
    Return(Option<Value>),
    /// The function aborted execution (e.g. meow_vm_abort).
    Abort { code: u64, message: String },
    /// The function encountered an error (e.g. wrong argument type).
    Error(String),
}

/// A registered native function entry.
pub struct NativeFnEntry {
    /// The function name as referenced in bytecode.
    pub name: String,
    /// The number of parameters this function expects.
    pub param_count: usize,
    /// The gas cost charged when this function is called.
    pub gas_cost: u64,
    /// The function implementation.
    pub func: Box<dyn Fn(Vec<Value>) -> NativeResult>,
}

//
// ─── Execution result ───
//

/// The outcome of a top-level VM function call.
#[derive(Debug, Clone)]
pub struct VmCallResult {
    /// Value returned by the function, if any.
    pub return_value: Option<Value>,
    /// Final state of each argument passed to the top-level call.
    ///
    /// `None` if the argument was consumed (moved into `meow_vm_transfer` or
    /// `meow_vm_destroy`). `Some(v)` if the argument was not consumed (may have
    /// been mutated via `StoreField`).
    pub final_args: Vec<Option<Value>>,
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
    /// `deps` maps each dep module's on-chain address to its compiled [`Module`].
    /// The address is the key used to resolve `@<address>::fn_name` bytecode references at runtime.
    pub fn new(
        module: Module,
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
        // Inject a default meow_vm_abort implementation if the caller didn't supply one.
        native_map
            .entry("meow_vm_abort".to_string())
            .or_insert_with(|| NativeFnEntry {
                name: "meow_vm_abort".to_string(),
                param_count: 3,
                gas_cost: 0,
                func: Box::new(|mut args| {
                    let condition = match args[0] {
                        Value::Bool(b) => b,
                        _ => {
                            return NativeResult::Error(
                                "meow_vm_abort: first argument must be bool".into(),
                            );
                        }
                    };
                    if condition {
                        NativeResult::Return(None)
                    } else {
                        let code = args[1].as_u64().unwrap_or(0);
                        let message = args
                            .remove(2)
                            .into_str()
                            .unwrap_or_else(|| "aborted".into());
                        NativeResult::Abort { code, message }
                    }
                }),
            });
        Self {
            module,
            deps,
            natives: native_map,
            gas_schedule,
            config,
        }
    }

    /// Call `fn_name` with the given arguments.
    ///
    /// Returns a [`VmCallResult`] with the optional return value and the final
    /// state of each argument (for executor-level object tracking).
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

        let param_count = func.params.len();

        let (return_value, final_locals) = self.call_inner(fn_name, &self.module, args, gas, 0)?;

        // Expose the final state of each parameter slot as final_args.
        let final_args = final_locals.into_iter().take(param_count).collect();

        Ok(VmCallResult {
            return_value,
            final_args,
        })
    }

    /// Inner recursive call.
    ///
    /// `context_module` — the module that owns the function being called. Used to
    /// resolve unqualified function/struct names within that function's bytecode.
    /// Cross-module calls (`module::fn`) look up the target in `self.deps`.
    ///
    /// Returns `(return_value, final_locals)`.
    fn call_inner(
        &self,
        fn_name: &str,
        context_module: &Module,
        args: Vec<Value>,
        gas: &mut GasMeter,
        depth: usize,
    ) -> Result<(Option<Value>, Vec<Option<Value>>)> {
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

                // ── Locals ────────────────────────────────────────────────────
                Instruction::Load(slot) => {
                    let idx = slot as usize;
                    if idx >= frame.locals.len() {
                        return Err(VmError::UndefinedVariable(slot));
                    }
                    let value = match &frame.locals[idx] {
                        Some(v) if v.uses_move_semantics() => {
                            // Object: move out of slot.
                            frame.locals[idx].take().unwrap()
                        }
                        Some(v) => {
                            // Primitive or Struct: copy.
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

                Instruction::LoadField(slot, ref field) => {
                    let idx = slot as usize;
                    let field_val = match frame.locals.get(idx).and_then(|o| o.as_ref()) {
                        Some(Value::Struct { fields, .. }) | Some(Value::Object { fields, .. }) => {
                            fields
                                .iter()
                                .find(|(n, _)| n == field)
                                .map(|(_, v)| v.clone())
                                .ok_or_else(|| VmError::UndefinedField {
                                    type_name: frame.locals[idx]
                                        .as_ref()
                                        .map(|v| v.type_name().to_string())
                                        .unwrap_or_default(),
                                    field: field.clone(),
                                })?
                        }
                        Some(other) => {
                            return Err(VmError::TypeError(format!(
                                "LoadField: expected struct/object, got {}",
                                other.type_name()
                            )));
                        }
                        None => {
                            return Err(VmError::UseAfterMove(format!(
                                "local slot {slot} has already been moved"
                            )));
                        }
                    };
                    frame.push(field_val);
                }

                Instruction::StoreField(slot, ref field) => {
                    let new_val = frame.pop()?;
                    let idx = slot as usize;
                    match frame.locals.get_mut(idx).and_then(|o| o.as_mut()) {
                        Some(Value::Struct { fields, .. }) | Some(Value::Object { fields, .. }) => {
                            let entry = fields.iter_mut().find(|(n, _)| n == field);
                            match entry {
                                Some((_, v)) => *v = new_val,
                                None => {
                                    return Err(VmError::UndefinedField {
                                        type_name: frame.locals[idx]
                                            .as_ref()
                                            .map(|v| v.type_name().to_string())
                                            .unwrap_or_default(),
                                        field: field.clone(),
                                    });
                                }
                            }
                        }
                        Some(other) => {
                            return Err(VmError::TypeError(format!(
                                "StoreField: expected struct/object, got {}",
                                other.type_name()
                            )));
                        }
                        None => {
                            return Err(VmError::UseAfterMove(format!(
                                "local slot {slot} has already been moved"
                            )));
                        }
                    }
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

                // ── Struct / Object operations ─────────────────────────────────
                Instruction::NewStruct {
                    type_name,
                    field_names,
                } => {
                    let def = if let Some((dep_addr, struct_name)) = parse_module_ref(&type_name) {
                        self.deps
                            .get(&dep_addr)
                            .and_then(|m| m.get_struct(struct_name))
                            .ok_or_else(|| VmError::UndefinedStruct(type_name.clone()))?
                    } else {
                        context_module
                            .get_struct(&type_name)
                            .ok_or_else(|| VmError::UndefinedStruct(type_name.clone()))?
                    };
                    let is_object = def.is_object;

                    // Pop values in reverse field order.
                    let mut fields: Vec<(String, Value)> = Vec::with_capacity(field_names.len());
                    for name in field_names.iter().rev() {
                        let v = frame.pop()?;
                        fields.push((name.clone(), v));
                    }
                    fields.reverse();

                    let value = if is_object {
                        Value::Object { type_name, fields }
                    } else {
                        Value::Struct { type_name, fields }
                    };
                    frame.push(value);
                }

                Instruction::GetField(field) => {
                    let v = frame.pop()?;
                    match v {
                        Value::Struct {
                            ref type_name,
                            ref fields,
                        }
                        | Value::Object {
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
                                "expected struct/object, got {}",
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

                // ── Functions ─────────────────────────────────────────────────
                Instruction::Call(name) => {
                    if let Some((dep_addr, fn_name_in_dep)) = parse_module_ref(&name) {
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
                        let (ret, _) =
                            self.call_inner(fn_name_in_dep, dep, args, gas, depth + 1)?;
                        frame.push(ret.unwrap_or(Value::Void));
                    } else if let Some(callee) = context_module.get_function(&name) {
                        let arg_count = callee.params.len();
                        let mut args: Vec<Value> = (0..arg_count)
                            .map(|_| frame.pop())
                            .collect::<Result<Vec<_>>>()?;
                        args.reverse();

                        let (ret, _) =
                            self.call_inner(&name, context_module, args, gas, depth + 1)?;
                        frame.push(ret.unwrap_or(Value::Void));
                    } else if let Some(native) = self.natives.get(&name) {
                        let param_count = native.param_count;
                        gas.charge(native.gas_cost)?;

                        let mut args: Vec<Value> = (0..param_count)
                            .map(|_| frame.pop())
                            .collect::<Result<Vec<_>>>()?;
                        args.reverse();

                        match (native.func)(args) {
                            NativeResult::Return(v) => {
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

                Instruction::Return => {
                    let ret = frame.stack.pop();
                    return Ok((ret, frame.locals));
                }
            }
        }

        Ok((frame.stack.pop(), frame.locals))
    }
}

//
// ─── Helpers ───
//

/// Parse a bytecode cross-module reference of the form `@<64-hex-chars>::<name>`.
///
/// Returns `(dep_address, name_within_dep)` on success, or `None` if the string
/// is not a cross-module reference (i.e. it is a plain local name).
fn parse_module_ref(s: &str) -> Option<(Address, &str)> {
    let rest = s.strip_prefix('@')?;
    let (hex_part, name) = rest.split_once("::")?;
    let address = Address::from_str(hex_part).ok()?;
    Some((address, name))
}

fn arith_op(l: Value, r: Value, op: impl Fn(u64, u64) -> u64) -> Result<Value> {
    let a = l
        .as_u64()
        .ok_or_else(|| VmError::TypeError(format!("expected integer, got {}", l.type_name())))?;
    let b = r
        .as_u64()
        .ok_or_else(|| VmError::TypeError(format!("expected integer, got {}", r.type_name())))?;
    Ok(Value::U64(op(a, b)))
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
