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
    ast::{AstFunction, BinOp, Expr, Stmt},
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

        // Validate return type: functions may not return an Object.
        // Note: the parser maps all named types to Type::Struct, so we also check
        // whether the name refers to an object definition in the structs list.
        if let Some(rt) = &ast_fn.return_type {
            let is_object_return = match rt {
                Type::Object(_) => true,
                Type::Struct(name) => structs.iter().any(|s| s.name == *name && s.is_object),
                _ => false,
            };
            if is_object_return {
                return Err(CompilerError::Message(format!(
                    "function '{}': cannot return Object type '{}'",
                    ast_fn.name,
                    rt.name()
                )));
            }
        }

        // Allocate slots for parameters and record their types.
        let mut params: Vec<(String, Type)> = Vec::with_capacity(ast_fn.params.len());
        for (name, ty) in ast_fn.params {
            validator::validate_identifier(
                &name,
                &format!("parameter in function '{}'", ast_fn.name),
                config,
            )?;
            cg.alloc_local(name.clone())?;
            // Record the source-level type name for type tracking.
            cg.local_types.insert(name.clone(), ty.name().to_string());
            params.push((name, ty));
        }

        let return_type = ast_fn.return_type;

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
                        // Qualify struct/object return types with the dep module name.
                        Type::Struct(local_name) | Type::Object(local_name) => {
                            format!("{dep_name}::{local_name}")
                        }
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

    /// Check that reading or writing `field` on a value of `type_name` is allowed.
    ///
    /// For cross-module types, only fields marked `pub` may be read; writes are
    /// always rejected regardless of field visibility. For same-module types all
    /// field reads are allowed (writes are handled separately via `check_field_write`).
    fn check_field_read(&self, type_name: &str, field: &str) -> Result<()> {
        if !Self::is_cross_module_type(type_name) {
            return Ok(());
        }
        let def = self
            .structs
            .iter()
            .find(|s| s.name == type_name)
            .ok_or_else(|| CompilerError::Message(format!("unknown type '{type_name}'")))?;
        let field_def = def.fields.iter().find(|f| f.name == field).ok_or_else(|| {
            CompilerError::Message(format!("field '{field}' not found in '{type_name}'",))
        })?;
        if !field_def.is_public {
            return Err(CompilerError::Message(format!(
                "field '{field}' of '{type_name}' is private",
            )));
        }
        Ok(())
    }

    /// Check that writing `field` on a value of the given type is allowed.
    ///
    /// Rules:
    /// - The `id` field of any object is always immutable (rejected everywhere).
    /// - All other field writes from an external module are rejected.
    /// - Within the declaring module, field writes are always allowed (except `id`).
    fn check_field_write(&self, type_name: &str, field: &str) -> Result<()> {
        // Look up the struct def — only needed for the object+id check.
        let def = self.structs.iter().find(|s| s.name == type_name);

        // Reject writes to the `id` field of any object.
        if field == "id" {
            let is_object = def.map(|d| d.is_object).unwrap_or(false);
            if is_object {
                return Err(CompilerError::Message(
                    "field 'id' of an object is immutable and cannot be reassigned".to_string(),
                ));
            }
        }

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
                // Optimise: if the base is a local variable, use LoadField (no move).
                // Otherwise, compile the expression (which may consume a value) then GetField.
                if let Expr::Ident(ref name) = *expr
                    && let Some(&slot) = self.locals.get(name)
                {
                    // Visibility check: reads of cross-module fields must be pub.
                    if let Some(type_name) = self.local_types.get(name).cloned() {
                        self.check_field_read(&type_name, &field)?;
                    }
                    self.emit(Instruction::LoadField(slot, field));
                    return Ok(());
                }
                // Fallback: compile base expression (may be consuming), then GetField.
                // Type inference for field-read check in the fallback case.
                if let Some(type_name) = self.infer_type(&expr) {
                    self.check_field_read(&type_name, &field)?;
                }
                self.compile_expr(*expr)?;
                self.emit(Instruction::GetField(field));
            }

            Expr::StructLit { name, fields } => {
                // Cross-module struct/object construction is always forbidden.
                if Self::is_cross_module_type(&name) {
                    return Err(CompilerError::Message(format!(
                        "cannot construct '{name}' outside its declaring module — \
                         structs and objects can only be created where they are defined",
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

                // Objects: the `id` field must be exactly `meow_vm_fresh_id()`.
                if def.is_object {
                    match literal_map.get("id") {
                        Some(Expr::Call {
                            name: fn_name,
                            args,
                        }) if fn_name == "meow_vm_fresh_id" && args.is_empty() => {}
                        _ => {
                            return Err(CompilerError::Message(format!(
                                "object '{}': 'id' field must be initialized with meow_vm_fresh_id()",
                                name
                            )));
                        }
                    }
                }

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

            Expr::Call { name, args } => {
                // Access check for cross-module function calls.
                self.check_fn_visibility(&name)?;

                // Special restriction: meow_vm_destroy may only be called on objects
                // declared in the current module.
                if name == "meow_vm_destroy"
                    && args.len() == 1
                    && let Some(obj_type) = self.infer_type(&args[0])
                    && Self::is_cross_module_type(&obj_type)
                {
                    return Err(CompilerError::Message(format!(
                        "cannot destroy object of type '{obj_type}': \
                                 objects can only be destroyed inside the module that declares them",
                    )));
                }

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

            Stmt::FieldAssign {
                obj_name,
                field,
                expr,
            } => {
                let slot = self.locals.get(&obj_name).copied().ok_or_else(|| {
                    CompilerError::Message(format!("undefined variable '{obj_name}'"))
                })?;

                // Visibility + immutability check.
                if let Some(type_name) = self.local_types.get(&obj_name).cloned() {
                    self.check_field_write(&type_name, &field)?;
                } else {
                    // Type unknown — still enforce the id-immutability rule by
                    // checking if any object in scope has this slot and field == "id".
                    // We do a conservative check: if field is "id", check if the slot
                    // holds an object type we don't know about, reject to be safe.
                    // (In practice, untyped locals almost never have field "id".)
                    if field == "id" {
                        return Err(CompilerError::Message(
                            "field 'id' of an object is immutable and cannot be reassigned"
                                .to_string(),
                        ));
                    }
                }

                self.compile_expr(expr)?;
                self.emit(Instruction::StoreField(slot, field));
            }
        }
        Ok(())
    }
}
