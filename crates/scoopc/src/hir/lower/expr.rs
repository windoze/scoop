//! 表达式 lowering（TODO T0103c）。
//!
//! 说明：
//! - 该模块只负责 AST → HIR 的表达式部分 lowering；
//! - 规则与 span 选择尽量保持与原先 `lower/mod.rs` 一致，避免 HIR fixtures 输出漂移。

use crate::ast;
use crate::resolve::{ConstructorOverload, ParamSig, Visibility};
use crate::span::Span;
use crate::syntax::char_literal::parse_char_literal;
use crate::syntax::float_literal::{FloatLiteralSuffix, parse_float_literal};
use crate::ty::{RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::HirLowering;
use super::types::*;
use super::util::*;

use super::super::{
    Block, CallArg, ClosureExpr, ClosureId, EffectOpRef, Expr, ExprKind, HandleArm, HandleArmKind,
    HandleBinder, HandleExpr, HandleOp, InterpolatedStringPart, LiteralKind, MemberAccess,
    MemberRef, Param, Stmt, StmtKind, StructLitField, ValDecl, ValueRef, WhenArm, WhenPat,
};

impl<'a> HirLowering<'a> {
    pub(super) fn lower_expr(&mut self, pkg_prefix: &str, e: &ast::Expr) -> Expr {
        self.lower_expr_with_expected(pkg_prefix, e, ExpectedExpr::default())
    }

    /// lowering 表达式并携带“期望类型 hint”。
    ///
    /// 注意：该 hint 仅用于把 `[...]` 降到稳定的 builder/intrinsics 调用形态（TODO T1317c），
    /// 不等价于完整 typecheck 的 expected-type 推断。
    pub(super) fn lower_expr_with_expected(
        &mut self,
        pkg_prefix: &str,
        e: &ast::Expr,
        expected: ExpectedExpr,
    ) -> Expr {
        let (kind, ty) = match &e.kind {
            ast::ExprKind::Missing => (ExprKind::Missing, self.builtins.any),
            ast::ExprKind::IntLit => {
                let ty = self
                    .typechecked_expr_ty(e.span)
                    .filter(|ty| self.is_integer_type(*ty))
                    .unwrap_or(self.builtins.int);
                (ExprKind::Literal(LiteralKind::Int), ty)
            }
            ast::ExprKind::FloatLit => {
                let parsed = parse_float_literal(self.source.slice(e.span));
                if self.typechecked_expr_ty(e.span) == Some(self.builtins.float32) {
                    (
                        ExprKind::Literal(LiteralKind::Float32(parsed.value as f32)),
                        self.builtins.float32,
                    )
                } else {
                    match parsed.suffix {
                        FloatLiteralSuffix::Float64 => (
                            ExprKind::Literal(LiteralKind::Float64(parsed.value)),
                            self.builtins.float64,
                        ),
                        FloatLiteralSuffix::Float32 => (
                            ExprKind::Literal(LiteralKind::Float32(parsed.value as f32)),
                            self.builtins.float32,
                        ),
                    }
                }
            }
            ast::ExprKind::CharLit => {
                let value = parse_char_literal(self.source.slice(e.span))
                    .expect("lexer validated Char literal before HIR lowering");
                (
                    ExprKind::Literal(LiteralKind::Char(value)),
                    self.builtins.char_,
                )
            }
            ast::ExprKind::StringLit => {
                (ExprKind::Literal(LiteralKind::String), self.builtins.string)
            }
            ast::ExprKind::UnitLit => (ExprKind::Literal(LiteralKind::Unit), self.builtins.unit),
            ast::ExprKind::ArrayLit { elements } => {
                if let Some((target, result_ty, element_expected_ty)) =
                    self.array_lit_lowering_hint(e.span, expected)
                {
                    self.lower_array_lit_expr(
                        pkg_prefix,
                        e.span,
                        elements,
                        target,
                        result_ty,
                        element_expected_ty,
                    )
                } else {
                    let lowered_elements: Vec<Expr> = elements
                        .iter()
                        .map(|element| self.lower_expr(pkg_prefix, element))
                        .collect();
                    match self.infer_array_lit_ty_from_lowered_elements(&lowered_elements) {
                        Some(result_ty) => self.build_array_lit_expr(
                            e.span,
                            lowered_elements,
                            ArrayLitTarget::Array,
                            result_ty,
                        ),
                        None => (ExprKind::Todo("array_lit"), self.builtins.any),
                    }
                }
            }
            ast::ExprKind::ClassLit { .. } => (ExprKind::Todo("class_lit"), self.builtins.string),
            ast::ExprKind::InterpolatedString { raw, parts } => {
                let parts = parts
                    .iter()
                    .map(|p| match p {
                        ast::InterpolatedStringPart::Text { span } => {
                            InterpolatedStringPart::Text { span: *span }
                        }
                        ast::InterpolatedStringPart::Expr { expr } => {
                            InterpolatedStringPart::Expr {
                                expr: self.lower_expr(pkg_prefix, expr),
                            }
                        }
                    })
                    .collect();
                (
                    ExprKind::InterpolatedString { raw: *raw, parts },
                    self.builtins.string,
                )
            }
            ast::ExprKind::Ident(id) => self
                .try_lower_top_level_fun_value_expr(e.span)
                .unwrap_or_else(|| self.lower_ident_expr(id)),
            ast::ExprKind::Block(b) => {
                let b = self.lower_block_with_expected(pkg_prefix, b, expected);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::DoBlock { body, .. } => {
                // `do { ... }` 在 HIR 层面与普通 block 表达式等价。
                let b = self.lower_block_with_expected(pkg_prefix, body, expected);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::UnsafeBlock { body, .. } => {
                // `@Unsafe do { ... }` 仅影响 typecheck 的 unsafe context，
                // 在 HIR/codegen 层面当前可按普通 block 表达式处理（T1004）。
                let b = self.lower_block_with_expected(pkg_prefix, body, expected);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::SafeBlock { body, .. } => {
                // `@Safe do { ... }` 同样仅影响 typecheck 的 unsafe context，
                // 在 HIR/codegen 层面当前可按普通 block 表达式处理（T1021）。
                let b = self.lower_block_with_expected(pkg_prefix, body, expected);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::TypeApply { callee, .. } => self
                .try_lower_top_level_fun_value_expr(e.span)
                .unwrap_or_else(|| {
                    // v0：HIR 暂不承载显式类型实参；先把它视为 callee 的透明包装。
                    // 反射 intrinsics 的 type args 语义目前由 comptime 解释器消费（T1204）。
                    let inner = self.lower_expr(pkg_prefix, callee);
                    (inner.kind, inner.ty)
                }),
            ast::ExprKind::Call { callee, args } => {
                // 调用表达式在 typecheck 后已经有稳定结果类型；这里即使后续把 member/extension/default-arg
                // 调用降糖成其它 HIR 形态，也要优先保留该结果类型，避免局部 `val x = call(...)`
                // 因为中间表达式被写成 `Any` 而在 codegen 时触发错误的 value coercion。
                let typechecked_call_ty = self.typechecked_expr_ty(e.span);
                let call_ty = typechecked_call_ty.unwrap_or(self.builtins.any);

                // T0108：safe call 方法调用：`receiver?.method(args)` → when desugar。
                if let ast::ExprKind::SafeMemberAccess {
                    receiver: inner_receiver,
                    op_span,
                    member,
                } = &callee.kind
                {
                    let (kind, ty) = self.lower_safe_call_expr(
                        pkg_prefix,
                        e.span,
                        inner_receiver,
                        *op_span,
                        member,
                        args,
                    );
                    return Expr {
                        span: e.span,
                        ty: typechecked_call_ty.unwrap_or(ty),
                        kind,
                    };
                }

                // 扩展函数调用（T0312）：把 `receiver.ext(args...)` 降糖为普通顶层调用：
                // `ext(receiver, args...)`。
                //
                // 说明：
                // - 运行期 codegen 当前只直接支持 `TopLevel` callee（以及少量特殊 member call）；
                // - 这里在 lowering 阶段提前把 extension call 改写为顶层调用，避免后端无法识别 `MemberAccess` callee。
                if let Some((kind, ty)) = (|| {
                    let ast::ExprKind::MemberAccess { receiver, member } = &callee.kind else {
                        return None;
                    };
                    let resolved = self.resolved_member_for_lowering(member);
                    let ast::ResolvedMemberRef::ExtensionFun { fqn } = resolved.as_ref()? else {
                        return None;
                    };

                    let sig = self.fun_sig_by_fqn(fqn);
                    // expected-type hint 目前只用于数组字面量 `[...]` 的 lowering（Array vs MutableArray）。
                    // receiver 不是数组字面量时无需解析签名里的 receiver TypeRef，避免跨文件 span 误用。
                    let receiver_is_array_lit =
                        matches!(receiver.kind, ast::ExprKind::ArrayLit { .. });
                    let receiver_expected = ExpectedExpr {
                        value_ty: None,
                        array_lit_target: match receiver_is_array_lit {
                            true => sig
                                .as_ref()
                                .and_then(|sig| sig.receiver.as_ref())
                                .and_then(|ty| self.array_lit_target_from_type_ref(ty)),
                            false => None,
                        },
                        array_lit_ty: None,
                        struct_lit_ty: None,
                    };
                    let receiver =
                        self.lower_expr_with_expected(pkg_prefix, receiver, receiver_expected);

                    let mut lowered_args = Vec::with_capacity(args.len() + 1);
                    lowered_args.push(CallArg::Positional(receiver));
                    let mut positional_index = 0usize;
                    for arg in args {
                        let expected = self.expected_expr_for_fun_call_arg(
                            sig.as_ref(),
                            arg,
                            positional_index,
                        );
                        if !matches!(arg.kind, ast::ExprKind::NamedArg { .. }) {
                            positional_index = positional_index.saturating_add(1);
                        }
                        lowered_args
                            .push(self.lower_call_arg_with_expected(pkg_prefix, arg, expected));
                    }

                    let callee = Expr {
                        span: callee.span,
                        ty: self.builtins.any,
                        kind: ExprKind::VarRef(ValueRef::TopLevel {
                            id: self.symbols.intern_top_level(fqn.clone()),
                            fqn: fqn.clone(),
                        }),
                    };

                    Some((
                        ExprKind::Call {
                            callee: Box::new(callee),
                            args: lowered_args,
                        },
                        call_ty,
                    ))
                })() {
                    (kind, ty)
                } else if let Some((kind, ty)) =
                    self.try_lower_effect_op_call_expr(pkg_prefix, e.span, callee, args)
                {
                    (kind, typechecked_call_ty.unwrap_or(ty))
                } else if let Some((kind, ty)) = (|| {
                    // T1508a：直连成员函数调用（final/private）：把 `receiver.method(args...)`
                    // 降糖为顶层调用 `Owner.method(receiver, args...)`。
                    //
                    // 注意：
                    // - 这里刻意只改写 value receiver 的 member call；type receiver（companion dispatch）
                    //   会走其它 lowering 路径（当前阶段可能仍保持为 `MemberAccess` 供后续任务落地）。
                    // - `GC.pin/unpin` 等少量内建 member call 依赖后端 `MemberAccess` special-case，
                    //   不能在这里改写为顶层调用。
                    let ast::ExprKind::MemberAccess { receiver, member } = &callee.kind else {
                        return None;
                    };
                    if self.should_keep_member_call_as_member_access(receiver, member) {
                        return None;
                    }
                    let resolved = self.resolved_member_for_lowering(member);
                    let ast::ResolvedMemberRef::Fun { fqn } = resolved.as_ref()? else {
                        return None;
                    };

                    if fqn == "scoop.core.GC.pin"
                        || fqn == "scoop.core.GC.unpin"
                        || fqn == "scoop.core.GC.handleNew"
                        || fqn == "scoop.core.GC.handleGet"
                        || fqn == "scoop.core.GC.handleDrop"
                    {
                        return None;
                    }

                    // T1508a/T1508c：对 class/object/interface member method 做降糖；
                    // struct/value type 的 member call 语义留给其它任务（保持既有 HIR fixtures 稳定）。
                    let (owner_fqn, _) = fqn.as_str().rsplit_once('.')?;
                    let owner_is_class =
                        matches!(self.type_kinds.get(owner_fqn), Some(ast::TypeKind::Class));
                    let owner_is_interface = matches!(
                        self.type_kinds.get(owner_fqn),
                        Some(ast::TypeKind::Interface)
                    );
                    let owner_is_object = self.index.object_types.contains(owner_fqn);
                    if !owner_is_class && !owner_is_interface && !owner_is_object {
                        return None;
                    }

                    // resolver 的 type receiver（例如 `TypeName.member` / `EffectName.op`）约定为：
                    // receiver ident 不写回 `resolved`。这里用该信号避免把 companion dispatch 误改写为
                    // “带隐式 receiver 参数”的普通 member call。
                    if let ast::ExprKind::Ident(id) = &receiver.kind
                        && id.resolved.is_none()
                        && self.source.slice(id.span) != "this"
                    {
                        return None;
                    }

                    let receiver = self.lower_expr(pkg_prefix, receiver);

                    let mut lowered_args = Vec::with_capacity(args.len() + 1);
                    lowered_args.push(CallArg::Positional(receiver));
                    for arg in args {
                        lowered_args.push(self.lower_call_arg_with_expected(
                            pkg_prefix,
                            arg,
                            ExpectedExpr::default(),
                        ));
                    }

                    let callee = Expr {
                        span: callee.span,
                        ty: self.builtins.any,
                        kind: ExprKind::VarRef(ValueRef::TopLevel {
                            id: self.symbols.intern_top_level(fqn.clone()),
                            fqn: fqn.clone(),
                        }),
                    };

                    Some((
                        ExprKind::Call {
                            callee: Box::new(callee),
                            args: lowered_args,
                        },
                        call_ty,
                    ))
                })() {
                    (kind, ty)
                } else if let Some((kind, ty)) = self.try_lower_struct_ctor_call_expr(
                    pkg_prefix,
                    e.span,
                    callee,
                    args,
                    typechecked_call_ty,
                ) {
                    (kind, typechecked_call_ty.unwrap_or(ty))
                } else if let Some((kind, ty)) =
                    self.try_lower_default_args_call_expr(pkg_prefix, e.span, callee, args)
                {
                    (kind, typechecked_call_ty.unwrap_or(ty))
                } else {
                    // class ctor call 仍会被降低为 `UnresolvedIdent`，
                    // 但 codegen 需要知道 typecheck 已选中的 ctor 目标与参数绑定。
                    if let ast::ExprKind::Ident(id) = &callee.kind
                        && let Some(binding) = self
                            .typechecked_ctor_call_binding(e.span)
                            .or_else(|| self.resolver_fallback_ctor_call_binding(id, args))
                        && matches!(
                            self.type_kinds.get(&binding.owner_fqn),
                            Some(ast::TypeKind::Class)
                        )
                    {
                        self.ctor_call_sites
                            .entry(self.call_site(e.span))
                            .or_insert(super::super::CtorCallInfo {
                                class_fqn: binding.owner_fqn,
                                ctor_span: binding.ctor_span,
                                arg_mapping: binding.arg_mapping,
                            });
                    }

                    let callee_fqn = self.callee_top_level_fqn(callee);
                    let sig = callee_fqn.and_then(|fqn| self.fun_sig_by_fqn(fqn));

                    // T0113: find the vararg param index (if any) from the callee sig.
                    let vararg_param_index = sig.as_ref().and_then(|s| {
                        // Account for receiver: if the function has a receiver, params
                        // in the sig start with it, but call args don't include receiver.
                        let offset = if s.receiver.is_some() { 1 } else { 0 };
                        s.params.iter().enumerate().find_map(|(i, p)| {
                            if p.is_vararg {
                                Some(i.saturating_sub(offset))
                            } else {
                                None
                            }
                        })
                    });

                    let callee = Box::new(self.lower_expr(pkg_prefix, callee));

                    // T0113: if there's a vararg param, split args into pre-vararg,
                    // vararg, and post-vararg, and wrap the vararg args in an array literal.
                    let lowered_args = if let Some(va_idx) = vararg_param_index {
                        self.lower_call_args_with_vararg(
                            pkg_prefix,
                            e.span,
                            args,
                            sig.as_ref(),
                            va_idx,
                        )
                    } else {
                        let mut positional_index = 0usize;
                        let mut out: Vec<CallArg> = Vec::with_capacity(args.len());
                        for arg in args {
                            let expected = self.expected_expr_for_fun_call_arg(
                                sig.as_ref(),
                                arg,
                                positional_index,
                            );
                            if !matches!(arg.kind, ast::ExprKind::NamedArg { .. }) {
                                positional_index = positional_index.saturating_add(1);
                            }
                            out.push(self.lower_call_arg_with_expected(pkg_prefix, arg, expected));
                        }
                        out
                    };

                    (
                        ExprKind::Call {
                            callee,
                            args: lowered_args,
                        },
                        call_ty,
                    )
                }
            }
            // Appendix B.5.5：spread 仅在调用实参语境下有意义；HIR v0 暂不承载该语义。
            ast::ExprKind::SpreadArg { .. } => (ExprKind::Todo("spread_arg"), self.builtins.any),
            ast::ExprKind::NamedArg { .. } => (ExprKind::Todo("named_arg"), self.builtins.any),
            ast::ExprKind::TupleLit { elements } => {
                let elements: Vec<Expr> = elements
                    .iter()
                    .map(|e| self.lower_expr(pkg_prefix, e))
                    .collect();
                let inferred_ty = if elements.is_empty() {
                    self.builtins.unit
                } else {
                    self.types.ty_tuple(elements.iter().map(|e| e.ty).collect())
                };
                let ty = self.typechecked_expr_ty(e.span).unwrap_or(inferred_ty);
                (ExprKind::TupleLit { elements }, ty)
            }
            ast::ExprKind::Lambda(lam) => self.lower_lambda_expr(pkg_prefix, e.span, lam),
            ast::ExprKind::StructLit { ty, fields } => {
                self.lower_struct_lit_expr(pkg_prefix, e.span, ty, fields, expected.struct_lit_ty)
            }
            ast::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond = Box::new(self.lower_expr(pkg_prefix, cond));
                let then_branch =
                    Box::new(self.lower_expr_with_expected(pkg_prefix, then_branch, expected));
                let else_branch = else_branch
                    .as_ref()
                    .map(|e| Box::new(self.lower_expr_with_expected(pkg_prefix, e, expected)));
                let ty = self
                    .typechecked_expr_ty(e.span)
                    .or(expected.array_lit_ty)
                    .or(expected.struct_lit_ty)
                    .unwrap_or(self.builtins.any);
                (
                    ExprKind::If {
                        cond,
                        then_branch,
                        else_branch,
                    },
                    ty,
                )
            }
            ast::ExprKind::When { subject, arms } => {
                let subject = Box::new(self.lower_expr(pkg_prefix, subject));
                let arms = arms
                    .iter()
                    .map(|a| self.lower_when_arm(pkg_prefix, a, expected))
                    .collect();
                let ty = self
                    .typechecked_expr_ty(e.span)
                    .or(expected.array_lit_ty)
                    .or(expected.struct_lit_ty)
                    .unwrap_or(self.builtins.any);
                (ExprKind::When { subject, arms }, ty)
            }
            ast::ExprKind::Handle {
                body,
                arms,
                finally,
            } => {
                let handle = self.lower_handle_expr(pkg_prefix, body, arms, finally.as_ref());
                let ty = self.typechecked_expr_ty(e.span).unwrap_or(handle.body.ty);
                (ExprKind::Handle(handle), ty)
            }
            ast::ExprKind::Async { body } => {
                let task_expr = self.lower_async_task_expr_from_block(pkg_prefix, e.span, body);
                (task_expr.kind, task_expr.ty)
            }
            ast::ExprKind::Spawn { .. } => (
                ExprKind::Todo("structured_concurrency_spawn_deferred"),
                self.typechecked_expr_ty(e.span)
                    .unwrap_or(self.builtins.any),
            ),
            ast::ExprKind::Await { await_span, expr } => {
                // T0619：`await expr`（async/await）作为 `Async.await(...)` 的语法糖。
                //
                // NOTE: 这里直接 lower 为 HIR `Perform`，不依赖 resolver 对 `Async.await`
                // 的成员解析写回；这样能避免“语法糖节点需要合成表达式 ident”的复杂度。
                let inner = self.lower_expr(pkg_prefix, expr);
                let op = EffectOpRef {
                    span: *await_span,
                    fqn: Self::ASYNC_AWAIT_FQN.to_string(),
                };
                let effect_ty = self
                    .typechecked_performed_effect_ty(e.span)
                    .unwrap_or_else(|| self.async_effect_type());
                let result_ty = self
                    .typechecked_expr_ty(e.span)
                    .or(expected.value_ty)
                    .or_else(|| {
                        self.typechecked_expr_ty(expr.span)
                            .and_then(|ty| self.task_inner_ty(ty))
                    })
                    .or_else(|| self.task_inner_ty(inner.ty))
                    .unwrap_or(self.builtins.any);
                (
                    ExprKind::Perform {
                        effect_ty,
                        op,
                        args: vec![CallArg::Positional(inner)],
                    },
                    result_ty,
                )
            }
            ast::ExprKind::Join { .. } => (
                ExprKind::Todo("structured_concurrency_join_deferred"),
                self.typechecked_expr_ty(e.span)
                    .unwrap_or(self.builtins.any),
            ),
            ast::ExprKind::MemberAccess { receiver, member } => {
                self.lower_member_access_expr(pkg_prefix, receiver, member)
            }
            ast::ExprKind::SpliceField { .. } => {
                (ExprKind::Todo("splice_field"), self.builtins.any)
            }
            ast::ExprKind::SafeMemberAccess {
                receiver,
                op_span,
                member,
            } => self.lower_safe_member_access_expr(pkg_prefix, e.span, receiver, *op_span, member),
            ast::ExprKind::NotNullAssert { expr, op_span } => {
                self.lower_not_null_assert_expr(pkg_prefix, e.span, expr, *op_span)
            }
            ast::ExprKind::Unary { op, op_span, expr } => {
                let expr = Box::new(self.lower_expr(pkg_prefix, expr));
                let heuristic_ty = match op {
                    ast::UnaryOp::Not => {
                        if expr.ty == self.builtins.bool_ {
                            self.builtins.bool_
                        } else {
                            self.builtins.any
                        }
                    }
                    ast::UnaryOp::Neg | ast::UnaryOp::BitNot => {
                        if self.is_integer_type(expr.ty) {
                            expr.ty
                        } else {
                            self.builtins.any
                        }
                    }
                };
                let ty = self.typechecked_expr_ty(e.span).unwrap_or(heuristic_ty);
                (
                    ExprKind::Unary {
                        op: *op,
                        op_span: *op_span,
                        expr,
                    },
                    ty,
                )
            }
            ast::ExprKind::Binary {
                lhs,
                op,
                op_span,
                rhs,
            } => {
                if *op == ast::BinaryOp::RangeInclusive {
                    return self.lower_range_inclusive_expr(pkg_prefix, e.span, *op_span, lhs, rhs);
                }
                if *op == ast::BinaryOp::Elvis {
                    return self.lower_elvis_expr(pkg_prefix, e.span, lhs, *op_span, rhs);
                }
                let lhs = Box::new(self.lower_expr(pkg_prefix, lhs));
                let rhs = Box::new(self.lower_expr(pkg_prefix, rhs));
                let ty = self
                    .typechecked_expr_ty(e.span)
                    .unwrap_or_else(|| self.lower_binary_expr_type(&lhs, &rhs, *op));
                (
                    ExprKind::Binary {
                        lhs,
                        op: *op,
                        op_span: *op_span,
                        rhs,
                    },
                    ty,
                )
            }
            ast::ExprKind::Assign { .. } => (ExprKind::Todo("assign"), self.builtins.any),
            ast::ExprKind::TypeCheck {
                expr,
                op,
                op_span,
                ty,
            } => {
                let expr = Box::new(self.lower_expr(pkg_prefix, expr));
                let target_ty = self.lower_type_ref(ty);
                (
                    ExprKind::TypeCheck {
                        expr,
                        op: *op,
                        op_span: *op_span,
                        target_ty,
                    },
                    self.builtins.bool_,
                )
            }
            ast::ExprKind::Cast {
                expr,
                op,
                op_span,
                ty,
            } => {
                let expr = Box::new(self.lower_expr(pkg_prefix, expr));
                let target_ty = self.lower_type_ref(ty);
                let out_ty = match op {
                    ast::CastOp::As => target_ty,
                    ast::CastOp::AsQ => self.types.ty_option(target_ty),
                };
                (
                    ExprKind::Cast {
                        expr,
                        op: *op,
                        op_span: *op_span,
                        target_ty,
                    },
                    out_ty,
                )
            }
            ast::ExprKind::WithUpdate {
                base,
                with_span,
                updates,
                resolved_copy_update_tys,
                resolved_copy_update_enums,
            } => {
                return self.lower_with_update_expr(
                    pkg_prefix,
                    e.span,
                    *with_span,
                    base,
                    updates,
                    resolved_copy_update_tys,
                    resolved_copy_update_enums,
                );
            }
        };

        Expr {
            span: e.span,
            ty,
            kind,
        }
    }

    /// `lhs .. rhs` → `{ val __range_start = lhs; val __range_end = rhs; rangeTo(__range_start, __range_end, __scoop_range_default_step(__range_start)) }`
    ///
    /// 说明：
    /// - 复用现有 `scoop.core.rangeTo(start, endInclusive, step)` 实现，不在后端新增 special-case；
    /// - 显式引入临时变量，保证左右端点只求值一次；
    /// - `step = 1` 通过 stdlib helper `__scoop_range_default_step` 派生，避免在 lowering 中伪造源码字面量。
    fn lower_range_inclusive_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        op_span: Span,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
    ) -> Expr {
        let progression_ty = self.typechecked_expr_ty(span).unwrap_or_else(|| {
            self.intern_nominal(Self::INT_PROGRESSION_FQN.to_string(), Vec::new(), None)
        });

        let start_expr = self.lower_expr(pkg_prefix, lhs);
        let end_expr = self.lower_expr(pkg_prefix, rhs);

        let start_decl_span = Span::new(op_span.start, op_span.start + 1);
        let end_decl_span = Span::new(op_span.start + 1, op_span.start + 2);

        let start_id = self.intern_local_symbol(start_decl_span, false);
        let end_id = self.intern_local_symbol(end_decl_span, false);
        let start_name = "__range_start".to_string();
        let end_name = "__range_end".to_string();

        let start_ty = start_expr.ty;
        let end_ty = end_expr.ty;

        let start_decl = Stmt {
            span: start_decl_span,
            ty: self.builtins.unit,
            kind: StmtKind::Val(ValDecl {
                span: start_decl_span,
                id: Some(start_id),
                name: Some(start_name.clone()),
                mutable: false,
                ty: start_ty,
                init: Some(start_expr),
            }),
        };

        let end_decl = Stmt {
            span: end_decl_span,
            ty: self.builtins.unit,
            kind: StmtKind::Val(ValDecl {
                span: end_decl_span,
                id: Some(end_id),
                name: Some(end_name.clone()),
                mutable: false,
                ty: end_ty,
                init: Some(end_expr),
            }),
        };

        let start_ref = Expr {
            span: start_decl_span,
            ty: start_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: start_id,
                name: start_name.clone(),
                decl_span: start_decl_span,
            }),
        };
        let end_ref = Expr {
            span: end_decl_span,
            ty: end_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: end_id,
                name: end_name.clone(),
                decl_span: end_decl_span,
            }),
        };

        let step_helper = Expr {
            span: op_span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self
                    .symbols
                    .intern_top_level(Self::RANGE_DEFAULT_STEP_FQN.to_string()),
                fqn: Self::RANGE_DEFAULT_STEP_FQN.to_string(),
            }),
        };
        let step_expr = Expr {
            span: op_span,
            ty: self.builtins.int,
            kind: ExprKind::Call {
                callee: Box::new(step_helper),
                args: vec![CallArg::Positional(start_ref.clone())],
            },
        };

        let range_to_callee = Expr {
            span: op_span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self
                    .symbols
                    .intern_top_level(Self::RANGE_TO_FQN.to_string()),
                fqn: Self::RANGE_TO_FQN.to_string(),
            }),
        };
        let range_call = Expr {
            span,
            ty: progression_ty,
            kind: ExprKind::Call {
                callee: Box::new(range_to_callee),
                args: vec![
                    CallArg::Positional(start_ref),
                    CallArg::Positional(end_ref),
                    CallArg::Positional(step_expr),
                ],
            },
        };

        Expr {
            span,
            ty: progression_ty,
            kind: ExprKind::Block(Block {
                span,
                ty: progression_ty,
                stmts: vec![
                    start_decl,
                    end_decl,
                    Stmt {
                        span,
                        ty: progression_ty,
                        kind: StmtKind::Expr(range_call),
                    },
                ],
            }),
        }
    }

    /// 从一个 `TypeRef` 判定数组字面量的目标容器类型（Array vs MutableArray）。
    pub(super) fn array_lit_target_from_type_ref(
        &self,
        ty: &ast::TypeRef,
    ) -> Option<ArrayLitTarget> {
        // 注意：`TypeRef` 可能来自其它源文件（例如通过 `Index::FunSig` 跨文件查询得到的签名）。
        // 当前 HIR lowering 仍以“单个 SourceFile 负责 span → 文本切片”为前提，因此当 span 不在
        // 当前文件范围内时，我们只能保守放弃该 hint，避免越界 panic。
        let span = ty.span();
        let text = self.source.text();
        if span.end > text.len() {
            return None;
        }
        // UTF-8 防线：跨文件 span（或内部 bug）可能导致 start/end 落在非字符边界上，
        // 直接 slice 会 panic。这里同样保守放弃 hint。
        if !text.is_char_boundary(span.start) || !text.is_char_boundary(span.end) {
            return None;
        }

        let fqn = self
            .index
            .type_ref_to_fqn_in_file(self.source, self.file, ty)?;
        match fqn.as_str() {
            "scoop.core.Array" => Some(ArrayLitTarget::Array),
            "scoop.core.MutableArray" => Some(ArrayLitTarget::MutableArray),
            // T1317f2：`List/MutableList` 在 sysroot 中作为 `Array/MutableArray` 的 typealias。
            // lowering 阶段只需要知道“数组字面量目标容器类型”，因此这里把别名也视为等价目标。
            "scoop.core.List" => Some(ArrayLitTarget::Array),
            "scoop.core.MutableList" => Some(ArrayLitTarget::MutableArray),
            // T1317f4：stdlib `Set/MutableSet/MapView/MutableMap` 当前阶段以数组为底座（typealias）。
            // 这里同样把它们视为 array literal 的等价目标，便于写 `val s: MutableSet = []` 等用例。
            "scoop.collections.Set" => Some(ArrayLitTarget::Array),
            "scoop.collections.MapView" => Some(ArrayLitTarget::Array),
            "scoop.collections.MutableSet" => Some(ArrayLitTarget::MutableArray),
            "scoop.collections.MutableMap" => Some(ArrayLitTarget::MutableArray),
            _ => None,
        }
    }

    /// 尝试把“当前文件内”的 `TypeRef` 直接 lower 为 `TypeId`。
    ///
    /// 说明：
    /// - `Index::FunSig` 里的 `TypeRef` 可能来自别的源文件；
    /// - HIR lowering 仍以当前 `SourceFile` 负责 span 切片为前提，因此这里只在
    ///   span 明确落在当前文件且满足 UTF-8 边界时才做该回退；
    /// - 失败时返回 `None`，让调用方继续走更保守的 fallback。
    fn local_type_ref_ty(&mut self, ty: &ast::TypeRef) -> Option<TypeId> {
        let span = ty.span();
        let text = self.source.text();
        if span.start > text.len() || span.end > text.len() {
            return None;
        }
        if !text.is_char_boundary(span.start) || !text.is_char_boundary(span.end) {
            return None;
        }
        Some(self.lower_type_ref(ty))
    }

    pub(super) fn typechecked_expr_ty(&mut self, span: Span) -> Option<TypeId> {
        let typecheck_types = self.typecheck_types?;
        let ty = self.file.inferred_expr_ty(span)?;
        Some(self.types.re_intern_from(typecheck_types, ty))
    }

    pub(super) fn typechecked_binding_ty(&mut self, span: Span) -> Option<TypeId> {
        let typecheck_types = self.typecheck_types?;
        let ty = self.file.inferred_binding_ty(span)?;
        Some(self.types.re_intern_from(typecheck_types, ty))
    }

    fn option_inner_ty(&self, ty: TypeId) -> Option<TypeId> {
        match self.types.kind(ty) {
            TypeKind::Value(ValueTypeKind::Option(inner)) => Some(*inner),
            _ => None,
        }
    }

    fn typechecked_performed_effect_ty(&mut self, span: Span) -> Option<TypeId> {
        let typecheck_types = self.typecheck_types?;
        let ty = self.file.inferred_performed_effect_ty(span)?;
        Some(self.types.re_intern_from(typecheck_types, ty))
    }

    fn typechecked_handle_arm_effect_ty(&mut self, span: Span) -> Option<TypeId> {
        let typecheck_types = self.typecheck_types?;
        let ty = self.file.inferred_handle_arm_effect_ty(span)?;
        Some(self.types.re_intern_from(typecheck_types, ty))
    }

    fn typechecked_effect_op_call_binding(
        &self,
        span: Span,
    ) -> Option<crate::ast::EffectOpCallBinding> {
        self.file.typechecked_effect_op_call_binding(span)
    }

    fn typechecked_top_level_fun_value_ref(&mut self, span: Span) -> Option<(String, Vec<TypeId>)> {
        let typecheck_types = self.typecheck_types?;
        let fun_ref = self.file.top_level_fun_value_ref(span)?;
        let type_args = fun_ref
            .type_args
            .iter()
            .copied()
            .map(|ty| self.types.re_intern_from(typecheck_types, ty))
            .collect();
        Some((fun_ref.fqn, type_args))
    }

    fn typechecked_ctor_call_binding(&self, span: Span) -> Option<ast::CtorCallBinding> {
        self.file.typechecked_ctor_call_binding(span)
    }

    fn struct_instance_from_type_id(&self, ty: TypeId) -> Option<(String, Vec<TypeId>)> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(ty).clone() else {
            return None;
        };
        if !matches!(
            self.type_kinds.get(&nominal.fqn),
            Some(ast::TypeKind::Struct)
        ) {
            return None;
        }
        Some((nominal.fqn, nominal.args))
    }

    fn with_bound_struct_default_context<T>(
        &mut self,
        decl_source: &'a crate::source::SourceFile,
        decl_file: &'a ast::File,
        type_params: &[String],
        concrete_args: &[TypeId],
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.with_foreign_ast_context(decl_source, decl_file, |this| {
            this.push_type_param_bindings(
                type_params
                    .iter()
                    .cloned()
                    .zip(concrete_args.iter().copied()),
            );
            let result = f(this);
            this.pop_type_params();
            result
        })
    }

    fn struct_default_param_expected_expr(
        &mut self,
        decl_source: &'a crate::source::SourceFile,
        decl_file: &'a ast::File,
        type_params: &[String],
        concrete_args: &[TypeId],
        param: &DefaultArgParamInfo,
    ) -> ExpectedExpr {
        self.with_bound_struct_default_context(
            decl_source,
            decl_file,
            type_params,
            concrete_args,
            |this| {
                let value_ty = param
                    .ty_ref
                    .as_ref()
                    .map(|ty| this.lower_type_ref(ty))
                    .unwrap_or(this.builtins.any);
                ExpectedExpr {
                    value_ty: Some(value_ty),
                    array_lit_target: param
                        .ty_ref
                        .as_ref()
                        .and_then(|ty| this.array_lit_target_from_type_ref(ty)),
                    array_lit_ty: Some(value_ty),
                    struct_lit_ty: Some(value_ty),
                }
            },
        )
    }

    fn struct_default_param_local_id(
        &mut self,
        decl_source: &'a crate::source::SourceFile,
        decl_file: &'a ast::File,
        decl_span: Span,
    ) -> crate::hir::SymbolId {
        self.with_foreign_ast_context(decl_source, decl_file, |this| {
            this.intern_local_symbol(decl_span, false)
        })
    }

    /// 无完整 typecheck 的 lowering/IR 测试入口仍可能需要识别简单 nominal ctor call。
    ///
    /// 说明：
    /// - 优先使用 typecheck side table；这里只作为 resolver 级 fallback；
    /// - 仅依据 resolver 的 ctor 候选集合与调用形状恢复“唯一可判定”的目标；
    /// - 若存在重载歧义、vararg/spread、或需要更深类型信息才能决定的情况，则返回 `None`，
    ///   让无 typecheck 路径保持保守失败，而不是猜错目标 ctor。
    fn resolver_fallback_ctor_call_binding(
        &self,
        callee: &ast::ValueIdent,
        args: &[ast::Expr],
    ) -> Option<ast::CtorCallBinding> {
        let call = callee.call.as_ref()?;
        let mut ctor_types: Vec<String> = call
            .candidates
            .iter()
            .filter_map(|candidate| match candidate {
                ast::CallCandidate::Constructor { ty_fqn } => Some(ty_fqn.clone()),
                ast::CallCandidate::Fun { .. } => None,
            })
            .collect();
        ctor_types.sort();
        ctor_types.dedup();

        if ctor_types.len() != 1 {
            return None;
        }
        let owner_fqn = ctor_types.pop()?;

        let visible_ctors: Vec<&ConstructorOverload> = self
            .index
            .constructors
            .get(&owner_fqn)
            .into_iter()
            .flatten()
            .filter(|ctor| self.resolver_ctor_visible(ctor))
            .collect();

        if visible_ctors.is_empty() {
            return if args.is_empty() {
                Some(ast::CtorCallBinding {
                    owner_fqn,
                    ctor_span: None,
                    arg_mapping: Vec::new(),
                })
            } else {
                None
            };
        }

        let mut matched: Vec<(Option<Span>, Vec<Option<usize>>)> = visible_ctors
            .iter()
            .filter_map(|ctor| {
                self.resolver_fallback_ctor_arg_mapping(&ctor.params, args)
                    .map(|mapping| (Some(ctor.span), mapping))
            })
            .collect();

        if matched.len() != 1 {
            return None;
        }
        let (ctor_span, arg_mapping) = matched.pop()?;
        Some(ast::CtorCallBinding {
            owner_fqn,
            ctor_span,
            arg_mapping,
        })
    }

    fn try_lower_struct_ctor_call_expr(
        &mut self,
        pkg_prefix: &str,
        call_span: Span,
        callee: &ast::Expr,
        args: &[ast::Expr],
        typechecked_call_ty: Option<TypeId>,
    ) -> Option<(ExprKind, TypeId)> {
        let ast::ExprKind::Ident(id) = &callee.kind else {
            return None;
        };
        let binding = self
            .typechecked_ctor_call_binding(call_span)
            .or_else(|| self.resolver_fallback_ctor_call_binding(id, args))?;
        if !matches!(
            self.type_kinds.get(&binding.owner_fqn),
            Some(ast::TypeKind::Struct)
        ) {
            return None;
        }

        let result_ty = typechecked_call_ty
            .unwrap_or_else(|| self.intern_nominal(binding.owner_fqn.clone(), Vec::new(), None));
        let (struct_fqn, concrete_args) = self
            .struct_instance_from_type_id(result_ty)
            .unwrap_or_else(|| (binding.owner_fqn.clone(), Vec::new()));

        let ctor = self
            .index
            .constructors
            .get(&binding.owner_fqn)?
            .iter()
            .find(|ctor| binding.ctor_span == Some(ctor.span))?;
        if binding.arg_mapping.len() != ctor.params.len() {
            return None;
        }

        let needs_defaults = binding.arg_mapping.iter().any(|slot| slot.is_none());
        if !needs_defaults {
            let mut fields = Vec::with_capacity(ctor.params.len());
            for (param_idx, param) in ctor.params.iter().enumerate() {
                let arg_idx = binding.arg_mapping.get(param_idx).copied().flatten()?;
                let arg = args.get(arg_idx)?;
                let value_expr = match &arg.kind {
                    ast::ExprKind::NamedArg { value, .. } => value.as_ref(),
                    _ => arg,
                };
                fields.push(StructLitField {
                    span: value_expr.span,
                    name: param.name.clone(),
                    name_span: call_span,
                    colon_span: call_span,
                    value: self.lower_expr(pkg_prefix, value_expr),
                });
            }
            return Some((
                ExprKind::StructLit {
                    ty: result_ty,
                    fields,
                },
                result_ty,
            ));
        }

        let info = self.default_arg_structs.get(&struct_fqn).cloned()?;
        if binding.arg_mapping.len() != info.params.len() {
            return None;
        }
        let (decl_source, decl_file) = self.decl_ast_context(&info.decl_file)?;
        let decl_pkg_prefix = package_prefix(decl_source, decl_file.package.as_ref());

        let expecteds: Vec<ExpectedExpr> = info
            .params
            .iter()
            .map(|param| {
                self.struct_default_param_expected_expr(
                    decl_source,
                    decl_file,
                    &info.type_params,
                    &concrete_args,
                    param,
                )
            })
            .collect();
        let param_ids: Vec<crate::hir::SymbolId> = info
            .params
            .iter()
            .map(|param| {
                self.struct_default_param_local_id(decl_source, decl_file, param.decl_span)
            })
            .collect();

        let mut arg_to_param: Vec<Option<usize>> = vec![None; args.len()];
        for (param_idx, arg_idx) in binding.arg_mapping.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                continue;
            };
            let slot = arg_to_param.get_mut(arg_idx)?;
            if slot.is_some() {
                return None;
            }
            *slot = Some(param_idx);
        }
        if arg_to_param.iter().any(|slot| slot.is_none()) {
            return None;
        }

        let mut stmts: Vec<Stmt> = Vec::with_capacity(info.params.len() + 1);
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param.get(arg_idx).copied().flatten()?;
            let param = info.params.get(param_idx)?;
            let expected = *expecteds.get(param_idx)?;
            let arg_value = match &arg.kind {
                ast::ExprKind::NamedArg { value, .. } => value.as_ref(),
                _ => arg,
            };
            let init = self.lower_expr_with_expected(pkg_prefix, arg_value, expected);
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(ValDecl {
                    span: call_span,
                    id: Some(*param_ids.get(param_idx)?),
                    name: Some(param.name.clone()),
                    mutable: false,
                    ty: expected.value_ty.unwrap_or(self.builtins.any),
                    init: Some(init),
                }),
            });
        }

        for (param_idx, param) in info.params.iter().enumerate() {
            if binding
                .arg_mapping
                .get(param_idx)
                .copied()
                .flatten()
                .is_some()
            {
                continue;
            }
            let default_value = param.default_value.as_ref()?;
            let expected = *expecteds.get(param_idx)?;
            let init = self.with_bound_struct_default_context(
                decl_source,
                decl_file,
                &info.type_params,
                &concrete_args,
                |this| this.lower_expr_with_expected(&decl_pkg_prefix, default_value, expected),
            );
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(ValDecl {
                    span: call_span,
                    id: Some(*param_ids.get(param_idx)?),
                    name: Some(param.name.clone()),
                    mutable: false,
                    ty: expected.value_ty.unwrap_or(self.builtins.any),
                    init: Some(init),
                }),
            });
        }

        let mut fields = Vec::with_capacity(info.params.len());
        for (param_idx, param) in info.params.iter().enumerate() {
            let expected = *expecteds.get(param_idx)?;
            fields.push(StructLitField {
                span: call_span,
                name: param.name.clone(),
                name_span: call_span,
                colon_span: call_span,
                value: Expr {
                    span: param.decl_span,
                    ty: expected.value_ty.unwrap_or(self.builtins.any),
                    kind: ExprKind::VarRef(ValueRef::Local {
                        id: *param_ids.get(param_idx)?,
                        name: param.name.clone(),
                        decl_span: param.decl_span,
                    }),
                },
            });
        }
        let struct_expr = Expr {
            span: call_span,
            ty: result_ty,
            kind: ExprKind::StructLit {
                ty: result_ty,
                fields,
            },
        };
        stmts.push(Stmt {
            span: call_span,
            ty: result_ty,
            kind: StmtKind::Expr(struct_expr),
        });

        Some((
            ExprKind::Block(Block {
                span: call_span,
                ty: result_ty,
                stmts,
            }),
            result_ty,
        ))
    }

    fn resolver_ctor_visible(&self, ctor: &ConstructorOverload) -> bool {
        match ctor.visibility {
            Visibility::Public => true,
            Visibility::Internal => ctor.decl_cone == self.index.cone_of_source(self.source),
            Visibility::Private => ctor.decl_file == self.source.path(),
        }
    }

    fn resolver_fallback_ctor_arg_mapping(
        &self,
        params: &[ParamSig],
        args: &[ast::Expr],
    ) -> Option<Vec<Option<usize>>> {
        if params.iter().any(|param| param.is_vararg) {
            return None;
        }

        let mut seen_named = false;
        let mut positional_count = 0usize;
        for arg in args {
            match &arg.kind {
                ast::ExprKind::NamedArg { .. } => {
                    seen_named = true;
                }
                ast::ExprKind::SpreadArg { .. } => {
                    return None;
                }
                _ => {
                    if seen_named {
                        return None;
                    }
                    positional_count = positional_count.saturating_add(1);
                }
            }
        }

        if positional_count > params.len() {
            return None;
        }

        let mut param_to_arg: Vec<Option<usize>> = vec![None; params.len()];
        for arg_idx in 0..positional_count {
            *param_to_arg.get_mut(arg_idx)? = Some(arg_idx);
        }

        for (arg_idx, arg) in args.iter().enumerate().skip(positional_count) {
            let ast::ExprKind::NamedArg { name, .. } = &arg.kind else {
                return None;
            };
            let name_text = name.text(self.source);
            let slot_idx = params.iter().position(|param| param.name == name_text)?;
            let slot = param_to_arg.get_mut(slot_idx)?;
            if slot.is_some() {
                return None;
            }
            *slot = Some(arg_idx);
        }

        for (idx, param) in params.iter().enumerate() {
            if param_to_arg.get(idx)?.is_some() {
                continue;
            }
            if !param.has_default {
                return None;
            }
        }

        Some(param_to_arg)
    }

    fn synthetic_top_level_fun_value_param_span(&self, base_span: Span, ordinal: usize) -> Span {
        let offset = base_span.end.saturating_add(ordinal).saturating_add(1);
        Span::new(offset, offset)
    }

    fn mangled_top_level_fun_value_fqn(&self, fqn: &str, type_args: &[TypeId]) -> String {
        if type_args.is_empty()
            || type_args
                .iter()
                .any(|ty| matches!(self.types.kind(*ty), TypeKind::Param(_)))
        {
            return fqn.to_string();
        }

        let args = type_args
            .iter()
            .map(|ty| self.types.display(*ty).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("{fqn}::<{args}>")
    }

    fn try_lower_top_level_fun_value_expr(&mut self, span: Span) -> Option<(ExprKind, TypeId)> {
        let (base_fqn, type_args) = self.typechecked_top_level_fun_value_ref(span)?;
        let fun_ty_id = self.typechecked_expr_ty(span)?;
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(fun_ty_id).clone()
        else {
            return None;
        };

        let mut params: Vec<Param> =
            Vec::with_capacity(fun_ty.params.len() + usize::from(fun_ty.receiver.is_some()));
        let mut call_args: Vec<CallArg> =
            Vec::with_capacity(fun_ty.params.len() + usize::from(fun_ty.receiver.is_some()));
        let mut ordinal = 0usize;

        if let Some(receiver_ty) = fun_ty.receiver {
            let decl_span = self.synthetic_top_level_fun_value_param_span(span, ordinal);
            let id = self.intern_local_symbol(decl_span, false);
            let name = "receiver".to_string();
            params.push(Param {
                span: decl_span,
                id,
                name: name.clone(),
                ty: receiver_ty,
            });
            call_args.push(CallArg::Positional(Expr {
                span: decl_span,
                ty: receiver_ty,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id,
                    name,
                    decl_span,
                }),
            }));
            ordinal += 1;
        }

        for (idx, param_ty) in fun_ty.params.iter().copied().enumerate() {
            let decl_span = self.synthetic_top_level_fun_value_param_span(span, ordinal);
            let id = self.intern_local_symbol(decl_span, false);
            let name = format!("a{idx}");
            params.push(Param {
                span: decl_span,
                id,
                name: name.clone(),
                ty: param_ty,
            });
            call_args.push(CallArg::Positional(Expr {
                span: decl_span,
                ty: param_ty,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id,
                    name,
                    decl_span,
                }),
            }));
            ordinal += 1;
        }

        let callee_fqn = self.mangled_top_level_fun_value_fqn(&base_fqn, &type_args);
        let callee = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(callee_fqn.clone()),
                fqn: callee_fqn,
            }),
        };
        let body = Expr {
            span,
            ty: fun_ty.return_ty,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args: call_args,
            },
        };

        Some((
            ExprKind::Closure(ClosureExpr {
                span,
                id: self.alloc_closure_id(),
                at_safe_span: None,
                captures: Vec::new(),
                params,
                body: Box::new(body),
            }),
            fun_ty_id,
        ))
    }

    fn array_lit_target_from_type_id(&self, ty: TypeId) -> Option<ArrayLitTarget> {
        let TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nominal)) = self.types.kind(ty) else {
            return None;
        };
        match nominal.fqn.as_str() {
            "scoop.core.Array"
            | "scoop.core.List"
            | "scoop.collections.Set"
            | "scoop.collections.MapView" => Some(ArrayLitTarget::Array),
            "scoop.core.MutableArray"
            | "scoop.core.MutableList"
            | "scoop.collections.MutableSet"
            | "scoop.collections.MutableMap" => Some(ArrayLitTarget::MutableArray),
            _ => None,
        }
    }

    fn array_lit_element_ty_from_type_id(&mut self, ty: TypeId) -> Option<TypeId> {
        let TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nominal)) = self.types.kind(ty) else {
            return None;
        };
        self.array_lit_target_from_type_id(ty)?;
        nominal
            .args
            .first()
            .copied()
            .map(|arg| self.canonicalize_builtin_scalar_alias_ty(arg))
    }

    fn array_lit_lowering_hint(
        &mut self,
        span: Span,
        expected: ExpectedExpr,
    ) -> Option<(ArrayLitTarget, TypeId, Option<TypeId>)> {
        let raw_result_ty = self
            .typechecked_expr_ty(span)
            .or(expected.array_lit_ty)
            .or(expected.struct_lit_ty)?;
        let result_ty = self.canonicalize_array_like_type_args(raw_result_ty);
        let target = self
            .array_lit_target_from_type_id(result_ty)
            .or(expected.array_lit_target)?;
        let element_ty = self.array_lit_element_ty_from_type_id(result_ty);
        Some((target, result_ty, element_ty))
    }

    fn canonicalize_array_like_type_args(&mut self, ty: TypeId) -> TypeId {
        let TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nominal)) = self.types.kind(ty).clone()
        else {
            return ty;
        };
        if self.array_lit_target_from_type_id(ty).is_none() || nominal.args.len() != 1 {
            return ty;
        }

        let canonical_arg = self.canonicalize_builtin_scalar_alias_ty(nominal.args[0]);
        if canonical_arg == nominal.args[0] {
            return ty;
        }

        self.intern_nominal(nominal.fqn, vec![canonical_arg], nominal.eff)
    }

    fn canonicalize_builtin_scalar_alias_ty(&mut self, ty: TypeId) -> TypeId {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(ty).clone() else {
            return ty;
        };
        if !nominal.args.is_empty() {
            return ty;
        }

        match nominal.fqn.as_str() {
            "scoop.core.Bool" => self.builtins.bool_,
            "scoop.core.Char" => self.builtins.char_,
            "scoop.core.Float64" => self.builtins.float64,
            "scoop.core.Float32" => self.builtins.float32,
            "scoop.core.Int" => self.builtins.int,
            "scoop.core.UInt" => self.builtins.uint,
            fqn => {
                if let Some(bits) = fqn
                    .strip_prefix("scoop.core.Int")
                    .and_then(|s| s.parse::<u16>().ok())
                {
                    return self.types.ty_int_n(bits);
                }
                if let Some(bits) = fqn
                    .strip_prefix("scoop.core.UInt")
                    .and_then(|s| s.parse::<u16>().ok())
                {
                    return self.types.ty_uint_n(bits);
                }
                ty
            }
        }
    }

    fn infer_array_lit_ty_from_lowered_elements(&mut self, elements: &[Expr]) -> Option<TypeId> {
        let first_ty = elements.first()?.ty;
        if first_ty == self.builtins.any {
            return None;
        }
        if elements
            .iter()
            .skip(1)
            .any(|element| element.ty == self.builtins.any || element.ty != first_ty)
        {
            return None;
        }
        Some(self.intern_nominal("scoop.core.Array".to_string(), vec![first_ty], None))
    }

    /// 根据 FQN 获取函数签名（用于从函数参数类型向下传播 expected-type hint）。
    fn fun_sig_by_fqn(&self, fqn: &str) -> Option<crate::resolve::FunSig> {
        let syms = self.index.by_fqn.get(fqn)?;
        let overload = syms.fun.first()?;
        Some(overload.sig.clone())
    }

    /// 尝试从 callee 表达式中提取“顶层函数 FQN”（用于向实参传播期望类型）。
    fn callee_top_level_fqn<'b>(&self, callee: &'b ast::Expr) -> Option<&'b str> {
        // `callee<T>()`：在“调用 callee”位置仍把 `TypeApply` 视为透明包装；
        // 若其处于普通值表达式位置，则会提前经由 top-level function value side table 合成为 closure。
        let callee = match &callee.kind {
            ast::ExprKind::TypeApply { callee, .. } => callee.as_ref(),
            _ => callee,
        };
        let ast::ExprKind::Ident(id) = &callee.kind else {
            return None;
        };
        let ast::ResolvedValueRef::TopLevel { fqn } = id.resolved.as_ref()? else {
            return None;
        };
        Some(fqn.as_str())
    }

    /// 为一次函数调用的某个实参计算 expected-type hint（目前仅用于数组字面量）。
    fn expected_expr_for_fun_call_arg(
        &mut self,
        sig: Option<&crate::resolve::FunSig>,
        arg: &ast::Expr,
        positional_index: usize,
    ) -> ExpectedExpr {
        // expected-type hint 目前只用于数组字面量 `[...]` 的 lowering（Array vs MutableArray）。
        //
        // 注意：`FunSig` 的参数 `TypeRef` 可能来自**其它源文件**（sysroot/stdlib/多文件编译单元），
        // 其 span 无法用当前文件的 `SourceFile` 回切；因此我们必须避免在“非数组字面量实参”
        // 的场景下无谓地解析参数类型，防止跨文件 span 误用导致 panic。
        let arg_is_array_lit = match &arg.kind {
            ast::ExprKind::ArrayLit { .. } => true,
            ast::ExprKind::NamedArg { value, .. } => {
                matches!(value.kind, ast::ExprKind::ArrayLit { .. })
            }
            _ => false,
        };
        if !arg_is_array_lit {
            return ExpectedExpr {
                value_ty: None,
                array_lit_target: None,
                array_lit_ty: None,
                struct_lit_ty: None,
            };
        }

        let param_ty = match (sig, &arg.kind) {
            (Some(sig), ast::ExprKind::NamedArg { name, .. }) => {
                let name = name.text(self.source);
                sig.params
                    .iter()
                    .find(|p| p.name == name)
                    .and_then(|p| p.ty.as_ref())
            }
            (Some(sig), _) => sig.params.get(positional_index).and_then(|p| p.ty.as_ref()),
            _ => None,
        };
        let array_lit_target = param_ty.and_then(|ty| self.array_lit_target_from_type_ref(ty));
        let array_lit_ty = param_ty.and_then(|ty| self.local_type_ref_ty(ty));

        ExpectedExpr {
            value_ty: None,
            array_lit_target,
            array_lit_ty,
            struct_lit_ty: None,
        }
    }

    /// 将 `[...]` 降到统一的 builder/intrinsics 调用形态（TODO T1317c）。
    ///
    /// 形态（概念上）：
    /// ```text
    /// [e0, e1, e2]
    /// =>
    /// {
    ///   val __array_builder = __scoop_array_builder_new()
    ///   __scoop_array_builder_push(__array_builder, e0)
    ///   __scoop_array_builder_push(__array_builder, e1)
    ///   __scoop_array_builder_push(__array_builder, e2)
    ///   __scoop_array_builder_build_array(__array_builder) // or build_mutable_array
    /// }
    /// ```
    fn lower_array_lit_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        elements: &[ast::Expr],
        target: ArrayLitTarget,
        result_ty: TypeId,
        element_expected_ty: Option<TypeId>,
    ) -> (ExprKind, TypeId) {
        let lowered_elements: Vec<Expr> = elements
            .iter()
            .enumerate()
            .map(|(index, element)| {
                let expected = element_expected_ty
                    .map(|ty| ExpectedExpr {
                        value_ty: Some(ty),
                        array_lit_target: self.array_lit_target_from_type_id(ty),
                        array_lit_ty: Some(ty),
                        struct_lit_ty: Some(ty),
                    })
                    .unwrap_or_default();
                let lowered = self.lower_expr_with_expected(pkg_prefix, element, expected);
                match element_expected_ty {
                    Some(expected_ty)
                        if Self::array_lit_element_needs_expected_binding(element) =>
                    {
                        self.wrap_array_lit_element_with_expected_binding(
                            element.span,
                            index,
                            expected_ty,
                            lowered,
                        )
                    }
                    _ => lowered,
                }
            })
            .collect();
        self.build_array_lit_expr(span, lowered_elements, target, result_ty)
    }

    fn array_lit_element_needs_expected_binding(element: &ast::Expr) -> bool {
        matches!(
            element.kind,
            ast::ExprKind::If { .. }
                | ast::ExprKind::When { .. }
                | ast::ExprKind::Block(_)
                | ast::ExprKind::DoBlock { .. }
                | ast::ExprKind::UnsafeBlock { .. }
                | ast::ExprKind::SafeBlock { .. }
                | ast::ExprKind::Handle { .. }
        )
    }

    fn wrap_array_lit_element_with_expected_binding(
        &mut self,
        span: Span,
        index: usize,
        expected_ty: TypeId,
        init: Expr,
    ) -> Expr {
        let decl_span = Span::new(span.start, span.start);
        let temp_id = self.intern_local_symbol(decl_span, false);
        let temp_name = format!("__array_elem_{index}");

        let val_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Val(ValDecl {
                span,
                id: Some(temp_id),
                name: Some(temp_name.clone()),
                mutable: false,
                ty: expected_ty,
                init: Some(init),
            }),
        };

        let temp_ref = Expr {
            span,
            ty: expected_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: temp_id,
                name: temp_name,
                decl_span,
            }),
        };
        let temp_expr_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(temp_ref),
        };

        Expr {
            span,
            ty: expected_ty,
            kind: ExprKind::Block(Block {
                span,
                ty: expected_ty,
                stmts: vec![val_stmt, temp_expr_stmt],
            }),
        }
    }

    fn build_array_lit_expr(
        &mut self,
        span: Span,
        elements: Vec<Expr>,
        target: ArrayLitTarget,
        result_ty: TypeId,
    ) -> (ExprKind, TypeId) {
        // 说明：使用 push-based builder 语义承载元素顺序。
        let builder_decl_span = Span::new(span.start, span.start);
        let builder_id = self.intern_local_symbol(builder_decl_span, false);
        let builder_name = "__array_builder".to_string();

        let new_fqn = Self::ARRAY_BUILDER_NEW_FQN.to_string();
        let new_callee = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(new_fqn.clone()),
                fqn: new_fqn,
            }),
        };
        let new_call = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::Call {
                callee: Box::new(new_callee),
                args: Vec::new(),
            },
        };

        let builder_decl = ValDecl {
            span,
            id: Some(builder_id),
            name: Some(builder_name.clone()),
            mutable: false,
            ty: self.builtins.any,
            init: Some(new_call),
        };

        let mut stmts: Vec<Stmt> = Vec::with_capacity(elements.len() + 2);
        stmts.push(Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Val(builder_decl),
        });

        for element_expr in elements {
            let builder_ref = Expr {
                span: builder_decl_span,
                ty: self.builtins.any,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id: builder_id,
                    name: builder_name.clone(),
                    decl_span: builder_decl_span,
                }),
            };

            let push_fqn = Self::ARRAY_BUILDER_PUSH_FQN.to_string();
            let push_callee = Expr {
                span,
                ty: self.builtins.any,
                kind: ExprKind::VarRef(ValueRef::TopLevel {
                    id: self.symbols.intern_top_level(push_fqn.clone()),
                    fqn: push_fqn,
                }),
            };
            let push_call = Expr {
                span,
                ty: self.builtins.unit,
                kind: ExprKind::Call {
                    callee: Box::new(push_callee),
                    args: vec![
                        CallArg::Positional(builder_ref),
                        CallArg::Positional(element_expr),
                    ],
                },
            };
            stmts.push(Stmt {
                span,
                ty: self.builtins.unit,
                kind: StmtKind::Expr(push_call),
            });
        }

        let build_fqn = match target {
            ArrayLitTarget::Array => Self::ARRAY_BUILDER_BUILD_ARRAY_FQN,
            ArrayLitTarget::MutableArray => Self::ARRAY_BUILDER_BUILD_MUTABLE_ARRAY_FQN,
        }
        .to_string();
        let build_callee = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(build_fqn.clone()),
                fqn: build_fqn,
            }),
        };
        let builder_ref = Expr {
            span: builder_decl_span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: builder_id,
                name: builder_name,
                decl_span: builder_decl_span,
            }),
        };
        let build_call = Expr {
            span,
            ty: result_ty,
            kind: ExprKind::Call {
                callee: Box::new(build_callee),
                args: vec![CallArg::Positional(builder_ref)],
            },
        };
        stmts.push(Stmt {
            span,
            ty: build_call.ty,
            kind: StmtKind::Expr(build_call),
        });

        (
            ExprKind::Block(Block {
                span,
                ty: result_ty,
                stmts,
            }),
            result_ty,
        )
    }

    fn lower_call_arg_with_expected(
        &mut self,
        pkg_prefix: &str,
        arg: &ast::Expr,
        expected: ExpectedExpr,
    ) -> CallArg {
        match &arg.kind {
            ast::ExprKind::NamedArg { name, value, .. } => CallArg::Named {
                name: name.text(self.source).to_string(),
                name_span: name.span,
                value: self.lower_expr_with_expected(pkg_prefix, value, expected),
            },
            _ => CallArg::Positional(self.lower_expr_with_expected(pkg_prefix, arg, expected)),
        }
    }

    /// T0113: Lower call arguments when the callee has a vararg parameter.
    ///
    /// Strategy:
    /// - Args before the vararg index are lowered as normal positional args.
    /// - Args at and after the vararg index (up to the end) are collected:
    ///   - If a single spread arg `*arr`: pass the inner expression directly as the array.
    ///   - Otherwise: wrap individual args into an array literal using the builder pattern.
    /// - The vararg slot becomes a single `CallArg::Positional(Array<T>)` expression.
    fn lower_call_args_with_vararg(
        &mut self,
        pkg_prefix: &str,
        call_span: Span,
        args: &[ast::Expr],
        sig: Option<&crate::resolve::FunSig>,
        vararg_idx: usize,
    ) -> Vec<CallArg> {
        let mut out: Vec<CallArg> = Vec::with_capacity(args.len());
        let mut positional_index = 0usize;
        let mut vararg_args: Vec<&ast::Expr> = Vec::new();
        let mut has_spread = false;

        for arg in args {
            // Named args are passed through without affecting positional index.
            if let ast::ExprKind::NamedArg { .. } = &arg.kind {
                let expected = self.expected_expr_for_fun_call_arg(sig, arg, positional_index);
                out.push(self.lower_call_arg_with_expected(pkg_prefix, arg, expected));
                continue;
            }

            if positional_index < vararg_idx {
                // Pre-vararg: normal positional arg.
                let expected = self.expected_expr_for_fun_call_arg(sig, arg, positional_index);
                out.push(self.lower_call_arg_with_expected(pkg_prefix, arg, expected));
            } else {
                // Vararg slot: collect for later wrapping.
                if matches!(&arg.kind, ast::ExprKind::SpreadArg { .. }) {
                    has_spread = true;
                }
                vararg_args.push(arg);
            }
            positional_index = positional_index.saturating_add(1);
        }

        // Build the vararg array arg.
        let vararg_expr = if vararg_args.is_empty() {
            // No args for vararg slot: pass an empty array.
            self.synth_empty_array_lit(call_span)
        } else if vararg_args.len() == 1 && has_spread {
            // Single spread arg: unwrap and pass the inner expression directly.
            let arg = vararg_args[0];
            match &arg.kind {
                ast::ExprKind::SpreadArg { expr: inner, .. } => self.lower_expr(pkg_prefix, inner),
                _ => unreachable!("has_spread is true but arg is not SpreadArg"),
            }
        } else {
            // Individual args: wrap in an array literal using the builder pattern.
            let elements: Vec<&ast::Expr> = vararg_args
                .into_iter()
                .map(|arg| match &arg.kind {
                    // Unwrap spread args — this is a mixed case, currently unsupported;
                    // fall back to passing the inner expr as an element.
                    ast::ExprKind::SpreadArg { expr: inner, .. } => inner.as_ref(),
                    _ => arg,
                })
                .collect();
            self.synth_array_lit_from_exprs(pkg_prefix, call_span, &elements)
        };

        out.push(CallArg::Positional(vararg_expr));
        out
    }

    /// Synthesize an empty array literal expression.
    fn synth_empty_array_lit(&mut self, span: Span) -> Expr {
        self.synth_array_lit_from_exprs("", span, &[])
    }

    /// Synthesize an array literal from a list of AST expressions.
    ///
    /// Uses the same builder pattern as `lower_array_lit_expr`:
    /// `__scoop_array_builder_new()` → push elements → `__scoop_array_builder_build_array()`.
    fn synth_array_lit_from_exprs(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        elements: &[&ast::Expr],
    ) -> Expr {
        let builder_decl_span = Span::new(span.start, span.start);
        let builder_id = self.intern_local_symbol(builder_decl_span, false);
        let builder_name = "__vararg_builder".to_string();

        // val __vararg_builder = __scoop_array_builder_new()
        let new_fqn = Self::ARRAY_BUILDER_NEW_FQN.to_string();
        let new_callee = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(new_fqn.clone()),
                fqn: new_fqn,
            }),
        };
        let new_call = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::Call {
                callee: Box::new(new_callee),
                args: Vec::new(),
            },
        };

        let builder_decl = ValDecl {
            span,
            id: Some(builder_id),
            name: Some(builder_name.clone()),
            mutable: false,
            ty: self.builtins.any,
            init: Some(new_call),
        };

        let mut stmts: Vec<Stmt> = Vec::with_capacity(elements.len() + 2);
        stmts.push(Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Val(builder_decl),
        });

        // __scoop_array_builder_push(builder, element) for each element
        for element in elements {
            let element_expr = self.lower_expr(pkg_prefix, element);
            let builder_ref = Expr {
                span: builder_decl_span,
                ty: self.builtins.any,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id: builder_id,
                    name: builder_name.clone(),
                    decl_span: builder_decl_span,
                }),
            };

            let push_fqn = Self::ARRAY_BUILDER_PUSH_FQN.to_string();
            let push_callee = Expr {
                span,
                ty: self.builtins.any,
                kind: ExprKind::VarRef(ValueRef::TopLevel {
                    id: self.symbols.intern_top_level(push_fqn.clone()),
                    fqn: push_fqn,
                }),
            };
            let push_call = Expr {
                span,
                ty: self.builtins.unit,
                kind: ExprKind::Call {
                    callee: Box::new(push_callee),
                    args: vec![
                        CallArg::Positional(builder_ref),
                        CallArg::Positional(element_expr),
                    ],
                },
            };
            stmts.push(Stmt {
                span,
                ty: self.builtins.unit,
                kind: StmtKind::Expr(push_call),
            });
        }

        // __scoop_array_builder_build_array(builder)
        let builder_ref_final = Expr {
            span: builder_decl_span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: builder_id,
                name: builder_name.clone(),
                decl_span: builder_decl_span,
            }),
        };
        let build_fqn = Self::ARRAY_BUILDER_BUILD_ARRAY_FQN.to_string();
        let build_callee = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(build_fqn.clone()),
                fqn: build_fqn,
            }),
        };
        let build_call = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::Call {
                callee: Box::new(build_callee),
                args: vec![CallArg::Positional(builder_ref_final)],
            },
        };
        stmts.push(Stmt {
            span,
            ty: self.builtins.any,
            kind: StmtKind::Expr(build_call),
        });

        Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::Block(Block {
                span,
                ty: self.builtins.any,
                stmts,
            }),
        }
    }

    pub(super) fn alloc_closure_id(&mut self) -> ClosureId {
        let id = ClosureId(self.next_closure);
        self.next_closure = self.next_closure.saturating_add(1);
        id
    }

    fn lower_lambda_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        lam: &ast::LambdaExpr,
    ) -> (ExprKind, TypeId) {
        let id = self.alloc_closure_id();
        let typechecked_fun_ty = self.typechecked_expr_ty(span).and_then(|ty| {
            let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(ty) else {
                return None;
            };
            Some((ty, fun_ty.clone()))
        });

        let params: Vec<Param> = lam
            .params
            .iter()
            .enumerate()
            .map(|(idx, p)| {
                let name = p.name.text(self.source).to_string();
                let ty =
                    p.ty.as_ref()
                        .map(|t| self.lower_type_ref(t))
                        .or_else(|| {
                            typechecked_fun_ty
                                .as_ref()
                                .and_then(|(_, fun_ty)| fun_ty.params.get(idx).copied())
                        })
                        .unwrap_or(self.builtins.any);
                Param {
                    span: p.name.span,
                    id: self.intern_local_symbol(p.name.span, false),
                    name,
                    ty,
                }
            })
            .collect();

        let receiver_this_decl_span = typechecked_fun_ty.as_ref().and_then(|(_, fun_ty)| {
            fun_ty
                .receiver
                .map(|_| ast::synthetic_lambda_receiver_this_decl_span(span))
        });
        let body = Box::new(match receiver_this_decl_span {
            Some(receiver_this_decl_span) => self
                .with_lambda_this_decl_span(Some(receiver_this_decl_span), |this| {
                    this.lower_expr(pkg_prefix, lam.body.as_ref())
                }),
            None => self.lower_expr(pkg_prefix, lam.body.as_ref()),
        });
        let captures = compute_closure_captures(&params, body.as_ref(), &self.local_mutability);
        (
            ExprKind::Closure(ClosureExpr {
                span,
                id,
                at_safe_span: lam.at_safe_span,
                captures,
                params,
                body,
            }),
            typechecked_fun_ty
                .map(|(ty, _)| ty)
                .unwrap_or(self.builtins.any),
        )
    }

    /// 把 AST 的 struct literal（`Type { field: expr, ... }`）降低为 HIR 表示。
    ///
    /// 说明：
    /// - 当前 lowering 不做字段存在性/类型匹配检查（这些属于 typecheck，参见 TODO T0423）；
    /// - 这里只保留“目标类型 + 字段初始化表达式列表”，供早期 LLVM codegen（T0811）构造值。
    fn lower_struct_lit_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        ty: &ast::TypePath,
        fields: &[ast::StructLitField],
        expected_ty: Option<TypeId>,
    ) -> (ExprKind, TypeId) {
        // T0124: For generic structs, use the expected type (from val declaration) when the
        // struct literal's type path has no type args but the expected type is a concrete
        // instantiation of the same struct.
        let ty_id = if ty.args.is_empty() {
            if let Some(expected) = expected_ty {
                if let crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Nominal(nominal)) =
                    self.types.kind(expected)
                {
                    if !nominal.args.is_empty() {
                        expected
                    } else {
                        self.lower_type_path(ty)
                    }
                } else {
                    self.lower_type_path(ty)
                }
            } else {
                self.lower_type_path(ty)
            }
        } else {
            self.lower_type_path(ty)
        };

        let Some((struct_fqn, concrete_args)) = self.struct_instance_from_type_id(ty_id) else {
            let lowered_fields = fields
                .iter()
                .map(|f| StructLitField {
                    span: f.span,
                    name: f.name.text(self.source).to_string(),
                    name_span: f.name.span,
                    colon_span: f.colon_span,
                    value: self.lower_expr(pkg_prefix, &f.value),
                })
                .collect::<Vec<_>>();

            return (
                ExprKind::StructLit {
                    ty: ty_id,
                    fields: lowered_fields,
                },
                ty_id,
            );
        };

        let Some(info) = self.default_arg_structs.get(&struct_fqn).cloned() else {
            let lowered_fields = fields
                .iter()
                .map(|f| StructLitField {
                    span: f.span,
                    name: f.name.text(self.source).to_string(),
                    name_span: f.name.span,
                    colon_span: f.colon_span,
                    value: self.lower_expr(pkg_prefix, &f.value),
                })
                .collect::<Vec<_>>();

            return (
                ExprKind::StructLit {
                    ty: ty_id,
                    fields: lowered_fields,
                },
                ty_id,
            );
        };

        let mut param_to_field: Vec<Option<usize>> = vec![None; info.params.len()];
        for (field_idx, field) in fields.iter().enumerate() {
            let field_name = field.name.text(self.source);
            let Some(param_idx) = info
                .params
                .iter()
                .position(|param| param.name == field_name)
            else {
                let lowered_fields = fields
                    .iter()
                    .map(|f| StructLitField {
                        span: f.span,
                        name: f.name.text(self.source).to_string(),
                        name_span: f.name.span,
                        colon_span: f.colon_span,
                        value: self.lower_expr(pkg_prefix, &f.value),
                    })
                    .collect::<Vec<_>>();
                return (
                    ExprKind::StructLit {
                        ty: ty_id,
                        fields: lowered_fields,
                    },
                    ty_id,
                );
            };
            let slot = param_to_field
                .get_mut(param_idx)
                .expect("param index in range");
            if slot.is_some() {
                let lowered_fields = fields
                    .iter()
                    .map(|f| StructLitField {
                        span: f.span,
                        name: f.name.text(self.source).to_string(),
                        name_span: f.name.span,
                        colon_span: f.colon_span,
                        value: self.lower_expr(pkg_prefix, &f.value),
                    })
                    .collect::<Vec<_>>();
                return (
                    ExprKind::StructLit {
                        ty: ty_id,
                        fields: lowered_fields,
                    },
                    ty_id,
                );
            }
            *slot = Some(field_idx);
        }

        let needs_defaults = param_to_field.iter().any(|slot| slot.is_none());
        if !needs_defaults {
            let lowered_fields = fields
                .iter()
                .map(|f| StructLitField {
                    span: f.span,
                    name: f.name.text(self.source).to_string(),
                    name_span: f.name.span,
                    colon_span: f.colon_span,
                    value: self.lower_expr(pkg_prefix, &f.value),
                })
                .collect::<Vec<_>>();

            return (
                ExprKind::StructLit {
                    ty: ty_id,
                    fields: lowered_fields,
                },
                ty_id,
            );
        }

        let Some((decl_source, decl_file)) = self.decl_ast_context(&info.decl_file) else {
            let lowered_fields = fields
                .iter()
                .map(|f| StructLitField {
                    span: f.span,
                    name: f.name.text(self.source).to_string(),
                    name_span: f.name.span,
                    colon_span: f.colon_span,
                    value: self.lower_expr(pkg_prefix, &f.value),
                })
                .collect::<Vec<_>>();

            return (
                ExprKind::StructLit {
                    ty: ty_id,
                    fields: lowered_fields,
                },
                ty_id,
            );
        };
        let decl_pkg_prefix = package_prefix(decl_source, decl_file.package.as_ref());
        let expecteds: Vec<ExpectedExpr> = info
            .params
            .iter()
            .map(|param| {
                self.struct_default_param_expected_expr(
                    decl_source,
                    decl_file,
                    &info.type_params,
                    &concrete_args,
                    param,
                )
            })
            .collect();
        let param_ids: Vec<crate::hir::SymbolId> = info
            .params
            .iter()
            .map(|param| {
                self.struct_default_param_local_id(decl_source, decl_file, param.decl_span)
            })
            .collect();

        let mut stmts: Vec<Stmt> = Vec::with_capacity(info.params.len() + 1);
        for field in fields {
            let field_name = field.name.text(self.source);
            let param_idx = info
                .params
                .iter()
                .position(|param| param.name == field_name)
                .expect("known field name mapped to param");
            let expected = *expecteds.get(param_idx).expect("expected info collected");
            let init = self.lower_expr_with_expected(pkg_prefix, &field.value, expected);
            let param = info.params.get(param_idx).expect("param index in range");
            stmts.push(Stmt {
                span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(ValDecl {
                    span,
                    id: Some(*param_ids.get(param_idx).expect("param id collected")),
                    name: Some(param.name.clone()),
                    mutable: false,
                    ty: expected.value_ty.unwrap_or(self.builtins.any),
                    init: Some(init),
                }),
            });
        }

        for (param_idx, param) in info.params.iter().enumerate() {
            if param_to_field.get(param_idx).copied().flatten().is_some() {
                continue;
            }
            let default_value = param
                .default_value
                .as_ref()
                .expect("missing field requires default");
            let expected = *expecteds.get(param_idx).expect("expected info collected");
            let init = self.with_bound_struct_default_context(
                decl_source,
                decl_file,
                &info.type_params,
                &concrete_args,
                |this| this.lower_expr_with_expected(&decl_pkg_prefix, default_value, expected),
            );
            stmts.push(Stmt {
                span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(ValDecl {
                    span,
                    id: Some(*param_ids.get(param_idx).expect("param id collected")),
                    name: Some(param.name.clone()),
                    mutable: false,
                    ty: expected.value_ty.unwrap_or(self.builtins.any),
                    init: Some(init),
                }),
            });
        }

        let mut lowered_fields = Vec::with_capacity(info.params.len());
        for (param_idx, param) in info.params.iter().enumerate() {
            let expected = *expecteds.get(param_idx).expect("expected info collected");
            lowered_fields.push(StructLitField {
                span,
                name: param.name.clone(),
                name_span: span,
                colon_span: span,
                value: Expr {
                    span: param.decl_span,
                    ty: expected.value_ty.unwrap_or(self.builtins.any),
                    kind: ExprKind::VarRef(ValueRef::Local {
                        id: *param_ids.get(param_idx).expect("param id collected"),
                        name: param.name.clone(),
                        decl_span: param.decl_span,
                    }),
                },
            });
        }

        let struct_expr = Expr {
            span,
            ty: ty_id,
            kind: ExprKind::StructLit {
                ty: ty_id,
                fields: lowered_fields,
            },
        };
        stmts.push(Stmt {
            span,
            ty: ty_id,
            kind: StmtKind::Expr(struct_expr),
        });

        (
            ExprKind::Block(Block {
                span,
                ty: ty_id,
                stmts,
            }),
            ty_id,
        )
    }

    /// `with` 表达式 lowering（spec §2.6）。
    ///
    /// 将 `base with { path: value }` 展开为一个 copy-update block：
    /// ```text
    /// {
    ///   val $with_base = <base>
    ///   <按具体值类型重建 aggregate>
    /// }
    /// ```
    /// 对于嵌套路径，递归重建内层 struct / tuple / enum。
    #[allow(clippy::too_many_arguments)]
    fn lower_with_update_expr(
        &mut self,
        pkg_prefix: &str,
        expr_span: Span,
        with_span: Span,
        base: &ast::Expr,
        updates: &[ast::WithUpdateField],
        resolved_copy_update_tys: &std::cell::OnceCell<std::collections::HashMap<String, TypeId>>,
        resolved_copy_update_enums: &std::cell::OnceCell<
            std::collections::HashMap<String, ast::WithUpdateResolvedEnum>,
        >,
    ) -> Expr {
        let typecheck_types = match self.typecheck_types {
            Some(types) => types,
            None => {
                return Expr {
                    span: expr_span,
                    ty: self.builtins.any,
                    kind: ExprKind::Todo("with_update"),
                };
            }
        };

        let aggregate_ty_map = match resolved_copy_update_tys.get() {
            Some(map) => map
                .iter()
                .map(|(prefix, ty)| {
                    (
                        prefix.clone(),
                        self.types.re_intern_from(typecheck_types, *ty),
                    )
                })
                .collect::<std::collections::HashMap<_, _>>(),
            None => {
                return Expr {
                    span: expr_span,
                    ty: self.builtins.any,
                    kind: ExprKind::Todo("with_update"),
                };
            }
        };

        let aggregate_enum_map = match resolved_copy_update_enums.get() {
            Some(map) => map
                .iter()
                .map(|(prefix, info)| {
                    (
                        prefix.clone(),
                        ast::WithUpdateResolvedEnum {
                            enum_fqn: info.enum_fqn.clone(),
                            variants: info
                                .variants
                                .iter()
                                .map(|variant| ast::WithUpdateResolvedEnumVariant {
                                    name: variant.name.clone(),
                                    fields: variant
                                        .fields
                                        .iter()
                                        .map(|field| ast::WithUpdateResolvedEnumField {
                                            name: field.name.clone(),
                                            ty: self
                                                .types
                                                .re_intern_from(typecheck_types, field.ty),
                                        })
                                        .collect(),
                                })
                                .collect(),
                        },
                    )
                })
                .collect::<std::collections::HashMap<_, _>>(),
            None => {
                return Expr {
                    span: expr_span,
                    ty: self.builtins.any,
                    kind: ExprKind::Todo("with_update"),
                };
            }
        };

        let base_ty = match aggregate_ty_map.get("") {
            Some(ty) => *ty,
            None => {
                return Expr {
                    span: expr_span,
                    ty: self.builtins.any,
                    kind: ExprKind::Todo("with_update"),
                };
            }
        };

        let base_lowered = self.lower_expr(pkg_prefix, base);
        let base_id = self.intern_local_symbol(with_span, false);

        let base_ref = Expr {
            span: with_span,
            ty: base_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: base_id,
                name: "$with_base".to_string(),
                decl_span: with_span,
            }),
        };

        let mut grouped: std::collections::HashMap<String, Vec<(&[ast::Ident], &ast::Expr)>> =
            std::collections::HashMap::new();
        for u in updates {
            let segs = &u.path.segments;
            if segs.is_empty() {
                continue;
            }
            let first = self.source.slice(segs[0].span).to_string();
            grouped
                .entry(first)
                .or_default()
                .push((&segs[1..], &u.value));
        }

        let rebuilt = self.build_with_copy_expr(
            pkg_prefix,
            expr_span,
            with_span,
            base_ty,
            &base_ref,
            &grouped,
            &aggregate_ty_map,
            &aggregate_enum_map,
            "",
        );

        let val_stmt = Stmt {
            span: with_span,
            ty: base_ty,
            kind: StmtKind::Val(ValDecl {
                span: with_span,
                id: Some(base_id),
                name: Some("$with_base".to_string()),
                mutable: false,
                ty: base_ty,
                init: Some(base_lowered),
            }),
        };

        let result_stmt = Stmt {
            span: expr_span,
            ty: base_ty,
            kind: StmtKind::Expr(rebuilt),
        };

        Expr {
            span: expr_span,
            ty: base_ty,
            kind: ExprKind::Block(Block {
                span: expr_span,
                ty: base_ty,
                stmts: vec![val_stmt, result_stmt],
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_with_copy_expr(
        &mut self,
        pkg_prefix: &str,
        expr_span: Span,
        with_span: Span,
        aggregate_ty: TypeId,
        base_access: &Expr,
        grouped: &std::collections::HashMap<String, Vec<(&[ast::Ident], &ast::Expr)>>,
        aggregate_ty_map: &std::collections::HashMap<String, TypeId>,
        aggregate_enum_map: &std::collections::HashMap<String, ast::WithUpdateResolvedEnum>,
        current_prefix: &str,
    ) -> Expr {
        enum LoweringAggregateKind {
            Struct(String),
            Tuple,
            Enum,
            Unsupported,
        }

        let lowering_kind = match self.types.kind(aggregate_ty) {
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if matches!(
                    self.type_kinds.get(&nominal.fqn),
                    Some(&ast::TypeKind::Struct)
                ) =>
            {
                LoweringAggregateKind::Struct(nominal.fqn.clone())
            }
            TypeKind::Value(ValueTypeKind::Tuple(_)) => LoweringAggregateKind::Tuple,
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if matches!(
                    self.type_kinds.get(&nominal.fqn),
                    Some(&ast::TypeKind::Enum)
                ) =>
            {
                LoweringAggregateKind::Enum
            }
            _ => LoweringAggregateKind::Unsupported,
        };

        match lowering_kind {
            LoweringAggregateKind::Struct(struct_fqn) => self.build_with_struct_lit(
                pkg_prefix,
                expr_span,
                with_span,
                &struct_fqn,
                aggregate_ty,
                base_access,
                grouped,
                aggregate_ty_map,
                aggregate_enum_map,
                current_prefix,
            ),
            LoweringAggregateKind::Tuple => self.build_with_tuple_lit(
                pkg_prefix,
                expr_span,
                with_span,
                aggregate_ty,
                base_access,
                grouped,
                aggregate_ty_map,
                aggregate_enum_map,
                current_prefix,
            ),
            LoweringAggregateKind::Enum => self.build_with_enum_expr(
                pkg_prefix,
                expr_span,
                with_span,
                aggregate_ty,
                base_access,
                grouped,
                aggregate_ty_map,
                aggregate_enum_map,
                current_prefix,
            ),
            LoweringAggregateKind::Unsupported => Expr {
                span: expr_span,
                ty: self.builtins.any,
                kind: ExprKind::Todo("with_update"),
            },
        }
    }

    /// 递归构造 with-update 的 StructLit 表达式。
    ///
    /// `base_access` 是访问当前层级 base 值的表达式（例如 `$with_base` 或 `$with_base.start`）。
    /// `grouped` 中 key 为当前层级的 field name，value 为 (remaining path segments, value expr)。
    /// `aggregate_ty_map` 为 typecheck 写回的 path_prefix → 具体 aggregate type 映射。
    /// `current_prefix` 为当前层级的路径前缀（例如 `""` 或 `"start"`）。
    #[allow(clippy::too_many_arguments)]
    fn build_with_struct_lit(
        &mut self,
        pkg_prefix: &str,
        expr_span: Span,
        with_span: Span,
        struct_fqn: &str,
        struct_ty: TypeId,
        base_access: &Expr,
        grouped: &std::collections::HashMap<String, Vec<(&[ast::Ident], &ast::Expr)>>,
        aggregate_ty_map: &std::collections::HashMap<String, TypeId>,
        aggregate_enum_map: &std::collections::HashMap<String, ast::WithUpdateResolvedEnum>,
        current_prefix: &str,
    ) -> Expr {
        let field_names: Vec<String> = self
            .index
            .constructors
            .get(struct_fqn)
            .and_then(|ctors| {
                ctors
                    .iter()
                    .find(|c| c.kind == crate::resolve::ConstructorKind::Primary)
            })
            .map(|ctor| ctor.params.iter().map(|p| p.name.clone()).collect())
            .unwrap_or_default();

        let mut fields = Vec::with_capacity(field_names.len());

        for field_name in &field_names {
            let field_fqn = format!("{}.{}", struct_fqn, field_name);
            let field_id = self.symbols.intern_top_level(field_fqn.clone());
            let field_access = Expr {
                span: with_span,
                ty: self.builtins.any,
                kind: ExprKind::MemberAccess {
                    receiver: Box::new(base_access.clone()),
                    member: MemberAccess {
                        span: with_span,
                        name: field_name.clone(),
                        resolved: Some(MemberRef::Value {
                            id: field_id,
                            fqn: field_fqn,
                        }),
                    },
                },
            };

            let value = if let Some(update_group) = grouped.get(field_name) {
                self.build_with_field_value(
                    pkg_prefix,
                    expr_span,
                    with_span,
                    field_name,
                    field_access,
                    update_group,
                    aggregate_ty_map,
                    aggregate_enum_map,
                    current_prefix,
                )
            } else {
                field_access
            };

            fields.push(StructLitField {
                span: with_span,
                name: field_name.clone(),
                name_span: with_span,
                colon_span: with_span,
                value,
            });
        }

        Expr {
            span: expr_span,
            ty: struct_ty,
            kind: ExprKind::StructLit {
                ty: struct_ty,
                fields,
            },
        }
    }

    /// 递归构造 with-update 的 TupleLit 表达式。
    ///
    /// tuple 元素沿用 `_0` / `_1` / ... 成员访问语法读取原值，再按 grouped 中的更新重建。
    #[allow(clippy::too_many_arguments)]
    fn build_with_tuple_lit(
        &mut self,
        pkg_prefix: &str,
        expr_span: Span,
        with_span: Span,
        tuple_ty: TypeId,
        base_access: &Expr,
        grouped: &std::collections::HashMap<String, Vec<(&[ast::Ident], &ast::Expr)>>,
        aggregate_ty_map: &std::collections::HashMap<String, TypeId>,
        aggregate_enum_map: &std::collections::HashMap<String, ast::WithUpdateResolvedEnum>,
        current_prefix: &str,
    ) -> Expr {
        let element_tys = match self.types.kind(tuple_ty) {
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => elements.to_vec(),
            _ => {
                return Expr {
                    span: expr_span,
                    ty: self.builtins.any,
                    kind: ExprKind::Todo("with_update"),
                };
            }
        };

        let mut elements = Vec::with_capacity(element_tys.len());
        for (idx, _) in element_tys.iter().enumerate() {
            let member_name = format!("_{idx}");
            let field_access = Expr {
                span: with_span,
                ty: self.builtins.any,
                kind: ExprKind::MemberAccess {
                    receiver: Box::new(base_access.clone()),
                    member: MemberAccess {
                        span: with_span,
                        name: member_name.clone(),
                        resolved: None,
                    },
                },
            };

            let value = if let Some(update_group) = grouped.get(&member_name) {
                self.build_with_field_value(
                    pkg_prefix,
                    expr_span,
                    with_span,
                    &member_name,
                    field_access,
                    update_group,
                    aggregate_ty_map,
                    aggregate_enum_map,
                    current_prefix,
                )
            } else {
                field_access
            };

            elements.push(value);
        }

        Expr {
            span: expr_span,
            ty: tuple_ty,
            kind: ExprKind::TupleLit { elements },
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_with_enum_expr(
        &mut self,
        pkg_prefix: &str,
        expr_span: Span,
        with_span: Span,
        enum_ty: TypeId,
        base_access: &Expr,
        grouped: &std::collections::HashMap<String, Vec<(&[ast::Ident], &ast::Expr)>>,
        aggregate_ty_map: &std::collections::HashMap<String, TypeId>,
        aggregate_enum_map: &std::collections::HashMap<String, ast::WithUpdateResolvedEnum>,
        current_prefix: &str,
    ) -> Expr {
        let Some(enum_info) = aggregate_enum_map.get(current_prefix) else {
            return Expr {
                span: expr_span,
                ty: self.builtins.any,
                kind: ExprKind::Todo("with_update"),
            };
        };

        let mut arms: Vec<WhenArm> = Vec::with_capacity(enum_info.variants.len());
        for variant in &enum_info.variants {
            let update_group = grouped.get(&variant.name);
            let mut pat_args: Vec<WhenPat> = Vec::with_capacity(variant.fields.len());
            let mut field_refs: Vec<(String, Expr)> = Vec::with_capacity(variant.fields.len());

            for field in &variant.fields {
                let (decl_span, id, name) =
                    self.fresh_synthetic_local(with_span, "__with_enum_field", false);
                self.record_when_pat_binding_ty(decl_span, field.ty);
                pat_args.push(WhenPat::Bind {
                    span: decl_span,
                    id,
                    name: name.clone(),
                });
                field_refs.push((
                    field.name.clone(),
                    Expr {
                        span: with_span,
                        ty: field.ty,
                        kind: ExprKind::VarRef(ValueRef::Local {
                            id,
                            name,
                            decl_span,
                        }),
                    },
                ));
            }

            let body = if let Some(update_group) = update_group {
                let mut grouped_by_field: std::collections::HashMap<
                    String,
                    Vec<(&[ast::Ident], &ast::Expr)>,
                > = std::collections::HashMap::new();
                for (rest, val) in update_group {
                    if rest.is_empty() {
                        return Expr {
                            span: expr_span,
                            ty: self.builtins.any,
                            kind: ExprKind::Todo("with_update"),
                        };
                    }
                    let next = self.source.slice(rest[0].span).to_string();
                    grouped_by_field
                        .entry(next)
                        .or_default()
                        .push((&rest[1..], *val));
                }

                let mut args: Vec<CallArg> = Vec::with_capacity(variant.fields.len());
                for field in &variant.fields {
                    let Some((_, field_ref)) =
                        field_refs.iter().find(|(name, _)| name == &field.name)
                    else {
                        return Expr {
                            span: expr_span,
                            ty: self.builtins.any,
                            kind: ExprKind::Todo("with_update"),
                        };
                    };
                    let variant_prefix = if current_prefix.is_empty() {
                        variant.name.clone()
                    } else {
                        format!("{}.{}", current_prefix, variant.name)
                    };
                    let value = if let Some(field_group) = grouped_by_field.get(&field.name) {
                        self.build_with_field_value(
                            pkg_prefix,
                            expr_span,
                            with_span,
                            &field.name,
                            field_ref.clone(),
                            field_group,
                            aggregate_ty_map,
                            aggregate_enum_map,
                            &variant_prefix,
                        )
                    } else {
                        field_ref.clone()
                    };
                    args.push(CallArg::Positional(value));
                }

                Expr {
                    span: expr_span,
                    ty: enum_ty,
                    kind: ExprKind::Call {
                        callee: Box::new(Expr {
                            span: with_span,
                            ty: self.builtins.any,
                            kind: ExprKind::UnresolvedIdent {
                                name: variant.name.clone(),
                            },
                        }),
                        args,
                    },
                }
            } else {
                base_access.clone()
            };

            arms.push(WhenArm {
                span: expr_span,
                pat: WhenPat::Variant {
                    span: with_span,
                    name_span: with_span,
                    name: variant.name.clone(),
                    args: pat_args,
                },
                guard: None,
                arrow_span: with_span,
                body,
            });
        }

        Expr {
            span: expr_span,
            ty: enum_ty,
            kind: ExprKind::When {
                subject: Box::new(base_access.clone()),
                arms,
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_with_field_value(
        &mut self,
        pkg_prefix: &str,
        expr_span: Span,
        with_span: Span,
        field_name: &str,
        field_access: Expr,
        update_group: &[(&[ast::Ident], &ast::Expr)],
        aggregate_ty_map: &std::collections::HashMap<String, TypeId>,
        aggregate_enum_map: &std::collections::HashMap<String, ast::WithUpdateResolvedEnum>,
        current_prefix: &str,
    ) -> Expr {
        if let Some((_, val_expr)) = update_group.iter().find(|(rest, _)| rest.is_empty()) {
            return self.lower_expr(pkg_prefix, val_expr);
        }

        let nested_prefix = if current_prefix.is_empty() {
            field_name.to_string()
        } else {
            format!("{}.{}", current_prefix, field_name)
        };

        let Some(nested_ty) = aggregate_ty_map.get(&nested_prefix).copied() else {
            return field_access;
        };

        let mut nested_grouped: std::collections::HashMap<
            String,
            Vec<(&[ast::Ident], &ast::Expr)>,
        > = std::collections::HashMap::new();
        for (rest, val) in update_group {
            if !rest.is_empty() {
                let next = self.source.slice(rest[0].span).to_string();
                nested_grouped
                    .entry(next)
                    .or_default()
                    .push((&rest[1..], *val));
            }
        }

        self.build_with_copy_expr(
            pkg_prefix,
            expr_span,
            with_span,
            nested_ty,
            &field_access,
            &nested_grouped,
            aggregate_ty_map,
            aggregate_enum_map,
            &nested_prefix,
        )
    }

    fn lower_member_access_expr(
        &mut self,
        pkg_prefix: &str,
        receiver: &ast::Expr,
        member: &ast::MemberIdent,
    ) -> (ExprKind, TypeId) {
        let receiver = self.lower_expr(pkg_prefix, receiver);
        self.lower_member_access_expr_from_receiver(pkg_prefix, receiver, member)
    }

    fn lower_member_access_expr_from_receiver(
        &mut self,
        pkg_prefix: &str,
        receiver: Expr,
        member: &ast::MemberIdent,
    ) -> (ExprKind, TypeId) {
        let resolved = self.resolved_member_for_lowering(member);

        // delegated property lowering（spec §10.4）：
        // `receiver.prop` → `receiver.prop$delegate.getValue(receiver, <PropertyMeta const>)`
        if let Some(ast::ResolvedMemberRef::Value { fqn }) = resolved.as_ref()
            && let Some(info) = self.delegated_properties.get(fqn).cloned()
        {
            match info {
                DelegatedPropertyInfo::Lazy(info) => {
                    return self.lower_lazy_delegated_property_get_from_receiver(
                        pkg_prefix,
                        member.span,
                        receiver,
                        &info,
                    );
                }
                DelegatedPropertyInfo::Generic(info) => {
                    let this_ref = receiver.clone();

                    let delegate = self.lower_generic_delegated_property_delegate_access_expr(
                        member.span,
                        receiver,
                        &info,
                    );
                    let callee = Expr {
                        span: member.span,
                        ty: self.builtins.any,
                        kind: ExprKind::MemberAccess {
                            receiver: Box::new(delegate),
                            member: MemberAccess {
                                span: member.span,
                                name: "getValue".to_string(),
                                resolved: None,
                            },
                        },
                    };
                    let meta =
                        self.lower_property_meta_ref_expr(member.span, &info.property_meta_fqn);

                    return (
                        ExprKind::Call {
                            callee: Box::new(callee),
                            args: vec![CallArg::Positional(this_ref), CallArg::Positional(meta)],
                        },
                        self.builtins.any,
                    );
                }
                DelegatedPropertyInfo::Observable(info) => {
                    return self.lower_observable_vetoable_delegated_property_get_from_receiver(
                        member.span,
                        receiver,
                        fqn,
                        info.decl,
                        info.ty.as_ref(),
                        info.mutex_field_fqn,
                    );
                }
                DelegatedPropertyInfo::Vetoable(info) => {
                    return self.lower_observable_vetoable_delegated_property_get_from_receiver(
                        member.span,
                        receiver,
                        fqn,
                        info.decl,
                        info.ty.as_ref(),
                        info.mutex_field_fqn,
                    );
                }
                DelegatedPropertyInfo::MapBacked => {
                    // map-backed：值在初始化时被拷贝到真实字段，后续只读；
                    // 读取不需要额外同步，按普通字段访问处理。
                }
            }
        }

        // T0112：extension property access → desugar to getter call.
        // `receiver.extProp` → `extPropGetterFqn(receiver)`
        if let Some(ast::ResolvedMemberRef::ExtensionValue { fqn }) = resolved.as_ref() {
            let callee_id = self.symbols.intern_top_level(fqn.clone());
            let callee = Expr {
                span: member.span,
                ty: self.builtins.any,
                kind: ExprKind::VarRef(ValueRef::TopLevel {
                    id: callee_id,
                    fqn: fqn.clone(),
                }),
            };
            return (
                ExprKind::Call {
                    callee: Box::new(callee),
                    args: vec![CallArg::Positional(receiver)],
                },
                self.builtins.any,
            );
        }

        // T4010b1：值类型 computed property access → getter(receiver)。
        if let Some(ast::ResolvedMemberRef::Value { fqn }) = resolved.as_ref()
            && self.value_type_computed_properties.contains(fqn)
        {
            let callee = Expr {
                span: member.span,
                ty: self.builtins.any,
                kind: ExprKind::VarRef(ValueRef::TopLevel {
                    id: self.symbols.intern_top_level(fqn.clone()),
                    fqn: fqn.clone(),
                }),
            };
            return (
                ExprKind::Call {
                    callee: Box::new(callee),
                    args: vec![CallArg::Positional(receiver)],
                },
                self.builtins.any,
            );
        }

        let receiver = Box::new(receiver);

        let resolved = resolved.as_ref().map(|r| self.lower_resolved_member_ref(r));

        let member = MemberAccess {
            span: member.span,
            name: self.source.slice(member.span).to_string(),
            resolved,
        };

        (
            ExprKind::MemberAccess { receiver, member },
            self.builtins.any,
        )
    }

    fn lower_resolved_member_ref(&mut self, resolved: &ast::ResolvedMemberRef) -> MemberRef {
        match resolved {
            ast::ResolvedMemberRef::Value { fqn } => MemberRef::Value {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
            ast::ResolvedMemberRef::Fun { fqn } => MemberRef::Fun {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
            ast::ResolvedMemberRef::ExtensionValue { fqn } => MemberRef::ExtensionValue {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
            ast::ResolvedMemberRef::ExtensionFun { fqn } => MemberRef::ExtensionFun {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
        }
    }

    pub(super) fn resolved_member_for_lowering(
        &self,
        member: &ast::MemberIdent,
    ) -> Option<ast::ResolvedMemberRef> {
        self.file
            .typechecked_member_resolved(member.span)
            .or_else(|| self.file.safe_member_access_resolved(member.span))
            .or_else(|| member.resolved.clone())
    }

    fn should_keep_member_call_as_member_access(
        &mut self,
        receiver: &ast::Expr,
        member: &ast::MemberIdent,
    ) -> bool {
        let Some(receiver_ty) = self.typechecked_expr_ty(receiver.span) else {
            return false;
        };
        let member_name = self.source.slice(member.span);

        if receiver_ty == self.builtins.string {
            return matches!(
                member_name,
                "trimIndent"
                    | "length"
                    | "toInt"
                    | "concat"
                    | "hash"
                    | "isEmpty"
                    | "replace"
                    | "charAt"
                    | "repeat"
                    | "compareTo"
                    | "byteLength"
                    | "getByte"
                    | "unsafeSliceBytes"
            );
        }

        if receiver_ty == self.builtins.int {
            return matches!(member_name, "toString" | "hash");
        }

        if receiver_ty == self.builtins.bool_ {
            return member_name == "toString";
        }

        if receiver_ty == self.builtins.char_ {
            return matches!(member_name, "toInt" | "toString" | "hash");
        }

        if receiver_ty == self.builtins.float64 || receiver_ty == self.builtins.float32 {
            return matches!(
                member_name,
                "toInt" | "toString" | "hash" | "abs" | "isNaN" | "isInfinite"
            );
        }

        false
    }

    fn lower_ident_expr(&mut self, id: &ast::ValueIdent) -> (ExprKind, TypeId) {
        let text = self.source.slice(id.span);
        if text == "true" {
            return (
                ExprKind::Literal(LiteralKind::Bool(true)),
                self.builtins.bool_,
            );
        }
        if text == "false" {
            return (
                ExprKind::Literal(LiteralKind::Bool(false)),
                self.builtins.bool_,
            );
        }

        if text == "this"
            && let Some(decl_span) = self.lambda_this_decl_span
        {
            let ty = self
                .typechecked_binding_ty(decl_span)
                .or_else(|| self.typechecked_expr_ty(id.span))
                .unwrap_or(self.builtins.any);
            return (
                ExprKind::VarRef(ValueRef::Local {
                    id: self.intern_local_symbol(decl_span, false),
                    name: "this".to_string(),
                    decl_span,
                }),
                ty,
            );
        }

        let Some(resolved) = id.resolved.as_ref() else {
            // 典型场景：enum variant ctor 的 callee（`Some(1)`）/0-参数 variant 值（`None`）；
            // resolver 会保留为“未 resolve”，让 typecheck 在期望类型语境下决议。
            return (
                ExprKind::UnresolvedIdent {
                    name: text.to_string(),
                },
                self.builtins.any,
            );
        };

        let resolved = match resolved {
            ast::ResolvedValueRef::Local { name, decl_span } => ValueRef::Local {
                id: self.intern_local_symbol(*decl_span, false),
                name: name.clone(),
                decl_span: *decl_span,
            },
            ast::ResolvedValueRef::TopLevel { fqn } => ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
        };

        let ty = match &resolved {
            ValueRef::Local { decl_span, .. } => self
                .typechecked_binding_ty(*decl_span)
                .or_else(|| self.typechecked_expr_ty(id.span))
                .unwrap_or(self.builtins.any),
            ValueRef::TopLevel { .. } => self
                .typechecked_expr_ty(id.span)
                .unwrap_or(self.builtins.any),
        };

        (ExprKind::VarRef(resolved), ty)
    }

    fn try_lower_effect_op_call_expr(
        &mut self,
        pkg_prefix: &str,
        call_span: Span,
        callee: &ast::Expr,
        args: &[ast::Expr],
    ) -> Option<(ExprKind, TypeId)> {
        let callee = match &callee.kind {
            // `Effect.op<T>(...)`：HIR lowering 也把 TypeApply 视为“只包住 callee 的透明外壳”，
            // 以便 generic effect-op call 与普通 effect-op call 进入同一条 perform lowering 主线。
            ast::ExprKind::TypeApply { callee, .. } => callee.as_ref(),
            _ => callee,
        };

        let ast::ExprKind::MemberAccess { member, .. } = &callee.kind else {
            return None;
        };
        let resolved = self.resolved_member_for_lowering(member);
        let Some(ast::ResolvedMemberRef::Fun { fqn }) = resolved.as_ref() else {
            return None;
        };
        if !self.is_effect_op_fqn(fqn) {
            return None;
        }

        let op = EffectOpRef {
            span: member.span,
            fqn: fqn.clone(),
        };
        let effect_ty = self
            .typechecked_performed_effect_ty(call_span)
            .unwrap_or(self.builtins.any);
        let args: Vec<CallArg> = args
            .iter()
            .map(|arg| self.lower_call_arg(pkg_prefix, arg))
            .collect();
        let arg_mapping = self
            .typechecked_effect_op_call_binding(call_span)
            .map(|binding| binding.arg_mapping)
            .unwrap_or_else(|| (0..args.len()).collect());
        let payload_tuple_ty = if args.len() > 1 {
            let mut elements = Vec::with_capacity(arg_mapping.len());
            for &arg_idx in &arg_mapping {
                let arg = args.get(arg_idx)?;
                elements.push(Self::call_arg_value_ty(arg));
            }
            Some(self.types.ty_tuple(elements))
        } else {
            None
        };
        self.effect_op_call_sites.insert(
            self.call_site(call_span),
            super::super::EffectOpCallInfo {
                arg_mapping,
                payload_tuple_ty,
            },
        );
        Some((
            ExprKind::Perform {
                effect_ty,
                op,
                args,
            },
            self.builtins.any,
        ))
    }

    fn is_effect_op_fqn(&self, fqn: &str) -> bool {
        let Some(syms) = self.index.by_fqn.get(fqn) else {
            return false;
        };
        syms.fun
            .iter()
            .any(|o| o.sig.kind == ast::FunDeclKind::EffectOp)
    }

    // ── T0108: nullable operators (`?.` and `!!`) desugar ──────────────────

    /// `expr!!` → `when (expr) { Some(v) -> v; None -> Raise.raise(RuntimeError.NullAssertionFailed) }`
    fn lower_not_null_assert_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        expr: &ast::Expr,
        op_span: Span,
    ) -> (ExprKind, TypeId) {
        let subject = Box::new(self.lower_expr(pkg_prefix, expr));
        let result_ty = self.typechecked_expr_ty(span).unwrap_or(self.builtins.any);
        let binder_ty = self.option_inner_ty(subject.ty).unwrap_or(result_ty);
        let v_sym = self.intern_local_symbol(op_span, false);
        self.record_when_pat_binding_ty(op_span, binder_ty);

        let some_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "Some".to_string(),
                args: vec![WhenPat::Bind {
                    span: op_span,
                    id: v_sym,
                    name: "__not_null_v".to_string(),
                }],
            },
            guard: None,
            arrow_span: op_span,
            body: Expr {
                span: op_span,
                ty: result_ty,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id: v_sym,
                    name: "__not_null_v".to_string(),
                    decl_span: op_span,
                }),
            },
        };

        let none_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "None".to_string(),
                args: vec![],
            },
            guard: None,
            arrow_span: op_span,
            body: self.synth_raise_null_assertion_failed(op_span),
        };

        (
            ExprKind::When {
                subject,
                arms: vec![some_arm, none_arm],
            },
            result_ty,
        )
    }

    /// `lhs ?: rhs` → `when (lhs) { Some(v) -> v; None -> rhs }`
    ///
    /// 语义要求：
    /// - `lhs` 只求值一次；
    /// - `rhs` 仅在 `lhs` 为 `None` 时求值；
    /// - 结果类型与 typecheck 对 Elvis 的 inner type 推断保持一致。
    fn lower_elvis_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        lhs: &ast::Expr,
        op_span: Span,
        rhs: &ast::Expr,
    ) -> Expr {
        let subject = Box::new(self.lower_expr(pkg_prefix, lhs));
        let rhs = self.lower_expr(pkg_prefix, rhs);
        let result_ty = self.typechecked_expr_ty(span).unwrap_or(rhs.ty);
        let binder_ty = self
            .option_inner_ty(subject.ty)
            .unwrap_or(self.builtins.any);
        let v_sym = self.intern_local_symbol(op_span, false);
        self.record_when_pat_binding_ty(op_span, binder_ty);

        let some_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "Some".to_string(),
                args: vec![WhenPat::Bind {
                    span: op_span,
                    id: v_sym,
                    name: "__elvis_v".to_string(),
                }],
            },
            guard: None,
            arrow_span: op_span,
            body: Expr {
                span: op_span,
                ty: result_ty,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id: v_sym,
                    name: "__elvis_v".to_string(),
                    decl_span: op_span,
                }),
            },
        };

        let none_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "None".to_string(),
                args: vec![],
            },
            guard: None,
            arrow_span: op_span,
            body: rhs,
        };

        Expr {
            span,
            ty: result_ty,
            kind: ExprKind::When {
                subject,
                arms: vec![some_arm, none_arm],
            },
        }
    }

    /// `receiver?.field` → `when (receiver) { Some(v) -> Some(v.field); None -> None }`
    fn lower_safe_member_access_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        receiver: &ast::Expr,
        op_span: Span,
        member: &ast::MemberIdent,
    ) -> (ExprKind, TypeId) {
        let subject = Box::new(self.lower_expr(pkg_prefix, receiver));
        let result_ty = self.typechecked_expr_ty(span).unwrap_or(self.builtins.any);
        let binder_ty = self
            .option_inner_ty(subject.ty)
            .unwrap_or(self.builtins.any);
        let v_sym = self.intern_local_symbol(op_span, false);
        self.record_when_pat_binding_ty(op_span, binder_ty);

        let v_ref = Expr {
            span: op_span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: v_sym,
                name: "__safe_v".to_string(),
                decl_span: op_span,
            }),
        };

        // T0152：Some 分支内与普通 member access 共享同一条 lowering 路径；
        // `?.` 只负责在外层包一层 `Some(...)` 并处理 `None` 分支。
        let (inner_kind, inner_ty) =
            self.lower_member_access_expr_from_receiver(pkg_prefix, v_ref.clone(), member);
        let inner_access = Expr {
            span: member.span,
            ty: inner_ty,
            kind: inner_kind,
        };

        let some_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "Some".to_string(),
                args: vec![WhenPat::Bind {
                    span: op_span,
                    id: v_sym,
                    name: "__safe_v".to_string(),
                }],
            },
            guard: None,
            arrow_span: op_span,
            body: self.synth_some_wrap(op_span, inner_access),
        };

        let none_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "None".to_string(),
                args: vec![],
            },
            guard: None,
            arrow_span: op_span,
            body: self.synth_none(op_span),
        };

        (
            ExprKind::When {
                subject,
                arms: vec![some_arm, none_arm],
            },
            result_ty,
        )
    }

    /// `receiver?.method(args)` → `when (receiver) { Some(v) -> Some(v.method(args)); None -> None }`
    fn lower_safe_call_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        receiver: &ast::Expr,
        op_span: Span,
        member: &ast::MemberIdent,
        args: &[ast::Expr],
    ) -> (ExprKind, TypeId) {
        let subject = Box::new(self.lower_expr(pkg_prefix, receiver));
        let result_ty = self.typechecked_expr_ty(span).unwrap_or(self.builtins.any);
        let binder_ty = self
            .option_inner_ty(subject.ty)
            .unwrap_or(self.builtins.any);
        let v_sym = self.intern_local_symbol(op_span, false);
        self.record_when_pat_binding_ty(op_span, binder_ty);

        let v_ref = Expr {
            span: op_span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: v_sym,
                name: "__safe_v".to_string(),
                decl_span: op_span,
            }),
        };

        // Build the inner call `v.method(args)` using the same lowering strategies
        // as the normal Call path (extension fun → TopLevel, class member → TopLevel, fallback).
        let inner_call =
            self.lower_safe_call_inner_call(pkg_prefix, span, op_span, member, &v_ref, args);

        let some_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "Some".to_string(),
                args: vec![WhenPat::Bind {
                    span: op_span,
                    id: v_sym,
                    name: "__safe_v".to_string(),
                }],
            },
            guard: None,
            arrow_span: op_span,
            body: self.synth_some_wrap(op_span, inner_call),
        };

        let none_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "None".to_string(),
                args: vec![],
            },
            guard: None,
            arrow_span: op_span,
            body: self.synth_none(op_span),
        };

        (
            ExprKind::When {
                subject,
                arms: vec![some_arm, none_arm],
            },
            result_ty,
        )
    }

    /// Build the inner call for safe call desugaring.
    /// Mirrors the normal Call lowering: extension fun → TopLevel call, class member → TopLevel call.
    fn lower_safe_call_inner_call(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        op_span: Span,
        member: &ast::MemberIdent,
        v_ref: &Expr,
        args: &[ast::Expr],
    ) -> Expr {
        let lowered_args_without_receiver: Vec<CallArg> = args
            .iter()
            .map(|arg| self.lower_call_arg(pkg_prefix, arg))
            .collect();
        let resolved = self.resolved_member_for_lowering(member);

        // Extension function: `receiver?.ext(args)` → `ext(v, args...)`
        if let Some(ast::ResolvedMemberRef::ExtensionFun { fqn }) = resolved.as_ref() {
            let mut all_args = Vec::with_capacity(lowered_args_without_receiver.len() + 1);
            all_args.push(CallArg::Positional(v_ref.clone()));
            all_args.extend(lowered_args_without_receiver);
            return Expr {
                span,
                ty: self.builtins.any,
                kind: ExprKind::Call {
                    callee: Box::new(Expr {
                        span: op_span,
                        ty: self.builtins.any,
                        kind: ExprKind::VarRef(ValueRef::TopLevel {
                            id: self.symbols.intern_top_level(fqn.clone()),
                            fqn: fqn.clone(),
                        }),
                    }),
                    args: all_args,
                },
            };
        }

        // Class/interface member function: `receiver?.method(args)` → `Owner.method(v, args...)`
        if let Some(ast::ResolvedMemberRef::Fun { fqn }) = resolved.as_ref()
            && let Some((owner_fqn, _)) = fqn.as_str().rsplit_once('.')
        {
            let owner_is_class =
                matches!(self.type_kinds.get(owner_fqn), Some(ast::TypeKind::Class));
            let owner_is_interface = matches!(
                self.type_kinds.get(owner_fqn),
                Some(ast::TypeKind::Interface)
            );
            let owner_is_object = self.index.object_types.contains(owner_fqn);
            if owner_is_class || owner_is_interface || owner_is_object {
                let mut all_args = Vec::with_capacity(lowered_args_without_receiver.len() + 1);
                all_args.push(CallArg::Positional(v_ref.clone()));
                all_args.extend(lowered_args_without_receiver);
                return Expr {
                    span,
                    ty: self.builtins.any,
                    kind: ExprKind::Call {
                        callee: Box::new(Expr {
                            span: op_span,
                            ty: self.builtins.any,
                            kind: ExprKind::VarRef(ValueRef::TopLevel {
                                id: self.symbols.intern_top_level(fqn.clone()),
                                fqn: fqn.clone(),
                            }),
                        }),
                        args: all_args,
                    },
                };
            }
        }

        // Fallback: `v.method(args)` as MemberAccess call.
        let resolved = resolved.as_ref().map(|r| self.lower_resolved_member_ref(r));
        let member_name = self.source.slice(member.span).to_string();
        let callee = Expr {
            span: member.span,
            ty: self.builtins.any,
            kind: ExprKind::MemberAccess {
                receiver: Box::new(v_ref.clone()),
                member: MemberAccess {
                    span: member.span,
                    name: member_name,
                    resolved,
                },
            },
        };
        Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args: lowered_args_without_receiver,
            },
        }
    }

    // ── Synthesized HIR helpers for nullable desugar ───────────────────────

    /// Synthesize `Raise.raise(RuntimeError.NullAssertionFailed)` as a `Perform` node.
    pub(super) fn synth_raise_null_assertion_failed(&mut self, span: Span) -> Expr {
        let error_expr = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::MemberAccess {
                receiver: Box::new(Expr {
                    span,
                    ty: self.builtins.any,
                    kind: ExprKind::Missing,
                }),
                member: MemberAccess {
                    span,
                    name: "NullAssertionFailed".to_string(),
                    resolved: Some(MemberRef::Value {
                        id: self.symbols.intern_top_level(
                            Self::RUNTIME_ERROR_NULL_ASSERTION_FAILED_FQN.to_string(),
                        ),
                        fqn: Self::RUNTIME_ERROR_NULL_ASSERTION_FAILED_FQN.to_string(),
                    }),
                },
            },
        };
        Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::Perform {
                effect_ty: self
                    .typechecked_performed_effect_ty(span)
                    .unwrap_or_else(|| self.synth_raise_runtime_error_effect_ty(span)),
                op: EffectOpRef {
                    span,
                    fqn: Self::RAISE_RAISE_FQN.to_string(),
                },
                args: vec![CallArg::Positional(error_expr)],
            },
        }
    }

    fn synth_raise_runtime_error_effect_ty(&mut self, span: Span) -> TypeId {
        let raise_path = ast::TypePath {
            span,
            segments: vec![
                ast::Ident::synthetic(span, "scoop"),
                ast::Ident::synthetic(span, "core"),
                ast::Ident::synthetic(span, "Raise"),
            ],
            args: vec![ast::TypeRef::Path(ast::TypePath {
                span,
                segments: vec![
                    ast::Ident::synthetic(span, "scoop"),
                    ast::Ident::synthetic(span, "core"),
                    ast::Ident::synthetic(span, "RuntimeError"),
                ],
                args: Vec::new(),
            })],
        };
        self.lower_type_path(&raise_path)
    }

    /// Synthesize `Some(inner)` as a `Call { callee: UnresolvedIdent("Some"), args: [inner] }`.
    fn synth_some_wrap(&self, span: Span, inner: Expr) -> Expr {
        Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    span,
                    ty: self.builtins.any,
                    kind: ExprKind::UnresolvedIdent {
                        name: "Some".to_string(),
                    },
                }),
                args: vec![CallArg::Positional(inner)],
            },
        }
    }

    /// Synthesize `None` as `UnresolvedIdent { name: "None" }`.
    fn synth_none(&self, span: Span) -> Expr {
        Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::UnresolvedIdent {
                name: "None".to_string(),
            },
        }
    }

    fn lower_handle_expr(
        &mut self,
        pkg_prefix: &str,
        body: &ast::Block,
        arms: &[ast::HandleArm],
        finally: Option<&ast::Block>,
    ) -> HandleExpr {
        let body = self.lower_block(pkg_prefix, body);
        let arms = arms
            .iter()
            .map(|arm| self.lower_handle_arm(pkg_prefix, arm))
            .collect();
        let finally = finally.map(|b| self.lower_block(pkg_prefix, b));
        HandleExpr {
            body,
            arms,
            finally,
        }
    }

    fn lower_handle_arm(&mut self, pkg_prefix: &str, arm: &ast::HandleArm) -> HandleArm {
        let kind = match arm.kind {
            ast::HandleArmKind::NonResuming => HandleArmKind::NonResuming,
            ast::HandleArmKind::EscapeContinuation { k_span } => {
                HandleArmKind::EscapeContinuation {
                    continuation: self.intern_local_symbol(k_span, false),
                }
            }
        };
        HandleArm {
            span: arm.span,
            op: self.lower_handle_op(pkg_prefix, &arm.op),
            kind,
            body: self.lower_expr(pkg_prefix, &arm.body),
        }
    }

    fn lower_handle_op(&mut self, _pkg_prefix: &str, op: &ast::HandleOp) -> HandleOp {
        let effect_ty = self
            .typechecked_handle_arm_effect_ty(op.span)
            .unwrap_or_else(|| self.lower_type_path(&op.effect));
        let effect_fqn = self.index.type_ref_to_fqn_in_file(
            self.source,
            self.file,
            &ast::TypeRef::Path(op.effect.clone()),
        );

        let op_name = op.op.text(self.source).to_string();
        let op_fqn = match effect_fqn {
            Some(effect_fqn) => format!("{effect_fqn}.{op_name}"),
            None => format!("{}.{}", self.source.slice(op.effect.span), op_name),
        };

        let binders = op
            .binders
            .iter()
            .map(|b| self.lower_handle_binder(b))
            .collect::<Vec<_>>();
        if binders.len() > 1 {
            let tuple_ty = self
                .types
                .ty_tuple(binders.iter().map(|binder| binder.ty).collect());
            self.handle_payload_tuple_tys
                .insert(self.call_site(op.span), tuple_ty);
        }

        HandleOp {
            span: op.span,
            effect_ty,
            op: EffectOpRef {
                span: op.op.span,
                fqn: op_fqn,
            },
            binders,
        }
    }

    fn lower_handle_binder(&mut self, b: &ast::HandleBinder) -> HandleBinder {
        let ty =
            b.ty.as_ref()
                .map(|t| self.lower_type_ref(t))
                .or_else(|| self.typechecked_binding_ty(b.name.span))
                .unwrap_or(self.builtins.any);
        HandleBinder {
            span: b.span,
            id: self.intern_local_symbol(b.name.span, false),
            name: b.name.text(self.source).to_string(),
            ty,
        }
    }

    pub(super) fn lower_call_arg(&mut self, pkg_prefix: &str, arg: &ast::Expr) -> CallArg {
        match &arg.kind {
            ast::ExprKind::NamedArg { name, value, .. } => CallArg::Named {
                name: name.text(self.source).to_string(),
                name_span: name.span,
                value: self.lower_expr(pkg_prefix, value),
            },
            _ => CallArg::Positional(self.lower_expr(pkg_prefix, arg)),
        }
    }

    fn call_arg_value_ty(arg: &CallArg) -> TypeId {
        match arg {
            CallArg::Positional(expr) => expr.ty,
            CallArg::Named { value, .. } => value.ty,
        }
    }

    /// 若该调用点满足“尾部默认参数可补齐”的规则，则把调用表达式 lowering 为一个 block：
    ///
    /// ```text
    /// f(a0, a1)   // 省略尾部默认参数
    /// =>
    /// {
    ///   val p0 = a0
    ///   val p1 = a1
    ///   val p2 = <default>
    ///   val p3 = <default>
    ///   f(p0, p1, p2, p3)
    /// }
    /// ```
    ///
    /// 说明：
    /// - 这样可以保证 default value 里对“更早参数”的引用能工作（通过局部 `val` 绑定）；
    /// - 也能保证“实参表达式”不会因简单替换而被重复求值（求值顺序与一次性语义可控）。
    fn try_lower_default_args_call_expr(
        &mut self,
        pkg_prefix: &str,
        call_span: Span,
        callee: &ast::Expr,
        args: &[ast::Expr],
    ) -> Option<(ExprKind, TypeId)> {
        let typechecked_call_ty = self.typechecked_expr_ty(call_span);
        let call_ty = typechecked_call_ty.unwrap_or(self.builtins.any);

        // 仅处理：顶层函数直接调用 `foo(...)`。
        let callee = match &callee.kind {
            // `callee<T>()`：HIR v0 视为透明包装（同 `lower_expr(TypeApply)`）。
            ast::ExprKind::TypeApply { callee, .. } => callee.as_ref(),
            _ => callee,
        };
        let ast::ExprKind::Ident(id) = &callee.kind else {
            return None;
        };
        let ast::ResolvedValueRef::TopLevel { fqn } = id.resolved.as_ref()? else {
            return None;
        };
        let info = self.default_arg_funs.get(fqn).cloned()?;

        let provided = args.len();
        let total = info.params.len();
        if provided >= total {
            return None;
        }
        if provided < info.required {
            return None;
        }

        // Kotlin-like：命名实参之后不能再出现位置实参（与 typecheck 对齐；不支持 trailing-lambda 例外）。
        let mut seen_named = false;
        let mut positional_count = 0usize;
        for arg in args {
            match &arg.kind {
                ast::ExprKind::NamedArg { .. } => {
                    seen_named = true;
                }
                _ => {
                    if seen_named {
                        return None;
                    }
                    positional_count += 1;
                }
            }
        }
        if positional_count > total {
            return None;
        }

        // 将调用点的实参映射到形参槽位：
        // - 位置实参：按序绑定到 [0..positional_count)
        // - 命名实参：按 name 查找形参槽位
        let mut param_to_arg: Vec<Option<usize>> = vec![None; total];
        for arg_idx in 0..positional_count {
            *param_to_arg.get_mut(arg_idx)? = Some(arg_idx);
        }
        for (arg_idx, arg) in args.iter().enumerate().skip(positional_count) {
            let ast::ExprKind::NamedArg { name, .. } = &arg.kind else {
                return None;
            };
            let name_text = name.text(self.source).to_string();
            let slot_idx = info.params.iter().position(|p| p.name == name_text)?;
            let slot = param_to_arg.get_mut(slot_idx)?;
            if slot.is_some() {
                // 同一形参不能被重复赋值（位置+命名/命名重复）。
                return None;
            }
            *slot = Some(arg_idx);
        }

        // 未填充的槽位必须有默认值。
        for (idx, param) in info.params.iter().enumerate() {
            if param_to_arg.get(idx)?.is_some() {
                continue;
            }
            param.default_value.as_ref()?;
        }

        // 反向映射：arg_idx -> param_idx（用于按调用点顺序求值实参）。
        let mut arg_to_param: Vec<Option<usize>> = vec![None; args.len()];
        for (param_idx, arg_idx) in param_to_arg.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                continue;
            };
            let slot = arg_to_param.get_mut(arg_idx)?;
            if slot.is_some() {
                return None;
            }
            *slot = Some(param_idx);
        }
        if arg_to_param.iter().any(|x| x.is_none()) {
            return None;
        }

        // 1) 先把“已提供的实参表达式”按参数名绑定为局部 val，避免重复求值。
        //    - 求值顺序：严格按调用点源码顺序（positional + named 的排列）。
        // 2) 再按形参顺序求值缺失的默认参数，并同样绑定为局部 val（供后续默认值引用）。
        let mut stmts: Vec<Stmt> = Vec::with_capacity(total + 1);

        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param.get(arg_idx).copied().flatten()?;
            let param = info.params.get(param_idx)?;
            let arg_value = match &arg.kind {
                ast::ExprKind::NamedArg { value, .. } => value.as_ref(),
                _ => arg,
            };
            let param_ty = param
                .ty_ref
                .as_ref()
                .map(|t| self.lower_type_ref(t))
                .unwrap_or(self.builtins.any);
            let expected = ExpectedExpr {
                value_ty: Some(param_ty),
                array_lit_target: param
                    .ty_ref
                    .as_ref()
                    .and_then(|t| self.array_lit_target_from_type_ref(t)),
                array_lit_ty: Some(param_ty),
                struct_lit_ty: Some(param_ty),
            };
            let init = self.lower_expr_with_expected(pkg_prefix, arg_value, expected);
            let id = self.intern_local_symbol(param.decl_span, false);
            let decl = ValDecl {
                span: call_span,
                id: Some(id),
                name: Some(param.name.clone()),
                mutable: false,
                ty: param_ty,
                init: Some(init),
            };
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(decl),
            });
        }

        for (param_idx, param) in info.params.iter().enumerate() {
            if param_to_arg.get(param_idx)?.is_some() {
                continue;
            }
            let default_value = param.default_value.as_ref()?;
            let expected = ExpectedExpr {
                value_ty: param.ty_ref.as_ref().map(|t| self.lower_type_ref(t)),
                array_lit_target: param
                    .ty_ref
                    .as_ref()
                    .and_then(|t| self.array_lit_target_from_type_ref(t)),
                array_lit_ty: param.ty_ref.as_ref().map(|t| self.lower_type_ref(t)),
                struct_lit_ty: None,
            };
            let init = self.lower_expr_with_expected(pkg_prefix, default_value, expected);
            let param_ty = param
                .ty_ref
                .as_ref()
                .map(|t| self.lower_type_ref(t))
                .unwrap_or(self.builtins.any);
            let id = self.intern_local_symbol(param.decl_span, false);
            let decl = ValDecl {
                span: call_span,
                id: Some(id),
                name: Some(param.name.clone()),
                mutable: false,
                ty: param_ty,
                init: Some(init),
            };
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(decl),
            });
        }

        // 最后一条语句：调用“完整参数形态”的原函数。
        let callee_id = self.symbols.intern_top_level(fqn.clone());
        let callee_expr = Expr {
            span: callee.span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: callee_id,
                fqn: fqn.clone(),
            }),
        };

        let mut full_args: Vec<CallArg> = Vec::with_capacity(total);
        for param in &info.params {
            let id = self.intern_local_symbol(param.decl_span, false);
            let vref = ValueRef::Local {
                id,
                name: param.name.clone(),
                decl_span: param.decl_span,
            };
            full_args.push(CallArg::Positional(Expr {
                span: param.decl_span,
                ty: self.builtins.any,
                kind: ExprKind::VarRef(vref),
            }));
        }

        let call_expr = Expr {
            span: call_span,
            ty: call_ty,
            kind: ExprKind::Call {
                callee: Box::new(callee_expr),
                args: full_args,
            },
        };
        stmts.push(Stmt {
            span: call_span,
            ty: call_expr.ty,
            kind: StmtKind::Expr(call_expr),
        });

        Some((
            ExprKind::Block(Block {
                span: call_span,
                ty: call_ty,
                stmts,
            }),
            call_ty,
        ))
    }

    fn is_integer_type(&self, ty: TypeId) -> bool {
        if ty == self.builtins.int || ty == self.builtins.uint {
            return true;
        }

        matches!(
            self.types.kind(ty),
            TypeKind::Value(ValueTypeKind::IntN(_) | ValueTypeKind::UIntN(_))
        )
    }

    fn is_char_type(&self, ty: TypeId) -> bool {
        ty == self.builtins.char_
    }

    /// 对齐 typecheck 阶段的最小规则：整数二元运算要求“相同的整数类型”，但允许一侧是整数字面量。
    ///
    /// 说明：HIR lowering 目前仅用于 dump/fixtures 与早期 codegen，因此这里的规则只覆盖：
    /// - 算术/位运算：`T op T -> T`（一侧为 int literal 时可吸收为另一侧的整数类型）
    /// - 移位：`T << Int -> T` / `T >> Int -> T`
    /// - 比较：`T < T -> Bool` 等
    /// - 相等：`T == T -> Bool` / `Bool == Bool -> Bool`
    fn lower_binary_expr_type(&self, lhs: &Expr, rhs: &Expr, op: ast::BinaryOp) -> TypeId {
        let unify_int_same_type = |lhs: &Expr, rhs: &Expr| -> Option<TypeId> {
            if lhs.ty == rhs.ty && self.is_integer_type(lhs.ty) {
                return Some(lhs.ty);
            }

            let lhs_is_int_lit = matches!(lhs.kind, ExprKind::Literal(LiteralKind::Int));
            let rhs_is_int_lit = matches!(rhs.kind, ExprKind::Literal(LiteralKind::Int));

            if lhs_is_int_lit && self.is_integer_type(rhs.ty) {
                return Some(rhs.ty);
            }
            if rhs_is_int_lit && self.is_integer_type(lhs.ty) {
                return Some(lhs.ty);
            }

            None
        };

        match op {
            // arithmetic + bitwise: T op T -> T
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::Mul
            | ast::BinaryOp::Div
            | ast::BinaryOp::Rem
            | ast::BinaryOp::BitAnd
            | ast::BinaryOp::BitXor
            | ast::BinaryOp::BitOr => unify_int_same_type(lhs, rhs).unwrap_or(self.builtins.any),

            // shifts: T << Int -> T
            ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
                if self.is_integer_type(lhs.ty) && rhs.ty == self.builtins.int {
                    lhs.ty
                } else {
                    self.builtins.any
                }
            }

            // comparisons: T < T -> Bool
            ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                if unify_int_same_type(lhs, rhs).is_some()
                    || (self.is_char_type(lhs.ty) && self.is_char_type(rhs.ty))
                {
                    self.builtins.bool_
                } else {
                    self.builtins.any
                }
            }

            // equality: (T == T) -> Bool; (Bool == Bool) -> Bool; (Char == Char) -> Bool
            ast::BinaryOp::Eq | ast::BinaryOp::Ne => {
                if lhs.ty == self.builtins.bool_ && rhs.ty == self.builtins.bool_ {
                    return self.builtins.bool_;
                }
                if self.is_char_type(lhs.ty) && self.is_char_type(rhs.ty) {
                    return self.builtins.bool_;
                }
                if unify_int_same_type(lhs, rhs).is_some() {
                    return self.builtins.bool_;
                }
                self.builtins.any
            }

            // boolean logic: Bool op Bool -> Bool
            ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
                if lhs.ty == self.builtins.bool_ && rhs.ty == self.builtins.bool_ {
                    self.builtins.bool_
                } else {
                    self.builtins.any
                }
            }

            // range/progression：正常路径会在 lowering 早期被展开为 `rangeTo(...)` 调用；
            // 这里保留 `Any` fallback，避免在无 typecheck 上下文的 dump-hir 路径里引入额外 interning 约束。
            ast::BinaryOp::RangeInclusive => self.builtins.any,

            // elvis not lowered in current HIR dump mode
            ast::BinaryOp::Elvis => self.builtins.any,
        }
    }
}
