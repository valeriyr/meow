//! Code generator: walks the AST and emits bytecode [`Instruction`]s.

use std::collections::HashMap;

use meow_vm_types::{
    address::Address,
    bytecode::Instruction,
    config::CompilerConfig,
    module::{Function, Module},
    types::{StructDef, Type},
};

use crate::{
    Result,
    ast::{AstFunction, BinOp, Expr, Stmt, UnaryOp},
    error::CompilerError,
    validator,
};

pub struct Codegen<'m> {
    config: &'m CompilerConfig,
    structs: &'m [StructDef],
    /// Maps a dep module's human-readable name to its on-chain address.
    /// Used to translate `module_name::something` → `@<hex>::something` in bytecode.
    dep_addresses: &'m HashMap<String, Address>,
    /// Maps each dep module's address to its compiled Module.
    /// Used to look up function visibility and return types for cross-module calls.
    dep_modules: HashMap<Address, &'m Module>,
    /// Maps local function names to their return types.
    /// Used to infer types of locals assigned from same-module function calls.
    local_fn_return_types: &'m HashMap<String, Option<Type>>,
    locals: HashMap<String, u8>,
    /// Tracks the inferred source-level type name for each local variable.
    /// Cross-module types use the `"dep_name::TypeName"` form.
    /// Primitives are tracked too (e.g. `"u64"`) for completeness.
    local_types: HashMap<String, String>,
    next_slot: u8,
    code: Vec<Instruction>,
}

impl<'m> Codegen<'m> {
    pub fn compile_function(
        structs: &'m [StructDef],
        dep_addresses: &'m HashMap<String, Address>,
        dep_modules: HashMap<Address, &'m Module>,
        local_fn_return_types: &'m HashMap<String, Option<Type>>,
        ast_fn: AstFunction,
        config: &'m CompilerConfig,
    ) -> Result<Function> {
        validator::validate_function_name(&ast_fn.name, config)?;

        let max_params = config.max_params();
        if ast_fn.params.len() > max_params {
            return Err(CompilerError::Message(format!(
                "function '{}': too many parameters ({} > limit of {})",
                ast_fn.name,
                ast_fn.params.len(),
                max_params,
            )));
        }

        let mut cg = Codegen::new(
            structs,
            dep_addresses,
            dep_modules,
            local_fn_return_types,
            config,
        );

        // Allocate slots for parameters and record their types.
        let mut params: Vec<(String, Type)> = Vec::with_capacity(ast_fn.params.len());
        for (name, ty) in ast_fn.params {
            validator::validate_identifier(
                &name,
                &format!("parameter in function '{}'", ast_fn.name),
                config,
            )?;
            cg.alloc_local(name.clone())?;
            let ty = cg.translate_type(ty)?;
            // Record the source-level type name for type tracking.
            cg.local_types.insert(name.clone(), ty.name().to_string());
            params.push((name, ty));
        }

        let return_type = ast_fn
            .return_type
            .map(|rt| cg.translate_type(rt))
            .transpose()?;

        for stmt in ast_fn.body {
            cg.compile_stmt(stmt)?;
        }

        // Ensure the function always ends with Return.
        if cg.code.last() != Some(&Instruction::Return) {
            cg.emit(Instruction::Return);
        }

        let max_fun_code_size = config.max_fun_code_size();
        if cg.code.len() > max_fun_code_size {
            return Err(CompilerError::Message(format!(
                "function '{}': bytecode too large ({} instructions > limit of {})",
                ast_fn.name,
                cg.code.len(),
                max_fun_code_size,
            )));
        }

        Ok(Function {
            name: ast_fn.name,
            is_public: ast_fn.is_public,
            params,
            return_type,
            local_count: cg.next_slot,
            code: cg.code,
        })
    }

    fn new(
        structs: &'m [StructDef],
        dep_addresses: &'m HashMap<String, Address>,
        dep_modules: HashMap<Address, &'m Module>,
        local_fn_return_types: &'m HashMap<String, Option<Type>>,
        config: &'m CompilerConfig,
    ) -> Self {
        Self {
            config,
            structs,
            dep_addresses,
            dep_modules,
            local_fn_return_types,
            locals: HashMap::new(),
            local_types: HashMap::new(),
            next_slot: 0,
            code: Vec::new(),
        }
    }

