//! AST node types produced by the parser and consumed by the code generator.

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
    /// A tuple literal: `(expr1, expr2, ...)`. Requires at least two elements.
    Tuple(Vec<Expr>),
    /// A unary operation: `op expr`.
    UnaryOp {
        op: UnaryOp,
        expr: Box<Expr>,
    },
}

/// Unary operators.
#[derive(Debug, Clone, Copy)]
pub enum UnaryOp {
    /// Boolean not: `!expr` → `bool`.
    Not,
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
    /// Field assignment: `ident.field.field... = expr;`
    FieldAssign {
        obj_name: String,
        /// One or more field names forming the path (e.g. `["balance", "amount"]`).
        field_path: Vec<String>,
        expr: Expr,
    },
    /// Destructuring let: `let (a, b) = expr;`
    LetTuple {
        names: Vec<String>,
        expr: Expr,
    },
    /// Struct destructuring: `let TypeName { field1, field2 } = expr;`
    ///
    /// When `rest_discarded` is `true`, a trailing `..` was present and any
    /// unmentioned fields are silently discarded (popped after `UnpackStruct`).
    LetStruct {
        type_name: String,
        /// Field bindings in source order. Each element is `(field_name, binding_name)`.
        /// For `let Point { x, y } = p;` this is `[("x","x"), ("y","y")]`.
        bindings: Vec<(String, String)>,
        expr: Expr,
        rest_discarded: bool,
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
    /// True if declared with `pub`.
    pub is_public: bool,
    pub name: String,
    /// Fields in declaration order: (name, type).
    /// All fields are private — accessible only within the declaring module.
    pub fields: Vec<(String, Type)>,
}

#[derive(Debug, Clone)]
pub enum AstItem {
    /// `mod NAME;` — declares the module's name. Must be the first item in source.
    ModuleDecl(String),
    /// `use module_name@0x...;` — declares a dependency on a module at a specific address.
    Use {
        name: String,
        address: Address,
    },
    Struct(AstStruct),
    Fn(AstFunction),
}
