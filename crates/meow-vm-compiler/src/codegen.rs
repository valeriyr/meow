use std::collections::HashMap;

use meow_vm_types::{
    bytecode::Instruction,
    config::CompilerConfig,
    module::Function,
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
    locals: HashMap<String, u8>,
    next_slot: u8,
    code: Vec<Instruction>,
}

impl<'m> Codegen<'m> {
    pub fn compile_function(
        structs: &'m [StructDef],
        ast_fn: AstFunction,
        config: &'m CompilerConfig,
    ) -> Result<Function> {
        validator::validate_identifier(&ast_fn.name, "function name", config)?;

        let max_params = config.max_params();
        if ast_fn.params.len() > max_params {
            return Err(CompilerError::Message(format!(
                "function '{}': too many parameters ({} > limit of {})",
                ast_fn.name,
                ast_fn.params.len(),
                max_params,
            )));
        }

        let mut cg = Codegen::new(structs, config);

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

        // Allocate slots for parameters.
        let mut params: Vec<(String, Type)> = Vec::with_capacity(ast_fn.params.len());
        for (name, ty) in ast_fn.params {
            validator::validate_identifier(
                &name,
                &format!("parameter in function '{}'", ast_fn.name),
                config,
            )?;
            cg.alloc_local(name.clone())?;
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
            params,
            return_type,
            local_count: cg.next_slot,
            code: cg.code,
        })
    }

    fn new(structs: &'m [StructDef], config: &'m CompilerConfig) -> Self {
        Self {
            config,
            structs,
            locals: HashMap::new(),
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

    fn compile_expr(&mut self, expr: Expr) -> Result<()> {
        match expr {
            Expr::Bool(v) => self.emit(Instruction::PushBool(v)),
            Expr::Int(v) => self.emit(Instruction::PushU64(v)),
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
                    self.emit(Instruction::LoadField(slot, field));
                    return Ok(());
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

                let field_names: Vec<String> = def.fields.iter().map(|(n, _)| n.clone()).collect();
                for field_name in &field_names {
                    let expr = literal_map.remove(field_name).ok_or_else(|| {
                        CompilerError::Message(format!(
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
                let slot = self.alloc_local(name)?;
                self.emit(Instruction::Store(slot));
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
                self.compile_expr(expr)?;
                self.emit(Instruction::StoreField(slot, field));
            }
        }
        Ok(())
    }
}
