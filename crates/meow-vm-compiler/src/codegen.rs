//! Code generator: walks the AST and emits bytecode instructions.

use std::collections::HashMap;

use meow_vm_types::{
    address::Address,
    bytecode::Instruction,
    config::CompilerConfig,
    module::{Function, Module},
    module_ref,
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
    /// Maps each dep module's alias (or name when no alias is given) to its on-chain address.
    /// Used to translate `alias::something` → `@<hex>::something` in bytecode.
    dep_addresses: &'m HashMap<String, Address>,
    /// Maps each dep module's address to its compiled Module.
    /// Used to look up function visibility and return types for cross-module calls.
    dep_modules: HashMap<Address, &'m Module>,
    /// Maps local function names to their return types.
    /// Used to infer types of locals assigned from same-module function calls.
    local_fn_return_types: &'m HashMap<String, Option<Type>>,
    /// Maps each variable name to `(slot, scope_depth)` where `scope_depth` is the
    /// if/else nesting level at which the binding was created. Used by `alloc_local`
    /// to distinguish same-scope shadowing (reuse slot) from inner-scope shadowing
    /// (allocate a new slot to preserve the outer binding).
    locals: HashMap<String, (u8, u32)>,
    /// Tracks the inferred type for each local slot.
    /// Keyed by slot index so that if-body shadowing (outer and inner slots for the
    /// same name) can be tracked independently. Cross-module struct types are stored
    /// as `Type::Struct("alias::TypeName")`.
    local_types: HashMap<u8, Type>,
    next_slot: u8,
    /// Nesting depth of if/else body scopes. Zero means the function's top-level body.
    /// Incremented when entering an if/else body, decremented on exit.
    scope_depth: u32,
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

        let max_params = config.max_params() as usize;
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
            let slot = cg.alloc_local(name.clone())?;
            let ty = cg.translate_type(ty)?;
            // Record the source-level type name for type tracking.
            cg.local_types.insert(slot, ty.clone());
            params.push((name, ty));
        }

        let return_type = ast_fn
            .return_type
            .map(|rt| cg.translate_type(rt))
            .transpose()?;

        for stmt in ast_fn.body {
            cg.compile_stmt(stmt)?;
        }

        cg.check_all_structs_consumed(&ast_fn.name)?;

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
            scope_depth: 0,
            code: Vec::new(),
        }
    }

    fn alloc_local(&mut self, name: String) -> Result<u8> {
        validator::validate_identifier(&name, "variable", self.config)?;
        if let Some(&(existing_slot, existing_depth)) = self.locals.get(&name)
            && existing_depth == self.scope_depth
        {
            // Same-scope shadowing: reuse the existing slot (no orphaned slots).
            // If the old binding still holds a live struct, reject — the struct would be leaked.
            if matches!(self.local_types.get(&existing_slot), Some(ty) if !ty.is_primitive()) {
                return Err(CompilerError::Message(format!(
                    "cannot shadow '{name}': the binding still holds a struct value — \
                         consume or destructure it first",
                )));
            }
            return Ok(existing_slot);
        }
        // Outer-scope binding: allocate a new slot so the outer binding is preserved.
        // The outer `locals` entry is restored when the if/else body exits.
        let max_locals = self.config.max_locals();
        if self.next_slot >= max_locals {
            return Err(CompilerError::Message(format!(
                "too many local variables: limit is {}",
                max_locals,
            )));
        }
        let slot = self.next_slot;
        self.locals.insert(name, (slot, self.scope_depth));
        self.next_slot += 1;
        Ok(slot)
    }

    fn emit(&mut self, instr: Instruction) {
        self.code.push(instr);
    }

    /// Translate a [`Type`] so that any `Struct("dep_name::TypeName")` is converted
    /// to `Struct("@<hex_address>::TypeName")`. Other types pass through unchanged.
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

    /// Translate a potentially-qualified name to its address-qualified bytecode form.
    ///
    /// `"module_name::something"` → `"@<hex_address>::something"`
    /// `"plain_name"` → `"plain_name"` (unchanged)
    ///
    /// Returns an error if `module_name` is not found in `dep_addresses`
    /// (i.e. the module was not declared via `use module_name;`).
    fn translate_name(&self, name: &str) -> Result<String> {
        if let Some((mod_name, rest)) = name.split_once("::") {
            match self.dep_addresses.get(mod_name) {
                Some(addr) => Ok(module_ref::qualify(addr, rest)),
                None => Err(CompilerError::Message(format!(
                    "reference to undeclared module '{mod_name}' — add `use {mod_name};` at the top of the file",
                ))),
            }
        } else {
            Ok(name.to_string())
        }
    }

    /// Infer the source-level type name of `expr` without compiling it.
    ///
    /// Returns `None` when the type cannot be determined statically (e.g. a
    /// function call whose return type is unknown). Callers treat `None` as
    /// "unknown" and skip visibility checks that would require the type.
    fn infer_type(&self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::Bool(_) => Some(Type::Bool),
            Expr::Int(_) => Some(Type::U64),
            Expr::Address(_) => Some(Type::Address),
            Expr::Str(_) => Some(Type::Str),
            Expr::Ident(name) => {
                let &(slot, _) = self.locals.get(name.as_str())?;
                self.local_types.get(&slot).cloned()
            }
            Expr::StructLit { name, .. } => Some(Type::Struct(name.clone())),
            Expr::FieldAccess { expr: base, field } => {
                let base_type = self.infer_type(base)?;
                let def = self.structs.iter().find(|s| s.name == base_type.name())?;
                let field_def = def.fields.iter().find(|f| f.name == *field)?;
                Some(field_def.ty.clone())
            }
            Expr::Call { name, .. } => {
                if let Some((dep_name, fn_local_name)) = name.split_once("::") {
                    // Cross-module call — look up the dep module's function return type.
                    let dep_addr = self.dep_addresses.get(dep_name)?;
                    let dep_module = self.dep_modules.get(dep_addr)?;
                    let func = dep_module.get_function(fn_local_name)?;
                    func.return_type.as_ref().map(|t| match t {
                        // Qualify struct return types with the dep module name.
                        Type::Struct(local_name) => {
                            Type::Struct(format!("{dep_name}::{local_name}"))
                        }
                        other => other.clone(),
                    })
                } else {
                    // Same-module call — look up in pre-collected local function map.
                    self.local_fn_return_types.get(name.as_str())?.clone()
                }
            }
            Expr::BinOp { op, .. } => Some(match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => Type::U64,
                _ => Type::Bool,
            }),
            Expr::Tuple(_) => None,
            Expr::UnaryOp { .. } => Some(Type::Bool),
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

    /// Check that reading a field on a value of the given type is allowed.
    ///
    /// All fields are private — cross-module field reads are always rejected.
    /// Within the declaring module, all field reads are allowed.
    fn check_field_read(&self, ty: &Type, _field: &str) -> Result<()> {
        if ty.is_cross_module() {
            return Err(CompilerError::Message(format!(
                "fields of '{}' are private — use a public getter function to access them",
                ty.name(),
            )));
        }
        Ok(())
    }

    /// Check that writing a field on a value of the given type is allowed.
    ///
    /// Rejects any field write to a type declared in another module.
    /// Within the declaring module, field writes are always allowed.
    fn check_field_write(&self, ty: &Type, field: &str) -> Result<()> {
        if ty.is_cross_module() {
            return Err(CompilerError::Message(format!(
                "field '{field}' of '{}' cannot be written from outside the declaring module",
                ty.name(),
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
                let slot = self.locals.get(&name).map(|&(s, _)| s).ok_or_else(|| {
                    CompilerError::Message(format!("undefined variable '{name}'"))
                })?;
                // Struct loads move the value out of the slot — mark the binding consumed
                // so that a subsequent `let name = ...` knows the slot is safe to reuse.
                if matches!(self.local_types.get(&slot), Some(ty) if !ty.is_primitive()) {
                    self.local_types.remove(&slot);
                }
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
                    && let Some(&(slot, _)) = self.locals.get(name)
                {
                    if let Some(ty) = self.local_types.get(&slot) {
                        self.check_field_read(ty, &path[0])?;
                    }
                    // Reject struct-typed field access — structs have move semantics.
                    if let Expr::Ident(ref name) = root_expr
                        && let Some(&(slot, _)) = self.locals.get(name.as_str())
                        && let Some(root_ty) = self.local_types.get(&slot)
                        && let Some(final_ty) = self.field_path_type(root_ty, &path)
                        && matches!(final_ty, Type::Struct(_))
                    {
                        return Err(CompilerError::Message(format!(
                            "field '{}' has struct type and cannot be accessed directly — structs have move semantics; use destructuring `let TypeName {{ {}, .. }} = expr;`",
                            path.last().unwrap(),
                            path.last().unwrap(),
                        )));
                    }
                    self.emit(Instruction::LoadField(slot, path));
                    return Ok(());
                }
                // Fallback: compile base expression, then GetField for each step.
                // The struct is consumed from the stack by each GetField — check linearity.
                let current_ty = self.infer_type(&root_expr);
                if let Some(ref ty) = current_ty {
                    self.check_field_read(ty, &path[0])?;
                    self.check_getfield_on_struct(ty, &path[0])?;
                }
                self.compile_expr(root_expr)?;
                for step in path {
                    self.emit(Instruction::GetField(step));
                }
            }

            Expr::StructLit { name, fields } => {
                // Cross-module struct construction is always forbidden.
                if module_ref::is_qualified(&name) {
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
                let max = self.config.max_tuple_elements() as usize;
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
                    self.local_types.insert(slot, ty);
                }
            }

            Stmt::Reassign { name, expr } => {
                let slot = self.locals.get(&name).map(|&(s, _)| s).ok_or_else(|| {
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

                // Then body — scoped: shadow+restore, structs must be consumed.
                let outer_locals = self.locals.clone();
                self.scope_depth += 1;
                for s in body {
                    self.compile_stmt(s)?;
                }
                self.scope_depth -= 1;
                self.check_branch_structs_consumed(&outer_locals, "if")?;
                self.locals = outer_locals;

                if let Some(else_stmts) = else_body {
                    let patch_jump = self.code.len();
                    self.emit(Instruction::Jump(0));
                    self.code[patch_cond] =
                        Instruction::JumpIfNot((self.code.len() - patch_cond) as i32);

                    // Else body — scoped: shadow+restore, structs must be consumed.
                    let outer_locals = self.locals.clone();
                    self.scope_depth += 1;
                    for s in else_stmts {
                        self.compile_stmt(s)?;
                    }
                    self.scope_depth -= 1;
                    self.check_branch_structs_consumed(&outer_locals, "else")?;
                    self.locals = outer_locals;

                    self.code[patch_jump] =
                        Instruction::Jump((self.code.len() - patch_jump) as i32);
                } else {
                    self.code[patch_cond] =
                        Instruction::JumpIfNot((self.code.len() - patch_cond) as i32);
                }
            }

            Stmt::LetTuple { names, expr } => {
                let n = names.len();
                let max = self.config.max_tuple_elements() as usize;
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
                            .clone();
                        let slot = self.alloc_local(binding_name.clone())?;
                        self.local_types.insert(slot, field_ty);
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
                let slot = self.locals.get(&obj_name).map(|&(s, _)| s).ok_or_else(|| {
                    CompilerError::Message(format!("undefined variable '{obj_name}'"))
                })?;

                // Visibility check on the first element of the path.
                let first_field = &field_path[0];
                if let Some(ty) = self.local_types.get(&slot) {
                    self.check_field_write(ty, first_field)?;
                }

                // Reject assignment to a struct-typed field — old value would be silently dropped.
                if let Some(obj_ty) = self.local_types.get(&slot)
                    && let Some(final_ty) = self.field_path_type(obj_ty, &field_path)
                    && matches!(final_ty, Type::Struct(_))
                {
                    return Err(CompilerError::Message(format!(
                        "cannot assign to struct-typed field '{}' — structs have move semantics",
                        field_path.last().unwrap()
                    )));
                }

                self.compile_expr(expr)?;
                self.emit(Instruction::StoreField(slot, field_path));
            }
        }
        Ok(())
    }

    /// Checks that all struct-typed locals are consumed at the end of the function body.
    ///
    /// Any slot that still has a non-primitive type in `local_types` was never loaded
    /// (consumed), meaning the value would be silently dropped. Called once after the
    /// top-level function body is compiled. This includes parameters — the compiler
    /// enforces linearity for all struct values, so no exemption is needed.
    fn check_all_structs_consumed(&self, fn_name: &str) -> Result<()> {
        for (name, &(slot, _)) in &self.locals {
            if let Some(ty) = self.local_types.get(&slot)
                && !ty.is_primitive()
            {
                return Err(CompilerError::Message(format!(
                    "in function '{fn_name}': '{name}' of type '{}' must be consumed before the function returns",
                    ty.name()
                )));
            }
        }
        Ok(())
    }

    /// Check that accessing `field` on `struct_ty` via GetField is linearity-safe.
    ///
    /// GetField consumes the entire struct from the stack. If the struct has
    /// struct-typed fields, they would be silently dropped — a linearity violation.
    /// Cross-module types are not in `self.structs` and are already blocked by
    /// `check_field_read`; this method only enforces rules for same-module types.
    fn check_getfield_on_struct(&self, struct_ty: &Type, field: &str) -> Result<()> {
        let def = match self.structs.iter().find(|s| s.name == struct_ty.name()) {
            Some(d) => d,
            None => return Ok(()),
        };
        if let Some(field_def) = def.fields.iter().find(|f| f.name == field)
            && matches!(field_def.ty, Type::Struct(_))
        {
            return Err(CompilerError::Message(format!(
                "field '{field}' has struct type and cannot be accessed directly — \
                     structs have move semantics; use destructuring `let {} {{ {field}, .. }} = expr;`",
                struct_ty.name(),
            )));
        }

        if let Some(linear) = def
            .fields
            .iter()
            .find(|f| f.name != field && matches!(f.ty, Type::Struct(_)))
        {
            return Err(CompilerError::Message(format!(
                "cannot access field '{field}' on '{}' — struct-typed field '{}' would be \
                 silently dropped; use destructuring `let {} {{ {field}, .. }} = expr;`",
                struct_ty.name(),
                linear.name,
                struct_ty.name(),
            )));
        }
        Ok(())
    }

    /// Walk a field path starting from `root_ty` and return the type of the final field.
    ///
    /// Returns `None` if any step in the path is unresolvable (unknown struct or field).
    fn field_path_type(&self, root_ty: &Type, path: &[String]) -> Option<Type> {
        let mut current = root_ty.clone();
        for field_name in path {
            let def = self.structs.iter().find(|s| s.name == current.name())?;
            let field_def = def.fields.iter().find(|f| &f.name == field_name)?;
            current = field_def.ty.clone();
        }
        Some(current)
    }

    /// Checks that all structs introduced inside a branch body have been consumed.
    ///
    /// `outer_locals` is the snapshot taken before entering the branch.
    /// Any name whose slot differs from the snapshot is a branch-local binding;
    /// if it still holds a struct type it was not consumed — an error.
    /// `local_types` is intentionally NOT restored: outer slot consumption that
    /// happened inside the branch is correctly preserved.
    fn check_branch_structs_consumed(
        &self,
        outer_locals: &HashMap<String, (u8, u32)>,
        branch: &str,
    ) -> Result<()> {
        for (name, &(slot, _)) in &self.locals {
            if outer_locals.get(name.as_str()).map(|&(s, _)| s) != Some(slot)
                && let Some(ty) = self.local_types.get(&slot)
                && !ty.is_primitive()
            {
                return Err(CompilerError::Message(format!(
                    "struct '{name}' introduced in {branch} body must be consumed before the branch ends",
                )));
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
