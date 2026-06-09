//! Type checker pass that runs between parsing and bytecode generation.
//!
//! Validates types at the source level — before module addresses are embedded into type names —
//! so that error messages refer to the names the developer wrote, not bytecode-qualified paths.
//! Running as a separate pass (rather than interleaved with codegen) means type errors are
//! always reported completely, not cut off by the first code-generation failure.

use std::{collections::HashMap, str::FromStr};

use meow_vm_types::{
    address::Address, module::Module, module_ref, natives::NativeParam, types::Type,
};

use crate::{
    NativeSig, Result,
    ast::{AstFunction, AstStruct, BinOp, Expr, Stmt, UnaryOp},
    error::CompilerError,
};

//
// ─── Type checker ───
//

struct StructInfo {
    fields: Vec<(String, Type)>,
}

pub struct TypeChecker<'m> {
    source_structs: HashMap<String, StructInfo>,
    /// Concrete function signatures for local fns and dep-module fns.
    source_fns: HashMap<String, (Vec<Type>, Option<Type>)>,
    /// Native function signatures; see `NativeParam` variants for accepted types.
    native_fns: HashMap<String, (Vec<NativeParam>, Option<Type>)>,
    fn_name: &'m str,
    return_type: Option<Type>,
}

impl<'m> TypeChecker<'m> {
    fn new(
        source_structs: HashMap<String, StructInfo>,
        source_fns: HashMap<String, (Vec<Type>, Option<Type>)>,
        native_fns: HashMap<String, (Vec<NativeParam>, Option<Type>)>,
    ) -> Self {
        TypeChecker {
            source_structs,
            source_fns,
            native_fns,
            fn_name: "",
            return_type: None,
        }
    }

    fn check_function(&mut self, func: &'m AstFunction) -> Result<()> {
        self.fn_name = &func.name;
        self.return_type = func.return_type.clone();

        let mut locals: HashMap<String, Type> = func
            .params
            .iter()
            .map(|(name, ty)| (name.clone(), ty.clone()))
            .collect();

        for stmt in &func.body {
            self.check_stmt(stmt, &mut locals)?;
        }

        Ok(())
    }

    fn type_err(&self, msg: impl Into<String>) -> CompilerError {
        CompilerError::Message(format!("in function '{}': {}", self.fn_name, msg.into()))
    }

    fn expr_type(&self, expr: &Expr, locals: &HashMap<String, Type>) -> Result<Option<Type>> {
        match expr {
            Expr::Bool(_) => Ok(Some(Type::Bool)),
            Expr::Int(_) => Ok(Some(Type::U64)),
            Expr::Address(_) => Ok(Some(Type::Address)),
            Expr::Str(_) => Ok(Some(Type::Str)),

            Expr::Ident(name) => {
                let ty = locals
                    .get(name.as_str())
                    .ok_or_else(|| self.type_err(format!("undefined variable '{name}'")))?;
                Ok(Some(ty.clone()))
            }

            Expr::UnaryOp {
                op: UnaryOp::Not,
                expr,
            } => {
                let ty = self.require_typed(expr, locals, "unary '!'")?;
                self.expect_type(&ty, &Type::Bool, "operand of '!'")?;
                Ok(Some(Type::Bool))
            }

            Expr::BinOp { left, op, right } => self.check_binop(left, *op, right, locals).map(Some),

            Expr::StructLit { name, fields } => {
                // Cross-module construction: codegen handles visibility — skip type check.
                if module_ref::is_qualified(name) {
                    return Ok(Some(Type::Struct(name.clone())));
                }

                let info = self
                    .source_structs
                    .get(name.as_str())
                    .ok_or_else(|| self.type_err(format!("unknown struct '{name}'")))?;
                let def_fields = info.fields.clone();

                let lit_map: HashMap<&str, &Expr> =
                    fields.iter().map(|(k, v)| (k.as_str(), v)).collect();

                for (field_name, field_ty) in &def_fields {
                    let field_expr = lit_map.get(field_name.as_str()).ok_or_else(|| {
                        self.type_err(format!(
                            "missing field '{field_name}' in struct literal '{name}'"
                        ))
                    })?;

                    let actual = self.require_typed(
                        field_expr,
                        locals,
                        &format!("field '{field_name}' in '{name}'"),
                    )?;
                    self.expect_type(
                        &actual,
                        field_ty,
                        &format!("field '{field_name}' in '{name}'"),
                    )?;
                }

                for (field_name, _) in fields {
                    if !def_fields.iter().any(|(n, _)| n == field_name) {
                        return Err(self.type_err(format!(
                            "unknown field '{field_name}' in struct literal '{name}'"
                        )));
                    }
                }

                Ok(Some(Type::Struct(name.clone())))
            }

            Expr::FieldAccess { expr: base, field } => {
                let base_ty = self.require_typed(base, locals, "field access base")?;
                let struct_name = match &base_ty {
                    Type::Struct(n) => n.clone(),
                    other => {
                        return Err(self.type_err(format!(
                            "field access '.{}' requires a struct, found {}",
                            field,
                            type_display(other)
                        )));
                    }
                };

                if module_ref::is_qualified(&struct_name) {
                    return Err(self.type_err(format!(
                        "fields of '{struct_name}' are private — use a public getter function to access them"
                    )));
                }

                let info = self
                    .source_structs
                    .get(struct_name.as_str())
                    .ok_or_else(|| self.type_err(format!("unknown struct '{struct_name}'")))?;
                let field_ty = info
                    .fields
                    .iter()
                    .find(|(n, _)| n == field)
                    .map(|(_, ty)| ty.clone())
                    .ok_or_else(|| {
                        self.type_err(format!("no field '{field}' on struct '{struct_name}'"))
                    })?;
                Ok(Some(field_ty))
            }

            Expr::Call { name, args } => self.check_call(name, args, locals),

            Expr::Tuple(items) => {
                let mut types = Vec::with_capacity(items.len());
                for item in items {
                    let ty = self.require_typed(item, locals, "tuple element")?;
                    types.push(ty);
                }
                Ok(Some(Type::Tuple(types)))
            }
        }
    }

