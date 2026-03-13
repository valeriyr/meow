use chumsky::prelude::*;
use meow_vm_types::types::Type;

use crate::ast::{AstFunction, AstItem, AstStruct, BinOp, Expr, Stmt};

type ParseErr<'src> = extra::Err<Rich<'src, char>>;

pub fn parser<'src>() -> impl Parser<'src, &'src str, Vec<AstItem>, ParseErr<'src>> {
    // Helpers
    let ident = text::ascii::ident().map(|s: &str| s.to_string()).padded();

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
        let postfix_op = just('.')
            .padded()
            .ignore_then(text::ascii::ident().map(|s: &str| s.to_string()).padded());

        let postfix = primary
            .clone()
            .then(postfix_op.repeated().collect::<Vec<_>>())
            .map(|(e, fields)| {
                fields.into_iter().fold(e, |e, f| Expr::FieldAccess {
                    expr: Box::new(e),
                    field: f,
                })
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
                Some((op, r)) => Expr::BinOp {
                    left: Box::new(l),
                    op,
                    right: Box::new(r),
                },
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

        // if statement: `if expr { stmts }` or `if expr { stmts } else { stmts }`
        let block = stmt
            .clone()
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(just('{').padded(), just('}').padded());

        let if_stmt = kw("if")
            .ignore_then(expr.clone())
            .then(block.clone())
            .then(kw("else").ignore_then(block.clone()).or_not())
            .map(|((cond, body), else_body)| Stmt::If {
                cond,
                body,
                else_body,
            });

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
            .map(|((obj_name, field), e)| Stmt::FieldAssign {
                obj_name,
                field,
                expr: e,
            });

        let expr_stmt = expr.clone().then_ignore(just(';').padded()).map(Stmt::Expr);

        choice((
            let_stmt,
            return_stmt,
            if_stmt,
            field_assign,
            reassign,
            expr_stmt,
        ))
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
            AstItem::Struct(AstStruct {
                name,
                fields,
                is_object: false,
            })
        });

    // object Foo { id: address, x: u64 }
    let object_item = kw("object")
        .ignore_then(ident.clone())
        .then(struct_body.clone())
        .map(|(name, fields)| {
            AstItem::Struct(AstStruct {
                name,
                fields,
                is_object: true,
            })
        });

    // fn foo(a: u64, b: bool): RetType { ... }
    let param = ident
        .clone()
        .then_ignore(just(':').padded())
        .then(ty.clone());

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
            AstItem::Fn(AstFunction {
                name,
                params,
                return_type,
                body,
            })
        });

    let item = choice((struct_item, object_item, fn_item));

    item.repeated().collect::<Vec<_>>().padded()
}
