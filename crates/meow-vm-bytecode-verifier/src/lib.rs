use std::collections::HashMap;

use meow_vm_types::{address::Address, config::CompilerConfig, module::Module};

pub mod error;
pub mod natives;

mod abstract_interpretation;
mod structural;

pub use error::VerificationError;
pub use natives::{NativeParam, NativeSignature, builtin_natives};

/// Run language-level bytecode verification on `module`.
///
/// Runs two phases in sequence, accumulating all errors before returning:
///
/// 1. **Structural checks** — static shape validation with no stack simulation:
///    identifiers, duplicate names, field rules, slot bounds, jump bounds, and
///    cross-module visibility.
/// 2. **Abstract interpretation** — per-function forward simulation: type safety,
///    struct move semantics (linearity), return type matching, and native call
///    argument types.
///
/// `natives` should contain adapter-supplied natives (`meow_vm_transfer`,
/// `meow_vm_destroy`, `meow_vm_sender`, etc.). Language built-ins (`meow_vm_abort`)
/// are always merged in automatically — callers do not need to include them.
///
/// Adapter-level rules (object layout, ID freshness) are enforced separately
/// and must not be checked here.
pub fn verify(
    module: &Module,
    deps: &HashMap<Address, &Module>,
    natives: &[NativeSignature],
    config: &CompilerConfig,
) -> Result<(), Vec<VerificationError>> {
    let all_natives: Vec<NativeSignature> = builtin_natives()
        .into_iter()
        .chain(natives.iter().cloned())
        .collect();

    let mut errors = structural::check_module(module, deps, config);

    for func in &module.functions {
        errors.extend(abstract_interpretation::check_function(
            func,
            module,
            deps,
            &all_natives,
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
