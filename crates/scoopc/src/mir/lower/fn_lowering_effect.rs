//! FnLowering effect/handle/resume/closure/if/pattern/when lowering.

#![allow(dead_code)]

use super::*;

impl<'a> FnLowering<'a> {
    pub(in crate::mir::lower) fn call_result_ty_from_callee(
        &self,
        span: Span,
        callee: &hir::Expr,
    ) -> Option<TypeId> {
        if let Some(binding) = self
            .facts
            .top_level_fun_call_binding(self.source_path.as_path(), span)
            .filter(|binding| top_level_binding_matches_callee(binding, callee))
            && let Some(return_ty) = self.top_level_fun_return_tys.get(&binding.fqn)
        {
            return Some(*return_ty);
        }
        match self.types.kind(callee.ty) {
            TypeKind::Ref(RefTypeKind::Function(fun)) => Some(fun.return_ty),
            _ => None,
        }
    }

    pub(in crate::mir::lower) fn lower_resume_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        resume_info: &ResumeCallInfo,
    ) {
        let Some(receiver) = self.resume_receiver_from_contract(callee, args, resume_info) else {
            self.lower_malformed_resume_call(span, result, resume_info.metadata.clone());
            return;
        };
        let Some(payload_args) = self.resume_payload_args_from_contract(args, resume_info) else {
            self.lower_malformed_resume_call(span, result, resume_info.metadata.clone());
            return;
        };

        let continuation_local = self.lower_expr_to_local(receiver);
        if self.current_is_terminated() {
            return;
        }

        let Some(args) = self.lower_call_args(&payload_args) else {
            return;
        };
        let continuation_ty = self.body.locals[continuation_local.as_u32() as usize].ty;
        let mut resume = resume_info.metadata.clone();
        resume.continuation_ty = continuation_ty;
        let site_id = self.fresh_site_id();
        let kind = CallKind::Resume {
            continuation: Operand::Local(continuation_local),
            resume,
        };
        let transport = self.call_transport_metadata(
            self.body.locals[result.as_u32() as usize].ty,
            &kind,
            &args,
            None,
        );
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
    }

    pub(in crate::mir::lower) fn resume_receiver_from_contract<'b>(
        &self,
        callee: &'b hir::Expr,
        args: &'b [hir::CallArg],
        info: &ResumeCallInfo,
    ) -> Option<&'b hir::Expr> {
        match info.receiver_route {
            ContinuationResumeReceiverRoute::CallArg { index } => {
                args.get(index).map(call_arg_expr)
            }
            ContinuationResumeReceiverRoute::MemberReceiver => match &callee.kind {
                hir::ExprKind::MemberAccess { receiver, .. } => Some(receiver.as_ref()),
                _ => None,
            },
        }
    }

    pub(in crate::mir::lower) fn resume_payload_args_from_contract(
        &self,
        args: &[hir::CallArg],
        info: &ResumeCallInfo,
    ) -> Option<Vec<hir::CallArg>> {
        info.payload_arg_indices
            .iter()
            .map(|index| args.get(*index).cloned())
            .collect()
    }

    pub(in crate::mir::lower) fn lower_malformed_resume_call(
        &mut self,
        span: Span,
        result: LocalId,
        mut metadata: ResumeMetadata,
    ) {
        metadata.runtime_error_effect_ty = None;
        let site_id = self.fresh_site_id();
        let kind = CallKind::Resume {
            continuation: Operand::Const(ConstValue::Unit),
            resume: metadata,
        };
        let args = Vec::new();
        let transport = self.call_transport_metadata(
            self.body.locals[result.as_u32() as usize].ty,
            &kind,
            &args,
            None,
        );
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
    }

    pub(in crate::mir::lower) fn capture_box_ty(&mut self, inner: TypeId) -> TypeId {
        self.types
            .intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
                fqn: CAPTURE_BOX_FQN.to_string(),
                args: vec![inner],
                eff: None,
            })))
    }

    /// 降低一个 effect operation 调用（HIR `Perform`）到 MIR。
    ///
    /// 当前阶段会把 `perform` 同时显式化为：
    /// - 普通恢复后的 continuation block（`resume_target`）；
    /// - 若 outward propagation 需要先跑 cleanup，则通过 `UnwindAction::Cleanup` 连到 cleanup block；
    /// - 若当前无本地 cleanup，则用 `UnwindAction::Propagate` 明确表示“直接继续向外 unwind”。
    pub(in crate::mir::lower) fn lower_perform_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        effect_ty: TypeId,
        op: &hir::EffectOpRef,
        args: &[hir::CallArg],
    ) -> LocalId {
        let Some(lowered_args) = self.lower_call_args(args) else {
            return self.push_temp_local(span, ty);
        };

        if self.current_is_terminated() {
            // 实参 lowering 提前终止了 CFG：该 perform 永远不会发生。
            return self.push_temp_local(span, ty);
        }

        let Some((perform_args, mut metadata)) =
            self.canonicalize_perform_args(span, ty, lowered_args)
        else {
            let result = self.push_temp_local(span, ty);
            self.assign(
                span,
                result,
                Rvalue::PerformResult {
                    op_fqn: op.fqn.clone(),
                    effect_ty,
                },
            );
            let perform_args = Vec::new();
            let resume_target = self.push_block(span);
            let site_id = self.fresh_site_id();
            let unwind = self.build_perform_unwind_action(span);
            self.set_terminator_with_unwind(
                self.current_bb,
                span,
                TerminatorKind::Perform {
                    site_id,
                    op_fqn: String::new(),
                    metadata: PerformMetadata {
                        effect_ty,
                        op_type_args: op.type_args.clone(),
                        result_ty: ty,
                        payload_tuple_ty: None,
                        payload_component_tys: Vec::new(),
                        payload_transport: Vec::new(),
                        arg_mapping: Vec::new(),
                    },
                    args: perform_args,
                    resume_target,
                },
                unwind,
            );
            self.current_bb = resume_target;
            return result;
        };
        metadata.effect_ty = effect_ty;
        if metadata.op_type_args.is_empty() {
            metadata.op_type_args = op.type_args.clone();
        }

        let result = self.push_temp_local(span, ty);
        self.assign(
            span,
            result,
            Rvalue::PerformResult {
                op_fqn: op.fqn.clone(),
                effect_ty,
            },
        );

        let resume_target = self.push_block(span);
        let site_id = self.fresh_site_id();
        let unwind = self.build_perform_unwind_action(span);
        self.set_terminator_with_unwind(
            self.current_bb,
            span,
            TerminatorKind::Perform {
                site_id,
                op_fqn: op.fqn.clone(),
                metadata,
                args: perform_args,
                resume_target,
            },
            unwind,
        );
        self.current_bb = resume_target;

        result
    }

    /// 降低一个 effect handler 表达式（HIR `Handle`）到 MIR。
    ///
    /// 当前阶段会把 `handle` 显式展开为 direct-style CFG：
    /// - 入口 block 以 `TerminatorKind::Handle` 指向 body/arms/finally/exit；
    /// - body 与 arm 正常完成后显式写回结果并跳向 `finally`/`exit_target`；
    /// - `finally` 自身作为 cleanup block 存在，`return` / `break` / `continue` 通过 cleanup chain
    ///   穿过它，而不是把这些续点留成 `Todo(...)`。
    pub(in crate::mir::lower) fn lower_handle_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        handle: &hir::HandleExpr,
    ) -> LocalId {
        let outer_bb = self.current_bb;

        let result = self.push_temp_local(span, ty);
        let handle_site = self
            .facts
            .handle_site_info(self.source_path.as_path(), span)
            .cloned();
        let handle_contract = if let Some(site) = handle_site {
            Some((site.metadata, site.arms))
        } else if self.facts.uses_typed_contracts() {
            None
        } else {
            Some(self.lower_handle_contract_from_hir(ty, handle))
        };
        let Some((metadata, mut arms)) = handle_contract else {
            let body_bb = self.push_block(handle.body.span);
            let exit_bb = self.push_block(span);
            let site_id = self.fresh_site_id();
            self.set_terminator(
                outer_bb,
                span,
                TerminatorKind::Handle {
                    site_id,
                    metadata: HandleMetadata {
                        result_ty: ty,
                        body_result_ty: handle.body.ty,
                        finally_result_ty: Some(ty),
                    },
                    arms: Vec::new(),
                    has_finally: false,
                    body_target: body_bb,
                    arm_targets: Vec::new(),
                    finally_target: None,
                    exit_target: exit_bb,
                },
            );
            self.current_bb = body_bb;
            self.set_terminator(body_bb, span, TerminatorKind::Goto { target: exit_bb });
            self.current_bb = exit_bb;
            return result;
        };
        for (hir_arm, lowered_arm) in handle.arms.iter().zip(arms.iter_mut()) {
            if lowered_arm.op_type_args.is_empty() {
                lowered_arm.op_type_args = hir_arm.op.op.type_args.clone();
            }
        }
        for (hir_arm, lowered_arm) in handle.arms.iter().zip(arms.iter_mut()) {
            self.allocate_handle_arm_locals(hir_arm, lowered_arm);
        }

        let body_bb = self.push_block(handle.body.span);
        let arm_bbs = handle
            .arms
            .iter()
            .map(|arm| self.push_block(arm.span))
            .collect::<Vec<_>>();
        let finally_bb = handle
            .finally
            .as_ref()
            .map(|finally| self.push_cleanup_block(finally.span));
        let exit_bb = self.push_block(span);

        let site_id = self.fresh_site_id();
        self.set_terminator(
            outer_bb,
            span,
            TerminatorKind::Handle {
                site_id,
                metadata,
                arms: arms.clone(),
                has_finally: handle.finally.is_some(),
                body_target: body_bb,
                arm_targets: arm_bbs.clone(),
                finally_target: finally_bb,
                exit_target: exit_bb,
            },
        );

        let handle_cleanup_scope = handle
            .finally
            .as_ref()
            .cloned()
            .map(|finally| CleanupScope { finally });

        self.current_bb = body_bb;
        if let Some(scope) = handle_cleanup_scope.clone() {
            self.cleanup_scopes.push(scope);
        }
        let body_value = self.lower_block_as_expr(&handle.body);
        if handle_cleanup_scope.is_some() {
            let _ = self.cleanup_scopes.pop();
        }
        if !self.current_is_terminated() {
            self.assign_use_to_local(handle.body.span, result, Operand::Local(body_value));
            self.set_terminator(
                self.current_bb,
                handle.body.span,
                TerminatorKind::Goto {
                    target: finally_bb.unwrap_or(exit_bb),
                },
            );
        }

        for ((arm, lowered_arm), arm_bb) in handle.arms.iter().zip(arms.iter()).zip(arm_bbs) {
            self.current_bb = arm_bb;
            if let Some(scope) = handle_cleanup_scope.clone() {
                self.cleanup_scopes.push(scope);
            }
            let shadowed = self.bind_handle_arm_symbols(arm, lowered_arm);
            let arm_value = self.lower_expr_to_local(&arm.body);
            if handle_cleanup_scope.is_some() {
                let _ = self.cleanup_scopes.pop();
            }
            self.restore_shadowed_symbols(shadowed);
            if !self.current_is_terminated() {
                self.assign_use_to_local(arm.span, result, Operand::Local(arm_value));
                self.set_terminator(
                    self.current_bb,
                    arm.span,
                    TerminatorKind::Goto {
                        target: finally_bb.unwrap_or(exit_bb),
                    },
                );
            }
        }

        if let Some((finally, finally_bb)) = handle.finally.as_ref().zip(finally_bb) {
            self.lower_cleanup_block_to_target(
                finally_bb,
                finally,
                exit_bb,
                self.cleanup_scopes.len(),
            );
        }

        self.current_bb = exit_bb;

        result
    }

    pub(in crate::mir::lower) fn lower_handle_contract_from_hir(
        &mut self,
        result_ty: TypeId,
        handle: &hir::HandleExpr,
    ) -> (HandleMetadata, Vec<HandlerArm>) {
        let arms = handle
            .arms
            .iter()
            .map(|arm| {
                let payload_component_tys = arm
                    .op
                    .binders
                    .iter()
                    .map(|binder| binder.ty)
                    .collect::<Vec<_>>();
                let payload_tuple_ty = payload_tuple_ty_from_components(
                    self.types,
                    self.builtins.unit,
                    &payload_component_tys,
                );
                HandlerArm {
                    op_fqn: arm.op.op.fqn.clone(),
                    op_type_args: arm.op.op.type_args.clone(),
                    binder_count: arm.op.binders.len(),
                    handled_effect_ty: arm.op.effect_ty,
                    payload_tuple_ty,
                    binder_locals: Vec::new(),
                    continuation_local: None,
                    payload_component_tys,
                    body_ty: arm.body.ty,
                    kind: match arm.kind {
                        hir::HandleArmKind::NonResuming => HandlerArmKind::NonResuming,
                        hir::HandleArmKind::EscapeContinuation { .. } => {
                            HandlerArmKind::EscapeContinuation
                        }
                    },
                }
            })
            .collect();
        (
            HandleMetadata {
                result_ty,
                body_result_ty: handle.body.ty,
                finally_result_ty: handle.finally.as_ref().map(|finally| finally.ty),
            },
            arms,
        )
    }

    pub(in crate::mir::lower) fn allocate_handle_arm_locals(
        &mut self,
        arm: &hir::HandleArm,
        lowered_arm: &mut HandlerArm,
    ) {
        lowered_arm.binder_locals = arm
            .op
            .binders
            .iter()
            .map(|binder| self.push_named_local(binder.span, &binder.name, binder.ty))
            .collect();
        lowered_arm.binder_count = lowered_arm.binder_locals.len();
        lowered_arm.continuation_local = match arm.kind {
            hir::HandleArmKind::EscapeContinuation { continuation } => {
                let ty = self
                    .infer_local_symbol_ty_in_expr(&arm.body, continuation)
                    .unwrap_or(self.builtins.any);
                Some(self.push_named_local(arm.span, "$continuation", ty))
            }
            hir::HandleArmKind::NonResuming => None,
        };
    }

    pub(in crate::mir::lower) fn bind_handle_arm_symbols(
        &mut self,
        arm: &hir::HandleArm,
        lowered_arm: &HandlerArm,
    ) -> Vec<(hir::SymbolId, Option<LocalId>)> {
        let mut shadowed = Vec::with_capacity(
            lowered_arm.binder_locals.len() + usize::from(lowered_arm.continuation_local.is_some()),
        );
        for (binder, local) in arm
            .op
            .binders
            .iter()
            .zip(lowered_arm.binder_locals.iter().copied())
        {
            let previous = self.symbol_locals.insert(binder.id, local);
            shadowed.push((binder.id, previous));
        }
        if let hir::HandleArmKind::EscapeContinuation { continuation } = arm.kind
            && let Some(local) = lowered_arm.continuation_local
        {
            let previous = self.symbol_locals.insert(continuation, local);
            shadowed.push((continuation, previous));
        }
        shadowed
    }

    pub(in crate::mir::lower) fn infer_local_symbol_ty_in_expr(
        &self,
        expr: &hir::Expr,
        symbol: hir::SymbolId,
    ) -> Option<TypeId> {
        match &expr.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) if *id == symbol => {
                Some(expr.ty)
            }
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Todo(_) => None,
            hir::ExprKind::StructLit { fields, .. } => fields
                .iter()
                .find_map(|field| self.infer_local_symbol_ty_in_expr(&field.value, symbol)),
            hir::ExprKind::TupleLit { elements } => elements
                .iter()
                .find_map(|element| self.infer_local_symbol_ty_in_expr(element, symbol)),
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|part| {
                let hir::InterpolatedStringPart::Expr { expr } = part else {
                    return None;
                };
                self.infer_local_symbol_ty_in_expr(expr, symbol)
            }),
            hir::ExprKind::Unary { expr, .. }
            | hir::ExprKind::TypeCheck { expr, .. }
            | hir::ExprKind::Cast { expr, .. }
            | hir::ExprKind::MemberAccess { receiver: expr, .. } => {
                self.infer_local_symbol_ty_in_expr(expr, symbol)
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => self
                .infer_local_symbol_ty_in_expr(lhs, symbol)
                .or_else(|| self.infer_local_symbol_ty_in_expr(rhs, symbol)),
            hir::ExprKind::Block(block) => block
                .stmts
                .iter()
                .find_map(|stmt| self.infer_local_symbol_ty_in_stmt(stmt, symbol)),
            hir::ExprKind::Call { callee, args } => self
                .infer_local_symbol_ty_in_expr(callee, symbol)
                .or_else(|| {
                    args.iter().find_map(|arg| match arg {
                        hir::CallArg::Positional(expr) => {
                            self.infer_local_symbol_ty_in_expr(expr, symbol)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.infer_local_symbol_ty_in_expr(value, symbol)
                        }
                    })
                }),
            hir::ExprKind::Closure(closure) => {
                self.infer_local_symbol_ty_in_expr(&closure.body, symbol)
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self
                .infer_local_symbol_ty_in_expr(cond, symbol)
                .or_else(|| self.infer_local_symbol_ty_in_expr(then_branch, symbol))
                .or_else(|| {
                    else_branch
                        .as_ref()
                        .and_then(|expr| self.infer_local_symbol_ty_in_expr(expr, symbol))
                }),
            hir::ExprKind::When { subject, arms } => self
                .infer_local_symbol_ty_in_expr(subject, symbol)
                .or_else(|| {
                    arms.iter().find_map(|arm| {
                        arm.guard
                            .as_ref()
                            .and_then(|guard| self.infer_local_symbol_ty_in_expr(guard, symbol))
                            .or_else(|| self.infer_local_symbol_ty_in_expr(&arm.body, symbol))
                    })
                }),
            hir::ExprKind::Perform { args, .. } => args.iter().find_map(|arg| match arg {
                hir::CallArg::Positional(expr) => self.infer_local_symbol_ty_in_expr(expr, symbol),
                hir::CallArg::Named { value, .. } => {
                    self.infer_local_symbol_ty_in_expr(value, symbol)
                }
            }),
            hir::ExprKind::Handle(handle) => self
                .infer_local_symbol_ty_in_block(&handle.body, symbol)
                .or_else(|| {
                    handle
                        .arms
                        .iter()
                        .find_map(|arm| self.infer_local_symbol_ty_in_expr(&arm.body, symbol))
                })
                .or_else(|| {
                    handle
                        .finally
                        .as_ref()
                        .and_then(|block| self.infer_local_symbol_ty_in_block(block, symbol))
                }),
        }
    }

    pub(in crate::mir::lower) fn infer_local_symbol_ty_in_block(
        &self,
        block: &hir::Block,
        symbol: hir::SymbolId,
    ) -> Option<TypeId> {
        block
            .stmts
            .iter()
            .find_map(|stmt| self.infer_local_symbol_ty_in_stmt(stmt, symbol))
    }

    pub(in crate::mir::lower) fn infer_local_symbol_ty_in_stmt(
        &self,
        stmt: &hir::Stmt,
        symbol: hir::SymbolId,
    ) -> Option<TypeId> {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => None,
            hir::StmtKind::Expr(expr) => self.infer_local_symbol_ty_in_expr(expr, symbol),
            hir::StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .and_then(|expr| self.infer_local_symbol_ty_in_expr(expr, symbol)),
            hir::StmtKind::Assign { lhs, rhs, .. } => self
                .infer_local_symbol_ty_in_expr(lhs, symbol)
                .or_else(|| self.infer_local_symbol_ty_in_expr(rhs, symbol)),
            hir::StmtKind::While { cond, body } => self
                .infer_local_symbol_ty_in_expr(cond, symbol)
                .or_else(|| self.infer_local_symbol_ty_in_block(body, symbol)),
            hir::StmtKind::Return { value } => value
                .as_ref()
                .and_then(|expr| self.infer_local_symbol_ty_in_expr(expr, symbol)),
        }
    }

    /// 降低字面量：把常量写入一个临时 local。
    pub(in crate::mir::lower) fn lower_literal(
        &mut self,
        span: Span,
        ty: TypeId,
        lit: &hir::LiteralKind,
    ) -> LocalId {
        let tmp = self.push_temp_local(span, ty);
        let c = match lit {
            hir::LiteralKind::Bool(b) => ConstValue::Bool(*b),
            hir::LiteralKind::Char(_) => ConstValue::Char,
            hir::LiteralKind::Unit => ConstValue::Unit,
            hir::LiteralKind::Int => ConstValue::Int,
            hir::LiteralKind::SynthInt(value) => ConstValue::SynthInt(*value),
            hir::LiteralKind::Float64(_) => ConstValue::Float64,
            hir::LiteralKind::Float32(_) => ConstValue::Float32,
            hir::LiteralKind::String => ConstValue::String,
            hir::LiteralKind::SynthString(value) => ConstValue::SynthString(value.clone()),
        };
        self.assign(span, tmp, Rvalue::Use(Operand::Const(c)));
        tmp
    }

    pub(in crate::mir::lower) fn lower_class_literal_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        class_lit: &hir::ClassLiteralExpr,
    ) -> LocalId {
        let tmp = self.push_temp_local(span, ty);
        let kind = match class_lit.metadata_kind {
            hir::TypeMetadataLiteralKind::TypeNameString => TypeMetadataLiteralKind::TypeNameString,
        };
        self.assign(
            span,
            tmp,
            Rvalue::TypeMetadataLiteral(TypeMetadataLiteral {
                source_ty: class_lit.source_ty,
                source_fqn: class_lit.source_fqn.clone(),
                kind,
            }),
        );
        tmp
    }

    pub(in crate::mir::lower) fn try_lower_compare_to_binary_expr(
        &mut self,
        span: Span,
        result_ty: TypeId,
        lhs: &hir::Expr,
        op: ast::BinaryOp,
        rhs: &hir::Expr,
    ) -> Option<LocalId> {
        let binding = self
            .facts
            .top_level_fun_call_binding(self.source_path.as_path(), span)?;
        let result = self.push_temp_local(span, result_ty);

        let (compare_lhs, compare_rhs) = self
            .already_lowered_compare_to_args(lhs, rhs, binding.fqn.as_str())
            .unwrap_or((lhs, rhs));
        let lhs_local = self.lower_expr_to_local(compare_lhs);
        if self.current_is_terminated() {
            return Some(result);
        }
        let rhs_local = self.lower_expr_to_local(compare_rhs);
        if self.current_is_terminated() {
            return Some(result);
        }

        let compare_result = self.push_temp_local(span, self.builtins.int);
        let compare_kind = CallKind::Direct {
            callee_fqn: binding.fqn.clone(),
        };
        let compare_args = vec![
            CallArg {
                span: compare_lhs.span,
                name: None,
                value: Operand::Local(lhs_local),
            },
            CallArg {
                span: compare_rhs.span,
                name: None,
                value: Operand::Local(rhs_local),
            },
        ];
        let compare_transport =
            self.call_transport_metadata(self.builtins.int, &compare_kind, &compare_args, None);
        let compare_site_id = self.fresh_site_id();
        self.assign(
            span,
            compare_result,
            Rvalue::Call {
                site_id: compare_site_id,
                kind: compare_kind,
                args: compare_args,
                transport: compare_transport,
            },
        );

        let zero = self.push_temp_local(span, self.builtins.int);
        self.assign(
            span,
            zero,
            Rvalue::Use(Operand::Const(ConstValue::SynthInt(0))),
        );
        self.assign_int_compare_method_call(
            span,
            result,
            op,
            Operand::Local(compare_result),
            Operand::Local(zero),
        );
        Some(result)
    }

    pub(in crate::mir::lower) fn try_lower_string_equality_binary_expr(
        &mut self,
        span: Span,
        result_ty: TypeId,
        lhs: &hir::Expr,
        op: ast::BinaryOp,
        rhs: &hir::Expr,
    ) -> Option<LocalId> {
        if lhs.ty != self.builtins.string || rhs.ty != self.builtins.string {
            return None;
        }
        let result = self.push_temp_local(span, result_ty);
        let lhs_local = self.lower_expr_to_local(lhs);
        if self.current_is_terminated() {
            return Some(result);
        }
        let rhs_local = self.lower_expr_to_local(rhs);
        if self.current_is_terminated() {
            return Some(result);
        }

        let compare_result = self.push_temp_local(span, self.builtins.int);
        let compare_kind = CallKind::Direct {
            callee_fqn: "scoop.core.String.compareTo".to_string(),
        };
        let compare_args = vec![
            CallArg {
                span: lhs.span,
                name: None,
                value: Operand::Local(lhs_local),
            },
            CallArg {
                span: rhs.span,
                name: None,
                value: Operand::Local(rhs_local),
            },
        ];
        let compare_transport =
            self.call_transport_metadata(self.builtins.int, &compare_kind, &compare_args, None);
        let compare_site_id = self.fresh_site_id();
        self.assign(
            span,
            compare_result,
            Rvalue::Call {
                site_id: compare_site_id,
                kind: compare_kind,
                args: compare_args,
                transport: compare_transport,
            },
        );

        let zero = self.push_temp_local(span, self.builtins.int);
        self.assign(
            span,
            zero,
            Rvalue::Use(Operand::Const(ConstValue::SynthInt(0))),
        );
        self.assign_int_compare_method_call(
            span,
            result,
            op,
            Operand::Local(compare_result),
            Operand::Local(zero),
        );
        Some(result)
    }

    pub(in crate::mir::lower) fn try_lower_scalar_binary_method_expr(
        &mut self,
        span: Span,
        result_ty: TypeId,
        lhs: &hir::Expr,
        op: ast::BinaryOp,
        rhs: &hir::Expr,
    ) -> Option<LocalId> {
        let method = Self::scalar_binary_operator_method(op)?;
        let result = self.push_temp_local(span, result_ty);
        let lhs_local = self.lower_expr_to_local(lhs);
        if self.current_is_terminated() {
            return Some(result);
        }
        let rhs_local = self.lower_expr_to_local(rhs);
        if self.current_is_terminated() {
            return Some(result);
        }
        let owner_fqn = self
            .scalar_operator_owner_fqn_for_expr(lhs)
            .or_else(|| self.scalar_operator_owner_fqn_for_local(lhs_local))
            .or_else(|| self.scalar_operator_owner_fqn_for_expr(rhs))
            .or_else(|| self.scalar_operator_owner_fqn_for_local(rhs_local));
        let Some(owner_fqn) = owner_fqn else {
            self.assign(span, result, Rvalue::Todo("missing expr"));
            return Some(result);
        };

        let kind = CallKind::Direct {
            callee_fqn: format!("{owner_fqn}.{method}"),
        };
        let args = vec![
            CallArg {
                span: lhs.span,
                name: None,
                value: Operand::Local(lhs_local),
            },
            CallArg {
                span: rhs.span,
                name: None,
                value: Operand::Local(rhs_local),
            },
        ];
        let transport = self.call_transport_metadata(result_ty, &kind, &args, None);
        let site_id = self.fresh_site_id();
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
        Some(result)
    }

    pub(in crate::mir::lower) fn try_lower_scalar_unary_method_expr(
        &mut self,
        span: Span,
        result_ty: TypeId,
        op: ast::UnaryOp,
        operand: &hir::Expr,
    ) -> Option<LocalId> {
        let method = Self::scalar_unary_operator_method(op)?;
        let result = self.push_temp_local(span, result_ty);
        let operand_local = self.lower_expr_to_local(operand);
        if self.current_is_terminated() {
            return Some(result);
        }
        let Some(owner_fqn) = self
            .scalar_operator_owner_fqn_for_expr(operand)
            .or_else(|| self.scalar_operator_owner_fqn_for_local(operand_local))
        else {
            self.assign(span, result, Rvalue::Todo("missing expr"));
            return Some(result);
        };
        let kind = CallKind::Direct {
            callee_fqn: format!("{owner_fqn}.{method}"),
        };
        let args = vec![CallArg {
            span: operand.span,
            name: None,
            value: Operand::Local(operand_local),
        }];
        let transport = self.call_transport_metadata(result_ty, &kind, &args, None);
        let site_id = self.fresh_site_id();
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
        Some(result)
    }

    fn scalar_operator_owner_fqn(&self, ty: TypeId) -> Option<String> {
        match self.types.kind(ty) {
            TypeKind::Value(ValueTypeKind::Bool) => Some("scoop.core.Bool".to_string()),
            TypeKind::Value(ValueTypeKind::Char) => Some("scoop.core.Char".to_string()),
            TypeKind::Value(ValueTypeKind::Float64) => Some("scoop.core.Float64".to_string()),
            TypeKind::Value(ValueTypeKind::Float32) => Some("scoop.core.Float32".to_string()),
            TypeKind::Value(ValueTypeKind::Int) => Some("scoop.core.Int".to_string()),
            TypeKind::Value(ValueTypeKind::UInt) => Some("scoop.core.UInt".to_string()),
            TypeKind::Value(ValueTypeKind::IntN(bits)) => Some(format!("scoop.core.Int{bits}")),
            TypeKind::Value(ValueTypeKind::UIntN(bits)) => Some(format!("scoop.core.UInt{bits}")),
            _ => None,
        }
    }

    fn scalar_operator_owner_fqn_for_expr(&self, expr: &hir::Expr) -> Option<String> {
        if let Some(owner_fqn) = self.scalar_operator_owner_fqn(expr.ty) {
            return Some(owner_fqn);
        }
        if let hir::ExprKind::Call { callee, .. } = &expr.kind
            && let Some(result_ty) = self.call_result_ty_from_callee(expr.span, callee)
        {
            return self.scalar_operator_owner_fqn(result_ty);
        }
        let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &expr.kind else {
            return None;
        };
        let local = self.symbol_locals.get(id)?;
        let local_ty = self.body.locals.get(local.as_u32() as usize)?.ty;
        self.scalar_operator_owner_fqn(local_ty)
    }

    fn scalar_operator_owner_fqn_for_local(&self, local: LocalId) -> Option<String> {
        let local_ty = self.body.locals.get(local.as_u32() as usize)?.ty;
        self.scalar_operator_owner_fqn(local_ty)
    }

    fn scalar_binary_operator_method(op: ast::BinaryOp) -> Option<&'static str> {
        match op {
            ast::BinaryOp::Add => Some("plus"),
            ast::BinaryOp::Sub => Some("minus"),
            ast::BinaryOp::Mul => Some("times"),
            ast::BinaryOp::Div => Some("div"),
            ast::BinaryOp::Rem => Some("rem"),
            ast::BinaryOp::BitAnd => Some("and"),
            ast::BinaryOp::BitXor => Some("xor"),
            ast::BinaryOp::BitOr => Some("or"),
            ast::BinaryOp::Shl => Some("shl"),
            ast::BinaryOp::Shr => Some("shr"),
            ast::BinaryOp::Lt => Some("lt"),
            ast::BinaryOp::Le => Some("le"),
            ast::BinaryOp::Gt => Some("gt"),
            ast::BinaryOp::Ge => Some("ge"),
            ast::BinaryOp::Eq => Some("equals"),
            ast::BinaryOp::Ne => Some("notEquals"),
            ast::BinaryOp::LogAnd
            | ast::BinaryOp::LogOr
            | ast::BinaryOp::RangeInclusive
            | ast::BinaryOp::Elvis => None,
        }
    }

    fn scalar_unary_operator_method(op: ast::UnaryOp) -> Option<&'static str> {
        match op {
            ast::UnaryOp::Not => Some("not"),
            ast::UnaryOp::Neg => Some("unaryMinus"),
            ast::UnaryOp::BitNot => Some("inv"),
        }
    }

    fn already_lowered_compare_to_args<'b>(
        &self,
        lhs: &'b hir::Expr,
        rhs: &hir::Expr,
        expected_fqn: &str,
    ) -> Option<(&'b hir::Expr, &'b hir::Expr)> {
        if rhs.ty != self.builtins.int
            || !matches!(
                rhs.kind,
                hir::ExprKind::Literal(hir::LiteralKind::SynthInt(0))
            )
        {
            return None;
        }
        let hir::ExprKind::Call { callee, args } = &lhs.kind else {
            return None;
        };
        if args.len() != 2 {
            return None;
        }
        let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
            return None;
        };
        if fqn != expected_fqn {
            return None;
        }
        Some((call_arg_expr(&args[0]), call_arg_expr(&args[1])))
    }

    fn assign_int_compare_method_call(
        &mut self,
        span: Span,
        result: LocalId,
        op: ast::BinaryOp,
        lhs: Operand,
        rhs: Operand,
    ) {
        let method = match op {
            ast::BinaryOp::Lt => "lt",
            ast::BinaryOp::Le => "le",
            ast::BinaryOp::Gt => "gt",
            ast::BinaryOp::Ge => "ge",
            ast::BinaryOp::Eq => "equals",
            ast::BinaryOp::Ne => "notEquals",
            _ => unreachable!("caller guarantees Int comparison/equality op"),
        };
        let kind = CallKind::Direct {
            callee_fqn: format!("scoop.core.Int.{method}"),
        };
        let args = vec![
            CallArg {
                span,
                name: None,
                value: lhs,
            },
            CallArg {
                span,
                name: None,
                value: rhs,
            },
        ];
        let transport = self.call_transport_metadata(self.builtins.bool_, &kind, &args, None);
        let site_id = self.fresh_site_id();
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
    }

    /// 降低变量引用：
    /// - 普通 local：直接返回其 local；
    /// - 被 capture 的 `var`（box 存储）：生成 `CaptureBoxGet` 并返回读取到的临时值 local；
    /// - 其它引用：降为 `Todo`。
    pub(in crate::mir::lower) fn lower_var_ref(
        &mut self,
        span: Span,
        ty: TypeId,
        v: &hir::ValueRef,
    ) -> LocalId {
        match v {
            hir::ValueRef::Local { id, name, .. } => {
                let local = match self.symbol_locals.get(id).copied() {
                    Some(local) => local,
                    None => {
                        if let Some(member_local) =
                            self.lower_implicit_this_member_ref(span, ty, name)
                        {
                            return member_local;
                        }
                        panic!("typed HIR local reference must have an allocated MIR local: {id:?}")
                    }
                };

                if self.boxed_symbols.contains(id) {
                    let tmp = self.push_temp_local(span, ty);
                    self.assign(
                        span,
                        tmp,
                        Rvalue::CaptureBoxGet {
                            box_operand: Operand::Local(local),
                            contract: self.capture_box_contract(
                                self.body.locals[local.as_u32() as usize].ty,
                                ty,
                            ),
                        },
                    );
                    tmp
                } else {
                    local
                }
            }
            hir::ValueRef::TopLevel { .. } => {
                let hir::ValueRef::TopLevel { fqn, .. } = v else {
                    unreachable!("matched above");
                };
                let tmp = self.push_temp_local(span, ty);
                let hidden_effects = self.facts.top_level_ref_hidden_effects(fqn);
                let site_id = (!hidden_effects.is_pure()).then(|| self.fresh_site_id());
                self.assign(
                    span,
                    tmp,
                    Rvalue::TopLevelRef(TopLevelRef {
                        fqn: fqn.clone(),
                        site_id,
                        hidden_effects,
                    }),
                );
                tmp
            }
        }
    }

    pub(in crate::mir::lower) fn lower_implicit_this_member_ref(
        &mut self,
        span: Span,
        ty: TypeId,
        member_name: &str,
    ) -> Option<LocalId> {
        let this_local = self
            .body
            .locals
            .iter()
            .enumerate()
            .find_map(|(idx, local)| {
                (local.name.as_deref() == Some("this")).then_some(LocalId::from_raw(idx as u32))
            })?;
        let receiver_ty = self.body.locals.get(this_local.as_u32() as usize)?.ty;
        let owner_fqn = self.owner_fqn.rsplit_once('.')?.0.to_string();
        let result = self.push_temp_local(span, ty);
        self.assign(
            span,
            result,
            Rvalue::MemberAccess {
                site_id: None,
                receiver: Operand::Local(this_local),
                member: MemberAccessMetadata {
                    name: member_name.to_string(),
                    receiver_ty,
                    resolved: Some(MemberTarget::Value {
                        fqn: format!("{owner_fqn}.{member_name}"),
                    }),
                    hidden_effects: EffectRow::pure(),
                },
            },
        );
        Some(result)
    }

    pub(in crate::mir::lower) fn lower_closure_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        closure: &hir::ClosureExpr,
    ) -> LocalId {
        let name = format!("$lambda{}", closure.id.as_u32());
        let fqn = format!("{}.{}", self.owner_fqn, name);

        // 1) 计算 capture set，并决定 env 的 tuple 类型。
        let mut captures: Vec<ClosureCaptureLayout> = Vec::new();
        for cap in &closure.captures {
            let Some(source_local) = self.symbol_locals.get(&cap.id).copied() else {
                // 防御性：若当前函数未为该 symbol 分配 local（理论上不应发生），跳过该 capture。
                continue;
            };
            let source_ty = self.body.locals[source_local.as_u32() as usize].ty;
            captures.push(ClosureCaptureLayout {
                id: cap.id,
                name: cap.name.clone(),
                decl_span: cap.decl_span,
                ty: source_ty,
                mutable: cap.mutable,
                source_local,
            });
        }

        let (env_ty, env_operand) = if captures.is_empty() {
            (self.builtins.unit, Operand::Const(ConstValue::Unit))
        } else {
            let env_ty = self.types.ty_tuple(captures.iter().map(|c| c.ty).collect());
            let env_local = self.push_temp_local(span, env_ty);
            self.assign(
                span,
                env_local,
                Rvalue::MakeTuple {
                    elements: captures
                        .iter()
                        .map(|c| Operand::Local(c.source_local))
                        .collect(),
                    transport: self.aggregate_transport(
                        env_ty,
                        AggregateTransportKind::ClosureEnv,
                        captures
                            .iter()
                            .map(|c| (Some(c.name.clone()), c.ty))
                            .collect::<Vec<_>>(),
                    ),
                },
            );
            (env_ty, Operand::Local(env_local))
        };
        let env_contract = self.closure_env_contract(env_ty, &captures);

        let (fun, nested) = {
            let types = &mut *self.types;
            FnLowering::new(
                self.builtins,
                types,
                self.facts,
                self.top_level_fun_return_tys.clone(),
                self.top_level_fun_param_tys.clone(),
                fqn.clone(),
                self.source_path.clone(),
            )
            .lower_closure_fun(fqn.clone(), name, closure, env_ty, &captures)
        };
        self.nested_funs.push(fun);
        self.nested_funs.extend(nested);

        let tmp = self.push_temp_local(span, ty);
        self.assign(
            span,
            tmp,
            Rvalue::MakeClosure {
                env: env_operand,
                fn_ptr: fqn,
                env_contract,
            },
        );
        tmp
    }

    pub(in crate::mir::lower) fn lower_closure_fun(
        mut self,
        closure_fqn: String,
        closure_name: String,
        closure: &hir::ClosureExpr,
        env_ty: TypeId,
        captures: &[ClosureCaptureLayout],
    ) -> (FunDecl, Vec<FunDecl>) {
        self.current_return_ty = closure.body.ty;
        // 0) 预扫描 closure body：本 closure 内部若存在嵌套 closure 捕获 `var`，则需要 box 存储（T0714）。
        self.boxed_symbols = boxed_symbols_in_expr(closure.body.as_ref());

        // 1) 创建入口块。
        let entry = self.push_block(closure.span);
        self.body.start = entry;
        self.current_bb = entry;

        // 2) env + captures + 参数变为 locals。
        let mut params = Vec::with_capacity(closure.params.len() + 1);

        let env_local = self.push_named_local(closure.span, "$env", env_ty);
        params.push(Param {
            span: closure.span,
            name: "$env".to_string(),
            ty: env_ty,
            local: env_local,
        });

        // 把捕获字段从 `$env` 解包到局部 local，并写入 SymbolId → LocalId 映射，使得后续 body lowering
        // 可以像普通局部变量一样引用它们。
        for (idx, cap) in captures.iter().enumerate() {
            let local = self.push_named_local(cap.decl_span, &cap.name, cap.ty);
            self.symbol_locals.insert(cap.id, local);
            if cap.mutable {
                self.boxed_symbols.insert(cap.id);
            }
            self.assign(
                cap.decl_span,
                local,
                Rvalue::TupleGet {
                    tuple: Operand::Local(env_local),
                    index: idx,
                },
            );
        }

        for p in &closure.params {
            let local = self.push_named_local(p.span, &p.name, p.ty);
            self.symbol_locals.insert(p.id, local);
            params.push(Param {
                span: p.span,
                name: p.name.clone(),
                ty: p.ty,
                local,
            });
        }

        // 3) lower lambda body. A closure body is an expression, so its value is the callable
        // result unless the body already terminated through an explicit control-flow edge.
        let body_result = self.lower_expr_to_local(closure.body.as_ref());
        if !self.current_is_terminated() {
            let value =
                self.operand_for_current_return_ty(closure.span, Operand::Local(body_result));
            self.set_terminator(
                self.current_bb,
                closure.span,
                TerminatorKind::Return { value: Some(value) },
            );
        }

        let out = FunDecl {
            span: closure.span,
            fqn: closure_fqn,
            name: closure_name,
            ty: self.builtins.any,
            params,
            return_ty: closure.body.ty,
            body: Some(self.body),
        };

        (out, self.nested_funs)
    }

    /// 降低 `if` 表达式：生成 then/else/merge 基本块，并在 merge 点写回一个临时结果 local。
    pub(in crate::mir::lower) fn lower_if_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        cond: &hir::Expr,
        then_branch: &hir::Expr,
        else_branch: Option<&hir::Expr>,
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);

        // 1) 先在当前块求值条件，并以 CondBr 结束当前块。
        let cond_local = self.lower_expr_to_local(cond);
        let parent = self.current_bb;
        let then_bb = self.push_block(then_branch.span);
        let else_bb = self.push_block(else_branch.map(|e| e.span).unwrap_or(span));
        let merge_bb = self.push_block(span);

        self.set_terminator(
            parent,
            span,
            TerminatorKind::CondBr {
                cond: Operand::Local(cond_local),
                then_target: then_bb,
                else_target: else_bb,
            },
        );

        // 2) then 分支：lower 表达式并写回 result，然后跳到 merge。
        self.current_bb = then_bb;
        let then_value = self.lower_expr_to_local(then_branch);
        if !self.current_is_terminated() {
            self.assign_use_to_local(then_branch.span, result, Operand::Local(then_value));
            self.set_terminator(
                self.current_bb,
                then_branch.span,
                TerminatorKind::Goto { target: merge_bb },
            );
        }

        // 3) else 分支：同上；若缺省 else，则使用 Unit 占位。
        self.current_bb = else_bb;
        let else_value = else_branch
            .map(|e| self.lower_expr_to_local(e))
            .unwrap_or_else(|| self.emit_unit(span));
        if !self.current_is_terminated() {
            self.assign_use_to_local(span, result, Operand::Local(else_value));
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Goto { target: merge_bb },
            );
        }

        // 4) merge：后续语句继续在 merge 块中生成。
        self.current_bb = merge_bb;
        result
    }

    pub(in crate::mir::lower) fn lower_pattern(
        &self,
        pat: &hir::WhenPat,
        subject_ty: TypeId,
    ) -> Pattern {
        match pat {
            hir::WhenPat::Else { .. } => Pattern::Else,
            hir::WhenPat::Or { pats, .. } => Pattern::Or {
                pats: pats
                    .iter()
                    .map(|pat| self.lower_pattern(pat, subject_ty))
                    .collect(),
            },
            hir::WhenPat::Wildcard { .. } => Pattern::Wildcard,
            hir::WhenPat::Rest { .. } => Pattern::Rest,
            hir::WhenPat::Is { ty, .. } => Pattern::Is {
                ty: *ty,
                metadata: self.runtime_pattern_type_test_metadata(subject_ty, *ty),
            },
            hir::WhenPat::Bind { span, name, .. } => Pattern::Bind {
                name: name.clone(),
                ty: self
                    .facts
                    .when_pat_binding_ty(*span)
                    .unwrap_or(self.builtins.any),
            },
            hir::WhenPat::Tuple { elements, .. } => Pattern::Tuple {
                elements: elements
                    .iter()
                    .enumerate()
                    .map(|(index, pat)| {
                        let element_ty = self.tuple_pattern_element_ty(subject_ty, index);
                        self.lower_pattern(pat, element_ty)
                    })
                    .collect(),
            },
            hir::WhenPat::Variant { name, args, .. } => Pattern::Variant {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|pat| self.lower_pattern(pat, self.builtins.any))
                    .collect(),
            },
            hir::WhenPat::IntLit { raw, .. } => Pattern::IntLit { raw: raw.clone() },
            hir::WhenPat::CharLit { value, .. } => Pattern::CharLit { value: *value },
            hir::WhenPat::StringLit { value, .. } => Pattern::StringLit {
                value: value.clone(),
            },
            hir::WhenPat::BoolLit { value, .. } => Pattern::BoolLit { value: *value },
        }
    }

    pub(in crate::mir::lower) fn tuple_pattern_element_ty(
        &self,
        subject_ty: TypeId,
        index: usize,
    ) -> TypeId {
        match self.types.kind(subject_ty) {
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                elements.get(index).copied().unwrap_or(self.builtins.any)
            }
            _ => self.builtins.any,
        }
    }

    pub(in crate::mir::lower) fn when_pat_is_irrefutable(&self, pat: &hir::WhenPat) -> bool {
        matches!(
            pat,
            hir::WhenPat::Else { .. } | hir::WhenPat::Wildcard { .. } | hir::WhenPat::Bind { .. }
        )
    }

    pub(in crate::mir::lower) fn collect_when_pattern_bindings(
        &self,
        pat: &hir::WhenPat,
        path: &mut Vec<PatternBindingStep>,
        out: &mut Vec<WhenPatternBinding>,
    ) {
        match pat {
            hir::WhenPat::Bind { span, id, name } => {
                out.push(WhenPatternBinding {
                    id: *id,
                    span: *span,
                    name: name.clone(),
                    ty: self
                        .facts
                        .when_pat_binding_ty(*span)
                        .unwrap_or(self.builtins.any),
                    path: path.clone(),
                });
            }
            hir::WhenPat::Tuple { elements, .. } => {
                for (index, element) in elements.iter().enumerate() {
                    path.push(PatternBindingStep::TupleIndex(index));
                    self.collect_when_pattern_bindings(element, path, out);
                    let _ = path.pop();
                }
            }
            hir::WhenPat::Variant { name, args, .. } => {
                for (field_index, arg) in args.iter().enumerate() {
                    if matches!(arg, hir::WhenPat::Rest { .. }) {
                        continue;
                    }
                    path.push(PatternBindingStep::VariantField {
                        variant: name.clone(),
                        field_index,
                    });
                    self.collect_when_pattern_bindings(arg, path, out);
                    let _ = path.pop();
                }
            }
            hir::WhenPat::Or { pats, .. } => {
                for pat in pats {
                    self.collect_when_pattern_bindings(pat, path, out);
                }
            }
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Rest { .. }
            | hir::WhenPat::Is { .. }
            | hir::WhenPat::IntLit { .. }
            | hir::WhenPat::CharLit { .. }
            | hir::WhenPat::StringLit { .. }
            | hir::WhenPat::BoolLit { .. } => {}
        }
    }

    pub(in crate::mir::lower) fn bind_when_pattern_locals(
        &mut self,
        subject_local: LocalId,
        pat: &hir::WhenPat,
    ) -> Vec<(hir::SymbolId, Option<LocalId>)> {
        let mut bindings = Vec::new();
        self.collect_when_pattern_bindings(pat, &mut Vec::new(), &mut bindings);

        let mut shadowed = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let local = self.push_named_local(binding.span, &binding.name, binding.ty);
            self.assign(
                binding.span,
                local,
                Rvalue::PatternExtract {
                    subject: Operand::Local(subject_local),
                    path: binding.path,
                },
            );
            let previous = self.symbol_locals.insert(binding.id, local);
            shadowed.push((binding.id, previous));
        }
        shadowed
    }

    pub(in crate::mir::lower) fn restore_shadowed_symbols(
        &mut self,
        shadowed: Vec<(hir::SymbolId, Option<LocalId>)>,
    ) {
        for (id, previous) in shadowed.into_iter().rev() {
            match previous {
                Some(local) => {
                    self.symbol_locals.insert(id, local);
                }
                None => {
                    self.symbol_locals.remove(&id);
                }
            }
        }
    }

    /// 降低 `when` 表达式：把每个 arm 降为显式 pattern test / binder extract / guard CFG。
    pub(in crate::mir::lower) fn lower_when_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        subject: &hir::Expr,
        arms: &[hir::WhenArm],
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);

        // 1) 先在当前块求值 subject。
        let subject_local = self.lower_expr_to_local(subject);
        if self.current_is_terminated() {
            return result;
        }

        // 2) 构造 merge block，并从当前块开始链式生成“匹配测试块”。
        let merge_bb = self.push_block(span);
        let mut test_bb = self.current_bb;

        for arm in arms {
            let irrefutable = self.when_pat_is_irrefutable(&arm.pat);
            let needs_next_test_bb = !irrefutable || arm.guard.is_some();
            let body_bb = arm.guard.as_ref().map(|_| self.push_block(arm.span));
            let next_test_bb = needs_next_test_bb.then(|| self.push_block(arm.span));
            let match_bb = if irrefutable {
                self.current_bb = test_bb;
                let match_bb = self.push_block(arm.span);
                self.set_terminator(test_bb, arm.span, TerminatorKind::Goto { target: match_bb });
                match_bb
            } else {
                let match_bb = self.push_block(arm.span);
                self.current_bb = test_bb;
                let cond = self.push_temp_local(arm.span, self.builtins.bool_);
                self.assign(
                    arm.pat.span(),
                    cond,
                    Rvalue::PatternMatch {
                        subject: Operand::Local(subject_local),
                        pattern: self.lower_pattern(&arm.pat, subject.ty),
                    },
                );
                self.set_terminator(
                    test_bb,
                    arm.span,
                    TerminatorKind::CondBr {
                        cond: Operand::Local(cond),
                        then_target: match_bb,
                        else_target: next_test_bb
                            .expect("refutable when arm should allocate next test block"),
                    },
                );
                match_bb
            };

            self.current_bb = match_bb;
            let shadowed = self.bind_when_pattern_locals(subject_local, &arm.pat);
            if let Some(guard) = &arm.guard {
                let guard_local = self.lower_expr_to_local(guard);
                if !self.current_is_terminated() {
                    self.set_terminator(
                        self.current_bb,
                        guard.span,
                        TerminatorKind::CondBr {
                            cond: Operand::Local(guard_local),
                            then_target: body_bb
                                .expect("guarded when arm should allocate body block"),
                            else_target: next_test_bb
                                .expect("guarded when arm should allocate next test block"),
                        },
                    );
                }
                self.current_bb = body_bb.expect("guarded when arm should allocate body block");
            }

            let body_value = self.lower_expr_to_local(&arm.body);
            if !self.current_is_terminated() {
                self.assign_use_to_local(arm.span, result, Operand::Local(body_value));
                self.set_terminator(
                    self.current_bb,
                    arm.span,
                    TerminatorKind::Goto { target: merge_bb },
                );
            }
            self.restore_shadowed_symbols(shadowed);

            // 继续下一个 arm 的测试块。
            if irrefutable && arm.guard.is_none() {
                self.current_bb = merge_bb;
                return result;
            }

            test_bb = next_test_bb.expect("fallthrough when arm should allocate next test block");
            self.current_bb = test_bb;
        }

        // 若没有兜底 arm，当前阶段以 `unreachable` 收束。
        self.set_terminator(test_bb, span, TerminatorKind::Unreachable);
        self.current_bb = merge_bb;
        result
    }
}
