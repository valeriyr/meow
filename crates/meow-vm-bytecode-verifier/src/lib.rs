//! Bytecode verifier for Meow VM modules.
//!
//! Validates a compiled module before it is stored on-chain, catching malformed bytecode
//! that the compiler would never emit but that a malicious publisher could craft by hand.
//! Verification is split into a structural pass (cheap, no stack simulation) followed by
//! abstract interpretation (type safety and move-semantics checks).

use std::collections::HashMap;

use meow_vm_types::{address::Address, config::CompilerConfig, module::Module};

pub mod error;

mod abstract_interpretation;
mod structural;

pub use error::VerificationError;
pub use meow_vm_types::natives::{NativeParam, NativeSig, builtin_natives};

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
/// `natives` should contain the adapter-supplied native signatures. Language
/// built-ins (`meow_vm_abort`) are always merged in automatically — callers do
/// not need to include them.
///
/// Adapter-level rules (object layout, ID freshness) are enforced separately
/// and must not be checked here.
pub fn verify(
    module: &Module,
    deps: &HashMap<Address, &Module>,
    natives: &[NativeSig],
    config: &CompilerConfig,
) -> Result<(), Vec<VerificationError>> {
    let all_natives: Vec<NativeSig> = builtin_natives()
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
