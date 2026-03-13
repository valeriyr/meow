use meow_vm_types::types::Type;

#[derive(Debug, Clone)]
pub enum Expr {
    Bool(bool),
    /// All integer literals are stored as u64.
    Int(u64),
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
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub return_type: Option<Type>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct AstStruct {
    pub name: String,
    pub fields: Vec<(String, Type)>,
    pub is_object: bool,
}

#[derive(Debug, Clone)]
pub enum AstItem {
    Struct(AstStruct),
    Fn(AstFunction),
}
