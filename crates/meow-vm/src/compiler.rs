/// Compiler: source text → [`Module`].
///
/// # Language overview
///
/// ```text
/// object Coin { id: address, value: u64 }
/// struct Point { x: u64, y: u64 }
///
/// fn add(a: u64, b: u64): u64 {
///     return a + b;
/// }
///
/// fn make_point(x: u64, y: u64): Point {
///     return Point { x: x, y: y };
/// }
/// ```
///
/// ## Supported features
/// - Primitive types: `bool`, `u64`, `address`
/// - User-defined structs with primitive-typed fields
/// - User-defined objects with `id: address` as first field
/// - Functions with parameters and an optional return type (not Object)
/// - `let` bindings, `return` statements
/// - `if` statements (without else)
/// - Field assignment: `obj.field = expr;`
/// - Binary operators: `+`, `-`, `*`, `/`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`
/// - Field access: `expr.field` (LoadField for locals, GetField for stack values)
/// - Struct/object literals: `Foo { field: expr, … }`
/// - Function calls: `foo(arg, …)` (module or native)
/// - String literals: `"..."` (for native function arguments)
use std::collections::HashMap;

use chumsky::prelude::*;

use crate::{
    bytecode::Instruction,
    error::{Result, VmError},
    module::{Function, Module},
    types::{StructDef, Type},
};

