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
//! use meow_vm::{compiler::Compiler, vm::{GasMeter, GasSchedule, Vm}, types::Value};
//!
//! let source = r#"
//!     fn add(a: u64, b: u64): u64 {
//!         return a + b;
//!     }
//! "#;
//!
//! let module = Compiler::compile("math", source).unwrap();
//! let vm = Vm::new(module, vec![], GasSchedule::default());
//! let mut gas = GasMeter::new(1_000);
//!
//! let result = vm.call("add", vec![Value::U64(3), Value::U64(4)], &mut gas).unwrap();
//! assert_eq!(result.return_value, Some(Value::U64(7)));
//! println!("gas consumed: {}", gas.consumed());
//! ```

pub mod bytecode;
pub mod compiler;
pub mod module;
pub mod types;
pub mod vm;
