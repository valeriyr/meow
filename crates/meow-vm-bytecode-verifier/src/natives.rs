use meow_vm_types::types::Type;

/// A parameter type accepted by a native function.
#[derive(Debug, Clone, PartialEq)]
pub enum NativeParam {
    /// A specific concrete Meow type.
    Concrete(Type),
    /// Any Object value — used by meow_vm_transfer and meow_vm_destroy.
    AnyObject,
}

/// Type signature for a native function, used by the verifier to type-check
/// `Call` instructions at native call sites.
#[derive(Debug, Clone)]
pub struct NativeSignature {
    pub name: String,
    pub params: Vec<NativeParam>,
    pub return_type: Option<Type>,
}

impl NativeSignature {
    pub fn new(
        name: impl Into<String>,
        params: Vec<NativeParam>,
        return_type: Option<Type>,
    ) -> Self {
        Self {
            name: name.into(),
            params,
            return_type,
        }
    }
}

/// Returns the language-level built-in native signatures.
///
/// These are always available regardless of which adapter is used:
/// - `meow_vm_abort(bool, u64, str)` → void
pub fn builtin_natives() -> Vec<NativeSignature> {
    vec![NativeSignature::new(
        "meow_vm_abort",
        vec![
            NativeParam::Concrete(Type::Bool),
            NativeParam::Concrete(Type::U64),
            NativeParam::Concrete(Type::Str),
        ],
        None,
    )]
}