// ─── AST ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum Expr {
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
enum BinOp {
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
enum Stmt {
    Let { name: String, expr: Expr },
    /// Reassign an existing variable: `name = expr;`
    Reassign { name: String, expr: Expr },
    Return(Option<Expr>),
    /// A bare expression whose value is discarded.
    Expr(Expr),
    /// An `if` statement without else.
    If { cond: Expr, body: Vec<Stmt> },
    /// Field assignment: `ident.field = expr;`
    FieldAssign { obj_name: String, field: String, expr: Expr },
}

#[derive(Debug, Clone)]
struct AstFunction {
    name: String,
    params: Vec<(String, Type)>,
    return_type: Option<Type>,
    body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
struct AstStruct {
    name: String,
    fields: Vec<(String, Type)>,
    is_object: bool,
}

#[derive(Debug, Clone)]
enum AstItem {
    Struct(AstStruct),
    Fn(AstFunction),
}

// ─── Parser ──────────────────────────────────────────────────────────────────

type ParseErr<'src> = extra::Err<Rich<'src, char>>;

fn parser<'src>() -> impl Parser<'src, &'src str, Vec<AstItem>, ParseErr<'src>> {
    // Helpers
    let ident = text::ascii::ident()
        .map(|s: &str| s.to_string())
        .padded();

    let kw = |s: &'static str| text::ascii::keyword(s).padded().ignored();

    // Type parser: only primitives and struct/object names (no tuples).
    let ty = text::ascii::ident()
        .map(|s: &str| Type::from_name(s))
        .padded();

    // ── Expressions ───────────────────────────────────────────────────────────
    let expr = recursive(|expr| {
        // bool literals
        let bool_lit = choice((
            text::ascii::keyword("true").to(Expr::Bool(true)),
            text::ascii::keyword("false").to(Expr::Bool(false)),
        ))
        .padded();

        // integer literal
        let int_lit = text::int(10)
            .map(|s: &str| Expr::Int(s.parse::<u64>().unwrap_or(0)))
            .padded();

        // string literal
        let str_lit = just('"')
            .ignore_then(none_of('"').repeated().collect::<String>())
            .then_ignore(just('"'))
            .padded()
            .map(Expr::Str);

        // function-call args: (expr, expr, ...)
        let call_args = expr
            .clone()
            .separated_by(just(',').padded())
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just('(').padded(), just(')').padded());

        // struct-literal fields: { name: expr, ... }
        let struct_fields = text::ascii::ident()
            .map(|s: &str| s.to_string())
            .padded()
            .then_ignore(just(':').padded())
            .then(expr.clone())
            .separated_by(just(',').padded())
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just('{').padded(), just('}').padded());

        // Identifier-based: call / struct-lit / plain ident
        #[derive(Clone)]
        enum IdentSuffix {
            Call(Vec<Expr>),
            Struct(Vec<(String, Expr)>),
            None,
        }

        let ident_expr = text::ascii::ident()
            .map(|s: &str| s.to_string())
            .padded()
            .then(choice((
                call_args.map(IdentSuffix::Call),
                struct_fields.map(IdentSuffix::Struct),
                empty().map(|()| IdentSuffix::None),
            )))
            .map(|(name, suffix)| match suffix {
                IdentSuffix::Call(args) => Expr::Call { name, args },
                IdentSuffix::Struct(fields) => Expr::StructLit { name, fields },
                IdentSuffix::None => Expr::Ident(name),
            });

        // Primary
        let primary = choice((bool_lit, str_lit, int_lit, ident_expr));

        // Postfix: field access `a.field` (no tuple indices).
        let postfix_op = just('.').padded().ignore_then(
            text::ascii::ident()
                .map(|s: &str| s.to_string())
                .padded(),
        );

        let postfix = primary
            .clone()
            .then(postfix_op.repeated().collect::<Vec<_>>())
            .map(|(e, fields)| {
                fields.into_iter().fold(e, |e, f| Expr::FieldAccess { expr: Box::new(e), field: f })
            });

        // Multiplicative
        let mul_op = choice((
            just('*').padded().to(BinOp::Mul),
            just('/').padded().to(BinOp::Div),
        ));
        let mul = postfix
            .clone()
            .then(mul_op.then(postfix).repeated().collect::<Vec<_>>())
            .map(|(first, rest)| {
                rest.into_iter().fold(first, |l, (op, r)| Expr::BinOp {
                    left: Box::new(l),
                    op,
                    right: Box::new(r),
                })
            });

        // Additive
        let add_op = choice((
            just('+').padded().to(BinOp::Add),
            just('-').padded().to(BinOp::Sub),
        ));
        let add = mul
            .clone()
            .then(add_op.then(mul).repeated().collect::<Vec<_>>())
            .map(|(first, rest)| {
                rest.into_iter().fold(first, |l, (op, r)| Expr::BinOp {
                    left: Box::new(l),
                    op,
                    right: Box::new(r),
                })
            });

        // Comparison (non-associative, at most one)
        let cmp_op = choice((
            just("==").padded().to(BinOp::Eq),
            just("!=").padded().to(BinOp::Ne),
            just("<=").padded().to(BinOp::Le),
            just(">=").padded().to(BinOp::Ge),
            just('<').padded().to(BinOp::Lt),
            just('>').padded().to(BinOp::Gt),
        ));
        let cmp = add
            .clone()
            .then(cmp_op.then(add).or_not())
            .map(|(l, rhs)| match rhs {
                Some((op, r)) => Expr::BinOp { left: Box::new(l), op, right: Box::new(r) },
                None => l,
            });

        // Logical AND
        let and = cmp
            .clone()
            .then(
                just("&&")
                    .padded()
                    .to(BinOp::And)
                    .then(cmp)
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .map(|(first, rest)| {
                rest.into_iter().fold(first, |l, (op, r)| Expr::BinOp {
                    left: Box::new(l),
                    op,
                    right: Box::new(r),
                })
            });

        // Logical OR
        let or = and
            .clone()
            .then(
                just("||")
                    .padded()
                    .to(BinOp::Or)
                    .then(and)
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .map(|(first, rest)| {
                rest.into_iter().fold(first, |l, (op, r)| Expr::BinOp {
                    left: Box::new(l),
                    op,
                    right: Box::new(r),
                })
            });

        or
    });

    // ── Statements ────────────────────────────────────────────────────────────
    let stmt = recursive(|stmt| {
        let let_stmt = kw("let")
            .ignore_then(ident.clone())
            .then_ignore(just('=').padded())
            .then(expr.clone())
            .then_ignore(just(';').padded())
            .map(|(name, e)| Stmt::Let { name, expr: e });

        let return_stmt = kw("return")
            .ignore_then(expr.clone().or_not())
            .then_ignore(just(';').padded())
            .map(Stmt::Return);

        // if statement: `if expr { stmts }`
        let if_stmt = kw("if")
            .ignore_then(expr.clone())
            .then(
                stmt.clone()
                    .repeated()
                    .collect::<Vec<_>>()
                    .delimited_by(just('{').padded(), just('}').padded()),
            )
            .map(|(cond, body)| Stmt::If { cond, body });

        // Variable reassignment: `ident = expr;`
        // Must be tried before expr_stmt. Uses `just('=').padded()` without `.` prefix
        // to distinguish from field_assign.
        let reassign = ident
            .clone()
            .then_ignore(just('=').padded())
            .then(expr.clone())
            .then_ignore(just(';').padded())
            .map(|(name, e)| Stmt::Reassign { name, expr: e });

        // Field assignment: `ident.field = expr;`
        // Must be tried before expr_stmt to avoid `ident.field` being parsed as an expression.
        let field_assign = ident
            .clone()
            .then_ignore(just('.').padded())
            .then(ident.clone())
            .then_ignore(just('=').padded())
            .then(expr.clone())
            .then_ignore(just(';').padded())
            .map(|((obj_name, field), e)| Stmt::FieldAssign { obj_name, field, expr: e });

        let expr_stmt = expr
            .clone()
            .then_ignore(just(';').padded())
            .map(Stmt::Expr);

        choice((let_stmt, return_stmt, if_stmt, field_assign, reassign, expr_stmt))
    });

    // ── Top-level items ───────────────────────────────────────────────────────

    let struct_field = ident
        .clone()
        .then_ignore(just(':').padded())
        .then(ty.clone());

    let struct_body = struct_field
        .clone()
        .separated_by(just(',').padded())
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just('{').padded(), just('}').padded());