    fn alloc_local(&mut self, name: String) -> Result<u8> {
        validator::validate_identifier(&name, "variable", self.config)?;
        let max_locals = self.config.max_locals();
        if self.next_slot as usize >= max_locals {
            return Err(CompilerError::Message(format!(
                "too many local variables: limit is {}",
                max_locals,
            )));
        }
        let slot = self.next_slot;
        self.locals.insert(name, slot);
        self.next_slot += 1;
        Ok(slot)
    }

    fn emit(&mut self, instr: Instruction) {
        self.code.push(instr);
    }

    /// Translate a potentially-qualified name to its address-qualified bytecode form.
    ///
    /// `"module_name::something"` → `"@<hex_address>::something"`
    /// `"plain_name"` → `"plain_name"` (unchanged)
    ///
    /// Returns an error if `module_name` is used but not found in `dep_addresses`
    /// (i.e. the module was not declared via `use module_name;`).
    /// Translate a [`Type`] so that any `Struct("dep_name::TypeName")` is converted
    /// to `Struct("@<hex_address>::TypeName")`.  Other types pass through unchanged.
    fn translate_type(&self, ty: Type) -> Result<Type> {
        match ty {
            Type::Struct(name) => Ok(Type::Struct(self.translate_name(&name)?)),
            Type::Tuple(types) => Ok(Type::Tuple(
                types
                    .into_iter()
                    .map(|t| self.translate_type(t))
                    .collect::<Result<_>>()?,
            )),
            _ => Ok(ty),
        }
    }

    fn translate_name(&self, name: &str) -> Result<String> {
        if let Some((mod_name, rest)) = name.split_once("::") {
            match self.dep_addresses.get(mod_name) {
                Some(addr) => Ok(format!("@{addr}::{rest}")),
                None => Err(CompilerError::Message(format!(
                    "reference to undeclared module '{mod_name}' — add `use {mod_name};` at the top of the file",
                ))),
            }
        } else {
            Ok(name.to_string())
        }
    }

    /// Returns true if `type_name` refers to a type declared in a different module
    /// (i.e. it uses the `dep_name::TypeName` qualified form).
    fn is_cross_module_type(type_name: &str) -> bool {
        type_name.contains("::")
    }

