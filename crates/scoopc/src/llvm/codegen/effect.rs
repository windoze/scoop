//! effect/continuation codegen（T0102e：从 `codegen/mod.rs` 拆分）。

use super::*;

/// flag-based unwinding（non-resuming effect）的“捕获边界”记录。
///
/// 说明：
/// - 当前阶段 `Raise.raise` 仍有独立的 `raise_target_stack`（历史原因，T0614）；
/// - T0625 起，为最小自定义 non-resuming effect 增加同样的“最近匹配”捕获边界栈，
///   用于在一个函数内把 `perform` 直接分发到最近的 `handle` catch block。
#[derive(Debug, Clone)]
pub(super) struct EffectUnwindTarget<'ctx> {
    op_fqn: String,
    target: inkwell::basic_block::BasicBlock<'ctx>,
}

/// `-> resume` lowering（T0616）在 codegen 阶段使用的“立即恢复”上下文。
///
/// 说明：
/// - 当前实现先只覆盖“单个 perform 点”的最小栈上 state machine；
/// - `resume(value)` 会写入 `resume_value_ptr`、更新 `state_ptr`，并跳回 `dispatch_bb`。
#[derive(Debug, Clone, Copy)]
pub(super) struct ImmediateResumeCtx<'ctx> {
    pub(super) resume_symbol: hir::SymbolId,
    resume_value_ty: CgTy,
    resume_value_ptr: Option<PointerValue<'ctx>>,
    resume_used_ptr: PointerValue<'ctx>,
    state_ptr: PointerValue<'ctx>,
    next_state: u32,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn current_raise_target(&self) -> Option<inkwell::basic_block::BasicBlock<'ctx>> {
        self.raise_target_stack.last().copied()
    }

    pub(super) fn push_raise_target(&mut self, target: inkwell::basic_block::BasicBlock<'ctx>) {
        self.raise_target_stack.push(target);
    }

    pub(super) fn pop_raise_target(&mut self) {
        let _ = self.raise_target_stack.pop();
    }

    pub(super) fn current_effect_unwind_target(
        &self,
        op_fqn: &str,
    ) -> Option<inkwell::basic_block::BasicBlock<'ctx>> {
        self.effect_unwind_target_stack
            .iter()
            .rev()
            .find(|t| t.op_fqn == op_fqn)
            .map(|t| t.target)
    }

    pub(super) fn push_effect_unwind_target(
        &mut self,
        op_fqn: &str,
        target: inkwell::basic_block::BasicBlock<'ctx>,
    ) {
        self.effect_unwind_target_stack.push(EffectUnwindTarget {
            op_fqn: op_fqn.to_string(),
            target,
        });
    }

    pub(super) fn pop_effect_unwind_target(&mut self) {
        let _ = self.effect_unwind_target_stack.pop();
    }

    pub(super) fn current_immediate_resume_ctx(&self) -> Option<ImmediateResumeCtx<'ctx>> {
        self.immediate_resume_ctx_stack.last().copied()
    }

    pub(super) fn push_immediate_resume_ctx(&mut self, ctx: ImmediateResumeCtx<'ctx>) {
        self.immediate_resume_ctx_stack.push(ctx);
    }

    pub(super) fn pop_immediate_resume_ctx(&mut self) {
        let _ = self.immediate_resume_ctx_stack.pop();
    }

    /// 读取运行时 TLS effect flag，并返回 `i1`（是否 active）。
    ///
    /// 说明：这里直接调用 runtime C ABI（`scoop_effect_is_active`），避免把该读取当作“普通函数调用”
    /// 从而触发递归插桩（call site 检查 flag → 再调用 is_active → 再检查...）。
    pub(super) fn emit_effect_is_active_i1(
        &mut self,
        at: crate::span::Span,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let rt = self.declare_runtime_effect_is_active();
        let call = self.builder.build_call(rt, &[], "effect_is_active")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect is_active return value",
                at: at.into(),
            })?;
        let BasicValueEnum::IntValue(active_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect is_active return type",
                at: at.into(),
            });
        };
        Ok(self.builder.build_int_compare(
            IntPredicate::NE,
            active_i32,
            self.context.i32_type().const_zero(),
            "effect_active",
        )?)
    }

    /// 在“最近 handler boundary”存在时跳转到 catch；否则返回默认值向外传播。
    ///
    /// 用途：
    /// - 普通函数调用返回后：callee 可能执行 `Raise.raise`，因此返回后需要检查 flag 并决定是否 unwind。
    pub(super) fn emit_effect_unwind_if_active(
        &mut self,
        at: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
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

        let cont_bb = self.context.append_basic_block(func, "effect_unwind_cont");
        let is_active = self.emit_effect_is_active_i1(at)?;

        if let Some(target) = self.current_raise_target() {
            self.builder
                .build_conditional_branch(is_active, target, cont_bb)?;
        } else {
            let ret_bb = self
                .context
                .append_basic_block(func, "effect_unwind_return");
            self.builder
                .build_conditional_branch(is_active, ret_bb, cont_bb)?;

            self.builder.position_at_end(ret_bb);
            let ret_ty = self
                .current_fun_return_ty
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect unwind needs function return type",
                    at: at.into(),
                })?;
            let v = self.default_value(ret_ty);
            self.emit_return(at, ret_ty, v)?;
        }

        self.builder.position_at_end(cont_bb);
        Ok(())
    }

    pub(super) fn fun_ty_effects_is_pure(&self, ty: TypeId) -> Option<bool> {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Function(fun_ty)) => Some(fun_ty.effects.is_pure()),
            _ => None,
        }
    }

    pub(super) fn expr_may_perform(&self, expr: &hir::Expr) -> bool {
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. } => false,

            hir::ExprKind::StructLit { fields, .. } => {
                fields.iter().any(|f| self.expr_may_perform(&f.value))
            }
            hir::ExprKind::TupleLit { elements } => {
                elements.iter().any(|e| self.expr_may_perform(e))
            }
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().any(|p| match p {
                hir::InterpolatedStringPart::Text { .. } => false,
                hir::InterpolatedStringPart::Expr { expr } => self.expr_may_perform(expr),
            }),

            hir::ExprKind::Unary { expr: inner, .. } => self.expr_may_perform(inner),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.expr_may_perform(lhs) || self.expr_may_perform(rhs)
            }
            hir::ExprKind::TypeCheck { expr: inner, .. } => self.expr_may_perform(inner),

            // `as` 失败会走 `Raise.raise(RuntimeError.ClassCastFailed)` 的语义落点，因此视为 perform 点；
            // `as?` 不会 raise（失败返回 None），仅递归检查 operand。
            hir::ExprKind::Cast {
                expr: inner, op, ..
            } => match op {
                ast::CastOp::As => true,
                ast::CastOp::AsQ => self.expr_may_perform(inner),
            },

            hir::ExprKind::Block(block) => self.block_may_perform(block),
            hir::ExprKind::Closure(_) => false,

            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr_may_perform(cond)
                    || self.expr_may_perform(then_branch)
                    || else_branch
                        .as_ref()
                        .is_some_and(|e| self.expr_may_perform(e))
            }

            hir::ExprKind::When { subject, arms } => {
                if self.expr_may_perform(subject) {
                    return true;
                }
                for arm in arms {
                    if arm.guard.as_ref().is_some_and(|g| self.expr_may_perform(g)) {
                        return true;
                    }
                    if self.expr_may_perform(&arm.body) {
                        return true;
                    }
                }
                false
            }

            // member access 本身不 perform，但 receiver 的求值可能 perform。
            hir::ExprKind::MemberAccess { receiver, .. } => self.expr_may_perform(receiver),

            hir::ExprKind::Call { callee, args } => {
                // 实参求值可能包含 perform。
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(e) => {
                            if self.expr_may_perform(e) {
                                return true;
                            }
                        }
                        hir::CallArg::Named { value, .. } => {
                            if self.expr_may_perform(value) {
                                return true;
                            }
                        }
                    }
                }

                // callee 若是已知顶层函数/方法且 effects 为 Pure，则调用点本身不会触发 flag-based unwinding；
                // 其它 callee（closure/local/未解析）先按“可能 perform”保守处理，避免误删 handler。
                let Some(fqn) = self.try_extract_callee_fqn(callee) else {
                    return true;
                };
                let Some(fun) = self.fun_index.get(fqn).copied() else {
                    return true;
                };
                self.fun_ty_effects_is_pure(fun.ty)
                    .map(|pure| !pure)
                    .unwrap_or(true)
            }

            // `perform`/`handle`：直接视为会触发 effect 机制（或其内部可能触发）。
            hir::ExprKind::Perform { .. } => true,
            hir::ExprKind::Handle(_) => true,

            hir::ExprKind::Todo(_) => true,
        }
    }

    pub(super) fn try_extract_callee_fqn<'b>(&self, callee: &'b hir::Expr) -> Option<&'b str> {
        match &callee.kind {
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => Some(fqn.as_str()),
            hir::ExprKind::MemberAccess { member, .. } => match member.resolved.as_ref()? {
                hir::MemberRef::Fun { fqn, .. } => Some(fqn.as_str()),
                hir::MemberRef::ExtensionFun { fqn, .. } => Some(fqn.as_str()),
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn block_may_perform(&self, block: &hir::Block) -> bool {
        for stmt in &block.stmts {
            if self.stmt_may_perform(stmt) {
                return true;
            }
        }
        false
    }

    pub(super) fn stmt_may_perform(&self, stmt: &hir::Stmt) -> bool {
        match &stmt.kind {
            hir::StmtKind::Empty => false,
            hir::StmtKind::Expr(expr) => self.expr_may_perform(expr),
            hir::StmtKind::Val(decl) => {
                decl.init.as_ref().is_some_and(|e| self.expr_may_perform(e))
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.expr_may_perform(lhs) || self.expr_may_perform(rhs)
            }
            hir::StmtKind::While { cond, body } => {
                self.expr_may_perform(cond) || self.block_may_perform(body)
            }
            // 当前阶段这些语句在 block expression 中不支持；为避免误删 handler，这里保守视为可能 perform。
            hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Return { .. }
            | hir::StmtKind::Todo(_) => true,
        }
    }

    pub(super) fn effect_trace_line_col(
        &self,
        at: crate::span::Span,
    ) -> Result<(u32, u32), LlvmEmitError> {
        // 注意：当前阶段 HIR span 仍是“无 file-id 的 byte offsets”，当 codegen 生成跨文件函数体
        //（例如 stdlib/helper 被内联为可 codegen 的顶层函数）时，span 可能不属于入口 `source`。
        //
        // 为避免把“诊断辅助信息”升级成 hard error，这里选择在无法映射时降级为 (0, 0)：
        // - 不影响 non-resuming effect 的语义（仍由 flag+slot 决定）；
        // - fixtures 可选择性断言：对入口文件的 raise/perform，line/col 仍可稳定；
        // - 未来当 span 携带 file-id 后，再把这里升级为精确映射。
        let Ok((line, col)) = self.source.offset_to_line_col(at.start) else {
            return Ok((0, 0));
        };
        let line_u32 = line.min(u32::MAX as usize) as u32;
        let col_u32 = col.min(u32::MAX as usize) as u32;
        Ok((line_u32, col_u32))
    }

    /// 将 `Raise.raise(error)` 的 `error` 值编码为 runtime perform slot 的 payload words。
    ///
    /// 当前阶段（T0818）的目标是先把 `Raise<RuntimeError>` 跑通，以支持：
    /// - `x!!` / `x as T` 等“运行期失败 → Raise<RuntimeError>”的语义落点；
    /// - `try/catch` 能读回并匹配 `RuntimeError` 的 unit variants。
    ///
    /// ABI（TODO T0630）：
    /// - payload 使用 2 个 word：`(kind, value)`
    ///   - `kind`：判别信息（union 风格），便于在 handler 边界做断言/调试
    ///   - `value`：实际载荷（按 u64 编码）
    pub(super) fn codegen_raise_error_payload_words(
        &mut self,
        err_expr: &hir::Expr,
    ) -> Result<(IntValue<'ctx>, IntValue<'ctx>), LlvmEmitError> {
        // slot 的 word 固定为 u64（runtime ABI，T0630）。
        let u64_ty = self.context.i64_type();
        let from_u64 = IntTy {
            bits: 64,
            signed: false,
        };

        // payload.kind（用于 union 风格判别；0 表示未初始化）。
        const KIND_INT: u64 = 1;
        const KIND_RUNTIME_ERROR: u64 = 2;

        // 注意：HIR 在早期阶段并不总是为每个表达式标注精确类型（例如 member access 常为 `Any`），
        // 因此这里以 codegen 后的 `CgValue.ty` 为准（避免过度依赖 `hir::Expr.ty`）。
        let err_v = self.codegen_expr(err_expr)?;

        match err_v.ty {
            CgTy::Int(from_ty) => {
                // 整数族：把值编码进 slot 的 u64。
                let (err_raw, _) = err_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Raise.raise arg value",
                    at: err_expr.span.into(),
                })?;
                let kind = u64_ty.const_int(KIND_INT, false);
                let value = self.cast_int(err_raw, from_ty, from_u64)?;
                Ok((kind, value))
            }
            CgTy::Enum(enum_ty) if self.is_sysroot_runtime_error_enum(enum_ty) => {
                // `RuntimeError`：写入 tag（u32）到 slot（u64）。
                //
                // 注意：当前 `RuntimeError` 的 enum 表示是 tagged union `{ tag: i32, payload: word }`，
                // 其中 payload 为空（unit variants），因此只需要写回 tag 即可。
                let repr = self.cg_enum_layout(err_expr.span, enum_ty)?.repr;
                if !matches!(repr, CgEnumRepr::TaggedUnion) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Raise<RuntimeError> niche repr (not supported)",
                        at: err_expr.span.into(),
                    });
                }

                let raw = err_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Raise.raise arg value",
                    at: err_expr.span.into(),
                })?;
                let enum_v = raw.into_struct_value();
                let extracted =
                    self.builder
                        .build_extract_value(enum_v, 0, "raise_runtime_error_tag")?;
                let BasicValueEnum::IntValue(tag_i32) = extracted else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Raise<RuntimeError> tag value",
                        at: err_expr.span.into(),
                    });
                };
                let kind = u64_ty.const_int(KIND_RUNTIME_ERROR, false);
                let value = self.builder.build_int_z_extend(
                    tag_i32,
                    u64_ty,
                    "raise_runtime_error_tag_u64",
                )?;
                Ok((kind, value))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Raise.raise arg type (payload encoding)",
                at: err_expr.span.into(),
            }),
        }
    }

    /// 判断一个 value nominal type 是否是 sysroot 内建的 `scoop.core.RuntimeError`。
    ///
    /// 说明：T0818 只要求打通 `Raise<RuntimeError>`；其它 `Raise<E>` 的复杂 payload ABI 留给 T0630。
    pub(super) fn is_sysroot_runtime_error_enum(&self, ty: TypeId) -> bool {
        matches!(
            self.types.kind(ty),
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.RuntimeError"
        )
    }

    pub(super) fn runtime_error_variant_tag(
        &self,
        at: crate::span::Span,
        variant: &str,
    ) -> Result<u64, LlvmEmitError> {
        let layout = self.enum_layouts.get("scoop.core.RuntimeError").ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "RuntimeError enum layout",
                at: at.into(),
            },
        )?;
        let v = layout.variants.iter().find(|v| v.name == variant).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "RuntimeError variant",
                at: at.into(),
            },
        )?;
        Ok(v.tag)
    }

    pub(super) fn emit_raise_runtime_error_variant(
        &mut self,
        at: crate::span::Span,
        variant: &str,
    ) -> Result<(), LlvmEmitError> {
        let tag = self.runtime_error_variant_tag(at, variant)?;
        self.emit_raise_runtime_error_tag(at, tag)
    }

    pub(super) fn emit_raise_runtime_error_tag(
        &mut self,
        span: crate::span::Span,
        tag: u64,
    ) -> Result<(), LlvmEmitError> {
        // 说明：复用 `Raise.raise(RuntimeError.X)` 的最小 ABI 约定（T0818），但避免在这里构造 HIR 节点：
        // - slot: (op_tag=Raise, payload_kind=RuntimeError, payload_value=tag)
        // - set flag 并携带 line/col trace
        const OP_TAG_RAISE: u64 = 1;
        const PAYLOAD_KIND_RUNTIME_ERROR: u64 = 2;

        let i32_ty = self.context.i32_type();
        let u64_ty = self.context.i64_type();

        let op_tag_i32 = i32_ty.const_int(OP_TAG_RAISE, false);
        let payload_kind_u64 = u64_ty.const_int(PAYLOAD_KIND_RUNTIME_ERROR, false);
        let payload_value_u64 = u64_ty.const_int(tag, false);

        let rt_write = self.declare_runtime_effect_perform_slot_write_u64_2();
        let _ = self.builder.build_call(
            rt_write,
            &[
                op_tag_i32.into(),
                payload_kind_u64.into(),
                payload_value_u64.into(),
            ],
            "runtime_error_write_slot",
        )?;

        let (src_line, src_col) = self.effect_trace_line_col(span)?;
        let src_line_i32 = i32_ty.const_int(src_line as u64, false);
        let src_col_i32 = i32_ty.const_int(src_col as u64, false);

        let rt_set = self.declare_runtime_effect_set_active_with_trace();
        let _ = self.builder.build_call(
            rt_set,
            &[src_line_i32.into(), src_col_i32.into()],
            "runtime_error_set_active",
        )?;

        // 早退：若存在 handler boundary，跳到 catch；否则返回默认值向外传播。
        if let Some(target) = self.current_raise_target() {
            self.builder.build_unconditional_branch(target)?;
        } else {
            let ret_ty = self
                .current_fun_return_ty
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Raise<RuntimeError> needs function return type",
                    at: span.into(),
                })?;
            let v = self.default_value(ret_ty);
            self.emit_return(span, ret_ty, v)?;
        }

        // 继续生成后续 IR：把 builder 移到一个“不可达 continuation block”，避免后续插入失败。
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
        let dead = self.context.append_basic_block(func, "after_raise_dead");
        self.builder.position_at_end(dead);
        Ok(())
    }

    /// codegen 一个 `Raise.raise(e)`（HIR `Perform` 的最小子集）。
    ///
    /// 当前阶段（T0614）约束：
    /// - 只支持 `scoop.core.Raise.raise`；
    /// - `e` 只支持：
    ///   - word-sized `Int`（沿用 T0614 的最小约定）；
    ///   - `RuntimeError`（T0818：写入 enum tag）；
    /// - 不支持 finally / 自定义 effect / `-> resume`。
    pub(super) fn codegen_perform_expr(
        &mut self,
        span: crate::span::Span,
        op: &hir::EffectOpRef,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if op.fqn != "scoop.core.Raise.raise" {
            return self.codegen_perform_expr_nonresuming_custom_int(span, op, args, expected);
        }

        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Raise.raise arity mismatch",
                at: span.into(),
            });
        }
        let hir::CallArg::Positional(err_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Raise.raise named arg",
                at: span.into(),
            });
        };

        // 1) 计算 `error` 值，并编码为 slot 的复合 payload（2 words）。
        let (payload_kind_u64, payload_value_u64) =
            self.codegen_raise_error_payload_words(err_expr)?;

        // 2) 写 slot + set flag。
        // 说明：当前阶段只需要“可观测的最小表示”；op_tag 未来会与更通用的 payload ABI 对齐（T0630）。
        const OP_TAG_RAISE: u64 = 1;
        let tag_i32 = self.context.i32_type().const_int(OP_TAG_RAISE, false);
        let rt_write = self.declare_runtime_effect_perform_slot_write_u64_2();
        let _ = self.builder.build_call(
            rt_write,
            &[
                tag_i32.into(),
                payload_kind_u64.into(),
                payload_value_u64.into(),
            ],
            "raise_write_slot",
        )?;

        let (src_line, src_col) = self.effect_trace_line_col(span)?;
        let rt_set = self.declare_runtime_effect_set_active_with_trace();
        let i32_ty = self.context.i32_type();
        let src_line_i32 = i32_ty.const_int(src_line as u64, false);
        let src_col_i32 = i32_ty.const_int(src_col as u64, false);
        let _ = self.builder.build_call(
            rt_set,
            &[src_line_i32.into(), src_col_i32.into()],
            "raise_set_active",
        )?;

        // 3) “早退”：在 handler boundary 内跳到 catch，否则返回默认值向外传播。
        if let Some(target) = self.current_raise_target() {
            self.builder.build_unconditional_branch(target)?;
        } else {
            let ret_ty = self
                .current_fun_return_ty
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Raise.raise needs function return type",
                    at: span.into(),
                })?;
            let v = self.default_value(ret_ty);
            self.emit_return(span, ret_ty, v)?;
        }

        // 4) 继续生成后续 IR：把 builder 移到一个“不可达 continuation block”，避免后续插入失败。
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
        let dead = self.context.append_basic_block(func, "after_raise_dead");
        self.builder.position_at_end(dead);

        // Raise 的返回类型在类型系统里是 `Nothing`，可用于任意期望类型；
        // 这里返回一个“期望类型的默认值”以保持后续 codegen 可继续推进。
        Ok(match expected {
            Some(ty) => self.default_value(ty),
            None => CgValue::unit(),
        })
    }

    /// codegen 一个最小自定义 non-resuming effect `perform`（T0625）。
    ///
    /// 当前阶段约束：
    /// - 仅支持 `op(arg)` 形式，且 `arg` 必须是 word-sized `Int`；
    /// - 仅支持在同一函数内存在匹配的 `handle ... with { Effect.op(x) -> ... }` 捕获边界：
    ///   若不存在，则直接报错（避免与现有 `Raise` 的“返回默认值向外传播”机制混淆）。
    ///
    /// 语义：
    /// - 写入 runtime perform slot（1 word payload）并 set flag；
    /// - 直接跳转到最近的匹配 catch block（最近匹配：从栈顶向外找）。
    pub(super) fn codegen_perform_expr_nonresuming_custom_int(
        &mut self,
        span: crate::span::Span,
        op: &hir::EffectOpRef,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect op arity mismatch (custom non-resuming)",
                at: span.into(),
            });
        }
        let hir::CallArg::Positional(payload_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect op named arg (custom non-resuming)",
                at: span.into(),
            });
        };

        let payload_v = self.codegen_expr(payload_expr)?;
        let CgTy::Int(from_ty) = payload_v.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect payload type (custom non-resuming, only Int supported)",
                at: payload_expr.span.into(),
            });
        };
        let (payload_raw, _) = payload_v
            .as_int()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect payload value (custom non-resuming)",
                at: payload_expr.span.into(),
            })?;

        // v0：自定义 effect 的 op_tag 暂不分配稳定编号（runtime 仍会记录到 slot 里便于调试）。
        let op_tag_i32 = self.context.i32_type().const_zero();
        let from_u64 = IntTy {
            bits: 64,
            signed: false,
        };
        let payload_u64 = self.cast_int(payload_raw, from_ty, from_u64)?;

        let rt_write = self.declare_runtime_effect_perform_slot_write_u64();
        let _ = self.builder.build_call(
            rt_write,
            &[op_tag_i32.into(), payload_u64.into()],
            "effect_write_slot",
        )?;
        let (src_line, src_col) = self.effect_trace_line_col(span)?;
        let rt_set = self.declare_runtime_effect_set_active_with_trace();
        let i32_ty = self.context.i32_type();
        let src_line_i32 = i32_ty.const_int(src_line as u64, false);
        let src_col_i32 = i32_ty.const_int(src_col as u64, false);
        let _ = self.builder.build_call(
            rt_set,
            &[src_line_i32.into(), src_col_i32.into()],
            "effect_set_active",
        )?;

        let Some(target) = self.current_effect_unwind_target(&op.fqn) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect op without handle boundary (custom non-resuming)",
                at: span.into(),
            });
        };
        self.builder.build_unconditional_branch(target)?;

        // 继续生成后续 IR：把 builder 移到一个“不可达 continuation block”，避免后续插入失败。
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
        let dead = self.context.append_basic_block(func, "after_effect_dead");
        self.builder.position_at_end(dead);

        Ok(match expected {
            Some(ty) => self.default_value(ty),
            None => CgValue::unit(),
        })
    }

    /// codegen 一个 `handle { ... } with { Raise.raise(e) -> ... }`（`try/catch` 的 lowering 产物）。
    ///
    /// 当前阶段（T0614）约束：
    /// - 只支持捕获 `scoop.core.Raise.raise`；
    /// - 只支持单个 arm（最小示例）；finally 语义由 T0615 补齐；
    /// - arm body 在“handler scope”之外生成，避免 self-capture（PLAN §6.2）。
    pub(super) fn codegen_handle_expr(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let out_ty = expected.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle needs expected type context",
            at: span.into(),
        })?;

        // T1604：无 perform（含 effectful call）时不生成 handler frame/stack 链接。
        //
        // 说明：
        // - 当前阶段的 effect 传播语义依赖 call-site 的 flag 检查与 handle boundary；
        // - 若 handle body 内没有任何可能触发 perform 的点，则 arms 永远不可达，因此可直接降为：
        //   `body; finally?; return body_value`，并避免引入 runtime effect 符号与 handler frame。
        if !self.block_may_perform(&handle.body) {
            return self.codegen_handle_expr_no_perform(span, handle, out_ty);
        }

        if handle.arms.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle arm count (only 1 supported)",
                at: span.into(),
            });
        }
        let arm = &handle.arms[0];
        if let hir::HandleArmKind::ImmediateResume { resume } = arm.kind {
            return self.codegen_handle_expr_immediate_resume(span, handle, arm, resume, out_ty);
        }
        if let hir::HandleArmKind::EscapeContinuation { continuation } = arm.kind {
            let seq = self.escape_continuation_seq;
            self.escape_continuation_seq = self.escape_continuation_seq.saturating_add(1);
            return self.codegen_handle_expr_escape_continuation(
                span,
                handle,
                arm,
                continuation,
                seq,
                out_ty,
            );
        }
        if arm.op.op.fqn != "scoop.core.Raise.raise" {
            return self
                .codegen_handle_expr_nonresuming_custom_int_payload(span, handle, arm, out_ty);
        }
        if arm.op.binders.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder count (only 1 supported)",
                at: arm.op.span.into(),
            });
        }
        let binder = &arm.op.binders[0];

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

        // TODO T0913：在动态层维护 handler stack（Appendix A）。
        // 当前阶段只需要：
        // - 进入 handle body 前 push；
        // - 正常结束或进入 arm/catch 前 pop（arm body 在 dispatch scope 外执行，Appendix A.4）。
        const OP_TAG_RAISE: u64 = 1;
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let handler_frame_ptr =
            self.create_entry_alloca_raw(span, "handle_effect_frame", handler_frame_ty.into())?;

        let outer_raise_target = self.current_raise_target();

        let body_bb = self.context.append_basic_block(func, "handle_body");
        let catch_bb = self.context.append_basic_block(func, "handle_catch");

        // `finally` 语义：保证在“正常路径 / catch 返回 / catch 继续 raise 向外传播”三种情况下都执行一次。
        // - 正常路径与 catch 返回：汇合到 finally_bb 再进入 merge；
        // - catch 内发生 raise：先进入 finally_unwind_bb 执行 finally，然后向外传播 raise（不清 flag/slot）。
        let finally_bb = self.context.append_basic_block(func, "handle_finally");
        let finally_unwind_bb = self
            .context
            .append_basic_block(func, "handle_finally_unwind");
        let merge_bb = self.context.append_basic_block(func, "handle_merge");

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_result", out_ty)?)
        };

        // 进入 handle body：push handler frame（动态上下文）。
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 =
            self.builder
                .build_bit_cast(handler_frame_ptr, i8_ptr_ty, "handle_effect_frame_i8")?;
        let op_tag_i32 = self.context.i32_type().const_int(OP_TAG_RAISE, false);
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), op_tag_i32.into()],
            "handle_effect_push",
        )?;

        // 进入 handle：先执行 body；若发生 Raise，则通过 flag/slot unwind 到 catch_bb。
        self.builder.build_unconditional_branch(body_bb)?;

        // --- body ---
        self.builder.position_at_end(body_bb);
        self.push_raise_target(catch_bb);
        let body_v = self.codegen_block_value(&handle.body)?;
        let body_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(handle.body.span, body_v, out_ty)?
        };
        self.pop_raise_target();

        // body 正常结束：进入 finally（并保存结果值）。
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                if let Some(ptr) = result_ptr {
                    let _ = self.store_local_value(handle.body.span, ptr, out_ty, body_v)?;
                }

                // body 正常结束：pop handler frame，使 finally 处于 handler scope 之外（与现有 lowering 一致）。
                let rt_pop = self.declare_runtime_effect_handler_stack_pop();
                let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
                let frame_i8 = self.builder.build_bit_cast(
                    handler_frame_ptr,
                    i8_ptr_ty,
                    "handle_effect_frame_i8",
                )?;
                let _ = self
                    .builder
                    .build_call(rt_pop, &[frame_i8.into()], "handle_effect_pop")?;

                self.builder.build_unconditional_branch(finally_bb)?;
            }
        }

        // --- catch ---
        self.builder.position_at_end(catch_bb);

        // 进入 handler arm：pop handler frame（Appendix A.4：arm body 在自身 handler scope 外执行）。
        let rt_pop = self.declare_runtime_effect_handler_stack_pop();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 =
            self.builder
                .build_bit_cast(handler_frame_ptr, i8_ptr_ty, "handle_effect_frame_i8")?;
        let _ = self
            .builder
            .build_call(rt_pop, &[frame_i8.into()], "handle_effect_pop")?;

        // 读取 slot（payload words）并清除 flag/slot。
        //
        // TODO T0630：目前 `Raise.raise` 统一写入 2 个 word（kind + value），这里做运行期断言，
        // 以便快速发现 lowering/codegen/runtime ABI 不一致的问题。
        let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
        let call = self
            .builder
            .build_call(rt_len, &[], "raise_read_slot_len_words")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_len_words return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(len_words_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_len_words return type",
                at: span.into(),
            });
        };

        let expected_len = self.context.i32_type().const_int(2, false);
        let len_ok = self.builder.build_int_compare(
            IntPredicate::EQ,
            len_words_i32,
            expected_len,
            "raise_slot_len_ok",
        )?;
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
        let len_ok_bb = self
            .context
            .append_basic_block(func, "raise_slot_len_ok_bb");
        let len_bad_bb = self
            .context
            .append_basic_block(func, "raise_slot_len_bad_bb");
        self.builder
            .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

        self.builder.position_at_end(len_bad_bb);
        let exit = self.declare_libc_exit();
        let code = self.context.i32_type().const_int(3, false);
        let _ = self
            .builder
            .build_call(exit, &[code.into()], "raise_slot_len_exit")?;
        self.builder.build_unreachable()?;

        self.builder.position_at_end(len_ok_bb);

        let rt_read_at = self.declare_runtime_effect_perform_slot_read_u64_at();
        let idx0 = self.context.i32_type().const_int(0, false);
        let idx1 = self.context.i32_type().const_int(1, false);

        let kind_call =
            self.builder
                .build_call(rt_read_at, &[idx0.into()], "raise_read_slot_word0")?;
        let kind_raw =
            kind_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word0 return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::IntValue(kind_u64) = kind_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_word0 return type",
                at: span.into(),
            });
        };

        let value_call =
            self.builder
                .build_call(rt_read_at, &[idx1.into()], "raise_read_slot_word1")?;
        let value_raw =
            value_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word1 return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::IntValue(value_u64) = value_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_word1 return type",
                at: span.into(),
            });
        };

        let rt_clear = self.declare_runtime_effect_clear();
        let _ = self.builder.build_call(rt_clear, &[], "raise_clear")?;

        // binder scope：arm body 在 handler scope 之外执行（因此不 push raise_target）。
        self.env.push_scope();

        let binder_cg_ty = self
            .cg_ty_of(binder.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder type",
                at: binder.span.into(),
            })?;
        let binder_value = match binder_cg_ty {
            CgTy::Int(int_ty) => {
                // kind 断言：避免把 `RuntimeError` 等误解码为整数。
                let expected = self.context.i64_type().const_int(1, false);
                let ok = self.builder.build_int_compare(
                    IntPredicate::EQ,
                    kind_u64,
                    expected,
                    "raise_kind_is_int",
                )?;
                let ok_bb = self.context.append_basic_block(func, "raise_kind_int_ok");
                let bad_bb = self.context.append_basic_block(func, "raise_kind_int_bad");
                self.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                self.builder.position_at_end(bad_bb);
                let exit = self.declare_libc_exit();
                let code = self.context.i32_type().const_int(3, false);
                let _ = self
                    .builder
                    .build_call(exit, &[code.into()], "raise_kind_int_exit")?;
                self.builder.build_unreachable()?;

                self.builder.position_at_end(ok_bb);

                // 传统路径：`Raise<Int>` —— 直接把 slot 的 u64 解码回整数。
                let from_u64 = IntTy {
                    bits: 64,
                    signed: false,
                };
                let decoded = self.cast_int(value_u64, from_u64, int_ty)?;
                CgValue::int(decoded, int_ty)
            }
            CgTy::Enum(enum_ty) if self.is_sysroot_runtime_error_enum(enum_ty) => {
                // kind 断言：避免把整数误解码为 RuntimeError。
                let expected = self.context.i64_type().const_int(2, false);
                let ok = self.builder.build_int_compare(
                    IntPredicate::EQ,
                    kind_u64,
                    expected,
                    "raise_kind_is_runtime_error",
                )?;
                let ok_bb = self
                    .context
                    .append_basic_block(func, "raise_kind_runtime_error_ok");
                let bad_bb = self
                    .context
                    .append_basic_block(func, "raise_kind_runtime_error_bad");
                self.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                self.builder.position_at_end(bad_bb);
                let exit = self.declare_libc_exit();
                let code = self.context.i32_type().const_int(3, false);
                let _ = self.builder.build_call(
                    exit,
                    &[code.into()],
                    "raise_kind_runtime_error_exit",
                )?;
                self.builder.build_unreachable()?;

                self.builder.position_at_end(ok_bb);

                // `Raise<RuntimeError>`：slot 里承载的是 enum tag（u64），这里把它恢复为 enum 值。
                let repr = self.cg_enum_layout(span, enum_ty)?.repr;
                if !matches!(repr, CgEnumRepr::TaggedUnion) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Raise<RuntimeError> niche repr (not supported)",
                        at: span.into(),
                    });
                }

                let tag_i32 = self.builder.build_int_truncate(
                    value_u64,
                    self.context.i32_type(),
                    "raise_runtime_error_tag_i32",
                )?;
                let payload_word_zero = self.int_type(self.enum_payload_ty()).const_int(0, false);
                let payload_ptr_zero = self.llvm_gc_i8_ptr_type().const_null();

                let llvm_enum_ty = self.llvm_enum_value_type(span, enum_ty)?;
                let llvm_enum_ty = llvm_enum_ty.into_struct_type();
                let mut agg: AggregateValueEnum<'ctx> = llvm_enum_ty.get_undef().into();
                agg =
                    self.builder
                        .build_insert_value(agg, tag_i32, 0, "raise_runtime_error_tag")?;
                agg = self.builder.build_insert_value(
                    agg,
                    payload_word_zero,
                    1,
                    "raise_runtime_error_payload_word",
                )?;
                agg = self.builder.build_insert_value(
                    agg,
                    payload_ptr_zero,
                    2,
                    "raise_runtime_error_payload_ptr",
                )?;
                CgValue {
                    ty: CgTy::Enum(enum_ty),
                    value: Some(agg.as_basic_value_enum()),
                }
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle binder type (Raise payload decode)",
                    at: binder.span.into(),
                });
            }
        };
        let binder_ptr = self.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
        let _stored =
            self.store_local_value(binder.span, binder_ptr, binder_cg_ty, binder_value)?;
        self.env.insert(
            binder.id,
            CgLocal {
                hir_ty: Some(binder.ty),
                ty: binder_cg_ty,
                ptr: binder_ptr,
                mutable: false,
            },
        );

        // catch body 若再次发生 Raise：先执行 finally，再向外传播（不在本 handler 内消费 slot）。
        self.push_raise_target(finally_unwind_bb);
        let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
        self.pop_raise_target();
        let arm_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(arm.body.span, arm_v, out_ty)?
        };

        let catch_end =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let catch_reaches_merge = catch_end.get_terminator().is_none();
        if catch_reaches_merge {
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
            }
            self.builder.build_unconditional_branch(finally_bb)?;
        }
        self.env.pop_scope();

        // --- finally_unwind ---
        self.builder.position_at_end(finally_unwind_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                if let Some(target) = outer_raise_target {
                    self.builder.build_unconditional_branch(target)?;
                } else {
                    let ret_ty =
                        self.current_fun_return_ty
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle finally unwind needs function return type",
                                at: span.into(),
                            })?;
                    let v = self.default_value(ret_ty);
                    self.emit_return(span, ret_ty, v)?;
                }
            }
        }

        // --- finally ---
        self.builder.position_at_end(finally_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                self.builder.build_unconditional_branch(merge_bb)?;
            }
        }

        // --- merge ---
        self.builder.position_at_end(merge_bb);

        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                let Some(ptr) = result_ptr else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle result slot",
                        at: span.into(),
                    });
                };

                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self.builder.build_load(llvm_ty, ptr, "handle_result")?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle result type",
                    at: span.into(),
                })
            }
        }
    }

    pub(super) fn codegen_handle_expr_no_perform(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // no-perform fast path：不生成 catch/merge CFG，仅保留顺序语义（body -> finally）。
        let result_ptr = match out_ty {
            CgTy::Unit => None,
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                Some(self.create_entry_alloca(span, "handle_noperform_result", out_ty)?)
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle result type",
                    at: span.into(),
                });
            }
        };

        // body
        let body_v = self.codegen_block_value_in_expected_context(&handle.body, Some(out_ty))?;
        let body_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(handle.body.span, body_v, out_ty)?
        };
        if let Some(ptr) = result_ptr {
            let _ = self.store_local_value(handle.body.span, ptr, out_ty, body_v)?;
        }

        // finally（仅在当前路径可达时执行）
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                if let Some(finally) = handle.finally.as_ref() {
                    let _ = self.codegen_block_value(finally)?;
                }
            }
        }

        // 若 body/finally 终止了当前块：为后续 codegen 创建一个 dead block。
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        if insert_block.get_terminator().is_some() {
            let func = insert_block
                .get_parent()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no parent function",
                    at: span.into(),
                })?;
            let dead = self
                .context
                .append_basic_block(func, "handle_noperform_dead");
            self.builder.position_at_end(dead);
            return Ok(match out_ty {
                CgTy::Unit => CgValue::unit(),
                other => self.default_value(other),
            });
        }

        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                let Some(ptr) = result_ptr else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle result slot",
                        at: span.into(),
                    });
                };
                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, ptr, "handle_noperform_value")?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle result type",
                    at: span.into(),
                })
            }
        }
    }

    /// codegen 一个最小自定义 non-resuming effect 的 `handle`（T0625）。
    ///
    /// 当前阶段约束：
    /// - 仅支持单 arm；
    /// - binder 仅支持 1 个且类型为 `Int`；
    /// - payload ABI：`perform` 往 slot 写 1 个 word（u64），catch 读取并清 flag/slot。
    ///
    /// 关键语义（Appendix A.4）：
    /// - handler arm body 在自身 dispatch scope 外执行：因此 arm codegen 期间不在
    ///   `effect_unwind_target_stack` 中保留 `catch_bb` 入口；
    /// - 但为了确保 `finally` 语义（若有）仍然成立，arm body 内若再次 perform 同一 op，
    ///   会先跳到 `finally_unwind_bb` 执行 finally，再向外层 handler 传播。
    pub(super) fn codegen_handle_expr_nonresuming_custom_int_payload(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        arm: &hir::HandleArm,
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if arm.op.binders.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder count (custom non-resuming, only 1 supported)",
                at: arm.op.span.into(),
            });
        }
        let binder = &arm.op.binders[0];

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

        // v0：自定义 effect 的 op_tag 暂用 0（与现有 resume/escape 代码保持一致）。
        let op_tag_i32 = self.context.i32_type().const_zero();

        let outer_target = self.current_effect_unwind_target(&arm.op.op.fqn);

        let body_bb = self.context.append_basic_block(func, "handle_custom_body");
        let catch_bb = self.context.append_basic_block(func, "handle_custom_catch");

        // `finally` 语义：保证在“正常路径 / catch 返回 / catch 继续 perform 向外传播”三种情况下都执行一次。
        let finally_bb = self
            .context
            .append_basic_block(func, "handle_custom_finally");
        let finally_unwind_bb = self
            .context
            .append_basic_block(func, "handle_custom_finally_unwind");
        let merge_bb = self.context.append_basic_block(func, "handle_custom_merge");

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_custom_result", out_ty)?)
        };

        // handler frame（动态上下文）。
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let handler_frame_ptr = self.create_entry_alloca_raw(
            span,
            "handle_custom_effect_frame",
            handler_frame_ty.into(),
        )?;

        // 进入 handle body：push handler frame（动态上下文）。
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 =
            self.builder
                .build_bit_cast(handler_frame_ptr, i8_ptr_ty, "handle_custom_frame_i8")?;
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), op_tag_i32.into()],
            "handle_custom_effect_push",
        )?;

        // 进入 handle：先执行 body；若发生 perform，则跳到 catch_bb。
        self.builder.build_unconditional_branch(body_bb)?;

        // --- body ---
        self.builder.position_at_end(body_bb);
        self.push_effect_unwind_target(&arm.op.op.fqn, catch_bb);
        let body_v = self.codegen_block_value_in_expected_context(&handle.body, Some(out_ty))?;
        self.pop_effect_unwind_target();

        let body_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(handle.body.span, body_v, out_ty)?
        };

        // body 正常结束：进入 finally（并保存结果值）。
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                if let Some(ptr) = result_ptr {
                    let _ = self.store_local_value(handle.body.span, ptr, out_ty, body_v)?;
                }

                // body 正常结束：pop handler frame，使 finally 处于 handler scope 之外（Appendix A.4）。
                let rt_pop = self.declare_runtime_effect_handler_stack_pop();
                let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
                let frame_i8 = self.builder.build_bit_cast(
                    handler_frame_ptr,
                    i8_ptr_ty,
                    "handle_custom_frame_i8",
                )?;
                let _ = self.builder.build_call(
                    rt_pop,
                    &[frame_i8.into()],
                    "handle_custom_effect_pop",
                )?;

                self.builder.build_unconditional_branch(finally_bb)?;
            }
        }

        // --- catch ---
        self.builder.position_at_end(catch_bb);

        // 进入 handler arm：pop handler frame（Appendix A.4：arm body 在自身 handler scope 外执行）。
        let rt_pop = self.declare_runtime_effect_handler_stack_pop();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 =
            self.builder
                .build_bit_cast(handler_frame_ptr, i8_ptr_ty, "handle_custom_frame_i8")?;
        let _ = self
            .builder
            .build_call(rt_pop, &[frame_i8.into()], "handle_custom_effect_pop")?;

        // 读取 slot（1 word payload）并清除 flag/slot。
        let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
        let call = self
            .builder
            .build_call(rt_len, &[], "custom_read_slot_len_words")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_len_words return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(len_words_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_len_words return type",
                at: span.into(),
            });
        };

        let expected_len = self.context.i32_type().const_int(1, false);
        let len_ok = self.builder.build_int_compare(
            IntPredicate::EQ,
            len_words_i32,
            expected_len,
            "custom_slot_len_ok",
        )?;
        let len_ok_bb = self
            .context
            .append_basic_block(func, "custom_slot_len_ok_bb");
        let len_bad_bb = self
            .context
            .append_basic_block(func, "custom_slot_len_bad_bb");
        self.builder
            .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

        self.builder.position_at_end(len_bad_bb);
        self.emit_exit_with_code(span, 3)?;

        self.builder.position_at_end(len_ok_bb);

        let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
        let value_call = self
            .builder
            .build_call(rt_read, &[], "custom_read_slot_word0")?;
        let value_raw =
            value_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word0 return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::IntValue(value_u64) = value_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_word0 return type",
                at: span.into(),
            });
        };

        let rt_clear = self.declare_runtime_effect_clear();
        let _ = self.builder.build_call(rt_clear, &[], "custom_clear")?;

        // binder scope：arm body 在 handler scope 之外执行（因此不 push effect_unwind_target_stack 的 catch_bb）。
        self.env.push_scope();

        let binder_cg_ty = self
            .cg_ty_of(binder.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder type (custom non-resuming)",
                at: binder.span.into(),
            })?;
        let CgTy::Int(int_ty) = binder_cg_ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder type (custom non-resuming, only Int supported)",
                at: binder.span.into(),
            });
        };

        let from_u64 = IntTy {
            bits: 64,
            signed: false,
        };
        let decoded = self.cast_int(value_u64, from_u64, int_ty)?;
        let binder_value = CgValue::int(decoded, int_ty);

        let binder_ptr = self.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
        let _stored =
            self.store_local_value(binder.span, binder_ptr, binder_cg_ty, binder_value)?;
        self.env.insert(
            binder.id,
            CgLocal {
                hir_ty: Some(binder.ty),
                ty: binder_cg_ty,
                ptr: binder_ptr,
                mutable: false,
            },
        );

        // catch body 若再次发生 perform：先执行 finally，再向外传播（不在本 handler 内消费 slot）。
        self.push_effect_unwind_target(&arm.op.op.fqn, finally_unwind_bb);
        let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
        self.pop_effect_unwind_target();
        let arm_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(arm.body.span, arm_v, out_ty)?
        };

        let catch_end =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let catch_reaches_merge = catch_end.get_terminator().is_none();
        if catch_reaches_merge {
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
            }
            self.builder.build_unconditional_branch(finally_bb)?;
        }
        self.env.pop_scope();

        // --- finally_unwind ---
        self.builder.position_at_end(finally_unwind_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                if let Some(target) = outer_target {
                    self.builder.build_unconditional_branch(target)?;
                } else {
                    // 当前阶段：自定义 effect 在程序边界的处理策略尚未固定；先按运行期错误处理。
                    self.emit_exit_with_code(span, 3)?;
                }
            }
        }

        // --- finally ---
        self.builder.position_at_end(finally_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                self.builder.build_unconditional_branch(merge_bb)?;
            }
        }

        // --- merge ---
        self.builder.position_at_end(merge_bb);

        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                let Some(ptr) = result_ptr else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle result slot",
                        at: span.into(),
                    });
                };

                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, ptr, "handle_custom_result")?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle result type",
                    at: span.into(),
                })
            }
        }
    }

    pub(super) fn codegen_handle_expr_immediate_resume(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        arm: &hir::HandleArm,
        resume_symbol: hir::SymbolId,
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // T0616：先实现最小“栈 state machine”版本的 `-> resume`：
        // - 只支持单个 perform 点（位于一个 `val x: T = Effect.op(...)` 的 init 中）
        // - `resume(value)` 必须恰好一次：重复/缺失先按运行期错误处理（exit(3)）
        if handle.finally.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle finally (immediate-resume)",
                at: span.into(),
            });
        }

        // 1) 在 handle body 中找到唯一的 perform 点（当前阶段只支持 `val x: T = perform` 这种形式）。
        let mut perform_site: Option<(usize, &hir::ValDecl, &hir::EffectOpRef, &[hir::CallArg])> =
            None;
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            match &stmt.kind {
                hir::StmtKind::Val(decl) => {
                    let Some(init) = decl.init.as_ref() else {
                        continue;
                    };
                    if let hir::ExprKind::Perform { op, args } = &init.kind {
                        if perform_site.is_some() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle resume body (multiple perform points)",
                                at: init.span.into(),
                            });
                        }
                        perform_site = Some((idx, decl, op, args.as_slice()));
                    }
                }
                hir::StmtKind::Expr(expr) => {
                    if matches!(expr.kind, hir::ExprKind::Perform { .. }) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (perform must be bound to val)",
                            at: expr.span.into(),
                        });
                    }
                }
                _ => {}
            }
        }

        let Some((perform_idx, perform_decl, perform_op, perform_args)) = perform_site else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume body (missing perform)",
                at: span.into(),
            });
        };

        if perform_op.fqn != arm.op.op.fqn {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume op mismatch",
                at: perform_op.span.into(),
            });
        }

        let Some(perform_id) = perform_decl.id else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume perform binding id",
                at: perform_decl.span.into(),
            });
        };

        let resume_value_ty =
            self.cg_ty_of(perform_decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume perform value type",
                    at: perform_decl.span.into(),
                })?;

        if arm.op.binders.len() != perform_args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume binder arity mismatch",
                at: arm.op.span.into(),
            });
        }

        // 2) 创建 state machine 所需的基本块与栈上存储。
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

        // TODO T0913：在动态层维护 handler stack（Appendix A）。
        //
        // - handle 进入后 push handler frame；
        // - arm body 执行期间将其标记为 inactive（Appendix A.4），避免 self-capture；
        // - 进入 resumed computation（dispatch/state1...）前再恢复为 active；
        // - handle 结束时 pop。
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let handler_frame_ptr = self.create_entry_alloca_raw(
            span,
            "handle_resume_effect_frame",
            handler_frame_ty.into(),
        )?;

        let dispatch_bb = self
            .context
            .append_basic_block(func, "handle_resume_dispatch");
        let state0_bb = self
            .context
            .append_basic_block(func, "handle_resume_state0");
        let state1_bb = self
            .context
            .append_basic_block(func, "handle_resume_state1");
        let arm_bb = self.context.append_basic_block(func, "handle_resume_arm");
        let done_bb = self.context.append_basic_block(func, "handle_resume_done");
        let bad_state_bb = self
            .context
            .append_basic_block(func, "handle_resume_bad_state");

        let i32_ty = self.context.i32_type();
        let state_ptr = self.create_entry_alloca_raw(span, "handle_state", i32_ty.into())?;
        let resume_used_ptr = self.create_entry_alloca_raw(
            span,
            "handle_resume_used",
            self.context.bool_type().into(),
        )?;
        let resume_value_ptr = if resume_value_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_resume_value", resume_value_ty)?)
        };

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_result", out_ty)?)
        };

        // binder locals：提前在 entry block 分配 slot；在 perform 点写入，在 arm body 内读取。
        struct BinderSlot<'ctx> {
            id: hir::SymbolId,
            hir_ty: TypeId,
            ty: CgTy,
            ptr: PointerValue<'ctx>,
        }
        let mut binder_slots: Vec<BinderSlot<'ctx>> = Vec::new();
        for binder in &arm.op.binders {
            let binder_ty = self
                .cg_ty_of(binder.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume binder type",
                    at: binder.span.into(),
                })?;
            let ptr = self.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
            binder_slots.push(BinderSlot {
                id: binder.id,
                hir_ty: binder.ty,
                ty: binder_ty,
                ptr,
            });
        }

        // 3) 初始化并进入 dispatch。
        let _ = self.builder.build_store(state_ptr, i32_ty.const_zero())?;
        let _ = self.builder.build_store(
            resume_used_ptr,
            self.context.bool_type().const_int(0, false),
        )?;

        // push handler frame（动态上下文）。
        //
        // 说明：op_tag 目前仅对 `Raise.raise` 固化为 1；其它 op 先写 0（未来由统一的 op_tag 分配规则补齐）。
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 = self.builder.build_bit_cast(
            handler_frame_ptr,
            i8_ptr_ty,
            "handle_resume_effect_frame_i8",
        )?;
        let op_tag_i32 = if arm.op.op.fqn == "scoop.core.Raise.raise" {
            self.context.i32_type().const_int(1, false)
        } else {
            self.context.i32_type().const_zero()
        };
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), op_tag_i32.into()],
            "handle_resume_effect_push",
        )?;

        self.builder.build_unconditional_branch(dispatch_bb)?;

        // --- dispatch ---
        self.builder.position_at_end(dispatch_bb);
        let state = self
            .builder
            .build_load(i32_ty, state_ptr, "handle_state")?
            .into_int_value();
        let cases = [
            (i32_ty.const_int(0, false), state0_bb),
            (i32_ty.const_int(1, false), state1_bb),
        ];
        self.builder.build_switch(state, bad_state_bb, &cases)?;

        // --- bad_state ---
        self.builder.position_at_end(bad_state_bb);
        self.emit_exit_with_code(span, 3)?;

        // `handle` body 的 locals 在整个 state machine 生命周期内有效（因此这里不使用 `codegen_block_value`）。
        self.env.push_scope();

        // --- state0：执行 perform 之前的片段，遇到 perform 则进入 arm ---
        self.builder.position_at_end(state0_bb);
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if idx == perform_idx {
                break;
            }
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => self.codegen_val_decl(decl)?,
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                }
                hir::StmtKind::Expr(expr) => {
                    let _ = self.codegen_expr(expr)?;
                }
                hir::StmtKind::Return { .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "`return` inside handle resume body",
                        at: stmt.span.into(),
                    });
                }
                hir::StmtKind::While { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement inside handle resume body",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        // perform 语句本身：当前阶段仅支持 `val x: T = Effect.op(args...)`。
        let target_ptr = {
            let name = perform_decl.name.as_deref().unwrap_or("perform_value");
            let ptr = self.create_entry_alloca(perform_decl.span, name, resume_value_ty)?;
            self.env.insert(
                perform_id,
                CgLocal {
                    hir_ty: Some(perform_decl.ty),
                    ty: resume_value_ty,
                    ptr,
                    mutable: perform_decl.mutable,
                },
            );
            ptr
        };

        // 写入 binder values（供 arm body 使用）。
        for (idx, arg) in perform_args.iter().enumerate() {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume perform args (named arg not supported)",
                    at: span.into(),
                });
            };
            let slot = &binder_slots[idx];
            if slot.ty == CgTy::Unit {
                continue;
            }

            let v = self.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
            let v = self.coerce_value(expr.span, v, slot.ty)?;
            let _stored = self.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
        }

        // 重置一次性标记，并进入 handler arm。
        let _ = self.builder.build_store(
            resume_used_ptr,
            self.context.bool_type().const_int(0, false),
        )?;
        self.builder.build_unconditional_branch(arm_bb)?;

        // --- arm：执行 handler 片段，必须调用 `resume(value)` 跳回 dispatch ---
        self.builder.position_at_end(arm_bb);

        // Appendix A.4：arm body 在自身 handler 的 dispatch scope 外执行（避免 self-capture）。
        let rt_set_active = self.declare_runtime_effect_handler_stack_set_active();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 = self.builder.build_bit_cast(
            handler_frame_ptr,
            i8_ptr_ty,
            "handle_resume_effect_frame_i8",
        )?;
        let inactive = self.context.i32_type().const_zero();
        let _ = self.builder.build_call(
            rt_set_active,
            &[frame_i8.into(), inactive.into()],
            "handle_resume_effect_inactive",
        )?;

        self.env.push_scope();
        for slot in &binder_slots {
            self.env.insert(
                slot.id,
                CgLocal {
                    hir_ty: Some(slot.hir_ty),
                    ty: slot.ty,
                    ptr: slot.ptr,
                    mutable: false,
                },
            );
        }

        let resume_ctx = ImmediateResumeCtx {
            resume_symbol,
            resume_value_ty,
            resume_value_ptr,
            resume_used_ptr,
            state_ptr,
            next_state: 1,
        };
        self.push_immediate_resume_ctx(resume_ctx);
        let _ = self.codegen_expr_in_expected_context(&arm.body, Some(CgTy::Unit))?;
        self.pop_immediate_resume_ctx();

        // `resume(value)` 必须恰好一次：
        // - 未调用：arm 结束时检测到 `resume_used == false`，运行期退出；
        // - 多次调用：在 `resume(value)` intrinsic 内部检测到 `resume_used == true`，运行期退出。
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
        let resume_ok_bb = self
            .context
            .append_basic_block(func, "handle_resume_arm_ok");
        let resume_missing_bb = self
            .context
            .append_basic_block(func, "handle_resume_arm_missing");

        let used = self
            .builder
            .build_load(self.context.bool_type(), resume_used_ptr, "resume_used")?
            .into_int_value();
        self.builder
            .build_conditional_branch(used, resume_ok_bb, resume_missing_bb)?;

        self.builder.position_at_end(resume_missing_bb);
        self.emit_exit_with_code(span, 3)?;

        self.builder.position_at_end(resume_ok_bb);

        // 恢复 handler 为 active：后续 resumed computation（dispatch/state1）应处于该 handler 的动态 scope 下。
        let rt_set_active = self.declare_runtime_effect_handler_stack_set_active();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 = self.builder.build_bit_cast(
            handler_frame_ptr,
            i8_ptr_ty,
            "handle_resume_effect_frame_i8",
        )?;
        let active = self.context.i32_type().const_int(1, false);
        let _ = self.builder.build_call(
            rt_set_active,
            &[frame_i8.into(), active.into()],
            "handle_resume_effect_active",
        )?;

        self.builder.build_unconditional_branch(dispatch_bb)?;

        self.env.pop_scope();

        // --- state1：恢复 perform 的返回值，并继续执行剩余片段，计算 handle 的结果 ---
        self.builder.position_at_end(state1_bb);

        if let Some(ptr) = resume_value_ptr {
            let llvm_ty = self.llvm_basic_type_of(span, resume_value_ty)?;
            let loaded = self.builder.build_load(llvm_ty, ptr, "resume_value")?;
            let v = CgValue {
                ty: resume_value_ty,
                value: Some(loaded),
            };
            let _stored = self.store_local_value(span, target_ptr, resume_value_ty, v)?;
        }

        let mut value: CgValue<'ctx> = CgValue::unit();
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if idx <= perform_idx {
                continue;
            }
            let is_last = idx + 1 == handle.body.stmts.len();
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
                hir::StmtKind::Return { .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "`return` inside handle resume body",
                        at: stmt.span.into(),
                    });
                }
                hir::StmtKind::While { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement inside handle resume body",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        let value = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(handle.body.span, value, out_ty)?
        };
        if let Some(ptr) = result_ptr {
            let _ = self.store_local_value(handle.body.span, ptr, out_ty, value)?;
        }
        self.builder.build_unconditional_branch(done_bb)?;

        // --- done：读取并返回结果 ---
        self.builder.position_at_end(done_bb);

        // handle 结束：pop handler frame（动态上下文）。
        let rt_pop = self.declare_runtime_effect_handler_stack_pop();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 = self.builder.build_bit_cast(
            handler_frame_ptr,
            i8_ptr_ty,
            "handle_resume_effect_frame_i8",
        )?;
        let _ = self
            .builder
            .build_call(rt_pop, &[frame_i8.into()], "handle_resume_effect_pop")?;

        self.env.pop_scope();

        Ok(match out_ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                let Some(ptr) = result_ptr else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle result slot",
                        at: span.into(),
                    });
                };
                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self.builder.build_load(llvm_ty, ptr, "handle_result")?;
                CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                }
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle result type",
                    at: span.into(),
                });
            }
        })
    }

    pub(super) fn codegen_handle_expr_escape_continuation(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        arm: &hir::HandleArm,
        continuation_symbol: hir::SymbolId,
        seq: u32,
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // T0617：`Effect.op(...), k -> { ... }`
        //
        // 当前阶段（最小可回归落点）：
        // - 仅支持单个 arm（在外层已校验）；
        // - 对匹配当前 arm 的 op：
        //   - 0 个 perform：退化为顺序执行 `body`（以及 `finally`，若存在），arm 不可达（T1606a）；
        //   - 1 个 perform：允许在 perform 前存在普通语句（val/assign/expr，T1606b）；
        // - heap state machine 先只承载 handler frame，并用 step trampoline 执行 perform 之后的剩余语句；
        // - continuation one-shot 与 handler stack 捕获由 runtime（T0914/T0915a）保证。

        // 1) 在 handle body 中找到唯一的 perform 点（当前阶段只支持 `val x: T = perform`）。
        let mut perform_site: Option<(usize, &hir::ValDecl, &hir::EffectOpRef, &[hir::CallArg])> =
            None;
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            match &stmt.kind {
                hir::StmtKind::Val(decl) => {
                    let Some(init) = decl.init.as_ref() else {
                        continue;
                    };
                    if let hir::ExprKind::Perform { op, args } = &init.kind {
                        if op.fqn != arm.op.op.fqn {
                            continue;
                        }
                        if perform_site.is_some() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle escape body (multiple perform points)",
                                at: init.span.into(),
                            });
                        }
                        perform_site = Some((idx, decl, op, args.as_slice()));
                    }
                }
                hir::StmtKind::Expr(expr) => {
                    if let hir::ExprKind::Perform { op, .. } = &expr.kind {
                        if op.fqn == arm.op.op.fqn {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle escape body (perform must be bound to val)",
                                at: expr.span.into(),
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        let Some((perform_idx, perform_decl, perform_op, perform_args)) = perform_site else {
            // T1606a：没有匹配 op 的 perform 点，arm 不可达；退化为顺序执行 `body -> finally` 并返回 body 值。
            return self.codegen_handle_expr_no_perform(span, handle, out_ty);
        };
        if handle.finally.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle finally (escape continuation)",
                at: span.into(),
            });
        }
        if perform_op.fqn != arm.op.op.fqn {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle escape op mismatch",
                at: perform_op.span.into(),
            });
        }
        if arm.op.binders.len() != perform_args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle escape binder arity mismatch",
                at: arm.op.span.into(),
            });
        }

        let Some(perform_id) = perform_decl.id else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle escape perform binding id",
                at: perform_decl.span.into(),
            });
        };

        let resume_value_ty =
            self.cg_ty_of(perform_decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape perform value type",
                    at: perform_decl.span.into(),
                })?;

        // 2) 生成 step trampoline：`void step(void* state, uint64_t resume_value)`
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

        let func_name = func.get_name().to_str().unwrap_or("anonymous").to_string();
        let func_name = sanitize_llvm_ident(&func_name);

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();

        // 1.5) 计算 perform 之后会用到的 locals（用于决定必须 lift 到 heap state 的 capture 集合）。
        //
        // 说明：
        // - step trampoline 执行在“原函数栈已不存在”的异步时刻；
        // - 因此：perform 之后引用到的“外层 locals / perform 前 locals”必须从 heap state 恢复；
        // - perform 之后新引入的 locals（val/var）会在 step 内按顺序声明，不需要 capture。
        fn collect_used_locals_in_block(block: &hir::Block, out: &mut HashSet<hir::SymbolId>) {
            for stmt in &block.stmts {
                collect_used_locals_in_stmt(stmt, out);
            }
        }

        fn collect_used_locals_in_stmt(stmt: &hir::Stmt, out: &mut HashSet<hir::SymbolId>) {
            match &stmt.kind {
                hir::StmtKind::Empty
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {}
                hir::StmtKind::Expr(expr) => collect_used_locals_in_expr(expr, out),
                hir::StmtKind::Val(decl) => {
                    if let Some(init) = &decl.init {
                        collect_used_locals_in_expr(init, out);
                    }
                }
                hir::StmtKind::Assign { lhs, rhs, .. } => {
                    collect_used_locals_in_expr(lhs, out);
                    collect_used_locals_in_expr(rhs, out);
                }
                hir::StmtKind::While { cond, body } => {
                    collect_used_locals_in_expr(cond, out);
                    collect_used_locals_in_block(body, out);
                }
                hir::StmtKind::Return { value } => {
                    if let Some(v) = value {
                        collect_used_locals_in_expr(v, out);
                    }
                }
            }
        }

        fn collect_used_locals_in_expr(expr: &hir::Expr, out: &mut HashSet<hir::SymbolId>) {
            match &expr.kind {
                hir::ExprKind::Missing
                | hir::ExprKind::Literal(_)
                | hir::ExprKind::UnresolvedIdent { .. }
                | hir::ExprKind::Todo(_) => {}
                hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                    out.insert(*id);
                }
                hir::ExprKind::VarRef(hir::ValueRef::TopLevel { .. }) => {}
                hir::ExprKind::StructLit { fields, .. } => {
                    for f in fields {
                        collect_used_locals_in_expr(&f.value, out);
                    }
                }
                hir::ExprKind::TupleLit { elements } => {
                    for e in elements {
                        collect_used_locals_in_expr(e, out);
                    }
                }
                hir::ExprKind::InterpolatedString { parts, .. } => {
                    for p in parts {
                        if let hir::InterpolatedStringPart::Expr { expr } = p {
                            collect_used_locals_in_expr(expr, out);
                        }
                    }
                }
                hir::ExprKind::Unary { expr, .. } => {
                    collect_used_locals_in_expr(expr.as_ref(), out)
                }
                hir::ExprKind::Binary { lhs, rhs, .. } => {
                    collect_used_locals_in_expr(lhs.as_ref(), out);
                    collect_used_locals_in_expr(rhs.as_ref(), out);
                }
                hir::ExprKind::TypeCheck { expr, .. } | hir::ExprKind::Cast { expr, .. } => {
                    collect_used_locals_in_expr(expr.as_ref(), out);
                }
                hir::ExprKind::Block(block) => collect_used_locals_in_block(block, out),
                hir::ExprKind::Closure(closure) => {
                    collect_used_locals_in_expr(closure.body.as_ref(), out);
                }
                hir::ExprKind::If {
                    cond,
                    then_branch,
                    else_branch,
                } => {
                    collect_used_locals_in_expr(cond, out);
                    collect_used_locals_in_expr(then_branch, out);
                    if let Some(e) = else_branch.as_deref() {
                        collect_used_locals_in_expr(e, out);
                    }
                }
                hir::ExprKind::When { subject, arms } => {
                    collect_used_locals_in_expr(subject, out);
                    for arm in arms {
                        if let Some(g) = &arm.guard {
                            collect_used_locals_in_expr(g, out);
                        }
                        collect_used_locals_in_expr(&arm.body, out);
                    }
                }
                hir::ExprKind::MemberAccess { receiver, .. } => {
                    collect_used_locals_in_expr(receiver, out)
                }
                hir::ExprKind::Call { callee, args } => {
                    collect_used_locals_in_expr(callee, out);
                    for arg in args {
                        match arg {
                            hir::CallArg::Positional(expr) => {
                                collect_used_locals_in_expr(expr, out)
                            }
                            hir::CallArg::Named { value, .. } => {
                                collect_used_locals_in_expr(value, out)
                            }
                        }
                    }
                }
                hir::ExprKind::Perform { args, .. } => {
                    for arg in args {
                        match arg {
                            hir::CallArg::Positional(expr) => {
                                collect_used_locals_in_expr(expr, out)
                            }
                            hir::CallArg::Named { value, .. } => {
                                collect_used_locals_in_expr(value, out)
                            }
                        }
                    }
                }
                hir::ExprKind::Handle(handle) => {
                    collect_used_locals_in_block(&handle.body, out);
                    for arm in &handle.arms {
                        collect_used_locals_in_expr(&arm.body, out);
                    }
                    if let Some(finally) = &handle.finally {
                        collect_used_locals_in_block(finally, out);
                    }
                }
            }
        }

        let mut used_after_perform: HashSet<hir::SymbolId> = HashSet::new();
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if idx <= perform_idx {
                continue;
            }
            collect_used_locals_in_stmt(stmt, &mut used_after_perform);
        }

        let mut locals_declared_after_perform: HashSet<hir::SymbolId> = HashSet::new();
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if idx <= perform_idx {
                continue;
            }
            if let hir::StmtKind::Val(decl) = &stmt.kind {
                if let Some(id) = decl.id {
                    locals_declared_after_perform.insert(id);
                }
            }
        }

        // escape continuation：把当前作用域内的引用类型 locals 捕获到 heap state 中，
        // 以便在 step trampoline（异步 resume）里继续访问它们。
        //
        // 注意：
        // - 当前 v0 实现捕获 `Ref/String/Bool/Int`：
        //   - `Ref/String`：用于保活 closure/env 等引用类型；
        //   - `Bool/Int`：用于保活 word-sized handle（例如 sysroot 的 `Task<T>`/`Executor` 早期落点）。
        // - 这里按“当前可见的绑定”去重（内层 scope shadow 外层），并按 SymbolId 排序保证 determinism。
        struct CapturedLocal {
            id: hir::SymbolId,
            hir_ty: Option<TypeId>,
            ty: CgTy,
            mutable: bool,
        }

        let mut pre_perform_decl_by_id: HashMap<hir::SymbolId, &hir::ValDecl> = HashMap::new();
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if idx >= perform_idx {
                break;
            }
            let hir::StmtKind::Val(decl) = &stmt.kind else {
                continue;
            };
            let Some(id) = decl.id else {
                continue;
            };
            pre_perform_decl_by_id.insert(id, decl);
        }

        // 1) outer locals：来自当前 codegen env
        let mut captures: Vec<CapturedLocal> = Vec::new();
        let mut seen: HashSet<hir::SymbolId> = HashSet::new();
        for scope in self.env.scopes.iter().rev() {
            for (&id, &local) in scope.iter() {
                if !seen.insert(id) {
                    continue;
                }
                if matches!(
                    local.ty,
                    CgTy::Ref | CgTy::String | CgTy::Bool | CgTy::Int(_)
                ) {
                    captures.push(CapturedLocal {
                        id,
                        hir_ty: local.hir_ty,
                        ty: local.ty,
                        mutable: local.mutable,
                    });
                }
            }
        }

        // 2) perform 前 locals：只捕获 perform 后确实会用到的 bindings（否则会无谓膨胀 state）。
        for &id in used_after_perform.iter() {
            if id == perform_id {
                continue;
            }
            if locals_declared_after_perform.contains(&id) {
                continue;
            }

            let Some(decl) = pre_perform_decl_by_id.get(&id).copied() else {
                continue;
            };
            if !seen.insert(id) {
                continue;
            }

            let decl_ty = self
                .cg_ty_of(decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape capture pre-perform local type",
                    at: decl.span.into(),
                })?;

            if !matches!(
                decl_ty,
                CgTy::Ref | CgTy::String | CgTy::Bool | CgTy::Int(_)
            ) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape capture local type",
                    at: decl.span.into(),
                });
            }

            captures.push(CapturedLocal {
                id,
                hir_ty: Some(decl.ty),
                ty: decl_ty,
                mutable: decl.mutable,
            });
        }

        captures.sort_by_key(|c| c.id.as_u32());

        let state_ty_name = format!("scoop.runtime.ContState__{func_name}_{seq}");
        let state_ty = if let Some(existing) = self.context.get_struct_type(&state_ty_name) {
            existing
        } else {
            let ty = self.context.opaque_struct_type(&state_ty_name);
            let header_ty = self.llvm_gc_object_header_type();
            let frame_ty = self.llvm_effect_handler_frame_type();
            let mut fields: Vec<BasicTypeEnum<'ctx>> = Vec::new();
            fields.push(header_ty.into());
            fields.push(frame_ty.into());
            for cap in &captures {
                fields.push(match cap.ty {
                    CgTy::Ref => gc_i8_ptr_ty.into(),
                    CgTy::String => gc_i8_ptr_ty.into(),
                    CgTy::Bool | CgTy::Int(_) => i64_ty.into(),
                    _ => unreachable!("captures filtered by type"),
                });
            }
            ty.set_body(&fields, false);
            ty
        };

        let step_name = format!("__scoop_cont_step__{func_name}_{seq}");
        let step_fn_ty = self
            .context
            .void_type()
            .fn_type(&[gc_i8_ptr_ty.into(), i64_ty.into()], false);
        let step_fn = self.module.add_function(&step_name, step_fn_ty, None);
        step_fn.set_linkage(Linkage::Internal);

        // 保存外层插入点：step 生成会重定位 builder。
        let saved_block = insert_block;

        // 生成 step 函数体：执行 perform 之后的剩余语句（state 参数当前阶段仅用于 keep-alive handler frame）。
        {
            let mut cg = MainCodegen::new(
                self.context,
                self.module,
                self.builder,
                self.target_data,
                self.host,
                self.source,
                self.types,
                self.struct_layouts,
                self.enum_layouts,
                self.top_level_vars,
                self.object_inits,
                self.class_inits,
                self.class_vtables,
                self.interfaces,
                self.class_itables,
                self.ctor_call_sites,
                self.extern_funs,
                self.fun_index,
            );

            let entry = self.context.append_basic_block(step_fn, "entry");
            cg.builder.position_at_end(entry);

            // step 为内部 trampoline：返回类型固定为 Unit。
            cg.current_fun_return_ty = Some(CgTy::Unit);

            cg.env.push_scope();

            // 恢复 captures：step 函数运行在“原函数栈已不存在”的异步时刻，
            // 因此需要从 heap state 里把所需 locals 读回到本函数的 env。
            if !captures.is_empty() {
                let state_raw = step_fn
                    .get_nth_param(0)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "continuation step state param",
                        at: span.into(),
                    })?
                    .into_pointer_value();
                let state_ptr_ty = state_ty.ptr_type(cg.gc_address_space());
                let state_ptr = cg.builder.build_pointer_cast(
                    state_raw,
                    state_ptr_ty,
                    "cont_step_state_ptr",
                )?;

                for (idx, cap) in captures.iter().enumerate() {
                    let field_idx = 2u32.saturating_add(idx as u32);
                    let field_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        field_idx,
                        "cont_step_capture_gep",
                    )?;
                    let name = format!("capture_{}", cap.id.as_u32());
                    match cap.ty {
                        CgTy::Ref => {
                            let loaded = cg
                                .builder
                                .build_load(gc_i8_ptr_ty, field_ptr, "cont_step_capture_load_ref")?
                                .into_pointer_value();
                            let ptr = cg.create_entry_alloca(span, &name, CgTy::Ref)?;
                            let _ = cg.builder.build_store(ptr, loaded)?;
                            cg.env.insert(
                                cap.id,
                                CgLocal {
                                    hir_ty: cap.hir_ty,
                                    ty: CgTy::Ref,
                                    ptr,
                                    mutable: cap.mutable,
                                },
                            );
                        }
                        CgTy::String => {
                            let loaded = cg
                                .builder
                                .build_load(gc_i8_ptr_ty, field_ptr, "cont_step_capture_load_str")?
                                .into_pointer_value();
                            let str_ptr_ty = cg.llvm_scoop_string_ptr_type();
                            let casted = cg.builder.build_pointer_cast(
                                loaded,
                                str_ptr_ty,
                                "cont_step_capture_str",
                            )?;
                            let ptr = cg.create_entry_alloca(span, &name, CgTy::String)?;
                            let _ = cg.builder.build_store(ptr, casted)?;
                            cg.env.insert(
                                cap.id,
                                CgLocal {
                                    hir_ty: cap.hir_ty,
                                    ty: CgTy::String,
                                    ptr,
                                    mutable: cap.mutable,
                                },
                            );
                        }
                        CgTy::Bool => {
                            let loaded = cg
                                .builder
                                .build_load(i64_ty, field_ptr, "cont_step_capture_load_bool")?
                                .into_int_value();
                            let zero = i64_ty.const_zero();
                            let b = cg.builder.build_int_compare(
                                IntPredicate::NE,
                                loaded,
                                zero,
                                "cont_step_capture_bool",
                            )?;
                            let ptr = cg.create_entry_alloca(span, &name, CgTy::Bool)?;
                            let _ = cg.builder.build_store(ptr, b)?;
                            cg.env.insert(
                                cap.id,
                                CgLocal {
                                    hir_ty: cap.hir_ty,
                                    ty: CgTy::Bool,
                                    ptr,
                                    mutable: cap.mutable,
                                },
                            );
                        }
                        CgTy::Int(int_ty) => {
                            if int_ty.bits > 64 {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "cont state capture int width > 64",
                                    at: span.into(),
                                });
                            }

                            let loaded = cg
                                .builder
                                .build_load(i64_ty, field_ptr, "cont_step_capture_load_int")?
                                .into_int_value();
                            let to = cg.int_type(int_ty);
                            let v = if int_ty.bits == 64 {
                                loaded
                            } else {
                                cg.builder.build_int_truncate(
                                    loaded,
                                    to,
                                    "cont_step_capture_trunc",
                                )?
                            };

                            let slot_ty = CgTy::Int(int_ty);
                            let ptr = cg.create_entry_alloca(span, &name, slot_ty)?;
                            let _ = cg.builder.build_store(ptr, v)?;
                            cg.env.insert(
                                cap.id,
                                CgLocal {
                                    hir_ty: cap.hir_ty,
                                    ty: slot_ty,
                                    ptr,
                                    mutable: cap.mutable,
                                },
                            );
                        }
                        _ => unreachable!("captures filtered by type"),
                    }
                }
            }

            // v0：只支持把 resume_value 当作一个 word-sized payload 写回到 perform binding。
            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "continuation step resume param",
                    at: span.into(),
                })?
                .into_int_value();

            let local_name = perform_decl.name.as_deref().unwrap_or("resume_value");
            let target_ptr = cg.create_entry_alloca(span, local_name, resume_value_ty)?;

            let resume_value = match resume_value_ty {
                CgTy::Unit => CgValue::unit(),
                CgTy::Bool => {
                    let zero = i64_ty.const_int(0, false);
                    let b = cg.builder.build_int_compare(
                        IntPredicate::NE,
                        resume_word,
                        zero,
                        "resume_bool",
                    )?;
                    CgValue::bool(b)
                }
                CgTy::Int(int_ty) => {
                    let to = cg.int_type(int_ty);
                    let v = if int_ty.bits == 64 {
                        resume_word
                    } else {
                        cg.builder
                            .build_int_truncate(resume_word, to, "resume_int")?
                    };
                    CgValue::int(v, int_ty)
                }
                CgTy::String | CgTy::Ref => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "continuation resume payload (gc ptr via u64 is forbidden)",
                        at: perform_decl.span.into(),
                    });
                }
                CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "continuation resume payload type",
                        at: perform_decl.span.into(),
                    });
                }
            };

            let _stored = cg.store_local_value(span, target_ptr, resume_value_ty, resume_value)?;
            cg.env.insert(
                perform_id,
                CgLocal {
                    hir_ty: Some(perform_decl.ty),
                    ty: resume_value_ty,
                    ptr: target_ptr,
                    mutable: false,
                },
            );

            // 执行 perform 之后的剩余语句。
            let mut _value: CgValue<'ctx> = CgValue::unit();
            for (idx, stmt) in handle.body.stmts.iter().enumerate() {
                if idx <= perform_idx {
                    continue;
                }
                match &stmt.kind {
                    hir::StmtKind::Empty => {}
                    hir::StmtKind::Val(decl) => {
                        cg.codegen_val_decl(decl)?;
                        _value = CgValue::unit();
                    }
                    hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                        cg.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                        _value = CgValue::unit();
                    }
                    hir::StmtKind::Expr(expr) => {
                        let _ = cg.codegen_expr(expr)?;
                        _value = CgValue::unit();
                    }
                    hir::StmtKind::Return { .. } => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "`return` inside continuation step",
                            at: stmt.span.into(),
                        });
                    }
                    hir::StmtKind::While { .. }
                    | hir::StmtKind::Break { .. }
                    | hir::StmtKind::Continue { .. }
                    | hir::StmtKind::Todo(_) => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "statement inside continuation step",
                            at: stmt.span.into(),
                        });
                    }
                }
            }

            cg.env.pop_scope();
            cg.builder.build_return(None)?;
        }

        // 恢复外层插入点。
        self.builder.position_at_end(saved_block);

        // 3) 生成 handle 的初始执行：push handler frame → 在 perform 点创建 continuation → 执行 arm → 返回。
        let body_bb = self.context.append_basic_block(func, "handle_escape_body");
        let arm_bb = self.context.append_basic_block(func, "handle_escape_arm");
        let done_bb = self.context.append_basic_block(func, "handle_escape_done");

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_escape_result", out_ty)?)
        };

        // binder slots：在 perform 点写入，在 arm body 中读取。
        struct BinderSlot<'ctx> {
            id: hir::SymbolId,
            hir_ty: TypeId,
            ty: CgTy,
            ptr: PointerValue<'ctx>,
        }
        let mut binder_slots: Vec<BinderSlot<'ctx>> = Vec::new();
        for binder in &arm.op.binders {
            let binder_ty = self
                .cg_ty_of(binder.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape binder type",
                    at: binder.span.into(),
                })?;
            let ptr = self.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
            binder_slots.push(BinderSlot {
                id: binder.id,
                hir_ty: binder.ty,
                ty: binder_ty,
                ptr,
            });
        }

        // continuation binder local：在 perform 点写入，在 arm body 中读取。
        let cont_ptr =
            self.create_entry_alloca(span, &format!("handle_escape_k_{seq}"), CgTy::Ref)?;

        self.builder.build_unconditional_branch(body_bb)?;

        // --- body ---
        self.builder.position_at_end(body_bb);
        self.env.push_scope();

        // heap state：`{ header, handler_frame, captured_refs... }`
        let total_size = self.target_data.get_store_size(&state_ty);

        // 分配点统一走 typed alloc：在 runtime 内部写入对象头 `type_desc`，确保 GC 能扫描 capture fields。
        let state_desc_global_name = format!("__scoop_type_desc_cont_state__{func_name}_{seq}");
        let size_bytes = self.target_data.get_store_size(&state_ty);
        let trace_start_offset_bytes = self
            .target_data
            .offset_of_element(&state_ty, 2)
            .unwrap_or(size_bytes);
        let state_desc = self.get_or_create_type_descriptor_global(
            span,
            &state_desc_global_name,
            &state_ty_name,
            state_ty,
            trace_start_offset_bytes,
            None,
            None,
            None,
        )?;

        let rt_alloc = self.declare_runtime_alloc_typed();
        let size_v = i64_ty.const_int(total_size, false);
        let state_desc_i8 = self.builder.build_pointer_cast(
            state_desc.as_pointer_value(),
            i8_ptr_ty,
            "cont_state_type_desc_i8",
        )?;
        let call = self.builder.build_call(
            rt_alloc,
            &[state_desc_i8.into(), size_v.into()],
            "rt_alloc_cont_state",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(state_raw) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return type",
                at: span.into(),
            });
        };

        let state_ptr_ty = state_ty.ptr_type(self.gc_address_space());
        let state_ptr =
            self.builder
                .build_pointer_cast(state_raw, state_ptr_ty, "cont_state_ptr")?;
        let frame_ptr =
            self.builder
                .build_struct_gep(state_ty, state_ptr, 1, "cont_state_frame_gep")?;

        // typed alloc 下，GC 会按 type_desc 扫描 capture fields。
        //
        // 为避免在执行 perform 前语句（其中可能触发分配/GC）时扫描到未初始化垃圾值，
        // 这里先把所有 capture fields 置零；在 perform 点再写入实际捕获值。
        for (idx, cap) in captures.iter().enumerate() {
            let field_idx = 2u32.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_ptr,
                field_idx,
                "cont_state_capture_init_gep",
            )?;
            match cap.ty {
                CgTy::Ref | CgTy::String => {
                    let _ = self
                        .builder
                        .build_store(field_ptr, gc_i8_ptr_ty.const_null())?;
                }
                CgTy::Bool | CgTy::Int(_) => {
                    let _ = self.builder.build_store(field_ptr, i64_ty.const_zero())?;
                }
                _ => unreachable!("captures filtered by type"),
            }
        }

        // push handler frame（动态上下文）。
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let frame_i8 = self.builder.build_address_space_cast(
            frame_ptr,
            i8_ptr_ty,
            "handle_escape_frame_i8",
        )?;
        let op_tag_i32 = if arm.op.op.fqn == "scoop.core.Raise.raise" {
            self.context.i32_type().const_int(1, false)
        } else {
            self.context.i32_type().const_zero()
        };
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), op_tag_i32.into()],
            "handle_escape_effect_push",
        )?;

        // 执行 perform 之前的语句（仍在 handler scope 内）。
        //
        // 说明：
        // - 当前阶段只支持单 perform 点；perform 之后的语句由 step trampoline 负责；
        // - perform 前若出现更复杂控制流（return/loop 等），需要更完整的 state machine 语义（T1606c）。
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if idx >= perform_idx {
                break;
            }
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    self.codegen_val_decl(decl)?;
                }
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                }
                hir::StmtKind::Expr(expr) => {
                    let _ = self.codegen_expr(expr)?;
                }
                hir::StmtKind::Return { .. }
                | hir::StmtKind::While { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement before perform (escape continuation)",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        // 把当前作用域内的 locals 写入 heap state：用于 step trampoline 在异步 resume 时恢复 env。
        for (idx, cap) in captures.iter().enumerate() {
            let field_idx = 2u32.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_ptr,
                field_idx,
                "cont_state_capture_gep",
            )?;

            let local = self
                .env
                .get(cap.id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "cont state capture local not found",
                    at: span.into(),
                })?;
            if local.ty != cap.ty {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "cont state capture local type mismatch",
                    at: span.into(),
                });
            }

            match cap.ty {
                CgTy::Ref => {
                    let llvm_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
                    let loaded = self.builder.build_load(
                        llvm_ty,
                        local.ptr,
                        "cont_state_capture_load_ref",
                    )?;
                    let BasicValueEnum::PointerValue(ptr) = loaded else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "cont state capture value type (ref ptr)",
                            at: span.into(),
                        });
                    };
                    let casted = self.builder.build_pointer_cast(
                        ptr,
                        gc_i8_ptr_ty,
                        "cont_state_capture_ref_i8",
                    )?;
                    let _ = self.store_local_value(
                        span,
                        field_ptr,
                        CgTy::Ref,
                        CgValue {
                            ty: CgTy::Ref,
                            value: Some(casted.into()),
                        },
                    )?;
                }
                CgTy::String => {
                    let llvm_ty = self.llvm_basic_type_of(span, CgTy::String)?;
                    let loaded = self.builder.build_load(
                        llvm_ty,
                        local.ptr,
                        "cont_state_capture_load_str",
                    )?;
                    let BasicValueEnum::PointerValue(ptr) = loaded else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "cont state capture value type (str ptr)",
                            at: span.into(),
                        });
                    };
                    let casted = self.builder.build_pointer_cast(
                        ptr,
                        gc_i8_ptr_ty,
                        "cont_state_capture_str_i8",
                    )?;
                    let _ = self.store_local_value(
                        span,
                        field_ptr,
                        CgTy::Ref,
                        CgValue {
                            ty: CgTy::Ref,
                            value: Some(casted.into()),
                        },
                    )?;
                }
                CgTy::Bool => {
                    let loaded = self
                        .builder
                        .build_load(
                            self.llvm_basic_type_of(span, CgTy::Bool)?,
                            local.ptr,
                            "cont_state_capture_load_bool",
                        )?
                        .into_int_value();
                    let extended = self.builder.build_int_z_extend(
                        loaded,
                        i64_ty,
                        "cont_state_capture_zext_bool",
                    )?;
                    let _ = self.builder.build_store(field_ptr, extended)?;
                }
                CgTy::Int(int_ty) => {
                    if int_ty.bits > 64 {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "cont state capture int width > 64",
                            at: span.into(),
                        });
                    }

                    let llvm_ty = self.llvm_basic_type_of(span, cap.ty)?;
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, local.ptr, "cont_state_capture_load_int")?
                        .into_int_value();
                    let extended = if int_ty.bits == 64 {
                        loaded
                    } else if int_ty.signed {
                        self.builder.build_int_s_extend(
                            loaded,
                            i64_ty,
                            "cont_state_capture_sext_int",
                        )?
                    } else {
                        self.builder.build_int_z_extend(
                            loaded,
                            i64_ty,
                            "cont_state_capture_zext_int",
                        )?
                    };
                    let _ = self.builder.build_store(field_ptr, extended)?;
                }
                _ => unreachable!("captures filtered by type"),
            }
        }

        // --- perform site：计算 args → 写 binder slots → 创建 continuation ---
        for (slot, arg) in binder_slots.iter().zip(perform_args.iter()) {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape named perform arg",
                    at: span.into(),
                });
            };
            let v = self.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
            let _stored = self.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
        }

        let rt_cont_alloc = self.declare_runtime_continuation_alloc();
        let step_ptr = step_fn.as_global_value().as_pointer_value();
        let call = self.builder.build_call(
            rt_cont_alloc,
            &[state_raw.into(), step_ptr.into()],
            "cont_alloc",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "continuation alloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(k_raw) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "continuation alloc return type",
                at: span.into(),
            });
        };

        let _stored = self.store_local_value(
            span,
            cont_ptr,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(k_raw.into()),
            },
        )?;

        // 将 handler frame 从当前线程的 handler stack 顶部“摘除”（不清理 frame 字段），以便：
        // - handler arm body 在 dispatch scope 外执行（Appendix A.4）
        // - continuation 捕获的 handler stack（frame->prev 链）保持完整（spec §5.5）
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let prev_ptr = self.builder.build_struct_gep(
            handler_frame_ty,
            frame_ptr,
            0,
            "handle_escape_prev_gep",
        )?;
        let prev_raw = self
            .builder
            .build_load(i8_ptr_ty, prev_ptr, "handle_escape_prev")?;
        let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
        let _ = self
            .builder
            .build_call(rt_swap, &[prev_raw.into()], "handle_escape_detach")?;

        // body locals 不应在 arm scope 可见：提前 pop。
        self.env.pop_scope();

        self.builder.build_unconditional_branch(arm_bb)?;

        // --- arm ---
        self.builder.position_at_end(arm_bb);
        self.env.push_scope();
        for slot in &binder_slots {
            self.env.insert(
                slot.id,
                CgLocal {
                    hir_ty: Some(slot.hir_ty),
                    ty: slot.ty,
                    ptr: slot.ptr,
                    mutable: false,
                },
            );
        }
        self.env.insert(
            continuation_symbol,
            CgLocal {
                hir_ty: None,
                ty: CgTy::Ref,
                ptr: cont_ptr,
                mutable: false,
            },
        );

        let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
        let arm_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(arm.body.span, arm_v, out_ty)?
        };
        if let Some(ptr) = result_ptr {
            let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
        }

        self.env.pop_scope();
        self.builder.build_unconditional_branch(done_bb)?;

        // --- done ---
        self.builder.position_at_end(done_bb);

        Ok(match out_ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                let Some(ptr) = result_ptr else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle escape result slot",
                        at: span.into(),
                    });
                };
                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, ptr, "handle_escape_result")?;
                CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                }
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape result type",
                    at: span.into(),
                })?
            }
        })
    }

    pub(super) fn codegen_immediate_resume_call(
        &mut self,
        span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
        ctx: ImmediateResumeCtx<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 语义：写回 resume value + 更新 state + 跳回 dispatch。
        //
        // 当前阶段（T0616）约束：
        // - 仅支持一个位置实参：`resume(value)`；
        // - 多次 resume 先按运行期错误处理（exit(3)）。
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "resume() arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(value_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "resume() named arg",
                at: span.into(),
            });
        };

        let value = self.codegen_expr_in_expected_context(value_expr, Some(ctx.resume_value_ty))?;
        let value = self.coerce_value(value_expr.span, value, ctx.resume_value_ty)?;

        // one-shot（运行期断言）：重复调用 resume 直接退出。
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

        let ok_bb = self.context.append_basic_block(func, "resume_ok");
        let err_bb = self.context.append_basic_block(func, "resume_twice");
        let cont_bb = self.context.append_basic_block(func, "resume_cont");

        let used = self
            .builder
            .build_load(self.context.bool_type(), ctx.resume_used_ptr, "resume_used")?
            .into_int_value();
        self.builder.build_conditional_branch(used, err_bb, ok_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        let _ = self.builder.build_store(
            ctx.resume_used_ptr,
            self.context.bool_type().const_int(1, false),
        )?;

        if let Some(ptr) = ctx.resume_value_ptr {
            let Some(raw) = value.value else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "resume(value) arg value",
                    at: value_expr.span.into(),
                });
            };
            let _ = self.builder.build_store(ptr, raw)?;
        }

        let _ = self.builder.build_store(
            ctx.state_ptr,
            self.context
                .i32_type()
                .const_int(ctx.next_state as u64, false),
        )?;

        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);

        Ok(match expected {
            Some(ty) => self.default_value(ty),
            None => CgValue::unit(),
        })
    }

    pub(super) fn codegen_continuation_resume_call(
        &mut self,
        span: crate::span::Span,
        receiver: &hir::Expr,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // spec §5.5：`k.resume(value)`。
        //
        // 约束（early stage）：
        // - 仅支持一个位置实参；
        // - `value` 会被编码为一个 `u64` word 传给 runtime（T0914：`scoop_continuation_resume_u64`）。
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(value_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume named arg",
                at: span.into(),
            });
        };

        let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::Ref))?;
        let recv = self.coerce_value(receiver.span, recv, CgTy::Ref)?;
        let Some(recv_raw) = recv.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(k_ptr) = recv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume receiver type",
                at: receiver.span.into(),
            });
        };

        let value = self.codegen_expr(value_expr)?;
        let word = self.coerce_u64_word(value_expr.span, value)?;

        let rt_resume = self.declare_runtime_continuation_resume_u64();
        let k_i8 =
            self.builder
                .build_pointer_cast(k_ptr, self.llvm_gc_i8_ptr_type(), "cont_k_i8")?;
        let _ = self
            .builder
            .build_call(rt_resume, &[k_i8.into(), word.into()], "cont_resume")?;
        // continuation resume 可能触发 `Raise<RuntimeError>`（例如 one-shot 违规），需要按 Raise 的最小约定传播。
        self.emit_effect_unwind_if_active(span)?;

        Ok(CgValue::unit())
    }

    pub(super) fn coerce_u64_word(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        // 将一个可表示为 “word-sized u64 payload” 的值转换为 `i64`（在 ABI 层作为 `uint64_t` 使用）。
        //
        // 注意：这里不引入额外的 tag/布局；更复杂的 payload 由 TODO T0630 扩展。
        let i64_ty = self.context.i64_type();
        match value.ty {
            CgTy::Unit => Ok(i64_ty.const_int(0, false)),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from bool",
                    at: at.into(),
                })?;
                Ok(self.builder.build_int_z_extend(b, i64_ty, "bool_to_u64")?)
            }
            CgTy::Int(_) => {
                let (raw, from) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from int",
                    at: at.into(),
                })?;
                let to = IntTy {
                    bits: 64,
                    signed: false,
                };
                Ok(self.cast_int(raw, from, to)?)
            }
            CgTy::String | CgTy::Ref => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "u64 word from gc pointer (ptr<->int is forbidden)",
                at: at.into(),
            }),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from composite value",
                    at: at.into(),
                })
            }
        }
    }

    pub(super) fn codegen_sysroot_effect_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let _handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };

        match fqn {
            "scoop.core.__scoop_effect_is_active" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_is_active();
                let call = self.builder.build_call(rt, &[], "effect_is_active")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 32,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            "scoop.core.__scoop_effect_set_active" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect set_active arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_set_active();
                let _ = self.builder.build_call(rt, &[], "effect_set_active")?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_clear" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect clear arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_clear();
                let _ = self.builder.build_call(rt, &[], "effect_clear")?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_write" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(tag_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write tag named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(value_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write value named arg",
                        at: span.into(),
                    });
                };

                let tag_v =
                    self.codegen_expr_in_expected_context(tag_expr, Some(CgTy::Int(value_word)))?;
                let tag_v = self.coerce_value(tag_expr.span, tag_v, CgTy::Int(value_word))?;
                let (tag_raw, tag_from) =
                    tag_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write tag value",
                        at: tag_expr.span.into(),
                    })?;
                let tag_to = IntTy {
                    bits: 32,
                    signed: false,
                };
                let tag_i32 = self.cast_int(tag_raw, tag_from, tag_to)?;

                let value_v =
                    self.codegen_expr_in_expected_context(value_expr, Some(CgTy::Int(value_word)))?;
                let value_v = self.coerce_value(value_expr.span, value_v, CgTy::Int(value_word))?;
                let (value_raw, value_from) =
                    value_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write value",
                        at: value_expr.span.into(),
                    })?;
                let value_to = IntTy {
                    bits: 64,
                    signed: false,
                };
                let value_i64 = self.cast_int(value_raw, value_from, value_to)?;

                let rt = self.declare_runtime_effect_perform_slot_write_u64();
                let _ = self.builder.build_call(
                    rt,
                    &[tag_i32.into(), value_i64.into()],
                    "effect_slot_write",
                )?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_write2" => {
                if args.len() != 3 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(tag_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 tag named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(word0_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 word0 named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(word1_expr) = &args[2] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 word1 named arg",
                        at: span.into(),
                    });
                };

                let tag_v =
                    self.codegen_expr_in_expected_context(tag_expr, Some(CgTy::Int(value_word)))?;
                let tag_v = self.coerce_value(tag_expr.span, tag_v, CgTy::Int(value_word))?;
                let (tag_raw, tag_from) =
                    tag_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 tag value",
                        at: tag_expr.span.into(),
                    })?;
                let tag_to = IntTy {
                    bits: 32,
                    signed: false,
                };
                let tag_i32 = self.cast_int(tag_raw, tag_from, tag_to)?;

                let word0_v =
                    self.codegen_expr_in_expected_context(word0_expr, Some(CgTy::Int(value_word)))?;
                let word0_v = self.coerce_value(word0_expr.span, word0_v, CgTy::Int(value_word))?;
                let (word0_raw, word0_from) =
                    word0_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 word0 value",
                        at: word0_expr.span.into(),
                    })?;

                let word1_v =
                    self.codegen_expr_in_expected_context(word1_expr, Some(CgTy::Int(value_word)))?;
                let word1_v = self.coerce_value(word1_expr.span, word1_v, CgTy::Int(value_word))?;
                let (word1_raw, word1_from) =
                    word1_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 word1 value",
                        at: word1_expr.span.into(),
                    })?;

                let word_to = IntTy {
                    bits: 64,
                    signed: false,
                };
                let word0_i64 = self.cast_int(word0_raw, word0_from, word_to)?;
                let word1_i64 = self.cast_int(word1_raw, word1_from, word_to)?;

                let rt = self.declare_runtime_effect_perform_slot_write_u64_2();
                let _ = self.builder.build_call(
                    rt,
                    &[tag_i32.into(), word0_i64.into(), word1_i64.into()],
                    "effect_slot_write2",
                )?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_read_op_tag" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_op_tag();
                let call = self
                    .builder
                    .build_call(rt, &[], "effect_slot_read_op_tag")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 32,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            "scoop.core.__scoop_effect_slot_read_len_words" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self
                    .builder
                    .build_call(rt, &[], "effect_slot_read_len_words")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 32,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            "scoop.core.__scoop_effect_slot_read_value" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_u64();
                let call = self.builder.build_call(rt, &[], "effect_slot_read_u64")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 64,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            "scoop.core.__scoop_effect_slot_read_word" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(index_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word index named arg",
                        at: span.into(),
                    });
                };

                let index_v =
                    self.codegen_expr_in_expected_context(index_expr, Some(CgTy::Int(value_word)))?;
                let index_v = self.coerce_value(index_expr.span, index_v, CgTy::Int(value_word))?;
                let (index_raw, index_from) =
                    index_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word index value",
                        at: index_expr.span.into(),
                    })?;
                let index_to = IntTy {
                    bits: 32,
                    signed: false,
                };
                let index_i32 = self.cast_int(index_raw, index_from, index_to)?;

                let rt = self.declare_runtime_effect_perform_slot_read_u64_at();
                let call = self.builder.build_call(
                    rt,
                    &[index_i32.into()],
                    "effect_slot_read_word_u64",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 64,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown sysroot effect intrinsic callee",
                at: callee_span.into(),
            }),
        }
    }
}
