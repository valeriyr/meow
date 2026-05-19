//! Bytecode instruction set executed by the Meow VM.

use serde::{Deserialize, Serialize};

use crate::address::Address;

/// A single VM instruction.
///
/// The VM is stack-based. Local variables occupy numbered slots; slot indices
/// start at 0 with the first function parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Instruction {
    //
    // ─── Literals ───
    //
    PushBool(bool),
    PushU64(u64),
    /// Push an address literal written as `@0x...` in source.
    PushAddress(Address),
    /// Push a string literal (only for native call arguments like meow_vm_abort).
    PushStr(String),

    //
    // ─── Local variables ───
    //
    /// Copy/move the value at local slot `n` onto the stack.
    /// For Struct values: moves (consumes the slot). For all others: copies.
    Load(u8),
    /// Pop the stack top and store it in local slot `n`.
    Store(u8),
    /// Read through a field path from local slot `n` WITHOUT consuming it. Push terminal value.
    ///
    /// `path` is a non-empty list of field names traversed in order (e.g. `["balance", "amount"]`).
    /// Every intermediate field must be a struct; the terminal value is pushed onto the stack.
    LoadField(u8, Vec<String>),
    /// Pop a value and write it into the terminal field reached by `path` on local slot `n`,
    /// WITHOUT consuming the slot. Every intermediate field must be a struct.
    StoreField(u8, Vec<String>),

    //
    // ─── Arithmetic ───
    //
    Add,
    Sub,
    Mul,
    /// Integer division; errors on division by zero.
    Div,
    /// Integer modulo; errors on division by zero.
    Mod,

    //
    // ─── Comparison ───
    //
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    //
    // ─── Boolean logic ───
    //
    Not,
    And,
    Or,

    //
    // ─── Struct operations ───
    //
    /// Pop `field_names.len()` values (pushed in field-definition order),
    /// then construct a struct and push it.
    NewStruct {
        type_name: String,
        /// Field names in struct-definition order.
        field_names: Vec<String>,
    },
    /// Pop a struct from the stack, push the value of the named field.
    /// This is a consuming operation — the struct is gone after this.
    GetField(String),
    /// Pop a struct from the stack and push all its fields in definition order,
    /// with the first field ending up on top (inverse of `NewStruct`).
    /// Used to implement struct destructuring: `let TypeName { f1, f2 } = expr;`
    UnpackStruct {
        type_name: String,
        field_names: Vec<String>,
    },

    //
    // ─── Stack manipulation ───
    //
    Pop,
    Dup,

    //
    // ─── Control flow ───
    //
    /// Unconditional jump by a relative offset (in instructions, signed).
    Jump(i32),
    /// Pop a Bool; jump if `true`.
    JumpIf(i32),
    /// Pop a Bool; jump if `false`.
    JumpIfNot(i32),

    //
    // ─── Functions ───
    //
    /// Call a function (module or native) by name. Arguments must already be
    /// on the stack in left-to-right order.
    Call(String),
    /// Return the top-of-stack value to the caller (or Void if stack is empty).
    Return,

    //
    // ─── Tuples ───
    //
    /// Pop `n` values (rightmost on top) and pack them into a `Tuple([v0, v1, ..., vn-1])`.
    MakeTuple(u8),
    /// Pop a `Tuple` of size `n` and push its elements (first element ends up on top).
    UnpackTuple(u8),
}
