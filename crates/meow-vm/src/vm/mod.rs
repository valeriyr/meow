pub mod error;

use std::collections::HashMap;

use meow_vm_types::{limits, types::Value};

use crate::{bytecode::Instruction, module::Module, vm::error::VmError};

/// An error that can occur during VM execution.
pub type Result<T> = std::result::Result<T, VmError>;

// ─── Native functions ────────────────────────────────────────────────────────

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
    pub name: String,
    pub param_count: usize,
    pub gas_cost: u64,
    pub func: Box<dyn Fn(Vec<Value>) -> NativeResult>,
}

// ─── Execution result ────────────────────────────────────────────────────────

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

// ─── Gas schedule ────────────────────────────────────────────────────────────

/// Per-instruction gas costs. Pass a custom schedule to [`Vm::with_gas_schedule`].
#[derive(Debug, Clone)]
pub struct GasSchedule {
    push_primitive: u64,
    push_str: u64,
    load_store: u64,
    load_field: u64,
    store_field: u64,
    add_sub_mul: u64,
    div: u64,
    compare: u64,
    logic: u64,
    new_struct_base: u64,
    new_struct_per_field: u64,
    get_field: u64,
    stack: u64,
    jump: u64,
    call: u64,
    return_: u64,
}

impl Default for GasSchedule {
    fn default() -> Self {
        Self {
            push_primitive: 1,
            push_str: 2,
            load_store: 1,
            load_field: 2,
            store_field: 5,
            add_sub_mul: 2,
            div: 5,
            compare: 2,
            logic: 1,
            new_struct_base: 10,
            new_struct_per_field: 2,
            get_field: 3,
            stack: 1,
            jump: 2,
            call: 20,
            return_: 2,
        }
    }
}

impl GasSchedule {
    pub fn cost_of(&self, instr: &Instruction) -> u64 {
        match instr {
            Instruction::PushBool(_) | Instruction::PushU64(_) => self.push_primitive,
            Instruction::PushStr(_) => self.push_str,

            Instruction::Load(_) | Instruction::Store(_) => self.load_store,
            Instruction::LoadField(_, _) => self.load_field,
            Instruction::StoreField(_, _) => self.store_field,

            Instruction::Add | Instruction::Sub | Instruction::Mul => self.add_sub_mul,
            Instruction::Div => self.div,

            Instruction::Eq
            | Instruction::Ne
            | Instruction::Lt
            | Instruction::Le
            | Instruction::Gt
            | Instruction::Ge => self.compare,

            Instruction::Not | Instruction::And | Instruction::Or => self.logic,

            Instruction::NewStruct { field_names, .. } => {
                self.new_struct_base + field_names.len() as u64 * self.new_struct_per_field
            }
            Instruction::GetField(_) => self.get_field,

            Instruction::Pop | Instruction::Dup => self.stack,

            Instruction::Jump(_) | Instruction::JumpIf(_) | Instruction::JumpIfNot(_) => self.jump,

            Instruction::Call(_) => self.call,
            Instruction::Return => self.return_,
        }
    }
}

// ─── Gas metering ────────────────────────────────────────────────────────────

/// Tracks gas consumption during execution.
#[derive(Debug, Clone)]
pub struct GasMeter {
    limit: u64,
    consumed: u64,
}

impl GasMeter {
    pub fn new(limit: u64) -> Self {
        Self { limit, consumed: 0 }
    }

    /// Unlimited gas meter (for testing / trusted contexts).
    pub fn unlimited() -> Self {
        Self::new(u64::MAX)
    }

    /// Charge `cost` units of gas. Returns [`VmError::OutOfGas`] if the limit is exceeded.
    pub fn charge(&mut self, cost: u64) -> Result<()> {
        let new = self.consumed.saturating_add(cost);
        if new > self.limit {
            return Err(VmError::OutOfGas {
                consumed: new,
                limit: self.limit,
            });
        }
        self.consumed = new;
        Ok(())
    }

    pub fn consumed(&self) -> u64 {
        self.consumed
    }

    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.consumed)
    }
}

// ─── Execution frame ─────────────────────────────────────────────────────────

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

// ─── VM ──────────────────────────────────────────────────────────────────────

/// The virtual machine executor.
///
/// Create a `Vm` from a [`Module`] and a list of native functions, then call
/// functions by name.
pub struct Vm {
    module: Module,
    natives: HashMap<String, NativeFnEntry>,
    gas_schedule: GasSchedule,
}

impl Vm {
    pub fn new(module: Module, natives: Vec<NativeFnEntry>, gas_schedule: GasSchedule) -> Self {
        let mut native_map = HashMap::new();
        for entry in natives {
            native_map.insert(entry.name.clone(), entry);
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
            natives: native_map,
            gas_schedule,
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
        let func = self
            .module
            .get_function(fn_name)
            .ok_or_else(|| VmError::UndefinedFunction(fn_name.to_string()))?;
        let param_count = func.params.len();

        let (return_value, final_locals) = self.call_inner(fn_name, args, gas, 0)?;

        // Expose the final state of each parameter slot as final_args.
        let final_args = final_locals.into_iter().take(param_count).collect();

        Ok(VmCallResult {
            return_value,
            final_args,
        })
    }

    /// Inner recursive call. Returns `(return_value, final_locals)`.
    fn call_inner(
        &self,
        fn_name: &str,
        args: Vec<Value>,
        gas: &mut GasMeter,
        depth: usize,
    ) -> Result<(Option<Value>, Vec<Option<Value>>)> {
        if depth >= limits::MAX_CALL_DEPTH {
            return Err(VmError::CallStackOverflow(limits::MAX_CALL_DEPTH));
        }

        let func = self
            .module
            .get_function(fn_name)
            .ok_or_else(|| VmError::UndefinedFunction(fn_name.to_string()))?;

        let mut frame = Frame::new(func.code.clone(), args, func.local_count);

        loop {
            let instr = match frame.code.get(frame.pc) {
                Some(i) => i.clone(),
                None => break,
            };

            gas.charge(self.gas_schedule.cost_of(&instr))?;
            frame.pc += 1;

            match instr {
                // ── Literals ──────────────────────────────────────────────────
                Instruction::PushBool(v) => frame.push(Value::Bool(v)),
                Instruction::PushU64(v) => frame.push(Value::U64(v)),
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
                    let def = self
                        .module
                        .get_struct(&type_name)
                        .ok_or_else(|| VmError::UndefinedStruct(type_name.clone()))?;
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
                    if let Some(callee) = self.module.get_function(&name) {
                        let arg_count = callee.params.len();
                        let mut args: Vec<Value> = (0..arg_count)
                            .map(|_| frame.pop())
                            .collect::<Result<Vec<_>>>()?;
                        args.reverse();

                        let (ret, _) = self.call_inner(&name, args, gas, depth + 1)?;
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

// ─── Helpers ─────────────────────────────────────────────────────────────────

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
