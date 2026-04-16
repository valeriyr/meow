use meow_vm_types::{address::Address, types::Type};

#[derive(Debug, Clone)]
pub enum Expr {
    Bool(bool),
    /// All integer literals are stored as u64.
    Int(u64),
    /// An address literal written as `@0x...` in source.
    Address(Address),
    /// A string literal (for native function arguments).
    Str(String),
    Ident(String),
    BinOp {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },
    FieldAccess {
        expr: Box<Expr>,
        field: String,
    },
    StructLit {
        name: String,
        fields: Vec<(String, Expr)>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: String,
        expr: Expr,
    },
    /// Reassign an existing variable: `name = expr;`
    Reassign {
        name: String,
        expr: Expr,
    },
    Return(Option<Expr>),
    /// A bare expression whose value is discarded.
    Expr(Expr),
    /// An `if` statement with an optional `else` branch.
    If {
        cond: Expr,
        body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    },
    /// Field assignment: `ident.field = expr;`
    FieldAssign {
        obj_name: String,
        field: String,
        expr: Expr,
    },
}

#[derive(Debug, Clone)]
pub struct AstFunction {
    /// True if this function is declared with `pub`.
    pub is_public: bool,
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub return_type: Option<Type>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct AstStruct {
    /// True if this struct/object is declared with `pub`.
    pub is_public: bool,
    pub name: String,
    /// Fields in declaration order: (is_public, name, type).
    /// A field marked `pub` is readable from other modules (writes remain module-local).
    pub fields: Vec<(bool, String, Type)>,
    pub is_object: bool,
}

#[derive(Debug, Clone)]
pub enum AstItem {
    /// `module NAME;` — declares the module's name. Must be the first item in source.
    ModuleDecl(String),
    /// `use module_name@0x...;` — declares a dependency on a module at a specific address.
    Use {
        name: String,
        address: Address,
    },
    Struct(AstStruct),
    Fn(AstFunction),
}
