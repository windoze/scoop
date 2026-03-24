//! LLVM IR 生成（早期阶段，T0808～T0810）。
//!
//! 当前落点：支持把**入口函数 `fun main`** 与其调用到的顶层函数降低到单个 LLVM module：
//! - 入口保持为 `i32 @main()`（C ABI），其返回值作为进程退出码；
//! - 额外生成（或声明）被调用的顶层函数（先按简单 C ABI）。
//!
//! 表达式/语句子集（当前只覆盖早期最小回归需要）：
//! - 整数/布尔字面量；
//! - 一元运算：`!`、`-`、`~`；
//! - 二元运算：算术/比较/位运算/移位（含 shift count mask）；
//! - 局部绑定：`val`/`var`（映射为 `alloca` + `load/store`）；
//! - 赋值语句：`x = expr`（仅支持 local `var`）；
//! - `return`（以及“block 最后表达式”作为隐式返回）。
//!
//! 非目标（后续任务逐步补齐）：
//! - if/when/loop 等控制流（依赖 MIR/CFG codegen 任务）。

use std::collections::HashMap;

use inkwell::IntPredicate;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::types::BasicTypeEnum;
use inkwell::types::IntType;
use inkwell::types::StructType;
use inkwell::values::FunctionValue;
use inkwell::values::AggregateValueEnum;
use inkwell::values::BasicValue;
use inkwell::values::BasicValueEnum;
use inkwell::values::IntValue;
use inkwell::values::PointerValue;

use crate::ast;
use crate::hir;
use crate::llvm::target::HostTargetInfo;
use crate::source::SourceFile;
use crate::ty::{TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::LlvmEmitError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntTy {
    bits: u32,
    signed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CgTy {
    Unit,
    Bool,
    Int(IntTy),
    Struct(TypeId),
}

#[derive(Debug, Clone, Copy)]
struct CgValue<'ctx> {
    ty: CgTy,
    value: Option<BasicValueEnum<'ctx>>,
}

impl<'ctx> CgValue<'ctx> {
    fn unit() -> Self {
        Self {
            ty: CgTy::Unit,
            value: None,
        }
    }

    fn int(value: IntValue<'ctx>, ty: IntTy) -> Self {
        Self {
            ty: CgTy::Int(ty),
            value: Some(value.into()),
        }
    }

    fn bool(value: IntValue<'ctx>) -> Self {
        Self {
            ty: CgTy::Bool,
            value: Some(value.into()),
        }
    }

    fn as_int(self) -> Option<(IntValue<'ctx>, IntTy)> {
        match self.ty {
            CgTy::Int(ty) => match self.value? {
                BasicValueEnum::IntValue(v) => Some((v, ty)),
                _ => None,
            },
            _ => None,
        }
    }

    fn as_bool(self) -> Option<IntValue<'ctx>> {
        match self.ty {
            CgTy::Bool => match self.value? {
                BasicValueEnum::IntValue(v) => Some(v),
                _ => None,
            },
            _ => None,
        }
    }
}

/// 一个局部变量（`val`/`var`）在 LLVM 里的存储形态。
///
/// 当前阶段（T0809）统一用栈分配（`alloca`）承载 locals，并用 `load/store` 实现读写。
#[derive(Debug, Clone, Copy)]
struct CgLocal<'ctx> {
    ty: CgTy,
    ptr: PointerValue<'ctx>,
    mutable: bool,
}

#[derive(Debug, Default)]
struct Env<'ctx> {
    scopes: Vec<HashMap<hir::SymbolId, CgLocal<'ctx>>>,
}

impl<'ctx> Env<'ctx> {
    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
    }

    fn insert(&mut self, id: hir::SymbolId, local: CgLocal<'ctx>) {
        if let Some(frame) = self.scopes.last_mut() {
            frame.insert(id, local);
        }
    }

    fn get(&self, id: hir::SymbolId) -> Option<CgLocal<'ctx>> {
        for frame in self.scopes.iter().rev() {
            if let Some(local) = frame.get(&id).copied() {
                return Some(local);
            }
        }
        None
    }
}

