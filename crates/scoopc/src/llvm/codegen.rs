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
//! - `when`（T0813：仅支持 enum tag 判别 + variant binder；不支持 guard/or-pattern）。
//!
//! 非目标（后续任务逐步补齐）：
//! - if/loop 等更复杂控制流（依赖 MIR/CFG codegen 任务）。

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
    Tuple(TypeId),
    Struct(TypeId),
    Enum(TypeId),
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
    enum_layouts: &'a hir::EnumLayoutIndex,
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
        enum_layouts: &'a hir::EnumLayoutIndex,
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
            enum_layouts,
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
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
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
            Some(expr) => self.codegen_expr_in_expected_context(expr, Some(target_ty))?,
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

    fn codegen_expr_in_expected_context(
        &mut self,
        expr: &hir::Expr,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::UnresolvedIdent { name } => {
                self.codegen_unresolved_ident(expr.span, name, expected)
            }
            hir::ExprKind::Call { callee, args } => self.codegen_call(expr.span, callee, args, expected),
            _ => self.codegen_expr(expr),
        }
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
            hir::ExprKind::UnresolvedIdent { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unresolved ident (missing expected type context)",
                at: expr.span.into(),
            }),
            hir::ExprKind::Literal(lit) => self.codegen_literal(expr.span, expr.ty, lit),
            hir::ExprKind::VarRef(v) => self.codegen_var_ref(expr.span, v),
            hir::ExprKind::StructLit { ty, fields } => self.codegen_struct_lit(expr.span, *ty, fields),
            hir::ExprKind::TupleLit { elements } => self.codegen_tuple_lit(expr.span, expr.ty, elements),
            hir::ExprKind::Unary {
                op, expr: inner, ..
            } => self.codegen_unary(expr.span, *op, inner),
            hir::ExprKind::Binary { lhs, op, rhs, .. } => {
                self.codegen_binary(expr.span, *op, lhs, rhs)
            }
            hir::ExprKind::Block(block) => self.codegen_block_value(block),
            hir::ExprKind::Call { callee, args } => self.codegen_call(expr.span, callee, args, None),
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.codegen_member_access(expr.span, receiver, member)
            }
            hir::ExprKind::When { subject, arms } => self.codegen_when_expr(expr.span, subject, arms),

            // 后续任务接入 MIR/CFG codegen
            hir::ExprKind::Closure(_)
            | hir::ExprKind::If { .. }
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
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 1) 普通顶层函数调用：`foo(args...)`
        if let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind {
            return self.codegen_top_level_fun_call(span, callee.span, fqn, args);
        }

        // 2) enum variant ctor：`Some(x)` 这类调用在 resolver 阶段不会 resolve，
        //    需要依赖“期望类型语境”才能决定属于哪个 enum。
        if let hir::ExprKind::UnresolvedIdent { name } = &callee.kind {
            let Some(CgTy::Enum(enum_ty)) = expected else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum variant ctor call without expected enum type",
                    at: callee.span.into(),
                });
            };
            return self.codegen_enum_variant_ctor_call(span, enum_ty, name, args);
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "call callee",
            at: callee.span.into(),
        })
    }

    fn codegen_top_level_fun_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let sig_fun = self
            .fun_index
            .get(fqn)
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "call callee type",
                at: callee_span.into(),
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
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "call return type",
                at: span.into(),
            }),
        }
    }

    fn codegen_unresolved_ident(
        &mut self,
        span: crate::span::Span,
        name: &str,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 0-参数 enum variant 值：`None`
        let Some(CgTy::Enum(enum_ty)) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unresolved ident without expected enum type",
                at: span.into(),
            });
        };

        let (tag, field_count) = {
            let layout = self.enum_layout_of(span, enum_ty)?;
            let variant = layout
                .variants
                .iter()
                .find(|v| v.name == name)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "unknown enum variant",
                    at: span.into(),
                })?;
            (variant.tag, variant.fields.len())
        };

        if field_count != 0 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "non-zero-arity enum variant used as value",
                at: span.into(),
            });
        }

        self.build_enum_value(span, enum_ty, tag, None)
    }

    fn codegen_enum_variant_ctor_call(
        &mut self,
        span: crate::span::Span,
        enum_ty: TypeId,
        variant_name: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (tag, fields) = {
            let layout = self.enum_layout_of(span, enum_ty)?;
            let variant = layout
                .variants
                .iter()
                .find(|v| v.name == variant_name)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "unknown enum variant",
                    at: span.into(),
                })?;
            (variant.tag, variant.fields.clone())
        };

        if fields.len() != args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum variant ctor arity mismatch",
                at: span.into(),
            });
        }

        // 当前阶段（T0813）只支持 “小 payload”：0 字段或 1 字段（标量）。
        if fields.len() > 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum variant payload (multi-field)",
                at: span.into(),
            });
        }

        let payload = if let Some(field) = fields.first() {
            let hir::CallArg::Positional(arg_expr) = &args[0] else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "named enum ctor arg",
                    at: span.into(),
                });
            };

            let field_cg = self.cg_ty_of_type_fqn(field.span, field.ty_fqn.as_deref())?;

            let arg_v = self.codegen_expr_in_expected_context(arg_expr, Some(field_cg))?;
            let coerced = self.coerce_value(arg_expr.span, arg_v, field_cg)?;

            Some(self.coerce_enum_payload_word(arg_expr.span, coerced, field_cg)?)
        } else {
            None
        };

        self.build_enum_value(span, enum_ty, tag, payload)
    }

    fn enum_layout_of(
        &self,
        at: crate::span::Span,
        enum_ty: TypeId,
    ) -> Result<&hir::EnumLayout, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(enum_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum type id",
                at: at.into(),
            });
        };

        self.enum_layouts
            .get(&nominal.fqn)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "enum layout",
                at: at.into(),
            })
    }

    fn enum_payload_ty(&self) -> IntTy {
        IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        }
    }

    fn coerce_enum_payload_word(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
        value_ty: CgTy,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let payload_ty = self.enum_payload_ty();
        let payload_int_ty = self.int_type(payload_ty);

        match value_ty {
            CgTy::Unit => Ok(payload_int_ty.const_int(0, false)),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum payload bool",
                    at: at.into(),
                })?;
                Ok(self
                    .builder
                    .build_int_z_extend(b, payload_int_ty, "enum_payload_bool")?)
            }
            CgTy::Int(from) => {
                let (v, _) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum payload int",
                    at: at.into(),
                })?;
                if from.bits > payload_ty.bits {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload larger than word",
                        at: at.into(),
                    });
                }
                Ok(self.cast_int(v, from, payload_ty)?)
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum payload (non-scalar)",
                at: at.into(),
            }),
        }
    }

    fn build_enum_value(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        tag: u32,
        payload: Option<IntValue<'ctx>>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let llvm_enum_ty = self.llvm_enum_type(at, enum_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_enum_ty.get_undef().into();

        let tag_ty = self.context.i32_type();
        let payload_ty = self.int_type(self.enum_payload_ty());

        agg = self
            .builder
            .build_insert_value(agg, tag_ty.const_int(u64::from(tag), false), 0, "enum_tag")?;

        let payload_v = payload.unwrap_or_else(|| payload_ty.const_int(0, false));
        agg = self
            .builder
            .build_insert_value(agg, payload_v, 1, "enum_payload")?;

        Ok(CgValue {
            ty: CgTy::Enum(enum_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_when_expr(
        &mut self,
        span: crate::span::Span,
        subject: &hir::Expr,
        arms: &[hir::WhenArm],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if arms.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when (no arms)",
                at: span.into(),
            });
        }

        for arm in arms {
            if arm.guard.is_some() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "when guard",
                    at: arm.span.into(),
                });
            }
        }

        let subject_v = self.codegen_expr(subject)?;
        let subject_ty = subject_v.ty;
        let subject_raw = subject_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "when subject value",
            at: subject.span.into(),
        })?;

        // 将 subject 落到一个栈 slot：便于在各 arm 中做 payload 解构（避免跨 block 的 dominance 细节）。
        let subject_ptr = self.create_entry_alloca(span, "when_subject", subject_ty)?;
        self.store_local_value(span, subject_ptr, subject_ty, subject_v)?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let merge_bb = self.context.append_basic_block(func, "when_merge");
        let arm_bbs = (0..arms.len())
            .map(|i| self.context.append_basic_block(func, &format!("when_arm_{i}")))
            .collect::<Vec<_>>();

        // 生成分派：enum/bool 优先降到 LLVM switch；tuple 仍用分支链并做字段比较。
        match subject_ty {
            CgTy::Enum(enum_ty) => {
                for arm in arms {
                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. }
                        | hir::WhenPat::Variant { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when pattern (enum)",
                                at: arm.pat.span().into(),
                            });
                        }
                    }
                }

                let subject_struct = subject_raw.into_struct_value();
                let tag = self
                    .builder
                    .build_extract_value(subject_struct, 0, "when_tag")?
                    .into_int_value();

                let layout = self.enum_layout_of(span, enum_ty)?;
                let tag_ty = self.context.i32_type();
                let default_bb = self.context.append_basic_block(func, "when_no_match");

                let mut cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
                    Vec::with_capacity(layout.variants.len());
                for variant in &layout.variants {
                    let Some(target_idx) =
                        self.when_first_matching_arm_for_enum_variant(arms, &variant.name)
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "when missing enum arm",
                            at: span.into(),
                        });
                    };
                    cases.push((
                        tag_ty.const_int(u64::from(variant.tag), false),
                        arm_bbs[target_idx],
                    ));
                }

                self.builder.build_switch(tag, default_bb, &cases)?;
                self.builder.position_at_end(default_bb);
                self.builder.build_unreachable()?;
            }
            CgTy::Bool => {
                for arm in arms {
                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. }
                        | hir::WhenPat::BoolLit { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when pattern (bool)",
                                at: arm.pat.span().into(),
                            });
                        }
                    }
                }

                let b = subject_raw.into_int_value();
                let bool_ty = self.context.bool_type();
                let default_bb = self.context.append_basic_block(func, "when_no_match");

                let Some(false_idx) = self.when_first_matching_arm_for_bool(arms, false) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when missing bool arm (false)",
                        at: span.into(),
                    });
                };
                let Some(true_idx) = self.when_first_matching_arm_for_bool(arms, true) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when missing bool arm (true)",
                        at: span.into(),
                    });
                };

                let cases = [
                    (bool_ty.const_int(0, false), arm_bbs[false_idx]),
                    (bool_ty.const_int(1, false), arm_bbs[true_idx]),
                ];
                self.builder.build_switch(b, default_bb, &cases)?;
                self.builder.position_at_end(default_bb);
                self.builder.build_unreachable()?;
            }
            CgTy::Tuple(tuple_ty) => {
                for arm in arms {
                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. }
                        | hir::WhenPat::Tuple { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when pattern (tuple)",
                                at: arm.pat.span().into(),
                            });
                        }
                    }
                }

                let check_bbs = (0..arms.len())
                    .map(|i| self.context.append_basic_block(func, &format!("when_check_{i}")))
                    .collect::<Vec<_>>();
                let no_match_bb = self.context.append_basic_block(func, "when_no_match");

                self.builder.build_unconditional_branch(check_bbs[0])?;

                for (idx, arm) in arms.iter().enumerate() {
                    self.builder.position_at_end(check_bbs[idx]);

                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. } => {
                            self.builder.build_unconditional_branch(arm_bbs[idx])?;
                        }
                        hir::WhenPat::Tuple { elements, .. } => {
                            let cond = self.codegen_when_tuple_pat_cond(
                                span,
                                tuple_ty,
                                elements,
                                subject_ptr,
                            )?;
                            let else_bb = if idx + 1 < arms.len() {
                                check_bbs[idx + 1]
                            } else {
                                no_match_bb
                            };
                            self.builder
                                .build_conditional_branch(cond, arm_bbs[idx], else_bb)?;
                        }
                        _ => unreachable!("tuple patterns validated above"),
                    }
                }

                self.builder.position_at_end(no_match_bb);
                self.builder.build_unreachable()?;
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "when subject type",
                    at: subject.span.into(),
                });
            }
        }

        // 生成各 arm body，并把结果汇合到 merge。
        let mut out_ty: Option<CgTy> = None;
        let mut incoming: Vec<(inkwell::basic_block::BasicBlock<'ctx>, CgValue<'ctx>)> = Vec::new();

        for (idx, arm) in arms.iter().enumerate() {
            self.builder.position_at_end(arm_bbs[idx]);

            self.env.push_scope();
            self.bind_when_pat(span, subject_ty, &arm.pat, subject_ptr)?;

            let v = self.codegen_expr(&arm.body)?;
            match out_ty {
                None => out_ty = Some(v.ty),
                Some(prev) if prev == v.ty => {}
                Some(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when arm type mismatch",
                        at: arm.body.span.into(),
                    });
                }
            }

            self.builder.build_unconditional_branch(merge_bb)?;
            self.env.pop_scope();

            incoming.push((arm_bbs[idx], v));
        }

        self.builder.position_at_end(merge_bb);

        let out_ty = out_ty.unwrap_or(CgTy::Unit);
        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool | CgTy::Int(_) => {
                let phi_ty = self.llvm_basic_type_of(span, out_ty)?;
                let phi = self.builder.build_phi(phi_ty, "when_phi")?;

                for (bb, v) in incoming {
                    let raw = v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "when arm value",
                        at: span.into(),
                    })?;
                    phi.add_incoming(&[(&raw, bb)]);
                }

                Ok(CgValue {
                    ty: out_ty,
                    value: Some(phi.as_basic_value()),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when result type",
                at: span.into(),
            }),
        }
    }

    fn bind_when_pat(
        &mut self,
        at: crate::span::Span,
        subject_ty: CgTy,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Rest { .. }
            | hir::WhenPat::Is { .. }
            | hir::WhenPat::IntLit { .. }
            | hir::WhenPat::StringLit { .. }
            | hir::WhenPat::BoolLit { .. } => Ok(()),
            hir::WhenPat::Bind { id, name, .. } => {
                // `x -> ...`：绑定整个 subject。
                let ptr = self.create_entry_alloca(at, name, subject_ty)?;
                let llvm_ty = self.llvm_basic_type_of(at, subject_ty)?;
                let loaded = self.builder.build_load(llvm_ty, subject_ptr, "bind_subject")?;
                let v = CgValue {
                    ty: subject_ty,
                    value: Some(loaded),
                };
                self.store_local_value(at, ptr, subject_ty, v)?;
                self.env.insert(
                    *id,
                    CgLocal {
                        ty: subject_ty,
                        ptr,
                        mutable: false,
                    },
                );
                Ok(())
            }
            hir::WhenPat::Variant { name, args, .. } => {
                let CgTy::Enum(enum_ty) = subject_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when variant pattern subject type",
                        at: pat.span().into(),
                    });
                };

                let layout = self.enum_layout_of(at, enum_ty)?;
                let Some(variant) = layout.variants.iter().find(|v| v.name == *name) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when unknown enum variant",
                        at: pat.span().into(),
                    });
                };

                if variant.fields.len() != args.len() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when variant arity mismatch",
                        at: pat.span().into(),
                    });
                }
                if variant.fields.len() > 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when variant payload (multi-field)",
                        at: pat.span().into(),
                    });
                }

                if let (Some(field), Some(arg_pat)) = (variant.fields.first(), args.first()) {
                    let field_cg = self.cg_ty_of_type_fqn(field.span, field.ty_fqn.as_deref())?;

                    let llvm_enum_ty = self.llvm_enum_type(at, enum_ty)?;
                    let loaded =
                        self.builder
                            .build_load(llvm_enum_ty, subject_ptr, "load_when_subject")?;
                    let raw_struct = loaded.into_struct_value();
                    let payload_raw = self
                        .builder
                        .build_extract_value(raw_struct, 1, "when_payload")?
                        .into_int_value();

                    // 当前阶段 payload 固定为 word-sized int；按字段类型截断/转换。
                    let extracted = match field_cg {
                        CgTy::Unit => CgValue::unit(),
                        CgTy::Bool => {
                            let b = self
                                .builder
                                .build_int_truncate(payload_raw, self.context.bool_type(), "payload_to_bool")?;
                            CgValue::bool(b)
                        }
                        CgTy::Int(int_ty) => {
                            let from = self.enum_payload_ty();
                            let casted = self.cast_int(payload_raw, from, int_ty)?;
                            CgValue::int(casted, int_ty)
                        }
                        CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when payload (non-scalar)",
                                at: field.span.into(),
                            });
                        }
                    };

                    match arg_pat {
                        hir::WhenPat::Bind { id, name, .. } => {
                            let ptr = self.create_entry_alloca(at, name, field_cg)?;
                            self.store_local_value(at, ptr, field_cg, extracted)?;
                            self.env.insert(
                                *id,
                                CgLocal {
                                    ty: field_cg,
                                    ptr,
                                    mutable: false,
                                },
                            );
                        }
                        hir::WhenPat::Wildcard { .. } | hir::WhenPat::Rest { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when variant arg pattern",
                                at: arg_pat.span().into(),
                            });
                        }
                    }
                }

                Ok(())
            }
            hir::WhenPat::Tuple { elements, .. } => {
                let CgTy::Tuple(tuple_ty) = subject_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple pattern subject type",
                        at: pat.span().into(),
                    });
                };

                let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) = self.types.kind(tuple_ty)
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "tuple type id",
                        at: pat.span().into(),
                    });
                };

                let mut has_rest = false;
                for (idx, elem_pat) in elements.iter().enumerate() {
                    if matches!(elem_pat, hir::WhenPat::Rest { .. }) {
                        if idx + 1 != elements.len() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when tuple pattern rest position",
                                at: elem_pat.span().into(),
                            });
                        }
                        has_rest = true;
                        break;
                    }
                }

                let pat_arity = if has_rest {
                    elements.len().saturating_sub(1)
                } else {
                    elements.len()
                };

                if (!has_rest && pat_arity != tuple_elems.len()) || (has_rest && pat_arity > tuple_elems.len()) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple pattern arity mismatch",
                        at: pat.span().into(),
                    });
                }

                let llvm_tuple_ty = self.llvm_tuple_type(at, tuple_ty)?;
                let loaded =
                    self.builder
                        .build_load(llvm_tuple_ty, subject_ptr, "load_when_tuple")?;
                let tuple_v = loaded.into_struct_value();

                for (idx, elem_pat) in elements.iter().enumerate() {
                    if matches!(elem_pat, hir::WhenPat::Rest { .. }) {
                        break;
                    }
                    let elem_ty =
                        self.lookup_tuple_element(tuple_ty, idx as u32, elem_pat.span())?;

                    let extracted_v = if elem_ty == CgTy::Unit {
                        CgValue::unit()
                    } else {
                        let raw = self
                            .builder
                            .build_extract_value(tuple_v, idx as u32, "when_tuple_elem")?;
                        self.cg_value_from_loaded(elem_pat.span(), elem_ty, raw)?
                    };

                    match elem_pat {
                        hir::WhenPat::Bind { .. } => {
                            // 直接把元素作为 subject 绑定（避免额外临时 slot）。
                            let hir::WhenPat::Bind { id, name, .. } = elem_pat else {
                                unreachable!()
                            };
                            let ptr = self.create_entry_alloca(at, name, elem_ty)?;
                            self.store_local_value(at, ptr, elem_ty, extracted_v)?;
                            self.env.insert(
                                *id,
                                CgLocal {
                                    ty: elem_ty,
                                    ptr,
                                    mutable: false,
                                },
                            );
                        }
                        hir::WhenPat::Tuple { .. } | hir::WhenPat::Variant { .. } => {
                            // 递归绑定：需要一个临时 slot 让子 pattern 能 load/extract。
                            let tmp_name = format!("when_tuple_elem_{idx}");
                            let tmp_ptr = self.create_entry_alloca(at, &tmp_name, elem_ty)?;
                            self.store_local_value(at, tmp_ptr, elem_ty, extracted_v)?;
                            self.bind_when_pat(at, elem_ty, elem_pat, tmp_ptr)?;
                        }
                        _ => {}
                    }
                }

                Ok(())
            }
        }
    }

    fn when_first_matching_arm_for_enum_variant(
        &self,
        arms: &[hir::WhenArm],
        variant_name: &str,
    ) -> Option<usize> {
        for (idx, arm) in arms.iter().enumerate() {
            match &arm.pat {
                hir::WhenPat::Else { .. }
                | hir::WhenPat::Wildcard { .. }
                | hir::WhenPat::Bind { .. } => return Some(idx),
                hir::WhenPat::Variant { name, .. } if name == variant_name => return Some(idx),
                _ => {}
            }
        }
        None
    }

    fn when_first_matching_arm_for_bool(&self, arms: &[hir::WhenArm], value: bool) -> Option<usize> {
        for (idx, arm) in arms.iter().enumerate() {
            match &arm.pat {
                hir::WhenPat::Else { .. }
                | hir::WhenPat::Wildcard { .. }
                | hir::WhenPat::Bind { .. } => return Some(idx),
                hir::WhenPat::BoolLit { value: v, .. } if *v == value => return Some(idx),
                _ => {}
            }
        }
        None
    }

    fn codegen_when_tuple_pat_cond(
        &mut self,
        at: crate::span::Span,
        tuple_ty: TypeId,
        elements: &[hir::WhenPat],
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple type id",
                at: at.into(),
            });
        };

        let mut rest_idx: Option<usize> = None;
        for (idx, pat) in elements.iter().enumerate() {
            if matches!(pat, hir::WhenPat::Rest { .. }) {
                rest_idx = Some(idx);
                break;
            }
        }

        if let Some(rest) = rest_idx {
            if rest + 1 != elements.len() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "when tuple pattern rest position",
                    at: elements[rest].span().into(),
                });
            }
        }

        let pat_arity = rest_idx.unwrap_or(elements.len());
        if (rest_idx.is_none() && pat_arity != tuple_elems.len()) || (rest_idx.is_some() && pat_arity > tuple_elems.len()) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when tuple pattern arity mismatch",
                at: at.into(),
            });
        }

        let llvm_tuple_ty = self.llvm_tuple_type(at, tuple_ty)?;
        let loaded = self
            .builder
            .build_load(llvm_tuple_ty, subject_ptr, "load_when_tuple")?;
        let tuple_v = loaded.into_struct_value();

        let mut cond = self.context.bool_type().const_int(1, false);
        for (idx, elem_pat) in elements.iter().enumerate().take(pat_arity) {
            let elem_ty = self.lookup_tuple_element(tuple_ty, idx as u32, elem_pat.span())?;
            let elem_cond =
                self.codegen_when_pat_cond_for_tuple_elem(at, tuple_ty, idx, elem_ty, tuple_v, elem_pat)?;
            cond = self.builder.build_and(cond, elem_cond, "when_tuple_and")?;
        }
        Ok(cond)
    }

    fn codegen_when_pat_cond_for_tuple_elem(
        &mut self,
        at: crate::span::Span,
        tuple_ty: TypeId,
        elem_idx: usize,
        elem_ty: CgTy,
        tuple_v: inkwell::values::StructValue<'ctx>,
        pat: &hir::WhenPat,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Bind { .. }
            | hir::WhenPat::Rest { .. } => Ok(self.context.bool_type().const_int(1, false)),
            hir::WhenPat::BoolLit { value, .. } => {
                let CgTy::Bool = elem_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple elem bool pattern type",
                        at: pat.span().into(),
                    });
                };
                let raw = self
                    .builder
                    .build_extract_value(tuple_v, elem_idx as u32, "when_tuple_elem")?
                    .into_int_value();
                let expected = self.context.bool_type().const_int(*value as u64, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    raw,
                    expected,
                    "when_tuple_bool_eq",
                )?)
            }
            hir::WhenPat::IntLit { span } => {
                let CgTy::Int(int_ty) = elem_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple elem int pattern type",
                        at: pat.span().into(),
                    });
                };
                let raw = self
                    .builder
                    .build_extract_value(tuple_v, elem_idx as u32, "when_tuple_elem")?
                    .into_int_value();
                let text = self.source.slice(*span);
                let value = parse_int_literal_decimal(text);
                let value = mask_to_bits(value, int_ty.bits) as u64;
                let expected = self.int_type(int_ty).const_int(value, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    raw,
                    expected,
                    "when_tuple_int_eq",
                )?)
            }
            hir::WhenPat::Tuple { elements, .. } => {
                let CgTy::Tuple(nested_tuple_ty) = elem_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple elem tuple pattern type",
                        at: pat.span().into(),
                    });
                };

                let TypeKind::Value(ValueTypeKind::Tuple(_)) = self.types.kind(nested_tuple_ty) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "tuple type id",
                        at: pat.span().into(),
                    });
                };

                // 由于 extractvalue 返回的是一个“by-value tuple struct”，我们先把它落到临时 slot，
                // 再复用 `codegen_when_tuple_pat_cond` 的逻辑生成递归比较。
                let nested_raw =
                    self.builder
                        .build_extract_value(tuple_v, elem_idx as u32, "when_tuple_elem")?;
                let nested_value = self.cg_value_from_loaded(pat.span(), elem_ty, nested_raw)?;
                let tmp_name = format!("when_tuple_nested_{}_{}", tuple_ty.as_u32(), elem_idx);
                let tmp_ptr = self.create_entry_alloca(at, &tmp_name, elem_ty)?;
                self.store_local_value(at, tmp_ptr, elem_ty, nested_value)?;
                self.codegen_when_tuple_pat_cond(at, nested_tuple_ty, elements, tmp_ptr)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when tuple pattern",
                at: pat.span().into(),
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
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
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
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
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
                CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
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
            // 说明：当前阶段不支持 tuple/struct 作为函数返回类型，因此这里仅提供占位值；
            // 若后续误用，会在 emit/store 阶段触发结构化错误而非 panic。
            CgTy::Tuple(ty) => CgValue {
                ty: CgTy::Tuple(ty),
                value: None,
            },
            CgTy::Struct(ty) => CgValue { ty: CgTy::Struct(ty), value: None },
            CgTy::Enum(ty) => CgValue { ty: CgTy::Enum(ty), value: None },
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
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "aggregate return type",
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
            CgTy::Tuple(_) => {
                let raw = self
                    .builder
                    .build_load(self.llvm_basic_type_of(span, local.ty)?, local.ptr, "load_tuple")?;
                Ok(CgValue {
                    ty: local.ty,
                    value: Some(raw),
                })
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
            CgTy::Enum(_) => {
                let raw = self
                    .builder
                    .build_load(self.llvm_basic_type_of(span, local.ty)?, local.ptr, "load_enum")?;
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

    fn codegen_tuple_lit(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        elements: &[hir::Expr],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(CgTy::Tuple(tuple_ty)) = self.cg_ty_of(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple literal type",
                at: span.into(),
            });
        };

        let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple literal type",
                at: span.into(),
            });
        };

        if element_tys.len() != elements.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple literal arity mismatch",
                at: span.into(),
            });
        }

        let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_tuple_ty.get_undef().into();

        for (idx, (elem_expr, elem_ty)) in elements.iter().zip(element_tys.iter()).enumerate() {
            let elem_cg = self.cg_ty_of(*elem_ty).ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple element type",
                at: elem_expr.span.into(),
            })?;

            let elem_v = self.codegen_expr(elem_expr)?;
            let coerced = self.coerce_value(elem_expr.span, elem_v, elem_cg)?;

            let raw: BasicValueEnum<'ctx> = match elem_cg {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "tuple element value",
                    at: elem_expr.span.into(),
                })?,
            };

            let name = format!("insert_elem_{idx}");
            agg = self.builder.build_insert_value(agg, raw, idx as u32, &name)?;
        }

        Ok(CgValue {
            ty: CgTy::Tuple(tuple_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_member_access(
        &mut self,
        _span: crate::span::Span,
        receiver: &hir::Expr,
        member: &hir::MemberAccess,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match member.resolved.as_ref() {
            Some(hir::MemberRef::Value { fqn, .. }) => {
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
                return self.cg_value_from_loaded(member.span, field_ty, extracted);
            }
            Some(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "member access target",
                    at: member.span.into(),
                });
            }
            None => {}
        }

        // tuple 元素访问（spec §2.3.3）：`t._0` / `t._1` / ...
        let Some(elem_idx) = parse_tuple_member_index(&member.name) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "member access target",
                at: member.span.into(),
            });
        };

        // 优先路径：`localTuple._0` —— 用 GEP 从 alloca slot 取元素。
        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind {
            if let Some(local) = self.env.get(*id) {
                if let CgTy::Tuple(tuple_ty) = local.ty {
                    let elem_ty = self.lookup_tuple_element(tuple_ty, elem_idx, member.span)?;
                    if elem_ty == CgTy::Unit {
                        return Ok(CgValue::unit());
                    }

                    let llvm_tuple_ty = self.llvm_tuple_type(member.span, tuple_ty)?;
                    let elem_ptr = self.builder.build_struct_gep(
                        llvm_tuple_ty,
                        local.ptr,
                        elem_idx,
                        "tuple_elem_gep",
                    )?;
                    let llvm_elem_ty = self.llvm_basic_type_of(member.span, elem_ty)?;
                    let loaded =
                        self.builder
                            .build_load(llvm_elem_ty, elem_ptr, "load_tuple_elem")?;
                    return self.cg_value_from_loaded(member.span, elem_ty, loaded);
                }
            }
        }

        // fallback：先把 receiver 降到值，再用 extractvalue 取元素。
        let recv = self.codegen_expr(receiver)?;
        let CgTy::Tuple(tuple_ty) = recv.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "member access receiver type",
                at: receiver.span.into(),
            });
        };

        let elem_ty = self.lookup_tuple_element(tuple_ty, elem_idx, member.span)?;
        if elem_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let raw = recv.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "member access receiver value",
            at: receiver.span.into(),
        })?;
        let tuple_v = raw.into_struct_value();
        let extracted = self
            .builder
            .build_extract_value(tuple_v, elem_idx, "extract_tuple_elem")?;
        self.cg_value_from_loaded(member.span, elem_ty, extracted)
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

    fn lookup_tuple_element(
        &self,
        tuple_ty: TypeId,
        elem_idx: u32,
        at: crate::span::Span,
    ) -> Result<CgTy, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple type id",
                at: at.into(),
            });
        };

        let elem_ty = elements
            .get(elem_idx as usize)
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple element out of bounds",
                at: at.into(),
            })?;

        self.cg_ty_of(elem_ty).ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "tuple element type",
            at: at.into(),
        })
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
            CgTy::Tuple(tuple_ty) => CgValue {
                ty: CgTy::Tuple(tuple_ty),
                value: Some(raw),
            },
            CgTy::Struct(struct_ty) => CgValue {
                ty: CgTy::Struct(struct_ty),
                value: Some(raw),
            },
            CgTy::Enum(enum_ty) => CgValue {
                ty: CgTy::Enum(enum_ty),
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
            (CgTy::Tuple(from), CgTy::Tuple(to)) if from == to => Ok(value),
            (CgTy::Struct(from), CgTy::Struct(to)) if from == to => Ok(value),
            (CgTy::Enum(from), CgTy::Enum(to)) if from == to => Ok(value),
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
            CgTy::Tuple(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple exit code",
                at: at.into(),
            }),
            CgTy::Struct(_) | CgTy::Enum(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "composite exit code",
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
            TypeKind::Value(ValueTypeKind::Tuple(_)) => Some(CgTy::Tuple(ty)),
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                if self.struct_layouts.contains_key(&nominal.fqn) {
                    return Some(CgTy::Struct(ty));
                }
                if self.enum_layouts.contains_key(&nominal.fqn) {
                    return Some(CgTy::Enum(ty));
                }
                None
            }
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
            CgTy::Tuple(tuple_ty) => self.llvm_tuple_type(at, tuple_ty)?.into(),
            CgTy::Struct(struct_ty) => self.llvm_struct_type(at, struct_ty)?.into(),
            CgTy::Enum(enum_ty) => self.llvm_enum_type(at, enum_ty)?.into(),
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

    fn llvm_enum_type(
        &self,
        at: crate::span::Span,
        ty: TypeId,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum type id",
                at: at.into(),
            });
        };

        let layout = self
            .enum_layouts
            .get(&nominal.fqn)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "enum layout",
                at: at.into(),
            })?;

        if let Some(existing) = self.context.get_struct_type(&layout.fqn) {
            return Ok(existing);
        }

        // 最小 rich enum 表示：`{ tag: i32, payload: iN }`
        // - tag：按声明顺序分配的 variant id
        // - payload：当前阶段只支持“单 machine word 承载的小 payload”
        let enum_ty = self.context.opaque_struct_type(&layout.fqn);
        let tag_ty = self.context.i32_type();
        let payload_ty = self.int_type(IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        });
        enum_ty.set_body(&[tag_ty.into(), payload_ty.into()], false);
        Ok(enum_ty)
    }

    fn llvm_tuple_type(
        &self,
        at: crate::span::Span,
        ty: TypeId,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple type id",
                at: at.into(),
            });
        };

        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(elements.len());
        for elem_ty in elements {
            let elem_cg = self.cg_ty_of(*elem_ty).ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple element type",
                at: at.into(),
            })?;
            llvm_fields.push(self.llvm_basic_type_of(at, elem_cg)?);
        }

        Ok(self.context.struct_type(&llvm_fields, false))
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
        // - struct/enum（值类型）：以 LLVM struct by-value 形式存入栈 slot（`alloca`）
        let v = self.coerce_value(at, value, ty)?;
        match ty {
            CgTy::Unit => {
                let zero = self.context.i8_type().const_int(0, false);
                let _ = self.builder.build_store(ptr, zero)?;
            }
            CgTy::Bool | CgTy::Int(_) | CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
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

fn parse_tuple_member_index(text: &str) -> Option<u32> {
    let digits = text.strip_prefix('_')?;
    if digits.is_empty() {
        return None;
    }
    if !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok()
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