    fn require_typed(
        &self,
        expr: &Expr,
        locals: &HashMap<String, Type>,
        context: &str,
    ) -> Result<Type> {
        match self.expr_type(expr, locals)? {
            Some(ty) => Ok(ty),
            None => Err(self.type_err(format!(
                "{context}: void expression cannot be used as a value"
            ))),
        }
    }

    fn expect_type(&self, actual: &Type, expected: &Type, context: &str) -> Result<()> {
        if actual == expected {
            Ok(())
        } else {
            Err(self.type_err(format!(
                "{context}: expected {}, found {}",
                type_display(expected),
                type_display(actual)
            )))
        }
    }

    fn check_binop(
        &self,
        left: &Expr,
        op: BinOp,
        right: &Expr,
        locals: &HashMap<String, Type>,
    ) -> Result<Type> {
        let lty = self.require_typed(left, locals, "left operand")?;
        let rty = self.require_typed(right, locals, "right operand")?;

        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                self.expect_type(&lty, &Type::U64, "left operand of arithmetic op")?;
                self.expect_type(&rty, &Type::U64, "right operand of arithmetic op")?;
                Ok(Type::U64)
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                self.expect_type(&lty, &Type::U64, "left operand of comparison")?;
                self.expect_type(&rty, &Type::U64, "right operand of comparison")?;
                Ok(Type::Bool)
            }
            BinOp::Eq | BinOp::Ne => {
                if lty.is_linear() || rty.is_linear() {
                    let linear_ty = if lty.is_linear() { &lty } else { &rty };
                    return Err(self.type_err(format!(
                        "'{linear_ty}': struct types and tuples containing structs cannot be compared with == or != — destructure and compare fields individually"
                    )));
                }
                self.expect_type(&rty, &lty, "right operand of equality")?;
                Ok(Type::Bool)
            }
            BinOp::And | BinOp::Or => {
                self.expect_type(&lty, &Type::Bool, "left operand of logical op")?;
                self.expect_type(&rty, &Type::Bool, "right operand of logical op")?;
                Ok(Type::Bool)
            }
        }
    }

    fn check_call(
        &self,
        name: &str,
        args: &[Expr],
        locals: &HashMap<String, Type>,
    ) -> Result<Option<Type>> {
        let mut arg_types: Vec<Type> = Vec::with_capacity(args.len());
        for arg in args {
            arg_types.push(self.require_typed(arg, locals, &format!("argument to '{name}'"))?);
        }

        // Check native functions first (may have `AnyStruct` params meaning "any struct").
        if let Some((param_types, ret_type)) = self.native_fns.get(name) {
            if arg_types.len() != param_types.len() {
                return Err(self.type_err(format!(
                    "call to '{name}': expected {} argument(s), found {}",
                    param_types.len(),
                    arg_types.len()
                )));
            }
            for (i, (actual, param)) in arg_types.iter().zip(param_types.iter()).enumerate() {
                match param {
                    NativeParam::Concrete(expected) => {
                        self.expect_type(
                            actual,
                            expected,
                            &format!("argument {} of '{name}'", i + 1),
                        )?;
                    }
                    NativeParam::AnyStruct => match actual {
                        Type::Struct(_) => {}
                        other => {
                            return Err(self.type_err(format!(
                                "argument {} of '{name}': expected a struct, found {}",
                                i + 1,
                                type_display(other)
                            )));
                        }
                    },
                    NativeParam::LocalStruct => match actual {
                        Type::Struct(n) if !module_ref::is_qualified(n) => {}
                        other => {
                            return Err(self.type_err(format!(
                                "argument {} of '{name}': expected a struct defined in this module, found {}",
                                i + 1,
                                type_display(other)
                            )));
                        }
                    },
                }
            }
            return Ok(ret_type.clone());
        }

        // Then check user-defined (local and dep-module) functions.
        if let Some((param_types, ret_type)) = self.source_fns.get(name) {
            if arg_types.len() != param_types.len() {
                return Err(self.type_err(format!(
                    "call to '{name}': expected {} argument(s), found {}",
                    param_types.len(),
                    arg_types.len()
                )));
            }
            for (i, (actual, expected)) in arg_types.iter().zip(param_types.iter()).enumerate() {
                self.expect_type(actual, expected, &format!("argument {} of '{name}'", i + 1))?;
            }
            return Ok(ret_type.clone());
        }

        // Unknown function — codegen will catch real errors.
        Ok(None)
    }

    fn check_stmt(&self, stmt: &Stmt, locals: &mut HashMap<String, Type>) -> Result<()> {
        match stmt {
            Stmt::Let { name, expr } => {
                let ty_opt = self.expr_type(expr, locals)?;
                match ty_opt {
                    Some(ty) => {
                        locals.insert(name.clone(), ty);
                    }
                    None => {
                        return Err(self.type_err(format!(
                            "let binding '{name}': void expression cannot be used as a value"
                        )));
                    }
                }
            }

            Stmt::Reassign { name, expr } => {
                let expected = locals.get(name.as_str()).cloned().ok_or_else(|| {
                    self.type_err(format!("assignment to undefined variable '{name}'"))
                })?;
                if let Some(actual) = self.expr_type(expr, locals)? {
                    self.expect_type(&actual, &expected, &format!("assignment to '{name}'"))?;
                }
            }

            Stmt::Return(opt_expr) => match (&self.return_type, opt_expr) {
                (None, None) => {}
                (None, Some(e)) => {
                    self.expr_type(e, locals)?;
                }
                (Some(ret_ty), None) => {
                    return Err(self.type_err(format!(
                        "return without value in function that returns {}",
                        type_display(ret_ty)
                    )));
                }
                (Some(ret_ty), Some(e)) => {
                    if let Some(actual) = self.expr_type(e, locals)? {
                        self.expect_type(&actual, ret_ty, "return value")?;
                    }
                }
            },

            Stmt::Expr(expr) => {
                self.expr_type(expr, locals)?;
            }

            Stmt::If {
                cond,
                body,
                else_body,
            } => {
                let cond_ty = self.require_typed(cond, locals, "if condition")?;
                self.expect_type(&cond_ty, &Type::Bool, "if condition")?;

                let mut then_locals = locals.clone();
                for s in body {
                    self.check_stmt(s, &mut then_locals)?;
                }
                if let Some(else_stmts) = else_body {
                    let mut else_locals = locals.clone();
                    for s in else_stmts {
                        self.check_stmt(s, &mut else_locals)?;
                    }
                }
            }

            Stmt::FieldAssign {
                obj_name,
                field_path,
                expr,
            } => {
                let obj_ty = locals
                    .get(obj_name.as_str())
                    .cloned()
                    .ok_or_else(|| self.type_err(format!("undefined variable '{obj_name}'")))?;

                // Traverse the field path to find the target field type.
                let mut current_ty = obj_ty;
                let path_display = format!("{}.{}", obj_name, field_path.join("."));
                for (i, field_name) in field_path.iter().enumerate() {
                    let struct_name = match &current_ty {
                        Type::Struct(n) => n.clone(),
                        other => {
                            return Err(self.type_err(format!(
                                "field access on non-struct type {} in '{path_display}'",
                                type_display(other)
                            )));
                        }
                    };

                    // Cross-module: skip type check, codegen enforces privacy.
                    if module_ref::is_qualified(&struct_name) {
                        self.expr_type(expr, locals)?;
                        return Ok(());
                    }

                    let info = self
                        .source_structs
                        .get(struct_name.as_str())
                        .ok_or_else(|| self.type_err(format!("unknown struct '{struct_name}'")))?;
                    let field_ty = info
                        .fields
                        .iter()
                        .find(|(n, _)| n == field_name)
                        .map(|(_, ty)| ty.clone())
                        .ok_or_else(|| {
                            self.type_err(format!(
                                "no field '{field_name}' on struct '{struct_name}'"
                            ))
                        })?;

                    if i == field_path.len() - 1 {
                        if let Some(actual) = self.expr_type(expr, locals)? {
                            self.expect_type(
                                &actual,
                                &field_ty,
                                &format!("assignment to '{path_display}'"),
                            )?;
                        }
                    } else {
                        current_ty = field_ty;
                    }
                }
            }

            Stmt::LetTuple { names, expr } => {
                let ty = self.require_typed(expr, locals, "tuple destructuring")?;
                let tuple_types = match ty {
                    Type::Tuple(ts) => ts,
                    other => {
                        return Err(self.type_err(format!(
                            "let tuple destructuring: expected a tuple, found {}",
                            type_display(&other)
                        )));
                    }
                };
                if names.len() != tuple_types.len() {
                    return Err(self.type_err(format!(
                        "let tuple: {} names but tuple has {} elements",
                        names.len(),
                        tuple_types.len()
                    )));
                }
                for (name, ty) in names.iter().zip(tuple_types.into_iter()) {
                    if name != "_" {
                        locals.insert(name.clone(), ty);
                    }
                }
            }

            Stmt::LetStruct {
                type_name,
                bindings,
                expr,
                rest_discarded: _,
            } => {
                let ty = self.require_typed(expr, locals, "struct destructuring")?;
                // Verify expr produces the expected struct type (skip for cross-module types).
                if !module_ref::is_qualified(type_name) {
                    let expected = Type::Struct(type_name.clone());
                    self.expect_type(
                        &ty,
                        &expected,
                        &format!("struct destructuring of '{type_name}'"),
                    )?;
                }
                // Bind each named field to locals, rejecting unknown fields.
                if let Some(info) = self.source_structs.get(type_name.as_str()) {
                    let fields = info.fields.clone();
                    for (field_name, binding_name) in bindings {
                        if binding_name == "_" {
                            continue;
                        }
                        match fields.iter().find(|(n, _)| n == field_name) {
                            Some((_, field_ty)) => {
                                locals.insert(binding_name.clone(), field_ty.clone());
                            }
                            None => {
                                return Err(self.type_err(format!(
                                    "struct destructuring of '{type_name}': unknown field '{field_name}'"
                                )));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

//
// ─── Public entry point ───
//

pub fn check(
    ast_structs: &[&AstStruct],
    dep_addresses: &HashMap<String, Address>,
    dep_modules: &HashMap<Address, &Module>,
    native_sigs: &[NativeSig],
    local_fn_param_types: &HashMap<String, Vec<Type>>,
    local_fn_return_types: &HashMap<String, Option<Type>>,
    ast_fns: &[&AstFunction],
) -> Result<()> {
    let addr_to_dep: HashMap<Address, String> = dep_addresses
        .iter()
        .map(|(name, addr)| (*addr, name.clone()))
        .collect();

    // ── Build source_structs ─────────────────────────────────────────────────
    let mut source_structs: HashMap<String, StructInfo> = HashMap::new();

    for s in ast_structs {
        source_structs.insert(
            s.name.clone(),
            StructInfo {
                fields: s.fields.clone(),
            },
        );
    }

    for (addr, dep_module) in dep_modules {
        let dep_name = match addr_to_dep.get(addr) {
            Some(n) => n.clone(),
            None => continue,
        };
        let dep_struct_names: Vec<&str> =
            dep_module.structs.iter().map(|s| s.name.as_str()).collect();
        for s in &dep_module.structs {
            if !s.is_public {
                continue;
            }
            let qualified_name = format!("{}::{}", dep_name, s.name);
            let source_fields: Vec<(String, Type)> = s
                .fields
                .iter()
                .map(|f| {
                    let ty = reverse_translate_type(&f.ty, &addr_to_dep);
                    let ty = qualify_dep_local_type(&ty, &dep_name, &dep_struct_names);
                    (f.name.clone(), ty)
                })
                .collect();
            source_structs.insert(
                qualified_name,
                StructInfo {
                    fields: source_fields,
                },
            );
        }
    }

    // ── Build source_fns (local and dep-module functions — concrete param types) ─
    let mut source_fns: HashMap<String, (Vec<Type>, Option<Type>)> = HashMap::new();

    for (fn_name, param_types) in local_fn_param_types {
        let ret = local_fn_return_types.get(fn_name).cloned().flatten();
        source_fns.insert(fn_name.clone(), (param_types.clone(), ret));
    }

    for (addr, dep_module) in dep_modules {
        let dep_name = match addr_to_dep.get(addr) {
            Some(n) => n.clone(),
            None => continue,
        };
        let dep_struct_names: Vec<&str> =
            dep_module.structs.iter().map(|s| s.name.as_str()).collect();
        for func in &dep_module.functions {
            if !func.is_public {
                continue;
            }
            let qualified_name = format!("{}::{}", dep_name, func.name);
            let params: Vec<Type> = func
                .params
                .iter()
                .map(|(_, ty)| {
                    let ty = reverse_translate_type(ty, &addr_to_dep);
                    qualify_dep_local_type(&ty, &dep_name, &dep_struct_names)
                })
                .collect();
            let ret = func.return_type.as_ref().map(|ty| {
                let ty = reverse_translate_type(ty, &addr_to_dep);
                qualify_dep_local_type(&ty, &dep_name, &dep_struct_names)
            });
            source_fns.insert(qualified_name, (params, ret));
        }
    }

    // ── Build native_fns (caller-provided + VM built-ins) ───────────────────
    let mut native_fns: HashMap<String, (Vec<NativeParam>, Option<Type>)> = HashMap::new();

    for sig in native_sigs {
        native_fns.insert(
            sig.name.clone(),
            (sig.params.clone(), sig.return_type.clone()),
        );
    }

    // ── Type-check each function ─────────────────────────────────────────────
    let mut checker = TypeChecker::new(source_structs, source_fns, native_fns);
    for func in ast_fns {
        checker.check_function(func)?;
    }

    Ok(())
}

//
// ─── Helpers ───
//

fn type_display(ty: &Type) -> String {
    match ty {
        Type::Bool => "bool".to_string(),
        Type::U64 => "u64".to_string(),
        Type::Address => "address".to_string(),
        Type::Str => "string".to_string(),
        Type::Struct(name) => name.clone(),
        Type::Tuple(types) => {
            format!(
                "({})",
                types
                    .iter()
                    .map(type_display)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

/// Translate a bytecode-qualified type (`@0xADDR::rest`) back to source-level form
/// (`dep_name::rest`), using the inverse address-to-dep-name map.
fn reverse_translate_type(ty: &Type, addr_to_dep: &HashMap<Address, String>) -> Type {
    match ty {
        Type::Struct(name) if name.starts_with('@') => {
            if let Some(rest_pos) = name.find("::") {
                let addr_str = &name[1..rest_pos]; // "0xADDR"
                if let Ok(addr) = Address::from_str(addr_str)
                    && let Some(dep_name) = addr_to_dep.get(&addr)
                {
                    return Type::Struct(format!("{}::{}", dep_name, &name[rest_pos + 2..]));
                }
            }
            ty.clone()
        }
        Type::Tuple(types) => Type::Tuple(
            types
                .iter()
                .map(|t| reverse_translate_type(t, addr_to_dep))
                .collect(),
        ),
        _ => ty.clone(),
    }
}

/// Qualify a type from within `dep_name`'s module: plain struct references become
/// `dep_name::StructName` so they match source-level qualified names in the caller.
fn qualify_dep_local_type(ty: &Type, dep_name: &str, dep_module_struct_names: &[&str]) -> Type {
    match ty {
        Type::Struct(name) if !module_ref::is_qualified(name) && !name.starts_with('@') => {
            if dep_module_struct_names.contains(&name.as_str()) {
                Type::Struct(format!("{}::{}", dep_name, name))
            } else {
                ty.clone()
            }
        }
        Type::Tuple(types) => Type::Tuple(
            types
                .iter()
                .map(|t| qualify_dep_local_type(t, dep_name, dep_module_struct_names))
                .collect(),
        ),
        _ => ty.clone(),
    }
}
