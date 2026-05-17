use crate::types::{Type, Value};

//
// ─── NativeParam ───
//

/// A parameter type accepted by a native function.
#[derive(Debug, Clone, PartialEq)]
pub enum NativeParam {
    /// A specific concrete Meow type.
    Concrete(Type),
    /// Accepts any struct value — used for natives that operate on arbitrary structs.
    AnyStruct,
}

//
// ─── Core native types ───
//

/// Result returned by a native function implementation.
pub enum NativeResult {
    /// Normal return, with an optional value pushed onto the VM stack.
    Return(Option<Value>),
    /// Abort execution with an error code and message.
    Abort { code: u64, message: String },
    /// Unrecoverable internal error; surfaces as `VmError::NativeError`.
    Error(String),
}

/// A registered native function entry used to build the VM's native table.
pub struct NativeFnEntry {
    /// Name as it appears in source code.
    pub name: String,
    /// Expected parameter types in call order.
    pub params: Vec<NativeParam>,
    /// Return type, or `None` for void functions.
    pub return_type: Option<Type>,
    /// Gas charged before the function body executes.
    pub gas_cost: u64,
    /// The native implementation.
    pub func: Box<dyn Fn(Vec<Value>) -> NativeResult>,
}

//
// ─── NativeSig ───
//

/// Type signature of a native function, used by the compiler and bytecode verifier
/// for call type-checking without requiring the full implementation.
#[derive(Clone)]
pub struct NativeSig {
    /// Name as it appears in source code.
    pub name: String,
    /// Expected parameter types in call order.
    pub params: Vec<NativeParam>,
    /// Return type, or `None` for void functions.
    pub return_type: Option<Type>,
}

//
// ─── Built-in natives ───
//

/// Returns the language-level built-in native signatures.
///
/// These are always available regardless of which adapter is used:
/// - `meow_vm_abort(bool, u64, str)` → void
pub fn builtin_natives() -> Vec<NativeSig> {
    vec![meow_vm_abort_sig()]
}

//
// ─── meow_vm_abort ───
//

/// The name of the built-in abort function as it appears in source code.
pub const MEOW_VM_ABORT: &str = "meow_vm_abort";

/// Compiler-facing signature for `meow_vm_abort(bool, u64, str)`.
pub fn meow_vm_abort_sig() -> NativeSig {
    NativeSig {
        name: MEOW_VM_ABORT.to_string(),
        params: vec![
            NativeParam::Concrete(Type::Bool),
            NativeParam::Concrete(Type::U64),
            NativeParam::Concrete(Type::Str),
        ],
        return_type: None,
    }
}

/// Builds the default `meow_vm_abort` native entry injected by the VM.
///
/// Aborts when `condition` is `false` (assert semantics). Adapters may override
/// this by registering their own entry with the same name.
pub fn meow_vm_abort_entry() -> NativeFnEntry {
    NativeFnEntry {
        name: MEOW_VM_ABORT.to_string(),
        params: vec![
            NativeParam::Concrete(Type::Bool),
            NativeParam::Concrete(Type::U64),
            NativeParam::Concrete(Type::Str),
        ],
        return_type: None,
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
    }
}