    /// Infer the source-level type name of `expr` without compiling it.
    ///
    /// Returns `None` when the type cannot be determined statically (e.g. a
    /// function call whose return type is unknown). Callers treat `None` as
    /// "unknown" and skip visibility checks that would require the type.
    fn infer_type(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Bool(_) => Some("bool".to_string()),
            Expr::Int(_) => Some("u64".to_string()),
            Expr::Address(_) => Some("address".to_string()),
            Expr::Str(_) => Some("string".to_string()),
            Expr::Ident(name) => self.local_types.get(name).cloned(),
            Expr::StructLit { name, .. } => Some(name.clone()),
            Expr::FieldAccess { expr: base, field } => {
                let base_type = self.infer_type(base)?;
                let def = self.structs.iter().find(|s| s.name == base_type)?;
                let field_def = def.fields.iter().find(|f| f.name == *field)?;
                Some(field_def.ty.name().to_string())
            }
            Expr::Call { name, .. } => {
                if let Some((dep_name, fn_local_name)) = name.split_once("::") {
                    // Cross-module call — look up the dep module's function return type.
                    let dep_addr = self.dep_addresses.get(dep_name)?;
                    let dep_module = self.dep_modules.get(dep_addr)?;
                    let func = dep_module.get_function(fn_local_name)?;
                    func.return_type.as_ref().map(|t| match t {
                        // Qualify struct return types with the dep module name.
                        Type::Struct(local_name) => format!("{dep_name}::{local_name}"),
                        _ => t.name().to_string(),
                    })
                } else {
                    // Same-module call — look up in pre-collected local function map.
                    let return_type = self.local_fn_return_types.get(name.as_str())?;
                    return_type.as_ref().map(|t| t.name().to_string())
                }
            }
            Expr::BinOp { op, .. } => Some(match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => "u64".to_string(),
                _ => "bool".to_string(),
            }),
            Expr::Tuple(_) => None,
            Expr::UnaryOp { .. } => Some("bool".to_string()),
        }
    }

    /// Check that calling `fn_name` (which may be `"dep::fn"`) is allowed.
    ///
    /// Cross-module function calls are only allowed when the target function
    /// is declared `pub` in the dep module.
    fn check_fn_visibility(&self, fn_name: &str) -> Result<()> {
        let Some((dep_name, fn_local_name)) = fn_name.split_once("::") else {
            return Ok(()); // local call — always allowed
        };
        let dep_addr = self.dep_addresses.get(dep_name).ok_or_else(|| {
            CompilerError::Message(format!("reference to undeclared module '{dep_name}'",))
        })?;
        let dep_module = self.dep_modules.get(dep_addr).ok_or_else(|| {
            CompilerError::Message(format!("dependency module '{dep_name}' not found",))
        })?;
        let func = dep_module.get_function(fn_local_name).ok_or_else(|| {
            CompilerError::Message(format!(
                "function '{fn_local_name}' not found in module '{dep_name}'",
            ))
        })?;
        if !func.is_public {
            return Err(CompilerError::Message(format!(
                "function '{fn_name}' is private — only `pub fn` can be called from other modules",
            )));
        }
        Ok(())
    }

    /// Check that reading `field` on a value of `type_name` is allowed.
    ///
    /// All fields are private — cross-module field reads are always rejected.
    /// Within the declaring module, all field reads are allowed.
    fn check_field_read(&self, type_name: &str, _field: &str) -> Result<()> {
        if Self::is_cross_module_type(type_name) {
            return Err(CompilerError::Message(format!(
                "fields of '{type_name}' are private — use a public getter function to access them",
            )));
        }
        Ok(())
    }

    /// Check that writing `field` on a value of the given type is allowed.
    ///
    /// Rejects any field write to a type declared in another module.
    /// Within the declaring module, field writes are always allowed.
    fn check_field_write(&self, type_name: &str, field: &str) -> Result<()> {
        // Reject all field writes from outside the declaring module.
        if Self::is_cross_module_type(type_name) {
            return Err(CompilerError::Message(format!(
                "field '{field}' of '{type_name}' cannot be written from outside the declaring module",
            )));
        }

        Ok(())
    }

    fn compile_expr(&mut self, expr: Expr) -> Result<()> {
        match expr {
            Expr::Bool(v) => self.emit(Instruction::PushBool(v)),
            Expr::Int(v) => self.emit(Instruction::PushU64(v)),
            Expr::Address(addr) => self.emit(Instruction::PushAddress(addr)),
            Expr::Str(s) => self.emit(Instruction::PushStr(s)),

            Expr::Ident(name) => {
                let slot = self.locals.get(&name).copied().ok_or_else(|| {
                    CompilerError::Message(format!("undefined variable '{name}'"))
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
                    BinOp::Mod => Instruction::Mod,
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
                // Flatten nested FieldAccess chains rooted on a local variable into a
                // single LoadField(slot, path) — this avoids cloning intermediate structs.
                let (root_expr, mut path) = flatten_field_chain(*expr, field);
                path.reverse(); // flatten_field_chain builds the path in reverse

                if let Expr::Ident(ref name) = root_expr
                    && let Some(&slot) = self.locals.get(name)
                {
                    if let Some(type_name) = self.local_types.get(name).cloned() {
                        self.check_field_read(&type_name, &path[0])?;
                    }
                    self.emit(Instruction::LoadField(slot, path));
                    return Ok(());
                }
                // Fallback: compile base expression, then GetField for each step.
                if let Some(type_name) = self.infer_type(&root_expr) {
                    self.check_field_read(&type_name, &path[0])?;
                }
                self.compile_expr(root_expr)?;
                for step in path {
                    self.emit(Instruction::GetField(step));
                }
            }

            Expr::StructLit { name, fields } => {
                // Cross-module struct construction is always forbidden.
                if Self::is_cross_module_type(&name) {
                    return Err(CompilerError::Message(format!(
                        "cannot construct '{name}' outside its declaring module — \
                         structs can only be created where they are defined",
                    )));
                }

                // Look up the struct def using the source-level name.
                let def = self
                    .structs
                    .iter()
                    .find(|s| s.name == name)
                    .ok_or_else(|| CompilerError::Message(format!("unknown struct '{name}'")))?
                    .clone();

                let mut literal_map: HashMap<String, Expr> = fields.into_iter().collect();

                let field_names: Vec<String> = def.fields.iter().map(|f| f.name.clone()).collect();
                for field_name in &field_names {
                    let expr = literal_map.remove(field_name).ok_or_else(|| {
                        CompilerError::Message(format!(
                            "missing field '{field_name}' in struct literal '{name}'"
                        ))
                    })?;
                    self.compile_expr(expr)?;
                }

                // Emit NewStruct with the address-qualified type name.
                let type_name = self.translate_name(&name)?;
                self.emit(Instruction::NewStruct {
                    type_name,
                    field_names,
                });
            }

            Expr::UnaryOp { op, expr } => {
                self.compile_expr(*expr)?;
                match op {
                    UnaryOp::Not => self.emit(Instruction::Not),
                }
            }

            Expr::Tuple(items) => {
                let n = items.len();
                let max = self.config.max_tuple_elements();
                if n > max {
                    return Err(CompilerError::Message(format!(
                        "tuple literal has {n} elements, exceeding the limit of {max}"
                    )));
                }
                for item in items {
                    self.compile_expr(item)?;
                }
                self.emit(Instruction::MakeTuple(n as u8));
            }

            Expr::Call { name, args } => {
                // Access check for cross-module function calls.
                self.check_fn_visibility(&name)?;

                for arg in args {
                    self.compile_expr(arg)?;
                }
                let translated = self.translate_name(&name)?;
                self.emit(Instruction::Call(translated));
            }
        }
        Ok(())
    }

    fn compile_stmt(&mut self, stmt: Stmt) -> Result<()> {
        match stmt {
            Stmt::Let { name, expr } => {
                // Infer the type before compiling (compile_expr consumes the expr).
                let inferred_type = self.infer_type(&expr);
                self.compile_expr(expr)?;
                let slot = self.alloc_local(name.clone())?;
                self.emit(Instruction::Store(slot));
                if let Some(ty) = inferred_type {
                    self.local_types.insert(name, ty);
                }
            }

            Stmt::Reassign { name, expr } => {
                let slot = self.locals.get(&name).copied().ok_or_else(|| {
                    CompilerError::Message(format!("undefined variable '{name}'"))
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

            Stmt::If {
                cond,
                body,
                else_body,
            } => {
                self.compile_expr(cond)?;
                let patch_cond = self.code.len();
                self.emit(Instruction::JumpIfNot(0));
                for s in body {
                    self.compile_stmt(s)?;
                }
                if let Some(else_stmts) = else_body {
                    // Emit Jump to skip else body after then body executes.
                    let patch_jump = self.code.len();
                    self.emit(Instruction::Jump(0));
                    // Patch JumpIfNot to land on first instruction of else body.
                    self.code[patch_cond] =
                        Instruction::JumpIfNot((self.code.len() - patch_cond) as i32);
                    for s in else_stmts {
                        self.compile_stmt(s)?;
                    }
                    // Patch Jump to land after else body.
                    self.code[patch_jump] =
                        Instruction::Jump((self.code.len() - patch_jump) as i32);
                } else {
                    self.code[patch_cond] =
                        Instruction::JumpIfNot((self.code.len() - patch_cond) as i32);
                }
            }

            Stmt::LetTuple { names, expr } => {
                let n = names.len();
                let max = self.config.max_tuple_elements();
                if n > max {
                    return Err(CompilerError::Message(format!(
                        "tuple destructuring has {n} elements, exceeding the limit of {max}"
                    )));
                }
                self.compile_expr(expr)?;
                self.emit(Instruction::UnpackTuple(n as u8));
                // UnpackTuple pushes elements with element[0] on top.
                for name in names {
                    let slot = self.alloc_local(name.clone())?;
                    self.emit(Instruction::Store(slot));
                }
            }

            Stmt::LetStruct {
                type_name,
                bindings,
                expr,
                rest_discarded,
            } => {
                let def = self
                    .structs
                    .iter()
                    .find(|s| s.name == type_name)
                    .ok_or_else(|| CompilerError::Message(format!("unknown struct '{type_name}'")))?
                    .clone();

                let def_field_names: Vec<String> =
                    def.fields.iter().map(|f| f.name.clone()).collect();

                // Check for unknown fields (in source order for deterministic errors)
                for (field_name, _) in &bindings {
                    if !def_field_names.contains(field_name) {
                        return Err(CompilerError::Message(format!(
                            "struct destructuring of '{type_name}': unknown field '{field_name}'"
                        )));
                    }
                }

                let binding_map: HashMap<String, String> = bindings.into_iter().collect();

                // Without `..`, all fields must be explicitly bound
                if !rest_discarded {
                    for field_name in &def_field_names {
                        if !binding_map.contains_key(field_name) {
                            return Err(CompilerError::Message(format!(
                                "struct destructuring of '{type_name}': missing binding for field '{field_name}' (use `..` to discard remaining fields)"
                            )));
                        }
                    }
                }

                self.compile_expr(expr)?;

                let translated_type = self.translate_name(&type_name)?;
                self.emit(Instruction::UnpackStruct {
                    type_name: translated_type,
                    field_names: def_field_names.clone(),
                });

                for field_name in &def_field_names {
                    if let Some(binding_name) = binding_map.get(field_name) {
                        let field_ty = def
                            .fields
                            .iter()
                            .find(|f| &f.name == field_name)
                            .unwrap()
                            .ty
                            .name()
                            .to_string();
                        let slot = self.alloc_local(binding_name.clone())?;
                        self.local_types.insert(binding_name.clone(), field_ty);
                        self.emit(Instruction::Store(slot));
                    } else {
                        // rest_discarded: unbound field is dropped — forbidden for linear types
                        let field_def = def.fields.iter().find(|f| &f.name == field_name).unwrap();
                        if matches!(field_def.ty, Type::Struct(_)) {
                            return Err(CompilerError::Message(format!(
                                "struct destructuring of '{type_name}': cannot discard linear field '{field_name}' with `..`; bind it explicitly"
                            )));
                        }
                        self.emit(Instruction::Pop);
                    }
                }
            }

            Stmt::FieldAssign {
                obj_name,
                field_path,
                expr,
            } => {
                let slot = self.locals.get(&obj_name).copied().ok_or_else(|| {
                    CompilerError::Message(format!("undefined variable '{obj_name}'"))
                })?;

                // Visibility check on the first element of the path.
                let first_field = &field_path[0];
                if let Some(type_name) = self.local_types.get(&obj_name).cloned() {
                    self.check_field_write(&type_name, first_field)?;
                }

                self.compile_expr(expr)?;
                self.emit(Instruction::StoreField(slot, field_path));
            }
        }
        Ok(())
    }
}

/// Unwind a chain of `FieldAccess` nodes into `(root_expr, reversed_path)`.
///
/// `a.b.c` is represented as `FieldAccess { expr: FieldAccess { expr: Ident("a"), field: "b" }, field: "c" }`.
/// This returns `(Ident("a"), ["c", "b"])` — caller must reverse the path before use.
fn flatten_field_chain(expr: Expr, field: String) -> (Expr, Vec<String>) {
    let mut path = vec![field];
    let mut current = expr;
    loop {
        match current {
            Expr::FieldAccess {
                expr: inner,
                field: f,
            } => {
                path.push(f);
                current = *inner;
            }
            other => return (other, path),
        }
    }
}