pub(crate) struct MainCodegen<'a, 'ctx> {
    context: &'ctx Context,
    module: &'a Module<'ctx>,
    builder: &'a Builder<'ctx>,
    host: &'a HostTargetInfo,
    source: &'a SourceFile,
    types: &'a TypeStore,
    struct_layouts: &'a hir::StructLayoutIndex,
    fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    env: Env<'ctx>,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(crate) fn new(
        context: &'ctx Context,
        module: &'a Module<'ctx>,
        builder: &'a Builder<'ctx>,
        host: &'a HostTargetInfo,
        source: &'a SourceFile,
        types: &'a TypeStore,
        struct_layouts: &'a hir::StructLayoutIndex,
        fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    ) -> Self {
        Self {
            context,
            module,
            builder,
            host,
            source,
            types,
            struct_layouts,
            fun_index,
            env: Env::default(),
        }
    }

    pub(crate) fn declare_top_level_fun(
        &self,
        fun: &hir::FunDecl,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.module.get_function(&fun.fqn) {
            return Ok(existing);
        }

        let llvm_params = fun
            .params
            .iter()
            .map(|p| self.llvm_param_ty(p.span, p.ty))
            .collect::<Result<Vec<_>, _>>()?;

        let return_cg = self
            .cg_ty_of(fun.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function return type",
                at: fun.span.into(),
            })?;

        let fn_ty = match return_cg {
            CgTy::Unit => self.context.void_type().fn_type(&llvm_params, false),
            CgTy::Bool => self.context.bool_type().fn_type(&llvm_params, false),
            CgTy::Int(int_ty) => self.int_type(int_ty).fn_type(&llvm_params, false),
            CgTy::Struct(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "function return type",
                    at: fun.span.into(),
                });
            }
        };

        Ok(self.module.add_function(&fun.fqn, fn_ty, None))
    }

    pub(crate) fn codegen_top_level_fun(
        mut self,
        fun: &hir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let Some(body) = fun.body.as_ref() else {
            // extern / declaration-only：由调用点按需声明即可，这里不生成 body。
            return Ok(());
        };

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);

        self.env.push_scope();
        self.codegen_fun_params(fun, llvm_fun)?;

        let declared_return_cg = self
            .cg_ty_of(fun.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function return type",
                at: fun.span.into(),
            })?;
        let ret_v = self.codegen_block_as_return_value(body, declared_return_cg)?;
        self.emit_return(fun.span, declared_return_cg, ret_v)?;

        self.env.pop_scope();
        Ok(())
    }

    pub(crate) fn codegen_main_exit_code(
        mut self,
        fun: &hir::FunDecl,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        self.env.push_scope();

        let exit = match fun.body.as_ref() {
            Some(body) => self.codegen_block_as_exit_code(body, fun.return_ty)?,
            None => self.context.i32_type().const_int(0, false),
        };

        self.env.pop_scope();
        Ok(exit)
    }

    fn codegen_block_as_exit_code(
        &mut self,
        block: &hir::Block,
        declared_return_ty: TypeId,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        // block 是表达式：若末尾是表达式语句，则它的值作为 block value。
        let mut tail_value: Option<CgValue<'ctx>> = None;

        self.env.push_scope();

        for (idx, stmt) in block.stmts.iter().enumerate() {
            let is_last = idx + 1 == block.stmts.len();
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    self.codegen_val_decl(decl)?;
                    tail_value = None;
                }
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                    tail_value = None;
                }
                hir::StmtKind::Expr(expr) => {
                    let v = self.codegen_expr(expr)?;
                    if is_last {
                        tail_value = Some(v);
                    } else {
                        tail_value = None;
                    }
                }
                hir::StmtKind::Return { value } => {
                    let exit = match value {
                        Some(expr) => {
                            let v = self.codegen_expr(expr)?;
                            self.coerce_exit_code(expr.span, v)?
                        }
                        None => self.context.i32_type().const_int(0, false),
                    };

                    self.env.pop_scope();
                    return Ok(exit);
                }
                // 控制流留待后续任务（需要 function-level CFG/MIR codegen）。
                hir::StmtKind::While { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        // 隐式返回：当函数声明了整数/Bool 返回类型时，允许用 block tail value 作为返回值。
        let exit = if let Some(v) = tail_value {
            match self.cg_ty_of(declared_return_ty) {
                Some(CgTy::Int(_) | CgTy::Bool) => self.coerce_exit_code(block.span, v)?,
                _ => self.context.i32_type().const_int(0, false),
            }
        } else {
            self.context.i32_type().const_int(0, false)
        };

        self.env.pop_scope();
        Ok(exit)
    }

    fn codegen_val_decl(&mut self, decl: &hir::ValDecl) -> Result<(), LlvmEmitError> {
        let Some(id) = decl.id else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "anonymous val binding",
                at: decl.span.into(),
            });
        };

        let target_ty = self
            .cg_ty_of(decl.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "val type",
                at: decl.span.into(),
            })?;

        let init = match decl.init.as_ref() {
            Some(expr) => self.codegen_expr(expr)?,
            None => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "val without initializer",
                    at: decl.span.into(),
                });
            }
        };

        // T0809：局部变量统一降为 alloca + store/load；`val/var` 仅在“是否允许赋值”上有差异。
        let name = decl.name.as_deref().unwrap_or("local");
        let ptr = self.create_entry_alloca(decl.span, name, target_ty)?;
        self.store_local_value(decl.span, ptr, target_ty, init)?;
        self.env.insert(
            id,
            CgLocal {
                ty: target_ty,
                ptr,
                mutable: decl.mutable,
            },
        );
        Ok(())
    }

    fn codegen_assign_stmt(
        &mut self,
        eq_span: crate::span::Span,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<(), LlvmEmitError> {
        let hir::ExprKind::VarRef(vref) = &lhs.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "assignment lhs",
                at: lhs.span.into(),
            });
        };

        let hir::ValueRef::Local { id, .. } = vref else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "assignment to non-local",
                at: lhs.span.into(),
            });
        };

        let local = self
            .env
            .get(*id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown local value",
                at: lhs.span.into(),
            })?;

        if !local.mutable {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "assignment to immutable local",
                at: eq_span.into(),
            });
        }

        let rhs_v = self.codegen_expr(rhs)?;
        self.store_local_value(eq_span, local.ptr, local.ty, rhs_v)?;
        Ok(())
    }

    fn codegen_expr(&mut self, expr: &hir::Expr) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::Missing | hir::ExprKind::Todo(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "expression",
                    at: expr.span.into(),
                })
            }
            hir::ExprKind::Literal(lit) => self.codegen_literal(expr.span, expr.ty, lit),
            hir::ExprKind::VarRef(v) => self.codegen_var_ref(expr.span, v),
            hir::ExprKind::StructLit { ty, fields } => self.codegen_struct_lit(expr.span, *ty, fields),
            hir::ExprKind::Unary {
                op, expr: inner, ..
            } => self.codegen_unary(expr.span, *op, inner),
            hir::ExprKind::Binary { lhs, op, rhs, .. } => {
                self.codegen_binary(expr.span, *op, lhs, rhs)
            }
            hir::ExprKind::Block(block) => self.codegen_block_value(block),
            hir::ExprKind::Call { callee, args } => self.codegen_call(expr.span, callee, args),
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.codegen_member_access(expr.span, receiver, member)
            }

            // 后续任务接入 MIR/CFG codegen
            hir::ExprKind::Closure(_)
            | hir::ExprKind::If { .. }
            | hir::ExprKind::When { .. }
            | hir::ExprKind::Perform { .. }
            | hir::ExprKind::Handle(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "expression kind",
                at: expr.span.into(),
            }),
        }
    }

    fn codegen_call(
        &mut self,
        span: crate::span::Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "call callee",
                at: callee.span.into(),
            });
        };

        let sig_fun = self
            .fun_index
            .get(fqn)
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "call callee type",
                at: callee.span.into(),
            })?;

        if args.len() != sig_fun.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "call arity mismatch",
                at: span.into(),
            });
        }

        let mut llvm_args = Vec::with_capacity(args.len());
        for (idx, arg) in args.iter().enumerate() {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "named call arg",
                    at: span.into(),
                });
            };

            let target_cg = self
                .cg_ty_of(sig_fun.params[idx].ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "call arg type",
                    at: expr.span.into(),
                })?;
            let v = self.codegen_expr(expr)?;
            let coerced = self.coerce_value(expr.span, v, target_cg)?;
            llvm_args.push(self.as_llvm_arg_value(expr.span, target_cg, coerced)?);
        }

        let llvm_fun = match self.module.get_function(fqn) {
            Some(f) => f,
            None => self.declare_top_level_fun(sig_fun)?,
        };
        let call_site = self.builder.build_call(llvm_fun, &llvm_args, "call")?;

        let ret_cg = self
            .cg_ty_of(sig_fun.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "call return type",
                at: span.into(),
            })?;

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "call return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::bool(value))
            }
            CgTy::Int(int_ty) => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "call return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::int(value, int_ty))
            }
            CgTy::Struct(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "call return type",
                at: span.into(),
            }),
        }
    }

    fn llvm_param_ty(
        &self,
        span: crate::span::Span,
        ty: TypeId,
    ) -> Result<BasicMetadataTypeEnum<'ctx>, LlvmEmitError> {
        let cg = self.cg_ty_of(ty).ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "function param type",
            at: span.into(),
        })?;

        Ok(match cg {
            CgTy::Unit => self.context.i8_type().into(),
            CgTy::Bool => self.context.bool_type().into(),
            CgTy::Int(int_ty) => self.int_type(int_ty).into(),
            CgTy::Struct(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "function param type",
                    at: span.into(),
                });
            }
        })
    }

    fn as_llvm_arg_value(
        &self,
        span: crate::span::Span,
        param_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<inkwell::values::BasicMetadataValueEnum<'ctx>, LlvmEmitError> {
        Ok(match param_ty {
            CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
            CgTy::Bool | CgTy::Int(_) => value
                .value
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "call arg value",
                    at: span.into(),
                })?
                .into(),
            CgTy::Struct(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "call arg value",
                    at: span.into(),
                });
            }
        })
    }

    fn codegen_fun_params(
        &mut self,
        fun: &hir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        for (idx, param) in fun.params.iter().enumerate() {
            let target_ty = self.cg_ty_of(param.ty).ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "param type",
                at: param.span.into(),
            })?;

            let ptr = self.create_entry_alloca(param.span, &param.name, target_ty)?;
            let init = match target_ty {
                CgTy::Unit => CgValue::unit(),
                CgTy::Bool => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm param",
                            at: param.span.into(),
                        })?
                        .into_int_value();
                    CgValue::bool(raw)
                }
                CgTy::Int(int_ty) => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm param",
                            at: param.span.into(),
                        })?
                        .into_int_value();
                    CgValue::int(raw, int_ty)
                }
                CgTy::Struct(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "param type",
                        at: param.span.into(),
                    });
                }
            };

            self.store_local_value(param.span, ptr, target_ty, init)?;
            self.env.insert(
                param.id,
                CgLocal {
                    ty: target_ty,
                    ptr,
                    mutable: false,
                },
            );
        }
        Ok(())
    }

    fn codegen_block_as_return_value(
        &mut self,
        block: &hir::Block,
        declared_return_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let mut tail_value: Option<CgValue<'ctx>> = None;

        self.env.push_scope();

        for (idx, stmt) in block.stmts.iter().enumerate() {
            let is_last = idx + 1 == block.stmts.len();
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    self.codegen_val_decl(decl)?;
                    tail_value = None;
                }
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                    tail_value = None;
                }
                hir::StmtKind::Expr(expr) => {
                    let v = self.codegen_expr(expr)?;
                    tail_value = if is_last { Some(v) } else { None };
                }
                hir::StmtKind::Return { value } => {
                    let out = match value {
                        Some(expr) => {
                            let v = self.codegen_expr(expr)?;
                            if declared_return_ty == CgTy::Unit {
                                CgValue::unit()
                            } else {
                                self.coerce_value(expr.span, v, declared_return_ty)?
                            }
                        }
                        None => self.default_value(declared_return_ty),
                    };

                    self.env.pop_scope();
                    return Ok(out);
                }
                hir::StmtKind::While { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        let out = if let Some(v) = tail_value {
            if declared_return_ty == CgTy::Unit {
                CgValue::unit()
            } else {
                self.coerce_value(block.span, v, declared_return_ty)?
            }
        } else {
            self.default_value(declared_return_ty)
        };

        self.env.pop_scope();
        Ok(out)
    }

    fn default_value(&self, ty: CgTy) -> CgValue<'ctx> {
        match ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool => CgValue::bool(self.context.bool_type().const_int(0, false)),
            CgTy::Int(int_ty) => CgValue::int(self.int_type(int_ty).const_int(0, false), int_ty),
            // 说明：当前阶段不支持 struct 作为函数返回类型，因此这里仅提供占位值；
            // 若后续误用，会在 emit/store 阶段触发结构化错误而非 panic。
            CgTy::Struct(ty) => CgValue { ty: CgTy::Struct(ty), value: None },
        }
    }

    fn emit_return(
        &mut self,
        span: crate::span::Span,
        declared_return_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        match declared_return_ty {
            CgTy::Unit => {
                self.builder.build_return(None)?;
                Ok(())
            }
            CgTy::Bool | CgTy::Int(_) => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "return value",
                        at: span.into(),
                    });
                };
                self.builder.build_return(Some(&raw))?;
                Ok(())
            }
            CgTy::Struct(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct return type",
                at: span.into(),
            }),
        }
    }

    fn codegen_block_value(&mut self, block: &hir::Block) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.env.push_scope();

        let mut value: CgValue<'ctx> = CgValue::unit();
        for (idx, stmt) in block.stmts.iter().enumerate() {
            let is_last = idx + 1 == block.stmts.len();
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    self.codegen_val_decl(decl)?;
                    value = CgValue::unit();
                }
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                    value = CgValue::unit();
                }
                hir::StmtKind::Expr(expr) => {
                    let v = self.codegen_expr(expr)?;
                    value = if is_last { v } else { CgValue::unit() };
                }
                // block 作为表达式时，`return` 语义在当前阶段暂不支持（需要 function-level CFG）。
                hir::StmtKind::Return { .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "`return` inside block expression",
                        at: stmt.span.into(),
                    });
                }
                hir::StmtKind::While { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement inside block expression",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        self.env.pop_scope();
        Ok(value)
    }

    fn codegen_literal(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        lit: &hir::LiteralKind,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match lit {
            hir::LiteralKind::Unit => Ok(CgValue::unit()),
            hir::LiteralKind::Bool(v) => Ok(CgValue::bool(
                self.context.bool_type().const_int(*v as u64, false),
            )),
            hir::LiteralKind::Int => {
                let Some(CgTy::Int(int_ty)) = self.cg_ty_of(ty) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "int literal type",
                        at: span.into(),
                    });
                };
                let text = self.source.slice(span);
                let value = parse_int_literal_decimal(text);
                let value = mask_to_bits(value, int_ty.bits) as u64;
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(value, false),
                    int_ty,
                ))
            }
            // 早期阶段：字符串/插值字符串不参与 main v1 codegen。
            hir::LiteralKind::String => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "string literal",
                at: span.into(),
            }),
        }
    }

    fn codegen_var_ref(
        &mut self,
        span: crate::span::Span,
        v: &hir::ValueRef,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let hir::ValueRef::Local { id, .. } = v else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level value ref",
                at: span.into(),
            });
        };

        let local = self
            .env
            .get(*id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown local value",
                at: span.into(),
            })?;

        match local.ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool => {
                let raw = self
                    .builder
                    .build_load(self.llvm_basic_type_of(span, local.ty)?, local.ptr, "load_bool")?
                    .into_int_value();
                Ok(CgValue::bool(raw))
            }
            CgTy::Int(int_ty) => {
                let raw = self
                    .builder
                    .build_load(self.llvm_basic_type_of(span, local.ty)?, local.ptr, "load_int")?
                    .into_int_value();
                Ok(CgValue::int(raw, int_ty))
            }
            CgTy::Struct(_) => {
                let raw = self
                    .builder
                    .build_load(self.llvm_basic_type_of(span, local.ty)?, local.ptr, "load_struct")?;
                Ok(CgValue {
                    ty: local.ty,
                    value: Some(raw),
                })
            }
        }
    }

    fn codegen_struct_lit(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        fields: &[hir::StructLitField],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(CgTy::Struct(struct_ty)) = self.cg_ty_of(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct literal type",
                at: span.into(),
            });
        };

        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct literal type",
                at: span.into(),
            });
        };

        let layout = self
            .struct_layouts
            .get(&nominal.fqn)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "struct literal layout",
                at: span.into(),
            })?;

        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();

        for (idx, field) in layout.fields.iter().enumerate() {
            let Some(init) = fields.iter().find(|f| f.name == field.name) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct literal missing field",
                    at: span.into(),
                });
            };

            let field_cg = self.cg_ty_of_type_fqn(init.span, field.ty_fqn.as_deref())?;

            let init_v = self.codegen_expr(&init.value)?;
            let coerced = self.coerce_value(init.value.span, init_v, field_cg)?;

            let raw = match field_cg {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct field value",
                    at: init.value.span.into(),
                })?,
            };

            let name = format!("insert_{}", field.name);
            agg = self.builder.build_insert_value(agg, raw, idx as u32, &name)?;
        }

        Ok(CgValue {
            ty: CgTy::Struct(struct_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_member_access(
        &mut self,
        _span: crate::span::Span,
        receiver: &hir::Expr,
        member: &hir::MemberAccess,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "member access target",
                at: member.span.into(),
            });
        };

        // 优先路径：`localStruct.field` —— 用 GEP 从 alloca slot 取字段（更贴近后续可变字段语义）。
        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind {
            if let Some(local) = self.env.get(*id) {
                if let CgTy::Struct(struct_ty) = local.ty {
                    let (field_idx, field_ty) =
                        self.lookup_struct_field(struct_ty, fqn, member.span)?;
                    if field_ty == CgTy::Unit {
                        return Ok(CgValue::unit());
                    }

                    let llvm_struct_ty = self.llvm_struct_type(member.span, struct_ty)?;
                    let field_ptr = self.builder.build_struct_gep(
                        llvm_struct_ty,
                        local.ptr,
                        field_idx,
                        "field_gep",
                    )?;
                    let llvm_field_ty = self.llvm_basic_type_of(member.span, field_ty)?;
                    let loaded =
                        self.builder
                            .build_load(llvm_field_ty, field_ptr, "load_field")?;
                    return self.cg_value_from_loaded(member.span, field_ty, loaded);
                }
            }
        }

        // fallback：先把 receiver 降到值，再用 extractvalue 取字段。
        let recv = self.codegen_expr(receiver)?;
        let CgTy::Struct(struct_ty) = recv.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "member access receiver type",
                at: receiver.span.into(),
            });
        };
        let (field_idx, field_ty) = self.lookup_struct_field(struct_ty, fqn, member.span)?;
        if field_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let raw = recv.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "member access receiver value",
            at: receiver.span.into(),
        })?;
        let struct_v = raw.into_struct_value();
        let extracted = self
            .builder
            .build_extract_value(struct_v, field_idx, "extract_field")?;
        self.cg_value_from_loaded(member.span, field_ty, extracted)
    }

    fn lookup_struct_field(
        &self,
        struct_ty: TypeId,
        field_fqn: &str,
        at: crate::span::Span,
    ) -> Result<(u32, CgTy), LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct type id",
                at: at.into(),
            });
        };

        let layout = self
            .struct_layouts
            .get(&nominal.fqn)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "struct layout",
                at: at.into(),
            })?;

        let idx = layout
            .fields
            .iter()
            .position(|f| f.fqn == field_fqn)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown struct field",
                at: at.into(),
            })?;

        let field = &layout.fields[idx];
        let field_ty = self.cg_ty_of_type_fqn(field.span, field.ty_fqn.as_deref())?;
        Ok((idx as u32, field_ty))
    }

    fn cg_value_from_loaded(
        &self,
        _at: crate::span::Span,
        ty: CgTy,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        Ok(match ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool => CgValue::bool(raw.into_int_value()),
            CgTy::Int(int_ty) => CgValue::int(raw.into_int_value(), int_ty),
            CgTy::Struct(struct_ty) => CgValue {
                ty: CgTy::Struct(struct_ty),
                value: Some(raw),
            },
        })
    }

    fn codegen_unary(
        &mut self,
        span: crate::span::Span,
        op: ast::UnaryOp,
        expr: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::UnaryOp::Not => {
                let v = self.codegen_expr(expr)?.as_bool().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "unary ! operand",
                        at: span.into(),
                    },
                )?;
                let out = self.builder.build_not(v, "not")?;
                Ok(CgValue::bool(out))
            }
            ast::UnaryOp::Neg => {
                let (v, ty) = self.codegen_expr(expr)?.as_int().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "unary - operand",
                        at: span.into(),
                    },
                )?;
                let out = self.builder.build_int_neg(v, "neg")?;
                Ok(CgValue::int(out, ty))
            }
            ast::UnaryOp::BitNot => {
                let (v, ty) = self.codegen_expr(expr)?.as_int().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "unary ~ operand",
                        at: span.into(),
                    },
                )?;
                let out = self.builder.build_not(v, "bitnot")?;
                Ok(CgValue::int(out, ty))
            }
        }
    }

    fn codegen_binary(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::Mul
            | ast::BinaryOp::Div
            | ast::BinaryOp::Rem
            | ast::BinaryOp::BitAnd
            | ast::BinaryOp::BitXor
            | ast::BinaryOp::BitOr => self.codegen_int_binary_same_type(span, op, lhs, rhs),

            ast::BinaryOp::Shl | ast::BinaryOp::Shr => self.codegen_shift(span, op, lhs, rhs),

            ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                self.codegen_int_compare(span, op, lhs, rhs)
            }

            ast::BinaryOp::Eq | ast::BinaryOp::Ne => self.codegen_equality(span, op, lhs, rhs),

            ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
                self.codegen_bool_logic(span, op, lhs, rhs)
            }

            ast::BinaryOp::Elvis => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "elvis operator",
                at: span.into(),
            }),
        }
    }

    fn codegen_int_binary_same_type(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op rhs",
                    at: span.into(),
                })?;

        let out_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "integer binary op type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, out_ty)?;
        let r = self.cast_int(r_raw, r_ty, out_ty)?;

        let out = match op {
            ast::BinaryOp::Add => self.builder.build_int_add(l, r, "add")?,
            ast::BinaryOp::Sub => self.builder.build_int_sub(l, r, "sub")?,
            ast::BinaryOp::Mul => self.builder.build_int_mul(l, r, "mul")?,
            ast::BinaryOp::Div => {
                if out_ty.signed {
                    self.builder.build_int_signed_div(l, r, "sdiv")?
                } else {
                    self.builder.build_int_unsigned_div(l, r, "udiv")?
                }
            }
            ast::BinaryOp::Rem => {
                if out_ty.signed {
                    self.builder.build_int_signed_rem(l, r, "srem")?
                } else {
                    self.builder.build_int_unsigned_rem(l, r, "urem")?
                }
            }
            ast::BinaryOp::BitAnd => self.builder.build_and(l, r, "and")?,
            ast::BinaryOp::BitXor => self.builder.build_xor(l, r, "xor")?,
            ast::BinaryOp::BitOr => self.builder.build_or(l, r, "or")?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::int(out, out_ty))
    }

    fn codegen_shift(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (lhs_value, lhs_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift lhs type",
                    at: span.into(),
                })?;

        let rhs_value =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift rhs type",
                    at: span.into(),
                })?;

        let shift_count = self.mask_shift_count(lhs_ty, rhs_value.0)?;

        let out = match op {
            ast::BinaryOp::Shl => self
                .builder
                .build_left_shift(lhs_value, shift_count, "shl")?,
            ast::BinaryOp::Shr => {
                self.builder
                    .build_right_shift(lhs_value, shift_count, lhs_ty.signed, "shr")?
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift operator",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::int(out, lhs_ty))
    }

    fn mask_shift_count(
        &mut self,
        lhs_ty: IntTy,
        rhs: IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let lhs_bits = lhs_ty.bits;
        let lhs_int = self.int_type(lhs_ty);

        // 1) 截断为 lhs 的位宽（只取低位，后续再 mask）。
        let rhs_trunc = self
            .builder
            .build_int_truncate(rhs, lhs_int, "shift_rhs_trunc")?;

        // 2) mask：shiftCount & (bitWidth - 1)，避免 LLVM 对“超范围 shift”的 UB。
        let mask = lhs_int.const_int((lhs_bits.saturating_sub(1)) as u64, false);
        Ok(self.builder.build_and(rhs_trunc, mask, "shift_masked")?)
    }

    fn codegen_int_compare(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison rhs",
                    at: span.into(),
                })?;

        let int_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "comparison operand type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, int_ty)?;
        let r = self.cast_int(r_raw, r_ty, int_ty)?;

        let pred = match (op, int_ty.signed) {
            (ast::BinaryOp::Lt, true) => IntPredicate::SLT,
            (ast::BinaryOp::Lt, false) => IntPredicate::ULT,
            (ast::BinaryOp::Le, true) => IntPredicate::SLE,
            (ast::BinaryOp::Le, false) => IntPredicate::ULE,
            (ast::BinaryOp::Gt, true) => IntPredicate::SGT,
            (ast::BinaryOp::Gt, false) => IntPredicate::UGT,
            (ast::BinaryOp::Ge, true) => IntPredicate::SGE,
            (ast::BinaryOp::Ge, false) => IntPredicate::UGE,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison operator",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::bool(
            self.builder.build_int_compare(pred, l, r, "icmp")?,
        ))
    }

    fn codegen_equality(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let lhs_v = self.codegen_expr(lhs)?;
        let rhs_v = self.codegen_expr(rhs)?;

        // Bool == Bool
        if matches!((lhs_v.ty, rhs_v.ty), (CgTy::Bool, CgTy::Bool)) {
            let l = lhs_v.as_bool().unwrap();
            let r = rhs_v.as_bool().unwrap();
            let pred = match op {
                ast::BinaryOp::Eq => IntPredicate::EQ,
                ast::BinaryOp::Ne => IntPredicate::NE,
                _ => unreachable!("filtered by caller"),
            };
            return Ok(CgValue::bool(self.builder.build_int_compare(
                pred,
                l,
                r,
                "icmp_bool",
            )?));
        }

        // Int == Int（含 int literal 吸收）
        let Some((l_raw, l_ty)) = lhs_v.as_int() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "equality lhs",
                at: span.into(),
            });
        };
        let Some((r_raw, r_ty)) = rhs_v.as_int() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "equality rhs",
                at: span.into(),
            });
        };

        let int_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "equality operand type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, int_ty)?;
        let r = self.cast_int(r_raw, r_ty, int_ty)?;

        let pred = match op {
            ast::BinaryOp::Eq => IntPredicate::EQ,
            ast::BinaryOp::Ne => IntPredicate::NE,
            _ => unreachable!("filtered by caller"),
        };
        Ok(CgValue::bool(
            self.builder.build_int_compare(pred, l, r, "icmp_eq")?,
        ))
    }

    fn codegen_bool_logic(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let l = self
            .codegen_expr(lhs)?
            .as_bool()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "bool operator lhs",
                at: span.into(),
            })?;
        let r = self
            .codegen_expr(rhs)?
            .as_bool()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "bool operator rhs",
                at: span.into(),
            })?;

        let out = match op {
            ast::BinaryOp::LogAnd => self.builder.build_and(l, r, "and")?,
            ast::BinaryOp::LogOr => self.builder.build_or(l, r, "or")?,
            _ => unreachable!("filtered by caller"),
        };
        Ok(CgValue::bool(out))
    }

    fn coerce_value(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
        target: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match (value.ty, target) {
            (CgTy::Unit, CgTy::Unit) => Ok(CgValue::unit()),
            (CgTy::Bool, CgTy::Bool) => Ok(value),
            (CgTy::Bool, CgTy::Int(int_ty)) => {
                let v = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "bool value",
                    at: at.into(),
                })?;
                let out =
                    self.builder
                        .build_int_z_extend(v, self.int_type(int_ty), "bool_to_int")?;
                Ok(CgValue::int(out, int_ty))
            }
            (CgTy::Int(from), CgTy::Int(to)) => {
                let (v, _) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "int value",
                    at: at.into(),
                })?;
                let out = self.cast_int(v, from, to)?;
                Ok(CgValue::int(out, to))
            }
            (CgTy::Struct(from), CgTy::Struct(to)) if from == to => Ok(value),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "value coercion",
                at: at.into(),
            }),
        }
    }

    fn cast_int(
        &mut self,
        value: IntValue<'ctx>,
        from: IntTy,
        to: IntTy,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        if from.bits == to.bits {
            return Ok(value);
        }

        let to_ty = self.int_type(to);
        if to.bits > from.bits {
            if from.signed {
                Ok(self.builder.build_int_s_extend(value, to_ty, "sext")?)
            } else {
                Ok(self.builder.build_int_z_extend(value, to_ty, "zext")?)
            }
        } else {
            Ok(self.builder.build_int_truncate(value, to_ty, "trunc")?)
        }
    }

    fn coerce_exit_code(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let i32_ty = self.context.i32_type();

        match value.ty {
            CgTy::Unit => Ok(i32_ty.const_int(0, false)),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "exit bool",
                    at: at.into(),
                })?;
                Ok(self.builder.build_int_z_extend(b, i32_ty, "exit_bool")?)
            }
            CgTy::Int(int_ty) => {
                let (v, from) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "exit int",
                    at: at.into(),
                })?;
                let to = IntTy {
                    bits: 32,
                    signed: int_ty.signed,
                };
                let casted = self.cast_int(v, from, to)?;
                Ok(casted)
            }
            CgTy::Struct(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct exit code",
                at: at.into(),
            }),
        }
    }

    fn cg_ty_of(&self, ty: TypeId) -> Option<CgTy> {
        match self.types.kind(ty) {
            TypeKind::Value(ValueTypeKind::Unit) => Some(CgTy::Unit),
            TypeKind::Value(ValueTypeKind::Bool) => Some(CgTy::Bool),
            TypeKind::Value(ValueTypeKind::Int) => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            })),
            TypeKind::Value(ValueTypeKind::UInt) => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::IntN(bits)) => Some(CgTy::Int(IntTy {
                bits: u32::from(*bits),
                signed: true,
            })),
            TypeKind::Value(ValueTypeKind::UIntN(bits)) => Some(CgTy::Int(IntTy {
                bits: u32::from(*bits),
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => self
                .struct_layouts
                .contains_key(&nominal.fqn)
                .then_some(CgTy::Struct(ty)),
            _ => None,
        }
    }

    fn cg_ty_of_type_fqn(
        &self,
        at: crate::span::Span,
        ty_fqn: Option<&str>,
    ) -> Result<CgTy, LlvmEmitError> {
        let Some(ty_fqn) = ty_fqn else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct field type",
                at: at.into(),
            });
        };

        match ty_fqn {
            "scoop.core.Unit" => Ok(CgTy::Unit),
            "scoop.core.Bool" => Ok(CgTy::Bool),
            "scoop.core.Int" => Ok(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            })),
            "scoop.core.UInt" => Ok(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            })),
            other => {
                // 固定位宽整数族（与 HIR lowering 的 special-case 规则对齐）。
                if let Some(bits) = other
                    .strip_prefix("scoop.core.Int")
                    .and_then(|s| s.parse::<u32>().ok())
                {
                    return Ok(CgTy::Int(IntTy { bits, signed: true }));
                }
                if let Some(bits) = other
                    .strip_prefix("scoop.core.UInt")
                    .and_then(|s| s.parse::<u32>().ok())
                {
                    return Ok(CgTy::Int(IntTy {
                        bits,
                        signed: false,
                    }));
                }

                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct field type",
                    at: at.into(),
                })
            }
        }
    }

    fn int_type(&self, ty: IntTy) -> IntType<'ctx> {
        self.context.custom_width_int_type(ty.bits)
    }

    fn llvm_basic_type_of(
        &self,
        at: crate::span::Span,
        ty: CgTy,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        Ok(match ty {
            // 说明：Unit 没有运行期值；当前阶段仅用于“可放入 alloca”与保持 load/store 管线统一。
            CgTy::Unit => self.context.i8_type().into(),
            CgTy::Bool => self.context.bool_type().into(),
            CgTy::Int(int_ty) => self.int_type(int_ty).into(),
            CgTy::Struct(struct_ty) => self.llvm_struct_type(at, struct_ty)?.into(),
        })
    }

    fn llvm_struct_type(
        &self,
        at: crate::span::Span,
        ty: TypeId,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct type id",
                at: at.into(),
            });
        };

        let layout = self
            .struct_layouts
            .get(&nominal.fqn)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "struct layout",
                at: at.into(),
            })?;

        if let Some(existing) = self.context.get_struct_type(&layout.fqn) {
            return Ok(existing);
        }

        let struct_ty = self.context.opaque_struct_type(&layout.fqn);

        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(layout.fields.len());
        for field in &layout.fields {
            let field_cg = self.cg_ty_of_type_fqn(field.span, field.ty_fqn.as_deref())?;
            llvm_fields.push(self.llvm_basic_type_of(field.span, field_cg)?);
        }

        struct_ty.set_body(&llvm_fields, false);
        Ok(struct_ty)
    }

    fn create_entry_alloca(
        &self,
        at: crate::span::Span,
        name: &str,
        ty: CgTy,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let alloca_builder = self.context.create_builder();
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;
        let entry = func
            .get_first_basic_block()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function has no entry block",
                at: at.into(),
            })?;

        match entry.get_first_instruction() {
            Some(inst) => alloca_builder.position_before(&inst),
            None => alloca_builder.position_at_end(entry),
        }

        let alloca_ty = self.llvm_basic_type_of(at, ty)?;
        Ok(alloca_builder.build_alloca(alloca_ty, name)?)
    }

    fn store_local_value(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        // 说明：当前阶段 locals 允许：
        // - 标量：`Unit/Bool/Int*`
        // - struct（值类型）：以 LLVM struct by-value 形式存入栈 slot（`alloca`）
        let v = self.coerce_value(at, value, ty)?;
        match ty {
            CgTy::Unit => {
                let zero = self.context.i8_type().const_int(0, false);
                let _ = self.builder.build_store(ptr, zero)?;
            }
            CgTy::Bool | CgTy::Int(_) | CgTy::Struct(_) => {
                let Some(raw) = v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "store value",
                        at: at.into(),
                    });
                };
                let _ = self.builder.build_store(ptr, raw)?;
            }
        }
        Ok(())
    }
}

fn unify_int_types(
    lhs_is_lit: bool,
    lhs_ty: IntTy,
    rhs_is_lit: bool,
    rhs_ty: IntTy,
) -> Option<IntTy> {
    if lhs_ty == rhs_ty {
        return Some(lhs_ty);
    }
    if lhs_is_lit {
        return Some(rhs_ty);
    }
    if rhs_is_lit {
        return Some(lhs_ty);
    }
    None
}

fn parse_int_literal_decimal(text: &str) -> u128 {
    let mut out: u128 = 0;
    for ch in text.chars() {
        if ch == '_' {
            continue;
        }
        if let Some(d) = ch.to_digit(10) {
            out = out.saturating_mul(10).saturating_add(u128::from(d));
        }
    }
    out
}

fn mask_to_bits(value: u128, bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return value;
    }
    let mask = (1u128 << bits) - 1;
    value & mask
}
