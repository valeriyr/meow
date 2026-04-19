/// Language-level built-in native functions.
///
/// `NativeResult`, `NativeFnEntry`, and the `meow_vm_abort` built-in all live
/// here so that both the VM (which provides the implementation) and the compiler
/// (which needs the signature for type-checking) share a single source of truth.
use crate::types::{Type, Value};

//
// ─── Core native types ───
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
    /// Parameter types. `None` means "accepts any struct value".
    pub params: Vec<Option<Type>>,
    /// Return type; `None` means void.
    pub return_type: Option<Type>,
    /// The gas cost charged when this function is called.
    pub gas_cost: u64,
    /// The function implementation.
    pub func: Box<dyn Fn(Vec<Value>) -> NativeResult>,
}

//
// ─── NativeSig ───
//

/// Signature of a native function, used by the compiler for type-checking.
///
/// `params` mirrors `NativeFnEntry::params`: `None` means "accepts any struct".
#[derive(Clone)]
pub struct NativeSig {
    pub name: String,
    pub params: Vec<Option<Type>>,
    pub return_type: Option<Type>,
}

//
// ─── meow_vm_abort ───
//

pub const MEOW_VM_ABORT: &str = "meow_vm_abort";

/// Parameter types for `meow_vm_abort(condition: bool, code: u64, message: str)`.
pub fn meow_vm_abort_params() -> Vec<Type> {
    vec![Type::Bool, Type::U64, Type::Str]
}

/// Compiler-facing signature for `meow_vm_abort`.
pub fn meow_vm_abort_sig() -> NativeSig {
    NativeSig {
        name: MEOW_VM_ABORT.to_string(),
        params: meow_vm_abort_params().into_iter().map(Some).collect(),
        return_type: None,
    }
}

/// Builds the default `meow_vm_abort` native entry injected by the VM.
pub fn meow_vm_abort_entry() -> NativeFnEntry {
    NativeFnEntry {
        name: MEOW_VM_ABORT.to_string(),
        params: meow_vm_abort_params().into_iter().map(Some).collect(),
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