    // struct Foo { x: u64, y: bool }
    let struct_item = kw("struct")
        .ignore_then(ident.clone())
        .then(struct_body.clone())
        .map(|(name, fields)| {
            AstItem::Struct(AstStruct { name, fields, is_object: false })
        });

    // object Foo { id: address, x: u64 }
    let object_item = kw("object")
        .ignore_then(ident.clone())
        .then(struct_body.clone())
        .map(|(name, fields)| {
            AstItem::Struct(AstStruct { name, fields, is_object: true })
        });

    // fn foo(a: u64, b: bool): RetType { ... }
    let param = ident.clone().then_ignore(just(':').padded()).then(ty.clone());

    let fn_item = kw("fn")
        .ignore_then(ident.clone())
        .then(
            param
                .separated_by(just(',').padded())
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just('(').padded(), just(')').padded()),
        )
        .then(just(':').padded().ignore_then(ty.clone()).or_not())
        .then(
            stmt.repeated()
                .collect::<Vec<_>>()
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map(|(((name, params), return_type), body)| {
            AstItem::Fn(AstFunction { name, params, return_type, body })
        });

    let item = choice((struct_item, object_item, fn_item));

    item.repeated().collect::<Vec<_>>().padded()
}

// ─── Codegen ─────────────────────────────────────────────────────────────────

struct Codegen<'m> {
    structs: &'m [StructDef],
    locals: HashMap<String, u8>,
    next_slot: u8,
    code: Vec<Instruction>,
}

impl<'m> Codegen<'m> {
    fn new(structs: &'m [StructDef]) -> Self {
        Self { structs, locals: HashMap::new(), next_slot: 0, code: Vec::new() }
    }

    fn alloc_local(&mut self, name: String) -> u8 {
        let slot = self.next_slot;
        self.locals.insert(name, slot);
        self.next_slot += 1;
        slot
    }

    fn emit(&mut self, instr: Instruction) {
        self.code.push(instr);
    }

