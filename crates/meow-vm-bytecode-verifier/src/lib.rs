use std::collections::HashMap;

use meow_vm_types::{address::Address, config::CompilerConfig, module::Module};

pub mod error;
pub mod natives;

mod abstract_interpretation;
mod structural;

pub use error::VerificationError;
pub use natives::{NativeParam, NativeSignature, builtin_natives};

pub struct BytecodeVerifier {
    config: CompilerConfig,
    natives: Vec<NativeSignature>,
}

impl BytecodeVerifier {
    /// `adapter_natives` — signatures for adapter-supplied natives
    /// (meow_vm_transfer, meow_vm_destroy, meow_vm_sender, etc.).
    /// Language built-ins (meow_vm_abort, meow_vm_fresh_id) are always included.
    pub fn new(adapter_natives: Vec<NativeSignature>, config: CompilerConfig) -> Self {
        let mut natives = builtin_natives();
        natives.extend(adapter_natives);
        Self { config, natives }
    }

    pub fn verify(
        &self,
        module: &Module,
        deps: &HashMap<Address, &Module>,
    ) -> Result<(), Vec<VerificationError>> {
        let mut errors = structural::check_module(module, deps, &self.config);

        for func in &module.functions {
            errors.extend(abstract_interpretation::check_function(
                func,
                module,
                deps,
                &self.natives,
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
