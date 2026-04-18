use std::str::FromStr;

use chumsky::prelude::*;
use meow_vm_types::{address::Address, types::Type};

use crate::ast::{AstFunction, AstItem, AstStruct, BinOp, Expr, Stmt};

/// Strip `//` line comments from source, preserving all character positions
/// by replacing comment text (not the newline) with spaces.
/// String literals are handled correctly — `//` inside `"..."` is not a comment.
pub fn strip_line_comments(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut in_string = false;
    let mut chars = source.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_string {
            result.push(ch);
            if ch == '"' {
                in_string = false;
            }
        } else if ch == '"' {
            result.push(ch);
            in_string = true;
        } else if ch == '/' && chars.peek() == Some(&'/') {
            // Consume the second '/' and everything up to (not including) the newline.
            chars.next(); // consume second '/'
            result.push(' ');
            result.push(' ');
            for ch2 in chars.by_ref() {
                if ch2 == '\n' {
                    result.push('\n');
                    break;
                } else {
                    result.push(' ');
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

type ParseErr<'src> = extra::Err<Rich<'src, char>>;
type BoxedParser<'src, T> = chumsky::Boxed<'src, 'src, &'src str, T, ParseErr<'src>>;

pub fn parser<'src>() -> impl Parser<'src, &'src str, Vec<AstItem>, ParseErr<'src>> {
    // Helpers
    let ident = text::ascii::ident().map(|s: &str| s.to_string()).padded();

    let kw = |s: &'static str| text::ascii::keyword(s).padded().ignored();

    // Type parser: primitives, struct/object names, and qualified names (`module::Type`).
    let ty = text::ascii::ident()
        .map(|s: &str| s.to_string())
        .then(
            just("::")
                .ignore_then(text::ascii::ident().map(|s: &str| s.to_string()))
                .or_not(),
        )
        .map(|(first, second)| {
            if let Some(second) = second {
                Type::Struct(format!("{}::{}", first, second))
            } else {
                Type::from_name(&first)
            }
        })
        .padded();

    // ── Address literal parser ─────────────────────────────────────────────────
    // Matches `@0x...` (with optional leading zeros, short forms accepted).
    let hex_digits = text::digits(16).at_least(1).collect::<String>().padded();

    // Boxed to cut the type chain — `try_map` produces a complex type.
    let address_literal: BoxedParser<'src, Address> = just("@0x")
        .padded()
        .ignore_then(hex_digits)
        .try_map(|hex: String, span| {
            Address::from_str(&format!("0x{hex}"))
                .map_err(|_| Rich::custom(span, format!("invalid address literal: @0x{hex}")))
        })
        .boxed();

    // ── Expressions ───────────────────────────────────────────────────────────
    let expr: BoxedParser<'src, Expr> = recursive(|expr| {
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

        // address literal: `@0x...`
        let addr_lit = address_literal.clone().map(Expr::Address);

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

        // Identifier-based: call / struct-lit / plain ident.
        // Supports both simple names (`foo`) and qualified names (`module::foo`).
        #[derive(Clone)]
        enum IdentSuffix {
            Call(Vec<Expr>),
            Struct(Vec<(String, Expr)>),
            None,
        }

        let ident_expr = text::ascii::ident()
            .map(|s: &str| s.to_string())
            .padded()
            .then(
                just("::")
                    .ignore_then(text::ascii::ident().map(|s: &str| s.to_string()).padded())
                    .or_not(),
            )
            .map(|(first, second)| {
                if let Some(s) = second {
                    format!("{}::{}", first, s)
                } else {
                    first
                }
            })
            .then(choice((
                call_args.map(IdentSuffix::Call),
                struct_fields.map(IdentSuffix::Struct),
                empty().map(|()| IdentSuffix::None),
            )))
            .map(|(name, suffix)| match suffix {
                IdentSuffix::Call(args) => Expr::Call { name, args },
                IdentSuffix::Struct(fields) => Expr::StructLit { name, fields },
                IdentSuffix::None => Expr::Ident(name),
            })
            .boxed();

        // Parenthesised expression: `(expr)` — used for explicit grouping.
        let grouped = expr
            .clone()
            .delimited_by(just('(').padded(), just(')').padded());

        // Primary — box to erase the large `choice` type.
        let primary: BoxedParser<'src, Expr> =
            choice((bool_lit, str_lit, addr_lit, int_lit, ident_expr, grouped)).boxed();

        // Postfix: field access `a.field` (no tuple indices).
        let postfix_op = just('.')
            .padded()
            .ignore_then(text::ascii::ident().map(|s: &str| s.to_string()).padded());

        let postfix: BoxedParser<'src, Expr> = primary
            .clone()
            .then(postfix_op.repeated().collect::<Vec<_>>())
            .map(|(e, fields)| {
                fields.into_iter().fold(e, |e, f| Expr::FieldAccess {
                    expr: Box::new(e),
                    field: f,
                })
            })
            .boxed();

        // Multiplicative
        let mul_op = choice((
            just('*').padded().to(BinOp::Mul),
            just('/').padded().to(BinOp::Div),
            just('%').padded().to(BinOp::Mod),
        ));
        let mul: BoxedParser<'src, Expr> = postfix
            .clone()
            .then(mul_op.then(postfix).repeated().collect::<Vec<_>>())
            .map(|(first, rest)| {
                rest.into_iter().fold(first, |l, (op, r)| Expr::BinOp {
                    left: Box::new(l),
                    op,
                    right: Box::new(r),
                })
            })
            .boxed();

        // Additive
        let add_op = choice((
            just('+').padded().to(BinOp::Add),
            just('-').padded().to(BinOp::Sub),
        ));
        let add: BoxedParser<'src, Expr> = mul
            .clone()
            .then(add_op.then(mul).repeated().collect::<Vec<_>>())
            .map(|(first, rest)| {
                rest.into_iter().fold(first, |l, (op, r)| Expr::BinOp {
                    left: Box::new(l),
                    op,
                    right: Box::new(r),
                })
            })
            .boxed();

        // Comparison (non-associative, at most one)
        let cmp_op = choice((
            just("==").padded().to(BinOp::Eq),
            just("!=").padded().to(BinOp::Ne),
            just("<=").padded().to(BinOp::Le),
            just(">=").padded().to(BinOp::Ge),
            just('<').padded().to(BinOp::Lt),
            just('>').padded().to(BinOp::Gt),
        ));
        let cmp: BoxedParser<'src, Expr> = add
            .clone()
            .then(cmp_op.then(add).or_not())
            .map(|(l, rhs)| match rhs {
                Some((op, r)) => Expr::BinOp {
                    left: Box::new(l),
                    op,
                    right: Box::new(r),
                },
                None => l,
            })
            .boxed();

        // Logical AND
        let and: BoxedParser<'src, Expr> = cmp
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
            })
            .boxed();

        // Logical OR
        and.clone()
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
            })
    })
    .boxed();

    // ── Statements ────────────────────────────────────────────────────────────
    let stmt: BoxedParser<'src, Stmt> = recursive(|stmt| {
        let let_stmt = kw("let")
            .ignore_then(ident)
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
        let reassign = ident
            .then_ignore(just('=').padded())
            .then(expr.clone())
            .then_ignore(just(';').padded())
            .map(|(name, e)| Stmt::Reassign { name, expr: e });

        // Field assignment: `ident.field = expr;`
        let field_assign = ident
            .then_ignore(just('.').padded())
            .then(ident)
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
    })
    .boxed();

    // ── Top-level items ───────────────────────────────────────────────────────

    // Optional `pub` keyword — produces `true` if present, `false` otherwise.
    let is_pub = text::ascii::keyword("pub")
        .padded()
        .to(true)
        .or_not()
        .map(|b| b.unwrap_or(false));

    // struct field: `pub? name: type`
    let struct_field = text::ascii::keyword("pub")
        .padded()
        .to(true)
        .or_not()
        .map(|b| b.unwrap_or(false))
        .then(ident)
        .then_ignore(just(':').padded())
        .then(ty)
        .map(|((is_field_pub, name), ty)| (is_field_pub, name, ty));

    let struct_body = struct_field
        .separated_by(just(',').padded())
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just('{').padded(), just('}').padded());

    // pub? struct Foo { pub? x: u64, y: bool }
    let struct_item: BoxedParser<'src, AstItem> = is_pub
        .clone()
        .then_ignore(kw("struct"))
        .then(ident)
        .then(struct_body.clone())
        .map(|((is_public, name), fields)| {
            AstItem::Struct(AstStruct {
                is_public,
                name,
                fields,
                is_object: false,
            })
        })
        .boxed();

    // pub? object Foo { id: address, pub? x: u64 }
    let object_item: BoxedParser<'src, AstItem> = is_pub
        .clone()
        .then_ignore(kw("object"))
        .then(ident)
        .then(struct_body)
        .map(|((is_public, name), fields)| {
            AstItem::Struct(AstStruct {
                is_public,
                name,
                fields,
                is_object: true,
            })
        })
        .boxed();

    // pub? fn foo(a: u64, b: bool) -> RetType { ... }
    let param = ident.then_ignore(just(':').padded()).then(ty);

    let fn_item: BoxedParser<'src, AstItem> = is_pub
        .then_ignore(kw("fn"))
        .then(ident)
        .then(
            param
                .separated_by(just(',').padded())
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just('(').padded(), just(')').padded()),
        )
        .then(just("->").padded().ignore_then(ty).or_not())
        .then(
            stmt.repeated()
                .collect::<Vec<_>>()
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map(|((((is_public, name), params), return_type), body)| {
            AstItem::Fn(AstFunction {
                is_public,
                name,
                params,
                return_type,
                body,
            })
        })
        .boxed();

    // mod NAME;
    let module_decl: BoxedParser<'src, AstItem> = kw("mod")
        .ignore_then(ident)
        .then_ignore(just(';').padded())
        .map(AstItem::ModuleDecl)
        .boxed();

    // use module_name@0x...;
    let use_item: BoxedParser<'src, AstItem> = kw("use")
        .ignore_then(ident)
        .then_ignore(just('@').padded())
        .then(
            just("0x")
                .padded()
                .ignore_then(hex_digits)
                .try_map(|hex: String, span| {
                    let full = format!("0x{hex}");
                    Address::from_str(&full).map_err(|_| {
                        Rich::custom(span, format!("invalid address in use declaration: 0x{hex}"))
                    })
                }),
        )
        .then_ignore(just(';').padded())
        .map(|(name, address)| AstItem::Use { name, address })
        .boxed();

    let item: BoxedParser<'src, AstItem> =
        choice((module_decl, use_item, struct_item, object_item, fn_item)).boxed();

    item.repeated().collect::<Vec<_>>().padded()
}
