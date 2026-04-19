use meow_vm_types::bytecode::Instruction;

/// Per-instruction gas costs. Pass a custom schedule to [`Vm::with_gas_schedule`].
#[derive(Debug, Clone)]
pub struct GasSchedule {
    /// Cost of pushing a primitive value (`bool`, `u64`, or `address`) onto the stack.
    push_primitive: u64,
    /// Cost of pushing a string literal onto the stack.
    push_str: u64,
    /// Cost of a local variable load or store (`Load`/`Store`).
    load_store: u64,
    /// Cost of borrowing a field from a struct or object slot via `LoadField`.
    load_field: u64,
    /// Cost of writing a value back into a field of a struct or object slot via `StoreField`.
    store_field: u64,
    /// Cost of addition, subtraction, or multiplication (`Add`, `Sub`, `Mul`).
    add_sub_mul: u64,
    /// Cost of integer division (`Div`), which is more expensive due to the divide-by-zero check.
    div: u64,
    /// Cost of integer modulo (`Mod`), same expense as division.
    mod_: u64,
    /// Cost of any comparison operation (`Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`).
    compare: u64,
    /// Cost of boolean logic operations (`Not`, `And`, `Or`).
    logic: u64,
    /// Base cost charged once per `NewStruct` instruction, before per-field costs.
    new_struct_base: u64,
    /// Additional cost charged per field when constructing a struct or object.
    new_struct_per_field: u64,
    /// Cost of extracting a field from a struct or object value on the stack via `GetField`.
    get_field: u64,
    /// Cost of stack manipulation instructions (`Pop`, `Dup`).
    stack: u64,
    /// Cost of any jump instruction (`Jump`, `JumpIf`, `JumpIfNot`).
    jump: u64,
    /// Cost of a function call dispatch (`Call`), covering frame setup overhead.
    call: u64,
    /// Cost of the `Return` instruction.
    return_: u64,
}

impl Default for GasSchedule {
    fn default() -> Self {
        Self {
            push_primitive: 1,
            push_str: 2,
            load_store: 1,
            load_field: 2,
            store_field: 5,
            add_sub_mul: 2,
            div: 5,
            mod_: 5,
            compare: 2,
            logic: 1,
            new_struct_base: 10,
            new_struct_per_field: 2,
            get_field: 3,
            stack: 1,
            jump: 2,
            call: 20,
            return_: 2,
        }
    }
}

impl GasSchedule {
    /// Returns the gas cost for a single instruction.
    pub fn cost_of(&self, instr: &Instruction) -> u64 {
        match instr {
            Instruction::PushBool(_) | Instruction::PushU64(_) | Instruction::PushAddress(_) => {
                self.push_primitive
            }
            Instruction::PushStr(_) => self.push_str,

            Instruction::Load(_) | Instruction::Store(_) => self.load_store,
            Instruction::LoadField(_, _) => self.load_field,
            Instruction::StoreField(_, _) => self.store_field,

            Instruction::Add | Instruction::Sub | Instruction::Mul => self.add_sub_mul,
            Instruction::Div => self.div,
            Instruction::Mod => self.mod_,

            Instruction::Eq
            | Instruction::Ne
            | Instruction::Lt
            | Instruction::Le
            | Instruction::Gt
            | Instruction::Ge => self.compare,

            Instruction::Not | Instruction::And | Instruction::Or => self.logic,

            Instruction::NewStruct { field_names, .. } => {
                self.new_struct_base + field_names.len() as u64 * self.new_struct_per_field
            }
            Instruction::GetField(_) => self.get_field,

            Instruction::Pop | Instruction::Dup => self.stack,

            Instruction::Jump(_) | Instruction::JumpIf(_) | Instruction::JumpIfNot(_) => self.jump,

            Instruction::Call(_) => self.call,
            Instruction::Return => self.return_,

            Instruction::MakeTuple(n) => *n as u64,
            Instruction::UnpackTuple(n) => *n as u64,
        }
    }
}
