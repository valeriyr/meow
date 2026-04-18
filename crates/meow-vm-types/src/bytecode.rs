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
    /// For Object values: moves (consumes the slot). For all others: copies.
    Load(u8),
    /// Pop the stack top and store it in local slot `n`.
    Store(u8),
    /// Read a field from local slot `n` WITHOUT consuming it. Push field value.
    LoadField(u8, String),
    /// Pop a value and write it into a field of local slot `n` WITHOUT consuming the slot.
    StoreField(u8, String),

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
    // ─── Struct / Object operations ───
    //
    /// Pop `field_names.len()` values (pushed in field-definition order),
    /// then construct a struct or object and push it.
    NewStruct {
        type_name: String,
        /// Field names in struct-definition order.
        field_names: Vec<String>,
    },
    /// Pop a struct/object from the stack, push the value of the named field.
    /// This is a consuming operation — the struct/object is gone after this.
    GetField(String),

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
}
