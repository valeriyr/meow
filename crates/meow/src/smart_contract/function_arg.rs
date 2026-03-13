use std::str::FromStr;

use meow_types::address::Address;
use meow_vm_adapter::Value;

/// A typed function argument parsed from the command line.
///
/// Parsing rules (applied in order):
/// - `"true"` or `"false"` → [`Bool`](FunctionArg::Bool)
/// - All-digit string → [`Number`](FunctionArg::Number)
/// - Starts with `0x` → [`Address`](FunctionArg::Address) (must be valid hex)
/// - Anything else → [`String`](FunctionArg::String)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionArg {
    /// A boolean argument.
    Bool(bool),
    /// A u64 argument.
    Number(u64),
    /// A 32-byte address argument (hex-encoded with `0x` prefix).
    Address(Address),
    /// A string argument.
    String(String),
}

impl FromStr for FunctionArg {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "true" {
            return Ok(FunctionArg::Bool(true));
        }
        if s == "false" {
            return Ok(FunctionArg::Bool(false));
        }
        if s.chars().all(|c| c.is_ascii_digit()) {
            return s
                .parse::<u64>()
                .map(FunctionArg::Number)
                .map_err(|e| e.into());
        }
        if s.starts_with("0x") || s.starts_with("0X") {
            return Address::from_str(s)
                .map(FunctionArg::Address)
                .map_err(|e| e.into());
        }
        Ok(FunctionArg::String(s.to_owned()))
    }
}

impl From<FunctionArg> for Value {
    fn from(arg: FunctionArg) -> Self {
        match arg {
            FunctionArg::Bool(b) => Value::Bool(b),
            FunctionArg::Number(n) => Value::U64(n),
            FunctionArg::Address(a) => Value::Address(a.into()),
            FunctionArg::String(s) => Value::Str(s),
        }
    }
}