    fn compile_expr(&mut self, expr: Expr) -> Result<()> {
        match expr {
            Expr::Bool(v) => self.emit(Instruction::PushBool(v)),
            Expr::Int(v) => self.emit(Instruction::PushU64(v)),
            Expr::Str(s) => self.emit(Instruction::PushStr(s)),

            Expr::Ident(name) => {
                let slot = self.locals.get(&name).copied().ok_or_else(|| {
                    VmError::CompileError(format!("undefined variable '{name}'"))
                })?;
                self.emit(Instruction::Load(slot));
            }

            Expr::BinOp { left, op, right } => {
                self.compile_expr(*left)?;
                self.compile_expr(*right)?;
                let instr = match op {
                    BinOp::Add => Instruction::Add,
                    BinOp::Sub => Instruction::Sub,
                    BinOp::Mul => Instruction::Mul,
                    BinOp::Div => Instruction::Div,
                    BinOp::Eq => Instruction::Eq,
                    BinOp::Ne => Instruction::Ne,
                    BinOp::Lt => Instruction::Lt,
                    BinOp::Le => Instruction::Le,
                    BinOp::Gt => Instruction::Gt,
                    BinOp::Ge => Instruction::Ge,
                    BinOp::And => Instruction::And,
                    BinOp::Or => Instruction::Or,
                };
                self.emit(instr);
            }

            Expr::FieldAccess { expr, field } => {
                // Optimise: if the base is a local variable, use LoadField (no move).
                // Otherwise, compile the expression (which may consume a value) then GetField.
                match *expr {
                    Expr::Ident(ref name) => {
                        if let Some(&slot) = self.locals.get(name) {
                            self.emit(Instruction::LoadField(slot, field));
                            return Ok(());
                        }
                    }
                    _ => {}
                }
                // Fallback: compile base expression (may be consuming), then GetField.
                self.compile_expr(*expr)?;
                self.emit(Instruction::GetField(field));
            }

            Expr::StructLit { name, fields } => {
                let def = self
                    .structs
                    .iter()
                    .find(|s| s.name == name)
                    .ok_or_else(|| {
                        VmError::CompileError(format!("unknown struct '{name}'"))
                    })?
                    .clone();

                let mut literal_map: HashMap<String, Expr> = fields.into_iter().collect();

                let field_names: Vec<String> =
                    def.fields.iter().map(|(n, _)| n.clone()).collect();
                for field_name in &field_names {
                    let expr = literal_map.remove(field_name).ok_or_else(|| {
                        VmError::CompileError(format!(
                            "missing field '{field_name}' in struct literal '{name}'"
                        ))
                    })?;
                    self.compile_expr(expr)?;
                }

                self.emit(Instruction::NewStruct {
                    type_name: name,
                    field_names,
                });
            }

            Expr::Call { name, args } => {
                for arg in args {
                    self.compile_expr(arg)?;
                }
                self.emit(Instruction::Call(name));
            }
        }
        Ok(())
    }

    fn compile_stmt(&mut self, stmt: Stmt) -> Result<()> {
        match stmt {
            Stmt::Let { name, expr } => {
                self.compile_expr(expr)?;
                let slot = self.alloc_local(name);
                self.emit(Instruction::Store(slot));
            }

            Stmt::Reassign { name, expr } => {
                let slot = self.locals.get(&name).copied().ok_or_else(|| {
                    VmError::CompileError(format!("undefined variable '{name}'"))
                })?;
                self.compile_expr(expr)?;
                self.emit(Instruction::Store(slot));
            }

            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.compile_expr(e)?;
                }
                self.emit(Instruction::Return);
            }

            Stmt::Expr(expr) => {
                self.compile_expr(expr)?;
                // Always pop: Call always pushes something (Void for void functions).
                self.emit(Instruction::Pop);
            }

            Stmt::If { cond, body } => {
                // Compile condition.
                self.compile_expr(cond)?;
                // Emit placeholder JumpIfNot.
                let patch_pos = self.code.len();
                self.emit(Instruction::JumpIfNot(0));
                // Compile body.
                for s in body {
                    self.compile_stmt(s)?;
                }
                // Patch: offset = code.len() - patch_pos.
                let offset = (self.code.len() - patch_pos) as i32;
                self.code[patch_pos] = Instruction::JumpIfNot(offset);
            }

            Stmt::FieldAssign { obj_name, field, expr } => {
                let slot = self.locals.get(&obj_name).copied().ok_or_else(|| {
                    VmError::CompileError(format!("undefined variable '{obj_name}'"))
                })?;
                self.compile_expr(expr)?;
                self.emit(Instruction::StoreField(slot, field));
            }
        }
        Ok(())
    }

    fn compile_function(
        structs: &'m [StructDef],
        ast_fn: AstFunction,
    ) -> Result<Function> {
        let mut cg = Codegen::new(structs);

        // Validate return type: functions may not return an Object.
        // Note: the parser maps all named types to Type::Struct, so we also check
        // whether the name refers to an object definition in the structs list.
        if let Some(rt) = &ast_fn.return_type {
            let is_object_return = match rt {
                Type::Object(_) => true,
                Type::Struct(name) => {
                    structs.iter().any(|s| s.name == *name && s.is_object)
                }
                _ => false,
            };
            if is_object_return {
                return Err(VmError::CompileError(format!(
                    "function '{}': cannot return Object type '{}'",
                    ast_fn.name,
                    rt.name()
                )));
            }
        }

        // Allocate slots for parameters.
        let params: Vec<(String, Type)> = ast_fn
            .params
            .into_iter()
            .map(|(name, ty)| {
                cg.alloc_local(name.clone());
                (name, ty)
            })
            .collect();

        let return_type = ast_fn.return_type;

        for stmt in ast_fn.body {
            cg.compile_stmt(stmt)?;
        }

        // Ensure the function always ends with Return.
        if cg.code.last() != Some(&Instruction::Return) {
            cg.emit(Instruction::Return);
        }

        Ok(Function {
            name: ast_fn.name,
            params,
            return_type,
            local_count: cg.next_slot,
            code: cg.code,
        })
    }
}

// ─── Validation ──────────────────────────────────────────────────────────────

fn validate_struct_def(def: &AstStruct) -> Result<()> {
    // Validate field types: only primitives allowed.
    for (field_name, ty) in &def.fields {
        if !ty.is_valid_field_type() {
            return Err(VmError::CompileError(format!(
                "{} '{}': field '{field_name}' has non-primitive type '{}' — only bool, u64, address are allowed",
                if def.is_object { "object" } else { "struct" },
                def.name,
                ty.name()
            )));
        }
    }

    // Objects must have `id: address` as their first field.
    if def.is_object {
        match def.fields.first() {
            Some((name, Type::Address)) if name == "id" => {}
            _ => {
                return Err(VmError::CompileError(format!(
                    "object '{}': first field must be 'id: address'",
                    def.name
                )));
            }
        }
    }

    Ok(())
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Compiles source text into a [`Module`].
pub struct Compiler;

impl Compiler {
    /// Compile `source` into a module named `module_name`.
    pub fn compile(module_name: &str, source: &str) -> Result<Module> {
        let items = parser()
            .parse(source)
            .into_result()
            .map_err(|errs| {
                let msg = errs
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ");
                VmError::CompileError(msg)
            })?;

        let mut module = Module::new(module_name);

        // First pass: collect struct/object definitions.
        for item in &items {
            if let AstItem::Struct(ast_struct) = item {
                validate_struct_def(ast_struct)?;
                module.structs.push(StructDef {
                    name: ast_struct.name.clone(),
                    fields: ast_struct.fields.clone(),
                    is_object: ast_struct.is_object,
                });
            }
        }

        // Second pass: compile functions.
        let structs_snapshot = module.structs.clone();
        for item in items {
            if let AstItem::Fn(ast_fn) = item {
                let func = Codegen::compile_function(&structs_snapshot, ast_fn)?;
                module.functions.push(func);
            }
        }

        Ok(module)
    }
}
