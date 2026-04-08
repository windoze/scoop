//! effect/continuation codegen（T0102e：从 `codegen/mod.rs` 拆分）。

use super::*;

/// flag-based unwinding（non-resuming effect）的"捕获边界"记录。
///
/// 说明：
/// - 当前阶段 `Raise.raise` 仍有独立的 `raise_target_stack`（历史原因，T0614）；
/// - T0625 起，为最小自定义 non-resuming effect 增加同样的"最近匹配"捕获边界栈，
///   用于在一个函数内把 `perform` 直接分发到最近的 `handle` catch block。
#[derive(Debug, Clone)]
pub(super) struct EffectUnwindTarget<'ctx> {
    op_fqn: String,
    target: inkwell::basic_block::BasicBlock<'ctx>,
}

/// `-> resume` lowering（T0616）在 codegen 阶段使用的"立即恢复"上下文。
///
/// 说明：
/// - 当前实现先只覆盖"单个 perform 点"的最小栈上 state machine；
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
    /// 说明：这里直接调用 runtime C ABI（`scoop_effect_is_active`），避免把该读取当作"普通函数调用"
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

    /// 在"最近 handler boundary"存在时跳转到 catch；否则返回默认值向外传播。
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
                // 其它 callee（closure/local/未解析）先按"可能 perform"保守处理，避免误删 handler。
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
        // 注意：当前阶段 HIR span 仍是"无 file-id 的 byte offsets"，当 codegen 生成跨文件函数体
        //（例如 stdlib/helper 被内联为可 codegen 的顶层函数）时，span 可能不属于入口 `source`。
        //
        // 为避免把"诊断辅助信息"升级成 hard error，这里选择在无法映射时降级为 (0, 0)：
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
    /// - `x!!` / `x as T` 等"运行期失败 → Raise<RuntimeError>"的语义落点；
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
        const PAYLOAD_KIND_RUNTIME_ERROR: u64 = 2;

        let i32_ty = self.context.i32_type();
        let u64_ty = self.context.i64_type();

        let raise_tag = self.effect_op_tag("scoop.core.Raise.raise");
        let op_tag_i32 = i32_ty.const_int(raise_tag as u64, false);
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

        // 继续生成后续 IR：把 builder 移到一个"不可达 continuation block"，避免后续插入失败。
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
        let raise_tag = self.effect_op_tag("scoop.core.Raise.raise");
        let tag_i32 = self.context.i32_type().const_int(raise_tag as u64, false);
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

        // 3) "早退"：在 handler boundary 内跳到 catch，否则返回默认值向外传播。
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

        // 4) 继续生成后续 IR：把 builder 移到一个"不可达 continuation block"，避免后续插入失败。
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
        // 这里返回一个"期望类型的默认值"以保持后续 codegen 可继续推进。
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
    ///   若不存在，则直接报错（避免与现有 `Raise` 的"返回默认值向外传播"机制混淆）。
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

        // T1608：使用统一的 op_tag 分配（按 FQN 精确匹配，与 handler_stack_push 一致）。
        let tag = self.effect_op_tag(&op.fqn);
        let op_tag_i32 = self.context.i32_type().const_int(tag as u64, false);
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

        // T1608: 跨 effect 传播——清理当前 handler stack 中不匹配的中间帧。
        let rt_unwind = self.declare_runtime_effect_handler_stack_unwind_to_tag();
        let _ = self.builder.build_call(
            rt_unwind,
            &[op_tag_i32.into()],
            "effect_unwind_to_tag",
        )?;

        if let Some(target) = self.current_effect_unwind_target(&op.fqn) {
            // 同一函数内存在匹配的 handle boundary → 直接跳转到 catch block。
            self.builder.build_unconditional_branch(target)?;
        } else {
            // T1606f-1: 无本函数内 handler boundary → 通过 flag-propagation 向外传播
            // （与 Raise.raise 的"返回默认值"路径一致）。
            // flag 与 slot 已在上方写入；caller 的 emit_effect_unwind_if_active 会检查 flag
            // 并路由到最近的匹配 handler 或继续向外 return。
            let ret_ty = self
                .current_fun_return_ty
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect perform (indirect) needs function return type",
                    at: span.into(),
                })?;
            let v = self.default_value(ret_ty);
            self.emit_return(span, ret_ty, v)?;
        }

        // 继续生成后续 IR：把 builder 移到一个"不可达 continuation block"，避免后续插入失败。
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
    /// - arm body 在"handler scope"之外生成，避免 self-capture（PLAN §6.2）。
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

        // T0913 / T1608：在动态层维护 handler stack（Appendix A），使用统一 op_tag 分配。
        let op_tag_val = self.effect_op_tag(&arm.op.op.fqn);
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let handler_frame_ptr =
            self.create_entry_alloca_raw(span, "handle_effect_frame", handler_frame_ty.into())?;

        let outer_raise_target = self.current_raise_target();

        let body_bb = self.context.append_basic_block(func, "handle_body");
        let catch_bb = self.context.append_basic_block(func, "handle_catch");

        // `finally` 语义：保证在"正常路径 / catch 返回 / catch 继续 raise 向外传播"三种情况下都执行一次。
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
        let op_tag_i32 = self.context.i32_type().const_int(op_tag_val as u64, false);
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
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
            | CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
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
        }
    }

    pub(super) fn codegen_handle_expr_no_perform(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // no-perform fast path：不生成 catch/merge CFG，仅保留顺序语义（body -> finally）。
        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_noperform_result", out_ty)?)
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
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
            | CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
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

        // T1608：使用统一的 op_tag 分配。
        let tag = self.effect_op_tag(&arm.op.op.fqn);
        let op_tag_i32 = self.context.i32_type().const_int(tag as u64, false);

        let outer_target = self.current_effect_unwind_target(&arm.op.op.fqn);

        let body_bb = self.context.append_basic_block(func, "handle_custom_body");
        let catch_bb = self.context.append_basic_block(func, "handle_custom_catch");

        // `finally` 语义：保证在"正常路径 / catch 返回 / catch 继续 perform 向外传播"三种情况下都执行一次。
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

        // T1606f-1: 间接 perform（跨函数调用）支持。
        //
        // 当 handle body 内调用的函数执行了匹配的 perform，该函数会通过 flag-propagation 返回
        // 默认值；call-site 的 emit_effect_unwind_if_active 检查 flag → 跳到最近的
        // raise_target_stack 入口。因此需要在 raise_target_stack 中放置一个"dispatch trampoline"，
        // 该 trampoline 读取 op_tag 并：
        //   - 若匹配本 handler 的 op_tag → 跳到 catch_bb（处理 effect）
        //   - 若不匹配（例如 Raise.raise 或其它 effect）→ pop handler frame 并向外传播。
        let dispatch_bb = self
            .context
            .append_basic_block(func, "handle_custom_dispatch");
        let dispatch_no_match_bb = self
            .context
            .append_basic_block(func, "handle_custom_dispatch_nomatch");
        let outer_raise_target = self.current_raise_target();

        // 进入 handle：先执行 body；若发生 perform，则跳到 catch_bb。
        self.builder.build_unconditional_branch(body_bb)?;

        // --- body ---
        self.builder.position_at_end(body_bb);
        self.push_effect_unwind_target(&arm.op.op.fqn, catch_bb);
        self.push_raise_target(dispatch_bb);
        let body_v = self.codegen_block_value_in_expected_context(&handle.body, Some(out_ty))?;
        self.pop_raise_target();
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
                    // T1608: 跨 effect 传播——清理中间帧后再跳到外层 handler。
                    let rt_unwind = self.declare_runtime_effect_handler_stack_unwind_to_tag();
                    let _ = self.builder.build_call(
                        rt_unwind,
                        &[op_tag_i32.into()],
                        "effect_unwind_to_tag",
                    )?;
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

        // --- dispatch trampoline (T1606f-1) ---
        // emit_effect_unwind_if_active 在 body 中的函数调用返回后，若 flag 被设置，
        // 会跳到 dispatch_bb（通过 raise_target_stack）。
        // dispatch_bb 读取 op_tag，判断是否为本 handler 匹配的 effect。
        self.builder.position_at_end(dispatch_bb);
        {
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = self
                .builder
                .build_call(rt_read_tag, &[], "dispatch_read_op_tag")?;
            let tag_raw = tag_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "dispatch read_op_tag return value",
                    at: span.into(),
                })?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "dispatch read_op_tag return type",
                    at: span.into(),
                });
            };

            let tag_matches = self.builder.build_int_compare(
                IntPredicate::EQ,
                slot_tag,
                op_tag_i32,
                "dispatch_tag_eq",
            )?;
            self.builder.build_conditional_branch(
                tag_matches,
                catch_bb,
                dispatch_no_match_bb,
            )?;
        }

        // --- dispatch no match (T1606f-1) ---
        // op_tag 不匹配：pop handler frame，向外传播。
        self.builder.position_at_end(dispatch_no_match_bb);
        {
            let rt_pop = self.declare_runtime_effect_handler_stack_pop();
            let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
            let frame_i8 = self.builder.build_bit_cast(
                handler_frame_ptr,
                i8_ptr_ty,
                "dispatch_nomatch_frame_i8",
            )?;
            let _ = self
                .builder
                .build_call(rt_pop, &[frame_i8.into()], "dispatch_nomatch_pop")?;

            if let Some(outer) = outer_raise_target {
                // 存在外层 raise handler → 传播到外层。
                self.builder.build_unconditional_branch(outer)?;
            } else {
                // 无外层 handler → 返回默认值向外传播（flag 保持 active）。
                let ret_ty = self
                    .current_fun_return_ty
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "dispatch no-match needs function return type",
                        at: span.into(),
                    })?;
                let v = self.default_value(ret_ty);
                self.emit_return(span, ret_ty, v)?;
            }
        }

        // --- merge ---
        self.builder.position_at_end(merge_bb);

        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
            | CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
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
        // T0616：先实现最小"栈 state machine"版本的 `-> resume`：
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
        // T1608：使用统一的 op_tag 分配（按 FQN 精确匹配）。
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 = self.builder.build_bit_cast(
            handler_frame_ptr,
            i8_ptr_ty,
            "handle_resume_effect_frame_i8",
        )?;
        let resume_tag = self.effect_op_tag(&arm.op.op.fqn);
        let op_tag_i32 = self
            .context
            .i32_type()
            .const_int(resume_tag as u64, false);
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
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
            | CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
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
        // 当前阶段（可回归语义子集）：
        // - 仅支持单个 arm（在外层已校验）；
        // - 对匹配当前 arm 的 op：
        //   - 0 个 perform：退化为顺序执行 `body`（以及 `finally`，若存在），arm 不可达（T1606a）；
        //   - N≥1：支持同一 handle body 内 1..N 个 perform 点（T1606c）：
        //     - perform 仍要求绑定到 `val x: T = perform`（early stage 约束）；
        //     - T1606e：支持 perform 嵌套在 if/else/while/when/block 内部（递归扫描 + resume path）；
        // - heap state machine 以 `{ frame, pc, lifted locals... }` 表达可重入执行；
        // - continuation one-shot 与 handler stack 捕获由 runtime（T0914/T0915a）保证。

        // 1) 扫描 handle body：递归收集所有匹配该 arm 的 perform 点（含嵌套在控制流内的）。
        //
        // T1606e 数据结构：ResumeFrame 描述 perform 点所在的一层控制流嵌套。
        // resume_path（outermost-first）记录从 handle body 顶层到 perform 点的嵌套路径。
        #[derive(Debug, Clone, Copy)]
        enum ResumeFrame<'hir> {
            /// Perform is in the then-branch of an if expression.
            IfThen {
                if_expr: &'hir hir::Expr,
                then_block_stmts: &'hir [hir::Stmt],
                resume_after_stmt: usize,
            },
            /// Perform is in the else-branch of an if expression.
            IfElse {
                if_expr: &'hir hir::Expr,
                else_block_stmts: &'hir [hir::Stmt],
                resume_after_stmt: usize,
            },
            /// Perform is inside a when arm body.
            WhenArm {
                when_expr: &'hir hir::Expr,
                arm_index: usize,
                arm_block_stmts: &'hir [hir::Stmt],
                resume_after_stmt: usize,
            },
            /// Perform is inside a while loop body.
            WhileBody {
                while_cond: &'hir hir::Expr,
                while_body: &'hir hir::Block,
                resume_after_stmt: usize,
            },
            /// Perform is inside a block expression.
            Block {
                block: &'hir hir::Block,
                resume_after_stmt: usize,
            },
        }

        impl<'hir> ResumeFrame<'hir> {
            fn set_resume_after_stmt(&mut self, idx: usize) {
                match self {
                    ResumeFrame::IfThen { resume_after_stmt, .. }
                    | ResumeFrame::IfElse { resume_after_stmt, .. }
                    | ResumeFrame::WhenArm { resume_after_stmt, .. }
                    | ResumeFrame::WhileBody { resume_after_stmt, .. }
                    | ResumeFrame::Block { resume_after_stmt, .. } => {
                        *resume_after_stmt = idx;
                    }
                }
            }
        }

        /// Compare two ResumeFrame variants by HIR pointer identity (ignoring resume_after_stmt).
        /// Used to determine if two perform sites share the same nesting context at a given level.
        fn resume_frame_same_structure<'a>(a: &ResumeFrame<'a>, b: &ResumeFrame<'a>) -> bool {
            match (a, b) {
                (
                    ResumeFrame::IfThen { if_expr: a_e, .. },
                    ResumeFrame::IfThen { if_expr: b_e, .. },
                ) => std::ptr::eq(*a_e, *b_e),
                (
                    ResumeFrame::IfElse { if_expr: a_e, .. },
                    ResumeFrame::IfElse { if_expr: b_e, .. },
                ) => std::ptr::eq(*a_e, *b_e),
                (
                    ResumeFrame::WhenArm { when_expr: a_e, arm_index: a_i, .. },
                    ResumeFrame::WhenArm { when_expr: b_e, arm_index: b_i, .. },
                ) => std::ptr::eq(*a_e, *b_e) && a_i == b_i,
                (
                    ResumeFrame::WhileBody { while_body: a_b, .. },
                    ResumeFrame::WhileBody { while_body: b_b, .. },
                ) => std::ptr::eq(*a_b, *b_b),
                (
                    ResumeFrame::Block { block: a_b, .. },
                    ResumeFrame::Block { block: b_b, .. },
                ) => std::ptr::eq(*a_b, *b_b),
                _ => false,
            }
        }

        /// Declaration info for lift analysis: tracks all val decls in handle body (at any nesting depth).
        struct DeclInfo<'hir> {
            decl: &'hir hir::ValDecl,
            /// Pre-order traversal position across the entire handle body tree.
            order: usize,
        }

        #[derive(Debug)]
        struct NestedPerformSite<'hir> {
            pc: usize,
            decl: &'hir hir::ValDecl,
            op: &'hir hir::EffectOpRef,
            args: &'hir [hir::CallArg],
            id: hir::SymbolId,
            /// Outermost-first stack of enclosing control flow frames.
            /// Empty for top-level performs (no nesting).
            resume_path: Vec<ResumeFrame<'hir>>,
            /// Index in handle.body.stmts of the top-level stmt that transitively contains this perform.
            top_level_stmt_idx: usize,
        }

        // T1606e：递归扫描 handle body 内所有 perform 点（含嵌套在 if/while/when/block 内的）。
        //
        // 算法：用 path 栈（outermost-first）追踪当前嵌套上下文。
        // 在每一层 block 迭代时，先更新最顶层 frame 的 resume_after_stmt 为当前 stmt index，
        // 再递归进入子结构。当找到 perform 时，clone 当前 path 作为 resume_path。
        fn scan_stmts_for_performs<'a>(
            stmts: &'a [hir::Stmt],
            arm_op_fqn: &str,
            path: &mut Vec<ResumeFrame<'a>>,
            top_level_stmt_idx: usize,
            pc: &mut usize,
            sites: &mut Vec<NestedPerformSite<'a>>,
            decl_order: &mut usize,
            decl_map: &mut HashMap<hir::SymbolId, DeclInfo<'a>>,
        ) -> Result<(), LlvmEmitError> {
            for (idx, stmt) in stmts.iter().enumerate() {
                // 如果 path 非空，更新最顶层 frame 的 resume_after_stmt 为当前 idx。
                if let Some(frame) = path.last_mut() {
                    frame.set_resume_after_stmt(idx);
                }
                match &stmt.kind {
                    hir::StmtKind::Val(decl) => {
                        // 追踪 decl 的声明顺序（pre-order traversal）。
                        if let Some(id) = decl.id {
                            decl_map.insert(id, DeclInfo { decl, order: *decl_order });
                            *decl_order += 1;
                        }
                        if let Some(init) = &decl.init {
                            if let hir::ExprKind::Perform { op, args } = &init.kind {
                                if op.fqn == arm_op_fqn {
                                    let Some(id) = decl.id else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle escape perform binding id",
                                            at: decl.span.into(),
                                        });
                                    };
                                    let this_pc = *pc;
                                    *pc += 1;
                                    sites.push(NestedPerformSite {
                                        pc: this_pc,
                                        decl,
                                        op,
                                        args: args.as_slice(),
                                        id,
                                        resume_path: path.clone(),
                                        top_level_stmt_idx,
                                    });
                                }
                            }
                        }
                    }
                    hir::StmtKind::Expr(expr) => {
                        // 裸 perform（未绑定到 val）仍然报错。
                        if let hir::ExprKind::Perform { op, .. } = &expr.kind {
                            if op.fqn == arm_op_fqn {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle escape body (perform must be bound to val)",
                                    at: expr.span.into(),
                                });
                            }
                        }
                        // 递归进入控制流表达式。
                        scan_expr_for_performs(
                            expr, arm_op_fqn, path, top_level_stmt_idx, pc, sites,
                            decl_order, decl_map,
                        )?;
                    }
                    hir::StmtKind::While { cond, body } => {
                        // 递归进入 while body，添加 WhileBody frame。
                        path.push(ResumeFrame::WhileBody {
                            while_cond: cond,
                            while_body: body,
                            resume_after_stmt: 0,
                        });
                        scan_stmts_for_performs(
                            &body.stmts, arm_op_fqn, path, top_level_stmt_idx, pc, sites,
                            decl_order, decl_map,
                        )?;
                        path.pop();
                    }
                    // 其他语句不含 perform 点（Break/Continue/Return/Empty/Todo）。
                    _ => {}
                }
            }
            Ok(())
        }

        fn scan_expr_for_performs<'a>(
            expr: &'a hir::Expr,
            arm_op_fqn: &str,
            path: &mut Vec<ResumeFrame<'a>>,
            top_level_stmt_idx: usize,
            pc: &mut usize,
            sites: &mut Vec<NestedPerformSite<'a>>,
            decl_order: &mut usize,
            decl_map: &mut HashMap<hir::SymbolId, DeclInfo<'a>>,
        ) -> Result<(), LlvmEmitError> {
            match &expr.kind {
                hir::ExprKind::If {
                    cond: _,
                    then_branch,
                    else_branch,
                } => {
                    // 递归进入 then-branch（如果是 Block）。
                    if let hir::ExprKind::Block(block) = &then_branch.kind {
                        path.push(ResumeFrame::IfThen {
                            if_expr: expr,
                            then_block_stmts: &block.stmts,
                            resume_after_stmt: 0,
                        });
                        scan_stmts_for_performs(
                            &block.stmts, arm_op_fqn, path, top_level_stmt_idx, pc, sites,
                            decl_order, decl_map,
                        )?;
                        path.pop();
                    }
                    // 递归进入 else-branch（如果存在且是 Block）。
                    if let Some(else_expr) = else_branch.as_deref() {
                        if let hir::ExprKind::Block(block) = &else_expr.kind {
                            path.push(ResumeFrame::IfElse {
                                if_expr: expr,
                                else_block_stmts: &block.stmts,
                                resume_after_stmt: 0,
                            });
                            scan_stmts_for_performs(
                                &block.stmts, arm_op_fqn, path, top_level_stmt_idx, pc, sites,
                                decl_order, decl_map,
                            )?;
                            path.pop();
                        }
                    }
                }
                hir::ExprKind::When { subject: _, arms } => {
                    for (arm_idx, when_arm) in arms.iter().enumerate() {
                        if let hir::ExprKind::Block(block) = &when_arm.body.kind {
                            path.push(ResumeFrame::WhenArm {
                                when_expr: expr,
                                arm_index: arm_idx,
                                arm_block_stmts: &block.stmts,
                                resume_after_stmt: 0,
                            });
                            scan_stmts_for_performs(
                                &block.stmts, arm_op_fqn, path, top_level_stmt_idx, pc, sites,
                                decl_order, decl_map,
                            )?;
                            path.pop();
                        }
                    }
                }
                hir::ExprKind::Block(block) => {
                    path.push(ResumeFrame::Block {
                        block,
                        resume_after_stmt: 0,
                    });
                    scan_stmts_for_performs(
                        &block.stmts, arm_op_fqn, path, top_level_stmt_idx, pc, sites,
                        decl_order, decl_map,
                    )?;
                    path.pop();
                }
                // 不递归进入 Closure（perform in closure 需要不同策略）和 Handle（嵌套 handle 有自己的状态机）。
                // 其他表达式不含语句级控制流。
                _ => {}
            }
            Ok(())
        }

        let mut perform_sites: Vec<NestedPerformSite<'_>> = Vec::new();
        let mut scan_path: Vec<ResumeFrame<'_>> = Vec::new();
        let mut pc_counter: usize = 0;
        let mut decl_order_counter: usize = 0;
        let mut decl_map: HashMap<hir::SymbolId, DeclInfo<'_>> = HashMap::new();
        for (top_idx, stmt) in handle.body.stmts.iter().enumerate() {
            scan_stmts_for_performs(
                std::slice::from_ref(stmt),
                &arm.op.op.fqn,
                &mut scan_path,
                top_idx,
                &mut pc_counter,
                &mut perform_sites,
                &mut decl_order_counter,
                &mut decl_map,
            )?;
        }

        let Some(first_site) = perform_sites.first() else {
            // T1606a：没有匹配 op 的 perform 点，arm 不可达；退化为顺序执行 `body -> finally` 并返回 body 值。
            return self.codegen_handle_expr_no_perform(span, handle, out_ty);
        };
        let perform_idx = first_site.top_level_stmt_idx;
        let perform_decl = first_site.decl;
        let perform_op = first_site.op;
        let perform_args = first_site.args;
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
        let perform_id = first_site.id;

        for site in &perform_sites {
            if arm.op.binders.len() != site.args.len() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape binder arity mismatch",
                    at: arm.op.span.into(),
                });
            }
        }

        let resume_value_ty =
            self.cg_ty_of(perform_decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape perform value type",
                    at: perform_decl.span.into(),
                })?;
        for site in &perform_sites {
            let site_ty =
                self.cg_ty_of(site.decl.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle escape perform value type",
                        at: site.decl.span.into(),
                    })?;
            if site_ty != resume_value_ty {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape perform value type mismatch",
                    at: site.decl.span.into(),
                });
            }
        }

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
        let i32_ty = self.context.i32_type();

        // 1.5) 计算 perform 之后会用到的 locals（用于决定必须 lift 到 heap state 的 capture 集合）。
        //
        // 说明：
        // - step trampoline 执行在"原函数栈已不存在"的异步时刻；
        // - 因此：perform 之后引用到的"外层 locals / perform 前 locals"必须从 heap state 恢复；
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

        // T1606e：top-level 拦截表 — 映射 top_level_stmt_idx -> [(pc, resume_path)]。
        // 对于 flat performs，resume_path 为空；对于嵌套 performs，resume_path 描述控制流嵌套。
        // 同一 top_level_stmt_idx 下可能有多个 perform（例如 if/else 两侧各有一个 perform）。
        let mut top_level_intercepts: HashMap<usize, Vec<(usize, &[ResumeFrame<'_>])>> = HashMap::new();
        for (pc, site) in perform_sites.iter().enumerate() {
            top_level_intercepts
                .entry(site.top_level_stmt_idx)
                .or_default()
                .push((pc, site.resume_path.as_slice()));
        }

        // escape continuation：把当前作用域内的引用类型 locals 捕获到 heap state 中，
        // 以便在 step trampoline（异步 resume）里继续访问它们。
        //
        // 注意：
        // - 当前 v0 实现捕获 `Ref/String/Bool/Int`：
        //   - `Ref/String`：用于保活 closure/env 等引用类型；
        //   - `Bool/Int`：用于保活 word-sized handle（例如 sysroot 的 `Task<T>`/`Executor` 早期落点）。
        // - 这里按"当前可见的绑定"去重（内层 scope shadow 外层），并按 SymbolId 排序保证 determinism。
        struct CapturedLocal {
            id: hir::SymbolId,
            hir_ty: Option<TypeId>,
            ty: CgTy,
            mutable: bool,
        }

        // 1) outer locals：来自当前 codegen env
        let mut outer_captures: Vec<CapturedLocal> = Vec::new();
        let mut seen_outer: HashSet<hir::SymbolId> = HashSet::new();
        for scope in self.env.scopes.iter().rev() {
            for (&id, &local) in scope.iter() {
                if !seen_outer.insert(id) {
                    continue;
                }
                if matches!(
                    local.ty,
                    CgTy::Ref | CgTy::String | CgTy::Bool | CgTy::Int(_)
                ) {
                    outer_captures.push(CapturedLocal {
                        id,
                        hir_ty: local.hir_ty,
                        ty: local.ty,
                        mutable: local.mutable,
                    });
                }
            }
        }

        outer_captures.sort_by_key(|c| c.id.as_u32());

        // 2) body lifted locals：跨 suspension 使用的 handle body locals（Ref/String/Bool/Int）。
        //
        // T1606e：使用 decl_map（递归扫描中收集）进行嵌套感知的 lift analysis：
        // - 对每个 perform 点 p：
        //   - 计算 "p 之后可达代码中会用到的 locals 集合" used_after[p]：
        //     - resume_path 每层 frame 的 continuation（stmts[ras+1..]）；
        //     - WhileBody 额外包含整个 while body + condition（循环重执行）；
        //     - 顶层 handle.body.stmts[top_level_stmt_idx+1..]；
        //   - 若某 local 在 used_after[p] 中，且 decl_map[id].order < decl_map[p.id].order，则必须 lift；
        // - 取并集，得到 state machine 生命周期内需要保存/恢复的 locals。
        fn collect_used_after_perform(
            site: &NestedPerformSite<'_>,
            top_level_stmts: &[hir::Stmt],
            used_after: &mut HashSet<hir::SymbolId>,
        ) {
            // resume_path 每层的 continuation（stmts after the frame's resume_after_stmt）。
            for frame in &site.resume_path {
                match frame {
                    ResumeFrame::IfThen { then_block_stmts, resume_after_stmt, .. } => {
                        for stmt in then_block_stmts.iter().skip(*resume_after_stmt + 1) {
                            collect_used_locals_in_stmt(stmt, used_after);
                        }
                    }
                    ResumeFrame::IfElse { else_block_stmts, resume_after_stmt, .. } => {
                        for stmt in else_block_stmts.iter().skip(*resume_after_stmt + 1) {
                            collect_used_locals_in_stmt(stmt, used_after);
                        }
                    }
                    ResumeFrame::WhenArm { arm_block_stmts, resume_after_stmt, .. } => {
                        for stmt in arm_block_stmts.iter().skip(*resume_after_stmt + 1) {
                            collect_used_locals_in_stmt(stmt, used_after);
                        }
                    }
                    ResumeFrame::WhileBody { while_cond, while_body, resume_after_stmt } => {
                        // 本次迭代的 continuation。
                        for stmt in while_body.stmts.iter().skip(*resume_after_stmt + 1) {
                            collect_used_locals_in_stmt(stmt, used_after);
                        }
                        // 循环重执行：整个 while body + condition。
                        collect_used_locals_in_block(while_body, used_after);
                        collect_used_locals_in_expr(while_cond, used_after);
                    }
                    ResumeFrame::Block { block, resume_after_stmt } => {
                        for stmt in block.stmts.iter().skip(*resume_after_stmt + 1) {
                            collect_used_locals_in_stmt(stmt, used_after);
                        }
                    }
                }
            }
            // 顶层 handle.body.stmts 中位于 top_level_stmt_idx 之后的部分。
            for stmt in top_level_stmts.iter().skip(site.top_level_stmt_idx + 1) {
                collect_used_locals_in_stmt(stmt, used_after);
            }
        }

        let mut body_lift_ids: HashSet<hir::SymbolId> = HashSet::new();
        for site in &perform_sites {
            let Some(site_decl_info) = decl_map.get(&site.id) else {
                continue;
            };
            let site_order = site_decl_info.order;

            let mut used_after: HashSet<hir::SymbolId> = HashSet::new();
            collect_used_after_perform(site, &handle.body.stmts, &mut used_after);

            for id in used_after {
                let Some(info) = decl_map.get(&id) else {
                    continue;
                };
                if info.order < site_order {
                    body_lift_ids.insert(id);
                }
            }
        }

        let mut body_lifts: Vec<CapturedLocal> = Vec::new();
        for &id in &body_lift_ids {
            let Some(info) = decl_map.get(&id) else {
                continue;
            };
            let decl = info.decl;

            let decl_ty = self
                .cg_ty_of(decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape capture local type",
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

            body_lifts.push(CapturedLocal {
                id,
                hir_ty: Some(decl.ty),
                ty: decl_ty,
                mutable: decl.mutable,
            });
        }

        body_lifts.sort_by_key(|c| c.id.as_u32());

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
            fields.push(i32_ty.into()); // pc
            for cap in &outer_captures {
                fields.push(match cap.ty {
                    CgTy::Ref => gc_i8_ptr_ty.into(),
                    CgTy::String => gc_i8_ptr_ty.into(),
                    CgTy::Bool | CgTy::Int(_) => i64_ty.into(),
                    _ => unreachable!("captures filtered by type"),
                });
            }
            for cap in &body_lifts {
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
        // T1607：step 签名扩展为 3 参数 (state, resume_word, resume_gc_ref)。
        let step_fn_ty = self.context.void_type().fn_type(
            &[gc_i8_ptr_ty.into(), i64_ty.into(), gc_i8_ptr_ty.into()],
            false,
        );
        let step_fn = self.module.add_function(&step_name, step_fn_ty, None);
        step_fn.set_linkage(Linkage::Internal);
        // continuation step 会执行 alloc/GC 相关调用，必须参与 statepoint rewrite；
        // 否则 `--gc-stress` 下会因为缺少 roots 而错误回收/失活。
        step_fn.set_gc(super::super::LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE);

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

            // state 参数
            let state_raw = step_fn
                .get_nth_param(0)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "continuation step state param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_ptr_ty = state_ty.ptr_type(cg.gc_address_space());
            let state_ptr =
                cg.builder
                    .build_pointer_cast(state_raw, state_ptr_ty, "cont_step_state_ptr")?;

            // T1607：resume payload 双通道——scalar 走 resume_word (i64)，GC ref 走 resume_gc_ref。
            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "continuation step resume_word param",
                    at: span.into(),
                })?
                .into_int_value();
            let resume_gc_ref = step_fn
                .get_nth_param(2)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "continuation step resume_gc_ref param",
                    at: span.into(),
                })?
                .into_pointer_value();

            // 恢复 lifted locals：把 heap state 中的字段读回到本函数栈 slot。
            //
            // 注意：这里选择"无条件恢复全部 lifted locals"，简化 pc 分支的环境构造；
            // 未初始化的字段在 alloc 时已置零（null/0），因此恢复是安全的。
            let outer_field_base = 3u32;
            let body_field_base = outer_field_base.saturating_add(outer_captures.len() as u32);

            for (idx, cap) in outer_captures.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "cont_step_lift_gep",
                )?;
                let name = format!("lift_{}", cap.id.as_u32());
                match cap.ty {
                    CgTy::Ref => {
                        // 注意：这里不能把 "state 字段地址（addrspace(1)）" 直接当作 local slot。
                        //
                        // 原因：
                        // - `field_ptr` 是一个 **derived pointer**（指向 state 对象内部某字段的地址），且位于 GC
                        //   address space；
                        // - LLVM statepoint/stackmap 可能把它当作 GC root，进入 roots slots；但 runtime 的 roots 更新
                        //   当前只支持 "slot value = 对象头指针"，不支持 derived pointer（否则 `--gc-stress` 下会出现
                        //   invalid root / silent mis-update）。
                        //
                        // v0 策略：把外层 capture 的值恢复到本函数栈 slot（alloca）中，依赖 stackmap roots 更新
                        // 来保持其在 moving GC 下可写回。
                        let loaded = cg
                            .builder
                            .build_load(gc_i8_ptr_ty, field_ptr, "lift_load_ref")?
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
                            .build_load(gc_i8_ptr_ty, field_ptr, "lift_load_str")?
                            .into_pointer_value();
                        let str_ptr_ty = cg.llvm_scoop_string_ptr_type();
                        let casted = cg
                            .builder
                            .build_pointer_cast(loaded, str_ptr_ty, "lift_str")?;
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
                            .build_load(i64_ty, field_ptr, "lift_load_bool")?
                            .into_int_value();
                        let zero = i64_ty.const_zero();
                        let b = cg.builder.build_int_compare(
                            IntPredicate::NE,
                            loaded,
                            zero,
                            "lift_bool",
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
                            .build_load(i64_ty, field_ptr, "lift_load_int")?
                            .into_int_value();
                        let to = cg.int_type(int_ty);
                        let v = if int_ty.bits == 64 {
                            loaded
                        } else {
                            cg.builder
                                .build_int_truncate(loaded, to, "lift_trunc_int")?
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

            for (idx, cap) in body_lifts.iter().enumerate() {
                let field_idx = body_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "cont_step_lift_gep",
                )?;
                let name = format!("lift_{}", cap.id.as_u32());
                match cap.ty {
                    CgTy::Ref => {
                        let loaded = cg
                            .builder
                            .build_load(gc_i8_ptr_ty, field_ptr, "lift_load_ref")?
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
                            .build_load(gc_i8_ptr_ty, field_ptr, "lift_load_str")?
                            .into_pointer_value();
                        let str_ptr_ty = cg.llvm_scoop_string_ptr_type();
                        let casted = cg
                            .builder
                            .build_pointer_cast(loaded, str_ptr_ty, "lift_str")?;
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
                            .build_load(i64_ty, field_ptr, "lift_load_bool")?
                            .into_int_value();
                        let zero = i64_ty.const_zero();
                        let b = cg.builder.build_int_compare(
                            IntPredicate::NE,
                            loaded,
                            zero,
                            "lift_bool",
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
                            .build_load(i64_ty, field_ptr, "lift_load_int")?
                            .into_int_value();
                        let to = cg.int_type(int_ty);
                        let v = if int_ty.bits == 64 {
                            loaded
                        } else {
                            cg.builder
                                .build_int_truncate(loaded, to, "lift_trunc_int")?
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

            // binder slots：在 perform 点写入，在 arm body 中读取。
            struct BinderSlot<'ctx> {
                id: hir::SymbolId,
                hir_ty: TypeId,
                ty: CgTy,
                ptr: PointerValue<'ctx>,
            }
            let mut binder_slots: Vec<BinderSlot<'ctx>> = Vec::new();
            for binder in &arm.op.binders {
                let binder_ty =
                    cg.cg_ty_of(binder.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape binder type",
                            at: binder.span.into(),
                        })?;
                let ptr = cg.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
                binder_slots.push(BinderSlot {
                    id: binder.id,
                    hir_ty: binder.ty,
                    ty: binder_ty,
                    ptr,
                });
            }

            // continuation binder local：在 perform 点写入，在 arm body 中读取。
            let cont_ptr =
                cg.create_entry_alloca(span, &format!("handle_escape_k_{seq}"), CgTy::Ref)?;

            // pc dispatch
            let dispatch_bb = self
                .context
                .append_basic_block(step_fn, "cont_step_dispatch");
            let bad_state_bb = self.context.append_basic_block(step_fn, "cont_step_bad_pc");
            let mut state_bbs = Vec::new();
            for pc in 0..perform_sites.len() {
                state_bbs.push(
                    self.context
                        .append_basic_block(step_fn, &format!("cont_step_pc_{pc}")),
                );
            }

            cg.builder.build_unconditional_branch(dispatch_bb)?;

            cg.builder.position_at_end(dispatch_bb);
            let pc_ptr = cg
                .builder
                .build_struct_gep(state_ty, state_ptr, 2, "cont_step_pc_gep")?;
            let pc = cg
                .builder
                .build_load(i32_ty, pc_ptr, "cont_step_pc")?
                .into_int_value();
            let mut cases = Vec::new();
            for (pc, bb) in state_bbs.iter().enumerate() {
                cases.push((i32_ty.const_int(pc as u64, false), *bb));
            }
            cg.builder.build_switch(pc, bad_state_bb, &cases)?;

            cg.builder.position_at_end(bad_state_bb);
            cg.emit_exit_with_code(span, 3)?;

            // --- T1606e: shared perform interception block ---
            // All perform interception points (both flat top-level and nested) branch here
            // after evaluating args → writing binder slots → storing next_pc.
            // The shared block handles: write back captures → set pc → create continuation →
            // pin → detach handler → arm body → unpin → return.
            let intercept_bb =
                self.context
                    .append_basic_block(step_fn, "cont_step_intercept");
            let intercept_next_pc_ptr =
                cg.create_entry_alloca_raw(span, "intercept_next_pc", i32_ty.into())?;

            // --- pc blocks ---
            for (pc, bb) in state_bbs.iter().enumerate() {
                let site = &perform_sites[pc];
                cg.builder.position_at_end(*bb);

                // 将本次 resume_value 写入对应的 perform binding。
                let target_ptr = if let Some(local) = cg.env.get(site.id) {
                    if local.ty != resume_value_ty {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape perform value type mismatch",
                            at: site.decl.span.into(),
                        });
                    }
                    local.ptr
                } else {
                    let local_name = site.decl.name.as_deref().unwrap_or("resume_value");
                    let ptr =
                        cg.create_entry_alloca(site.decl.span, local_name, resume_value_ty)?;
                    cg.env.insert(
                        site.id,
                        CgLocal {
                            hir_ty: Some(site.decl.ty),
                            ty: resume_value_ty,
                            ptr,
                            mutable: site.decl.mutable,
                        },
                    );
                    ptr
                };

                // T1607：按 resume_value_ty 从 resume_word（scalar）或 resume_gc_ref（GC ptr / boxed）解码。
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
                    CgTy::String => {
                        // resume_gc_ref 直接是 ScoopString* addrspace(1)
                        let str_ptr_ty = cg.llvm_scoop_string_ptr_type();
                        let s = cg.builder.build_pointer_cast(
                            resume_gc_ref,
                            str_ptr_ty,
                            "resume_string",
                        )?;
                        CgValue {
                            ty: CgTy::String,
                            value: Some(s.into()),
                        }
                    }
                    CgTy::Ref => {
                        // resume_gc_ref 直接是 i8* addrspace(1)
                        CgValue {
                            ty: CgTy::Ref,
                            value: Some(resume_gc_ref.into()),
                        }
                    }
                    CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                        // resume_gc_ref 指向 boxed payload: { GcObjectHeader, <payload> }
                        let payload_llvm_ty = cg.llvm_basic_type_of(site.decl.span, resume_value_ty)?;
                        let header_ty = cg.llvm_gc_object_header_type();
                        let box_ty_name = format!(
                            "scoop.runtime.ResumeBox__{func_name}_{seq}_pc{pc}"
                        );
                        let box_ty = if let Some(existing) = cg.context.get_struct_type(&box_ty_name)
                        {
                            existing
                        } else {
                            let t = cg.context.opaque_struct_type(&box_ty_name);
                            t.set_body(&[header_ty.into(), payload_llvm_ty], false);
                            t
                        };
                        let box_ptr_ty = box_ty.ptr_type(cg.gc_address_space());
                        let box_ptr = cg.builder.build_pointer_cast(
                            resume_gc_ref,
                            box_ptr_ty,
                            "resume_box_ptr",
                        )?;
                        let payload_ptr = cg.builder.build_struct_gep(
                            box_ty,
                            box_ptr,
                            1,
                            "resume_box_payload_gep",
                        )?;
                        let loaded = cg.builder.build_load(
                            payload_llvm_ty,
                            payload_ptr,
                            "resume_box_payload",
                        )?;
                        CgValue {
                            ty: resume_value_ty,
                            value: Some(loaded),
                        }
                    }
                };

                let _stored =
                    cg.store_local_value(span, target_ptr, resume_value_ty, resume_value)?;

                // T1606e：继续执行该 perform 之后的语句，直到下一次 perform 或结束。
                // 支持嵌套在 if/while/when/block 内的 perform 点。
                let mut terminated = false;

                if site.resume_path.is_empty() {
                    // --- FLAT PERFORM: iterate top-level stmts after site.top_level_stmt_idx ---
                    for (idx, stmt) in handle.body.stmts.iter().enumerate() {
                        if idx <= site.top_level_stmt_idx {
                            continue;
                        }
                        if let Some(intercepts) = top_level_intercepts.get(&idx) {
                            // Check for flat (top-level) intercept first.
                            if let Some(&(next_pc, _)) = intercepts.iter().find(|(_, rp)| rp.is_empty()) {
                                // Direct flat intercept.
                                let next_site = &perform_sites[next_pc];
                                for (slot, arg) in
                                    binder_slots.iter().zip(next_site.args.iter())
                                {
                                    let hir::CallArg::Positional(expr) = arg else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle escape named perform arg",
                                            at: span.into(),
                                        });
                                    };
                                    let v = cg.codegen_expr_in_expected_context(
                                        expr,
                                        Some(slot.ty),
                                    )?;
                                    let _stored = cg.store_local_value(
                                        expr.span, slot.ptr, slot.ty, v,
                                    )?;
                                }
                                let _ = cg.builder.build_store(
                                    intercept_next_pc_ptr,
                                    i32_ty.const_int(next_pc as u64, false),
                                )?;
                                cg.builder
                                    .build_unconditional_branch(intercept_bb)?;
                                terminated = true;
                                break;
                            }
                            // T1606e: nested intercepts — generate control flow with interception.
                            let first = &intercepts[0];
                            let (intercept_pc, inner_path) = *first;
                            if !inner_path.is_empty() {
                                match &inner_path[0] {
                                    ResumeFrame::IfThen { if_expr, then_block_stmts, resume_after_stmt: perform_stmt_idx, .. } => {
                                        if let hir::ExprKind::If { cond: if_cond, then_branch: _, else_branch } = &if_expr.kind {
                                            let cond_v = cg.codegen_expr_in_expected_context(if_cond, Some(CgTy::Bool))?;
                                            let cond_b = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                                                kind: "if cond (tail nested intercept)",
                                                at: if_cond.span.into(),
                                            })?;
                                            let then_bb_i = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_tail_if_then"));
                                            let has_else = else_branch.is_some();
                                            let else_or_after = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_tail_if_{}", if has_else { "else" } else { "after" }));
                                            let after_if_bb = if has_else {
                                                self.context.append_basic_block(step_fn, &format!("step_pc{pc}_tail_if_after"))
                                            } else {
                                                else_or_after
                                            };
                                            cg.builder.build_conditional_branch(cond_b, then_bb_i, else_or_after)?;

                                            // Then-branch: stmts before perform, then intercept
                                            cg.builder.position_at_end(then_bb_i);
                                            for (ti, tstmt) in then_block_stmts.iter().enumerate() {
                                                if ti == *perform_stmt_idx {
                                                    let is = &perform_sites[intercept_pc];
                                                    for (slot, arg) in binder_slots.iter().zip(is.args.iter()) {
                                                        let hir::CallArg::Positional(expr) = arg else {
                                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                                kind: "handle escape named perform arg",
                                                                at: span.into(),
                                                            });
                                                        };
                                                        let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                        let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                    }
                                                    let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(intercept_pc as u64, false))?;
                                                    cg.builder.build_unconditional_branch(intercept_bb)?;
                                                    break;
                                                }
                                                match &tstmt.kind {
                                                    hir::StmtKind::Empty => {}
                                                    hir::StmtKind::Val(decl) => {
                                                        if let Some(id) = decl.id {
                                                            if body_lift_ids.contains(&id) {
                                                                let Some(init) = decl.init.as_ref() else {
                                                                    return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                };
                                                                let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                            } else { cg.codegen_val_decl(decl)?; }
                                                        } else { cg.codegen_val_decl(decl)?; }
                                                    }
                                                    hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                    hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                    _ => {}
                                                }
                                            }

                                            // Else-branch: check if also intercepted (both branches have performs).
                                            let else_intercept = intercepts.iter().find(|(_, rp)| matches!(rp.first(), Some(ResumeFrame::IfElse { .. })));
                                            if let Some(&(else_ipc, else_rp)) = else_intercept {
                                                if let ResumeFrame::IfElse { else_block_stmts: ebs, resume_after_stmt: epi, .. } = &else_rp[0] {
                                                    cg.builder.position_at_end(else_or_after);
                                                    for (ei, estmt) in ebs.iter().enumerate() {
                                                        if ei == *epi {
                                                            let es = &perform_sites[else_ipc];
                                                            for (slot, arg) in binder_slots.iter().zip(es.args.iter()) {
                                                                let hir::CallArg::Positional(expr) = arg else {
                                                                    return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                                };
                                                                let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                            }
                                                            let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(else_ipc as u64, false))?;
                                                            cg.builder.build_unconditional_branch(intercept_bb)?;
                                                            break;
                                                        }
                                                        match &estmt.kind {
                                                            hir::StmtKind::Empty => {}
                                                            hir::StmtKind::Val(decl) => {
                                                                if let Some(id) = decl.id {
                                                                    if body_lift_ids.contains(&id) {
                                                                        let Some(init) = decl.init.as_ref() else {
                                                                            return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                        };
                                                                        let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                        let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                        let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                        let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                                    } else { cg.codegen_val_decl(decl)?; }
                                                                } else { cg.codegen_val_decl(decl)?; }
                                                            }
                                                            hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                            hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                            _ => {}
                                                        }
                                                    }
                                                }
                                            } else if let Some(else_expr) = else_branch.as_deref() {
                                                cg.builder.position_at_end(else_or_after);
                                                let _ = cg.codegen_expr(else_expr)?;
                                                cg.builder.build_unconditional_branch(after_if_bb)?;
                                            }
                                            // After-if: continue with remaining tail stmts
                                            cg.builder.position_at_end(after_if_bb);
                                            continue;
                                        }
                                    }
                                    ResumeFrame::IfElse { if_expr, else_block_stmts, resume_after_stmt: perform_stmt_idx, .. } => {
                                        if let hir::ExprKind::If { cond: if_cond, then_branch, else_branch: _ } = &if_expr.kind {
                                            let cond_v = cg.codegen_expr_in_expected_context(if_cond, Some(CgTy::Bool))?;
                                            let cond_b = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                                                kind: "if cond (tail nested intercept)",
                                                at: if_cond.span.into(),
                                            })?;
                                            let then_bb_i = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_tail_if_then"));
                                            let else_bb_i = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_tail_if_else"));
                                            let after_if_bb = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_tail_if_after"));
                                            cg.builder.build_conditional_branch(cond_b, then_bb_i, else_bb_i)?;

                                            // Then-branch: check if also intercepted (both branches have performs).
                                            let then_intercept = intercepts.iter().find(|(_, rp)| matches!(rp.first(), Some(ResumeFrame::IfThen { .. })));
                                            if let Some(&(then_ipc, then_rp)) = then_intercept {
                                                if let ResumeFrame::IfThen { then_block_stmts: tbs, resume_after_stmt: tpi, .. } = &then_rp[0] {
                                                    cg.builder.position_at_end(then_bb_i);
                                                    for (ti, tstmt) in tbs.iter().enumerate() {
                                                        if ti == *tpi {
                                                            let ts = &perform_sites[then_ipc];
                                                            for (slot, arg) in binder_slots.iter().zip(ts.args.iter()) {
                                                                let hir::CallArg::Positional(expr) = arg else {
                                                                    return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                                };
                                                                let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                            }
                                                            let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(then_ipc as u64, false))?;
                                                            cg.builder.build_unconditional_branch(intercept_bb)?;
                                                            break;
                                                        }
                                                        match &tstmt.kind {
                                                            hir::StmtKind::Empty => {}
                                                            hir::StmtKind::Val(decl) => {
                                                                if let Some(id) = decl.id {
                                                                    if body_lift_ids.contains(&id) {
                                                                        let Some(init) = decl.init.as_ref() else {
                                                                            return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                        };
                                                                        let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                        let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                        let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                        let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                                    } else { cg.codegen_val_decl(decl)?; }
                                                                } else { cg.codegen_val_decl(decl)?; }
                                                            }
                                                            hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                            hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                            _ => {}
                                                        }
                                                    }
                                                }
                                            } else {
                                                cg.builder.position_at_end(then_bb_i);
                                                let _ = cg.codegen_expr(then_branch)?;
                                                cg.builder.build_unconditional_branch(after_if_bb)?;
                                            }

                                            // Else-branch: stmts before perform, then intercept
                                            cg.builder.position_at_end(else_bb_i);
                                            for (ei, estmt) in else_block_stmts.iter().enumerate() {
                                                if ei == *perform_stmt_idx {
                                                    let is = &perform_sites[intercept_pc];
                                                    for (slot, arg) in binder_slots.iter().zip(is.args.iter()) {
                                                        let hir::CallArg::Positional(expr) = arg else {
                                                            return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                        };
                                                        let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                        let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                    }
                                                    let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(intercept_pc as u64, false))?;
                                                    cg.builder.build_unconditional_branch(intercept_bb)?;
                                                    break;
                                                }
                                                match &estmt.kind {
                                                    hir::StmtKind::Empty => {}
                                                    hir::StmtKind::Val(decl) => {
                                                        if let Some(id) = decl.id {
                                                            if body_lift_ids.contains(&id) {
                                                                let Some(init) = decl.init.as_ref() else {
                                                                    return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                };
                                                                let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                            } else { cg.codegen_val_decl(decl)?; }
                                                        } else { cg.codegen_val_decl(decl)?; }
                                                    }
                                                    hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                    hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                    _ => {}
                                                }
                                            }
                                            // After-if: continue
                                            cg.builder.position_at_end(after_if_bb);
                                            continue;
                                        }
                                    }
                                    ResumeFrame::WhileBody { while_cond, while_body, resume_after_stmt: perform_body_idx, .. } => {
                                        // Generate while loop with interception at the perform point.
                                        let wc_bb = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_tail_while_cond"));
                                        let wb_bb = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_tail_while_body"));
                                        let wa_bb = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_tail_while_after"));
                                        cg.builder.build_unconditional_branch(wc_bb)?;
                                        cg.builder.position_at_end(wc_bb);
                                        let cv = cg.codegen_expr_in_expected_context(while_cond, Some(CgTy::Bool))?;
                                        let cb = cv.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                                            kind: "while cond (tail nested intercept)",
                                            at: while_cond.span.into(),
                                        })?;
                                        cg.builder.build_conditional_branch(cb, wb_bb, wa_bb)?;

                                        cg.builder.position_at_end(wb_bb);
                                        cg.env.push_scope();
                                        let mut while_term = false;
                                        for (bi, bstmt) in while_body.stmts.iter().enumerate() {
                                            if while_term { break; }
                                            if bi == *perform_body_idx {
                                                // T1606e: check for deeper nesting (if/else within while body).
                                                if inner_path.len() > 1 {
                                                    match &inner_path[1] {
                                                        ResumeFrame::IfThen { if_expr: nested_if, then_block_stmts: nested_tbs, resume_after_stmt: nested_tpi, .. } => {
                                                            if let hir::ExprKind::If { cond: if_cond, then_branch: _, else_branch } = &nested_if.kind {
                                                                let cond_v = cg.codegen_expr_in_expected_context(if_cond, Some(CgTy::Bool))?;
                                                                let cond_b = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                    kind: "if cond in while body (tail intercept)",
                                                                    at: if_cond.span.into(),
                                                                })?;
                                                                let if_then_bb = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_wif_then"));
                                                                let has_else = else_branch.is_some();
                                                                let if_else_or_after = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_wif_{}", if has_else { "else" } else { "after" }));
                                                                let if_after_bb = if has_else { self.context.append_basic_block(step_fn, &format!("step_pc{pc}_wif_after")) } else { if_else_or_after };
                                                                cg.builder.build_conditional_branch(cond_b, if_then_bb, if_else_or_after)?;

                                                                // Then-branch: stmts before perform, then intercept
                                                                cg.builder.position_at_end(if_then_bb);
                                                                for (ti, tstmt) in nested_tbs.iter().enumerate() {
                                                                    if ti == *nested_tpi {
                                                                        let is = &perform_sites[intercept_pc];
                                                                        for (slot, arg) in binder_slots.iter().zip(is.args.iter()) {
                                                                            let hir::CallArg::Positional(expr) = arg else {
                                                                                return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                                            };
                                                                            let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                            let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                                        }
                                                                        let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(intercept_pc as u64, false))?;
                                                                        cg.builder.build_unconditional_branch(intercept_bb)?;
                                                                        break;
                                                                    }
                                                                    match &tstmt.kind {
                                                                        hir::StmtKind::Empty => {}
                                                                        hir::StmtKind::Val(decl) => {
                                                                            if let Some(id) = decl.id {
                                                                                if body_lift_ids.contains(&id) {
                                                                                    let Some(init) = decl.init.as_ref() else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() }); };
                                                                                    let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                                    let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                                    let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                                    let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                                                } else { cg.codegen_val_decl(decl)?; }
                                                                            } else { cg.codegen_val_decl(decl)?; }
                                                                        }
                                                                        hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                                        hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                                        _ => {}
                                                                    }
                                                                }

                                                                // Else-branch: check if also intercepted, else codegen normally.
                                                                let else_in_while = intercepts.iter().find(|(_, rp)| {
                                                                    rp.len() > 1 && matches!(rp[0], ResumeFrame::WhileBody { .. }) && matches!(rp[1], ResumeFrame::IfElse { .. })
                                                                });
                                                                if let Some(&(else_wpc, else_wrp)) = else_in_while {
                                                                    if let ResumeFrame::IfElse { else_block_stmts: ebs, resume_after_stmt: epi, .. } = &else_wrp[1] {
                                                                        cg.builder.position_at_end(if_else_or_after);
                                                                        for (ei, estmt) in ebs.iter().enumerate() {
                                                                            if ei == *epi {
                                                                                let es = &perform_sites[else_wpc];
                                                                                for (slot, arg) in binder_slots.iter().zip(es.args.iter()) {
                                                                                    let hir::CallArg::Positional(expr) = arg else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() }); };
                                                                                    let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                                    let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                                                }
                                                                                let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(else_wpc as u64, false))?;
                                                                                cg.builder.build_unconditional_branch(intercept_bb)?;
                                                                                break;
                                                                            }
                                                                            match &estmt.kind {
                                                                                hir::StmtKind::Empty => {}
                                                                                hir::StmtKind::Val(decl) => {
                                                                                    if let Some(id) = decl.id {
                                                                                        if body_lift_ids.contains(&id) {
                                                                                            let Some(init) = decl.init.as_ref() else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() }); };
                                                                                            let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                                            let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                                            let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                                            let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                                                        } else { cg.codegen_val_decl(decl)?; }
                                                                                    } else { cg.codegen_val_decl(decl)?; }
                                                                                }
                                                                                hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                                                hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                                                _ => {}
                                                                            }
                                                                        }
                                                                    }
                                                                } else if let Some(else_expr) = else_branch.as_deref() {
                                                                    cg.builder.position_at_end(if_else_or_after);
                                                                    let _ = cg.codegen_expr(else_expr)?;
                                                                    cg.builder.build_unconditional_branch(if_after_bb)?;
                                                                }

                                                                // After-if: remaining while body stmts, loop back
                                                                cg.builder.position_at_end(if_after_bb);
                                                                for remaining in while_body.stmts[bi+1..].iter() {
                                                                    match &remaining.kind {
                                                                        hir::StmtKind::Empty => {}
                                                                        hir::StmtKind::Val(decl) => {
                                                                            if let Some(id) = decl.id {
                                                                                if body_lift_ids.contains(&id) {
                                                                                    let Some(init) = decl.init.as_ref() else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() }); };
                                                                                    let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                                    let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                                    let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                                    let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                                                } else { cg.codegen_val_decl(decl)?; }
                                                                            } else { cg.codegen_val_decl(decl)?; }
                                                                        }
                                                                        hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                                        hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                                        hir::StmtKind::While { cond, body } => { cg.codegen_while_stmt(remaining.span, cond, body)?; }
                                                                        _ => {}
                                                                    }
                                                                }
                                                                cg.builder.build_unconditional_branch(wc_bb)?;
                                                                while_term = true;
                                                            }
                                                        }
                                                        ResumeFrame::IfElse { if_expr: nested_if, else_block_stmts: nested_ebs, resume_after_stmt: nested_epi, .. } => {
                                                            if let hir::ExprKind::If { cond: if_cond, then_branch, else_branch: _ } = &nested_if.kind {
                                                                let cond_v = cg.codegen_expr_in_expected_context(if_cond, Some(CgTy::Bool))?;
                                                                let cond_b = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                    kind: "if cond in while body (tail intercept)",
                                                                    at: if_cond.span.into(),
                                                                })?;
                                                                let if_then_bb = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_wif_then"));
                                                                let if_else_bb = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_wif_else"));
                                                                let if_after_bb = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_wif_after"));
                                                                cg.builder.build_conditional_branch(cond_b, if_then_bb, if_else_bb)?;

                                                                // Then-branch: check if also intercepted, else codegen normally.
                                                                let then_in_while = intercepts.iter().find(|(_, rp)| {
                                                                    rp.len() > 1 && matches!(rp[0], ResumeFrame::WhileBody { .. }) && matches!(rp[1], ResumeFrame::IfThen { .. })
                                                                });
                                                                if let Some(&(then_wpc, then_wrp)) = then_in_while {
                                                                    if let ResumeFrame::IfThen { then_block_stmts: tbs, resume_after_stmt: tpi, .. } = &then_wrp[1] {
                                                                        cg.builder.position_at_end(if_then_bb);
                                                                        for (ti, tstmt) in tbs.iter().enumerate() {
                                                                            if ti == *tpi {
                                                                                let ts = &perform_sites[then_wpc];
                                                                                for (slot, arg) in binder_slots.iter().zip(ts.args.iter()) {
                                                                                    let hir::CallArg::Positional(expr) = arg else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() }); };
                                                                                    let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                                    let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                                                }
                                                                                let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(then_wpc as u64, false))?;
                                                                                cg.builder.build_unconditional_branch(intercept_bb)?;
                                                                                break;
                                                                            }
                                                                            match &tstmt.kind {
                                                                                hir::StmtKind::Empty => {}
                                                                                hir::StmtKind::Val(decl) => {
                                                                                    if let Some(id) = decl.id {
                                                                                        if body_lift_ids.contains(&id) {
                                                                                            let Some(init) = decl.init.as_ref() else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() }); };
                                                                                            let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                                            let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                                            let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                                            let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                                                        } else { cg.codegen_val_decl(decl)?; }
                                                                                    } else { cg.codegen_val_decl(decl)?; }
                                                                                }
                                                                                hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                                                hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                                                _ => {}
                                                                            }
                                                                        }
                                                                    }
                                                                } else {
                                                                    cg.builder.position_at_end(if_then_bb);
                                                                    let _ = cg.codegen_expr(then_branch)?;
                                                                    cg.builder.build_unconditional_branch(if_after_bb)?;
                                                                }

                                                                // Else-branch: stmts before perform, then intercept
                                                                cg.builder.position_at_end(if_else_bb);
                                                                for (ei, estmt) in nested_ebs.iter().enumerate() {
                                                                    if ei == *nested_epi {
                                                                        let is = &perform_sites[intercept_pc];
                                                                        for (slot, arg) in binder_slots.iter().zip(is.args.iter()) {
                                                                            let hir::CallArg::Positional(expr) = arg else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() }); };
                                                                            let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                            let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                                        }
                                                                        let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(intercept_pc as u64, false))?;
                                                                        cg.builder.build_unconditional_branch(intercept_bb)?;
                                                                        break;
                                                                    }
                                                                    match &estmt.kind {
                                                                        hir::StmtKind::Empty => {}
                                                                        hir::StmtKind::Val(decl) => {
                                                                            if let Some(id) = decl.id {
                                                                                if body_lift_ids.contains(&id) {
                                                                                    let Some(init) = decl.init.as_ref() else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() }); };
                                                                                    let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                                    let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                                    let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                                    let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                                                } else { cg.codegen_val_decl(decl)?; }
                                                                            } else { cg.codegen_val_decl(decl)?; }
                                                                        }
                                                                        hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                                        hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                                        _ => {}
                                                                    }
                                                                }

                                                                // After-if: remaining while body stmts, loop back
                                                                cg.builder.position_at_end(if_after_bb);
                                                                for remaining in while_body.stmts[bi+1..].iter() {
                                                                    match &remaining.kind {
                                                                        hir::StmtKind::Empty => {}
                                                                        hir::StmtKind::Val(decl) => {
                                                                            if let Some(id) = decl.id {
                                                                                if body_lift_ids.contains(&id) {
                                                                                    let Some(init) = decl.init.as_ref() else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() }); };
                                                                                    let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                                    let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                                    let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                                    let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                                                } else { cg.codegen_val_decl(decl)?; }
                                                                            } else { cg.codegen_val_decl(decl)?; }
                                                                        }
                                                                        hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                                        hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                                        hir::StmtKind::While { cond, body } => { cg.codegen_while_stmt(remaining.span, cond, body)?; }
                                                                        _ => {}
                                                                    }
                                                                }
                                                                cg.builder.build_unconditional_branch(wc_bb)?;
                                                                while_term = true;
                                                            }
                                                        }
                                                        _ => {
                                                            // Direct flat intercept in while body (inner_path[1] is unsupported nested type)
                                                            let is = &perform_sites[intercept_pc];
                                                            for (slot, arg) in binder_slots.iter().zip(is.args.iter()) {
                                                                let hir::CallArg::Positional(expr) = arg else {
                                                                    return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                                };
                                                                let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                            }
                                                            let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(intercept_pc as u64, false))?;
                                                            cg.builder.build_unconditional_branch(intercept_bb)?;
                                                            while_term = true;
                                                        }
                                                    }
                                                } else {
                                                    // Direct flat intercept in while body (perform is directly at this stmt)
                                                    let is = &perform_sites[intercept_pc];
                                                    for (slot, arg) in binder_slots.iter().zip(is.args.iter()) {
                                                        let hir::CallArg::Positional(expr) = arg else {
                                                            return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                        };
                                                        let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                        let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                    }
                                                    let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(intercept_pc as u64, false))?;
                                                    cg.builder.build_unconditional_branch(intercept_bb)?;
                                                    while_term = true;
                                                }
                                                break;
                                            }
                                            match &bstmt.kind {
                                                hir::StmtKind::Empty => {}
                                                hir::StmtKind::Val(decl) => {
                                                    if let Some(id) = decl.id {
                                                        if body_lift_ids.contains(&id) {
                                                            let Some(init) = decl.init.as_ref() else {
                                                                return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                            };
                                                            let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                            let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                            let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                            let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                        } else { cg.codegen_val_decl(decl)?; }
                                                    } else { cg.codegen_val_decl(decl)?; }
                                                }
                                                hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                hir::StmtKind::While { cond, body } => { cg.codegen_while_stmt(bstmt.span, cond, body)?; }
                                                _ => {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "stmt in while body (tail nested intercept)",
                                                        at: bstmt.span.into(),
                                                    });
                                                }
                                            }
                                        }
                                        cg.env.pop_scope();
                                        if !while_term {
                                            cg.builder.build_unconditional_branch(wc_bb)?;
                                        }
                                        cg.builder.position_at_end(wa_bb);
                                        continue;
                                    }
                                    _ => {
                                        // Unsupported nested case — fall through to normal codegen
                                    }
                                }
                            }
                        }
                        // Normal stmt codegen (with lifted local handling)
                        match &stmt.kind {
                            hir::StmtKind::Empty => {}
                            hir::StmtKind::Val(decl) => {
                                if let Some(id) = decl.id {
                                    if body_lift_ids.contains(&id) {
                                        let Some(init) = decl.init.as_ref()
                                        else {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "lifted local without init",
                                                at: decl.span.into(),
                                            });
                                        };
                                        let decl_ty =
                                            cg.cg_ty_of(decl.ty).ok_or(
                                                LlvmEmitError::UnsupportedMainBody {
                                                    kind: "lifted local type",
                                                    at: decl.span.into(),
                                                },
                                            )?;
                                        let local =
                                            cg.env.get(id).ok_or(
                                                LlvmEmitError::UnsupportedMainBody {
                                                    kind: "lifted local slot missing",
                                                    at: decl.span.into(),
                                                },
                                            )?;
                                        let v =
                                            cg.codegen_expr_in_expected_context(
                                                init,
                                                Some(decl_ty),
                                            )?;
                                        let _stored = cg.store_local_value(
                                            decl.span, local.ptr, decl_ty, v,
                                        )?;
                                    } else {
                                        cg.codegen_val_decl(decl)?;
                                    }
                                } else {
                                    cg.codegen_val_decl(decl)?;
                                }
                            }
                            hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                                cg.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                            }
                            hir::StmtKind::Expr(expr) => {
                                let _ = cg.codegen_expr(expr)?;
                            }
                            hir::StmtKind::Return { .. } => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "`return` inside continuation step",
                                    at: stmt.span.into(),
                                });
                            }
                            hir::StmtKind::While { cond, body } => {
                                cg.codegen_while_stmt(stmt.span, cond, body)?;
                            }
                            hir::StmtKind::Break { .. }
                            | hir::StmtKind::Continue { .. }
                            | hir::StmtKind::Todo(_) => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "statement inside continuation step",
                                    at: stmt.span.into(),
                                });
                            }
                        }
                    }
                } else {
                    // --- NESTED PERFORM: resume inside control flow (T1606e) ---
                    // Walk resume_path from innermost to outermost, emitting
                    // tail stmts at each level. For WhileBody frames, also
                    // re-enter the while loop with perform interception.
                    for level in (0..site.resume_path.len()).rev() {
                        if terminated {
                            break;
                        }
                        let frame = &site.resume_path[level];
                        // Get the tail stmts for this frame.
                        let (tail_stmts, tail_base_idx): (&[hir::Stmt], usize) =
                            match frame {
                                ResumeFrame::IfThen {
                                    then_block_stmts,
                                    resume_after_stmt,
                                    ..
                                } => (
                                    &then_block_stmts[*resume_after_stmt + 1..],
                                    *resume_after_stmt + 1,
                                ),
                                ResumeFrame::IfElse {
                                    else_block_stmts,
                                    resume_after_stmt,
                                    ..
                                } => (
                                    &else_block_stmts[*resume_after_stmt + 1..],
                                    *resume_after_stmt + 1,
                                ),
                                ResumeFrame::WhenArm {
                                    arm_block_stmts,
                                    resume_after_stmt,
                                    ..
                                } => (
                                    &arm_block_stmts[*resume_after_stmt + 1..],
                                    *resume_after_stmt + 1,
                                ),
                                ResumeFrame::Block {
                                    block,
                                    resume_after_stmt,
                                } => (
                                    &block.stmts[*resume_after_stmt + 1..],
                                    *resume_after_stmt + 1,
                                ),
                                ResumeFrame::WhileBody {
                                    while_body,
                                    resume_after_stmt,
                                    ..
                                } => (
                                    &while_body.stmts[*resume_after_stmt + 1..],
                                    *resume_after_stmt + 1,
                                ),
                            };

                        // Build intercept map for this frame's tail: which
                        // tail stmts contain performs from other sites?
                        let mut tail_intercept_map: HashMap<
                            usize,
                            Vec<(usize, &[ResumeFrame<'_>])>,
                        > = HashMap::new();
                        for (other_pc, other_site) in
                            perform_sites.iter().enumerate()
                        {
                            if other_pc == site.pc {
                                continue;
                            }
                            // Check if other_site shares the same nesting
                            // context at levels 0..level.
                            if other_site.resume_path.len() <= level {
                                continue;
                            }
                            let mut frames_match = true;
                            for i in 0..level {
                                if i >= other_site.resume_path.len()
                                    || !resume_frame_same_structure(
                                        &site.resume_path[i],
                                        &other_site.resume_path[i],
                                    )
                                {
                                    frames_match = false;
                                    break;
                                }
                            }
                            if !frames_match {
                                continue;
                            }
                            // At this level, check if the other site's frame
                            // matches and has a stmt index in the tail range.
                            let other_frame = &other_site.resume_path[level];
                            if !resume_frame_same_structure(frame, other_frame)
                            {
                                continue;
                            }
                            let other_ras = match other_frame {
                                ResumeFrame::IfThen {
                                    resume_after_stmt, ..
                                }
                                | ResumeFrame::IfElse {
                                    resume_after_stmt, ..
                                }
                                | ResumeFrame::WhenArm {
                                    resume_after_stmt, ..
                                }
                                | ResumeFrame::WhileBody {
                                    resume_after_stmt, ..
                                }
                                | ResumeFrame::Block {
                                    resume_after_stmt, ..
                                } => *resume_after_stmt,
                            };
                            if other_ras >= tail_base_idx {
                                let inner_path =
                                    &other_site.resume_path[level + 1..];
                                tail_intercept_map
                                    .entry(other_ras)
                                    .or_default()
                                    .push((other_pc, inner_path));
                            }
                        }

                        // Emit tail stmts with interception.
                        for (i, tail_stmt) in tail_stmts.iter().enumerate() {
                            if terminated {
                                break;
                            }
                            let actual_idx = tail_base_idx + i;
                            if let Some(intercepts) =
                                tail_intercept_map.get(&actual_idx)
                            {
                                let (intercept_pc, inner_path) = intercepts
                                    .iter()
                                    .find(|(_, ip)| ip.is_empty())
                                    .copied()
                                    .unwrap_or(intercepts[0]);
                                if inner_path.is_empty() {
                                    // Direct perform at this level
                                    let intercept_site =
                                        &perform_sites[intercept_pc];
                                    for (slot, arg) in binder_slots
                                        .iter()
                                        .zip(intercept_site.args.iter())
                                    {
                                        let hir::CallArg::Positional(expr) =
                                            arg
                                        else {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "handle escape named perform arg",
                                                at: span.into(),
                                            });
                                        };
                                        let v =
                                            cg.codegen_expr_in_expected_context(
                                                expr,
                                                Some(slot.ty),
                                            )?;
                                        let _stored = cg.store_local_value(
                                            expr.span, slot.ptr, slot.ty, v,
                                        )?;
                                    }
                                    let _ = cg.builder.build_store(
                                        intercept_next_pc_ptr,
                                        i32_ty.const_int(
                                            intercept_pc as u64,
                                            false,
                                        ),
                                    )?;
                                    cg.builder.build_unconditional_branch(
                                        intercept_bb,
                                    )?;
                                    terminated = true;
                                    break;
                                }
                                // Nested perform in control flow within tail:
                                // fall through to normal codegen for now
                                // (T1606e scope limit: nested-in-nested).
                            }
                            // Normal stmt codegen with lifted local handling
                            match &tail_stmt.kind {
                                hir::StmtKind::Empty => {}
                                hir::StmtKind::Val(decl) => {
                                    if let Some(id) = decl.id {
                                        if body_lift_ids.contains(&id) {
                                            let Some(init) =
                                                decl.init.as_ref()
                                            else {
                                                return Err(LlvmEmitError::UnsupportedMainBody {
                                                    kind: "lifted local without init",
                                                    at: decl.span.into(),
                                                });
                                            };
                                            let decl_ty = cg
                                                .cg_ty_of(decl.ty)
                                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                                    kind: "lifted local type",
                                                    at: decl.span.into(),
                                                })?;
                                            let local = cg
                                                .env
                                                .get(id)
                                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                                    kind: "lifted local slot missing",
                                                    at: decl.span.into(),
                                                })?;
                                            let v = cg
                                                .codegen_expr_in_expected_context(
                                                    init,
                                                    Some(decl_ty),
                                                )?;
                                            let _stored =
                                                cg.store_local_value(
                                                    decl.span, local.ptr,
                                                    decl_ty, v,
                                                )?;
                                        } else {
                                            cg.codegen_val_decl(decl)?;
                                        }
                                    } else {
                                        cg.codegen_val_decl(decl)?;
                                    }
                                }
                                hir::StmtKind::Assign {
                                    lhs,
                                    eq_span,
                                    rhs,
                                } => {
                                    cg.codegen_assign_stmt(
                                        *eq_span, lhs, rhs,
                                    )?;
                                }
                                hir::StmtKind::Expr(expr) => {
                                    let _ = cg.codegen_expr(expr)?;
                                }
                                hir::StmtKind::While { cond, body } => {
                                    cg.codegen_while_stmt(
                                        tail_stmt.span,
                                        cond,
                                        body,
                                    )?;
                                }
                                _ => {
                                    return Err(
                                        LlvmEmitError::UnsupportedMainBody {
                                            kind: "statement inside continuation step (nested tail)",
                                            at: tail_stmt.span.into(),
                                        },
                                    );
                                }
                            }
                        }

                        // For WhileBody: after the tail, re-enter the while
                        // loop with perform interception for all sites in
                        // this body.
                        if !terminated {
                            if let ResumeFrame::WhileBody {
                                while_cond,
                                while_body,
                                ..
                            } = frame
                            {
                                // Build intercept map for the full while body.
                                let mut while_intercept_map: HashMap<
                                    usize,
                                    Vec<(usize, &[ResumeFrame<'_>])>,
                                > = HashMap::new();
                                for (other_pc, other_site) in
                                    perform_sites.iter().enumerate()
                                {
                                    for (fi, fcheck) in other_site
                                        .resume_path
                                        .iter()
                                        .enumerate()
                                    {
                                        if let ResumeFrame::WhileBody {
                                            while_body: wb,
                                            resume_after_stmt: ras,
                                            ..
                                        } = fcheck
                                        {
                                            if std::ptr::eq(*wb, *while_body) {
                                                while_intercept_map
                                                    .entry(*ras)
                                                    .or_default()
                                                    .push((
                                                        other_pc,
                                                        &other_site
                                                            .resume_path
                                                            [fi + 1..],
                                                    ));
                                                break;
                                            }
                                        }
                                    }
                                }

                                // Generate while loop.
                                let while_cond_bb =
                                    self.context.append_basic_block(
                                        step_fn,
                                        &format!(
                                            "step_pc{pc}_while_cond"
                                        ),
                                    );
                                let while_body_bb =
                                    self.context.append_basic_block(
                                        step_fn,
                                        &format!(
                                            "step_pc{pc}_while_body"
                                        ),
                                    );
                                let while_after_bb =
                                    self.context.append_basic_block(
                                        step_fn,
                                        &format!(
                                            "step_pc{pc}_while_after"
                                        ),
                                    );

                                cg.builder.build_unconditional_branch(
                                    while_cond_bb,
                                )?;
                                cg.builder.position_at_end(while_cond_bb);
                                let cv =
                                    cg.codegen_expr_in_expected_context(
                                        while_cond,
                                        Some(CgTy::Bool),
                                    )?;
                                let cb = cv.as_bool().ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "while cond value (step while re-exec)",
                                        at: while_cond.span.into(),
                                    },
                                )?;
                                cg.builder.build_conditional_branch(
                                    cb,
                                    while_body_bb,
                                    while_after_bb,
                                )?;

                                cg.builder.position_at_end(while_body_bb);
                                cg.env.push_scope();
                                let mut while_terminated = false;
                                for (body_idx, body_stmt) in
                                    while_body.stmts.iter().enumerate()
                                {
                                    if while_terminated {
                                        break;
                                    }
                                    if let Some(intercepts) =
                                        while_intercept_map.get(&body_idx)
                                    {
                                        let (intercept_pc, inner_path) =
                                            intercepts
                                                .iter()
                                                .find(|(_, ip)| {
                                                    ip.is_empty()
                                                })
                                                .copied()
                                                .unwrap_or(intercepts[0]);
                                        if inner_path.is_empty() {
                                            // Direct perform in while body
                                            let intercept_site =
                                                &perform_sites[intercept_pc];
                                            for (slot, arg) in binder_slots
                                                .iter()
                                                .zip(
                                                    intercept_site.args.iter(),
                                                )
                                            {
                                                let hir::CallArg::Positional(
                                                    expr,
                                                ) = arg
                                                else {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "handle escape named perform arg",
                                                        at: span.into(),
                                                    });
                                                };
                                                let v = cg
                                                    .codegen_expr_in_expected_context(
                                                        expr,
                                                        Some(slot.ty),
                                                    )?;
                                                let _stored =
                                                    cg.store_local_value(
                                                        expr.span,
                                                        slot.ptr,
                                                        slot.ty,
                                                        v,
                                                    )?;
                                            }
                                            let _ = cg.builder.build_store(
                                                intercept_next_pc_ptr,
                                                i32_ty.const_int(
                                                    intercept_pc as u64,
                                                    false,
                                                ),
                                            )?;
                                            cg.builder
                                                .build_unconditional_branch(
                                                    intercept_bb,
                                                )?;
                                            while_terminated = true;
                                            break;
                                        } else {
                                            // Nested perform in if/etc inside while body.
                                            // Generate control flow with interception.
                                            match &inner_path[0] {
                                                ResumeFrame::IfThen {
                                                    if_expr,
                                                    then_block_stmts,
                                                    resume_after_stmt:
                                                        perform_stmt_idx,
                                                    ..
                                                } => {
                                                    if let hir::ExprKind::If {
                                                        cond: if_cond,
                                                        then_branch: _,
                                                        else_branch,
                                                    } = &if_expr.kind
                                                    {
                                                        let cond_v = cg
                                                            .codegen_expr_in_expected_context(
                                                                if_cond,
                                                                Some(
                                                                    CgTy::Bool,
                                                                ),
                                                            )?;
                                                        let cond_b = cond_v
                                                            .as_bool()
                                                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                kind: "if cond value (while body intercept)",
                                                                at: if_cond.span.into(),
                                                            })?;
                                                        let then_bb_i = self
                                                            .context
                                                            .append_basic_block(
                                                                step_fn,
                                                                &format!(
                                                                    "step_pc{pc}_wif_then"
                                                                ),
                                                            );
                                                        let has_else =
                                                            else_branch
                                                                .is_some();
                                                        let else_or_after = self
                                                            .context
                                                            .append_basic_block(
                                                                step_fn,
                                                                &format!(
                                                                    "step_pc{pc}_wif_{}",
                                                                    if has_else {
                                                                        "else"
                                                                    } else {
                                                                        "after"
                                                                    }
                                                                ),
                                                            );
                                                        let after_if_bb =
                                                            if has_else {
                                                                self.context
                                                                    .append_basic_block(
                                                                        step_fn,
                                                                        &format!(
                                                                            "step_pc{pc}_wif_after"
                                                                        ),
                                                                    )
                                                            } else {
                                                                else_or_after
                                                            };
                                                        cg.builder
                                                            .build_conditional_branch(
                                                                cond_b,
                                                                then_bb_i,
                                                                else_or_after,
                                                            )?;

                                                        // Then-branch: codegen stmts before perform, then intercept
                                                        cg.builder
                                                            .position_at_end(
                                                                then_bb_i,
                                                            );
                                                        for (ti, tstmt) in
                                                            then_block_stmts
                                                                .iter()
                                                                .enumerate()
                                                        {
                                                            if ti
                                                                == *perform_stmt_idx
                                                            {
                                                                let is =
                                                                    &perform_sites
                                                                        [intercept_pc];
                                                                for (
                                                                    slot,
                                                                    arg,
                                                                ) in binder_slots
                                                                    .iter()
                                                                    .zip(
                                                                        is.args
                                                                            .iter(),
                                                                    )
                                                                {
                                                                    let hir::CallArg::Positional(expr) = arg else {
                                                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                                                            kind: "handle escape named perform arg",
                                                                            at: span.into(),
                                                                        });
                                                                    };
                                                                    let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                    let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                                }
                                                                let _ = cg
                                                                    .builder
                                                                    .build_store(
                                                                        intercept_next_pc_ptr,
                                                                        i32_ty.const_int(
                                                                            intercept_pc as u64,
                                                                            false,
                                                                        ),
                                                                    )?;
                                                                cg.builder.build_unconditional_branch(intercept_bb)?;
                                                                break;
                                                            }
                                                            // Normal stmt before the perform
                                                            match &tstmt.kind {
                                                                hir::StmtKind::Empty => {}
                                                                hir::StmtKind::Val(decl) => {
                                                                    if let Some(id) = decl.id {
                                                                        if body_lift_ids.contains(&id) {
                                                                            let Some(init) = decl.init.as_ref() else {
                                                                                return Err(LlvmEmitError::UnsupportedMainBody {
                                                                                    kind: "lifted local without init",
                                                                                    at: decl.span.into(),
                                                                                });
                                                                            };
                                                                            let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                                kind: "lifted local type",
                                                                                at: decl.span.into(),
                                                                            })?;
                                                                            let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                                kind: "lifted local slot missing",
                                                                                at: decl.span.into(),
                                                                            })?;
                                                                            let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                            let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                                        } else {
                                                                            cg.codegen_val_decl(decl)?;
                                                                        }
                                                                    } else {
                                                                        cg.codegen_val_decl(decl)?;
                                                                    }
                                                                }
                                                                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                                                                    cg.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                                                                }
                                                                hir::StmtKind::Expr(expr) => {
                                                                    let _ = cg.codegen_expr(expr)?;
                                                                }
                                                                _ => {}
                                                            }
                                                        }

                                                        // Else-branch: codegen normally
                                                        if let Some(
                                                            else_expr,
                                                        ) = else_branch
                                                            .as_deref()
                                                        {
                                                            cg.builder.position_at_end(else_or_after);
                                                            let _ = cg.codegen_expr(else_expr)?;
                                                            cg.builder.build_unconditional_branch(after_if_bb)?;
                                                        }
                                                        // After-if: continue while body
                                                        cg.builder
                                                            .position_at_end(
                                                                after_if_bb,
                                                            );
                                                    }
                                                }
                                                ResumeFrame::IfElse {
                                                    if_expr,
                                                    else_block_stmts,
                                                    resume_after_stmt:
                                                        perform_stmt_idx,
                                                    ..
                                                } => {
                                                    if let hir::ExprKind::If {
                                                        cond: if_cond,
                                                        then_branch,
                                                        else_branch: _,
                                                    } = &if_expr.kind
                                                    {
                                                        let cond_v = cg
                                                            .codegen_expr_in_expected_context(
                                                                if_cond,
                                                                Some(
                                                                    CgTy::Bool,
                                                                ),
                                                            )?;
                                                        let cond_b = cond_v
                                                            .as_bool()
                                                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                kind: "if cond value (while body intercept)",
                                                                at: if_cond.span.into(),
                                                            })?;
                                                        let then_bb_i = self
                                                            .context
                                                            .append_basic_block(
                                                                step_fn,
                                                                &format!(
                                                                    "step_pc{pc}_wif_then"
                                                                ),
                                                            );
                                                        let else_bb_i = self
                                                            .context
                                                            .append_basic_block(
                                                                step_fn,
                                                                &format!(
                                                                    "step_pc{pc}_wif_else"
                                                                ),
                                                            );
                                                        let after_if_bb = self
                                                            .context
                                                            .append_basic_block(
                                                                step_fn,
                                                                &format!(
                                                                    "step_pc{pc}_wif_after"
                                                                ),
                                                            );
                                                        cg.builder
                                                            .build_conditional_branch(
                                                                cond_b,
                                                                then_bb_i,
                                                                else_bb_i,
                                                            )?;

                                                        // Then-branch: codegen normally
                                                        cg.builder
                                                            .position_at_end(
                                                                then_bb_i,
                                                            );
                                                        let _ = cg
                                                            .codegen_expr(
                                                                then_branch,
                                                            )?;
                                                        cg.builder.build_unconditional_branch(after_if_bb)?;

                                                        // Else-branch: stmts before perform, then intercept
                                                        cg.builder
                                                            .position_at_end(
                                                                else_bb_i,
                                                            );
                                                        for (ei, estmt) in
                                                            else_block_stmts
                                                                .iter()
                                                                .enumerate()
                                                        {
                                                            if ei
                                                                == *perform_stmt_idx
                                                            {
                                                                let is =
                                                                    &perform_sites
                                                                        [intercept_pc];
                                                                for (
                                                                    slot,
                                                                    arg,
                                                                ) in binder_slots
                                                                    .iter()
                                                                    .zip(
                                                                        is.args
                                                                            .iter(),
                                                                    )
                                                                {
                                                                    let hir::CallArg::Positional(expr) = arg else {
                                                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                                                            kind: "handle escape named perform arg",
                                                                            at: span.into(),
                                                                        });
                                                                    };
                                                                    let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                    let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                                }
                                                                let _ = cg
                                                                    .builder
                                                                    .build_store(
                                                                        intercept_next_pc_ptr,
                                                                        i32_ty.const_int(
                                                                            intercept_pc as u64,
                                                                            false,
                                                                        ),
                                                                    )?;
                                                                cg.builder.build_unconditional_branch(intercept_bb)?;
                                                                break;
                                                            }
                                                            match &estmt.kind {
                                                                hir::StmtKind::Empty => {}
                                                                hir::StmtKind::Val(decl) => {
                                                                    if let Some(id) = decl.id {
                                                                        if body_lift_ids.contains(&id) {
                                                                            let Some(init) = decl.init.as_ref() else {
                                                                                return Err(LlvmEmitError::UnsupportedMainBody {
                                                                                    kind: "lifted local without init",
                                                                                    at: decl.span.into(),
                                                                                });
                                                                            };
                                                                            let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                                kind: "lifted local type",
                                                                                at: decl.span.into(),
                                                                            })?;
                                                                            let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                                kind: "lifted local slot missing",
                                                                                at: decl.span.into(),
                                                                            })?;
                                                                            let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                            let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                                        } else {
                                                                            cg.codegen_val_decl(decl)?;
                                                                        }
                                                                    } else {
                                                                        cg.codegen_val_decl(decl)?;
                                                                    }
                                                                }
                                                                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                                                                    cg.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                                                                }
                                                                hir::StmtKind::Expr(expr) => {
                                                                    let _ = cg.codegen_expr(expr)?;
                                                                }
                                                                _ => {}
                                                            }
                                                        }

                                                        // After-if
                                                        cg.builder
                                                            .position_at_end(
                                                                after_if_bb,
                                                            );
                                                    }
                                                }
                                                _ => {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "T1606e: unsupported nested perform path in while body",
                                                        at: span.into(),
                                                    });
                                                }
                                            }
                                        }
                                    } else {
                                        // Normal stmt in while body
                                        match &body_stmt.kind {
                                            hir::StmtKind::Empty => {}
                                            hir::StmtKind::Val(decl) => {
                                                if let Some(id) = decl.id {
                                                    if body_lift_ids
                                                        .contains(&id)
                                                    {
                                                        let Some(init) =
                                                            decl.init.as_ref()
                                                        else {
                                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                                kind: "lifted local without init",
                                                                at: decl.span.into(),
                                                            });
                                                        };
                                                        let decl_ty = cg
                                                            .cg_ty_of(decl.ty)
                                                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                kind: "lifted local type",
                                                                at: decl.span.into(),
                                                            })?;
                                                        let local = cg
                                                            .env
                                                            .get(id)
                                                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                kind: "lifted local slot missing",
                                                                at: decl.span.into(),
                                                            })?;
                                                        let v = cg
                                                            .codegen_expr_in_expected_context(
                                                                init,
                                                                Some(decl_ty),
                                                            )?;
                                                        let _stored = cg
                                                            .store_local_value(
                                                                decl.span,
                                                                local.ptr,
                                                                decl_ty,
                                                                v,
                                                            )?;
                                                    } else {
                                                        cg.codegen_val_decl(
                                                            decl,
                                                        )?;
                                                    }
                                                } else {
                                                    cg.codegen_val_decl(
                                                        decl,
                                                    )?;
                                                }
                                            }
                                            hir::StmtKind::Assign {
                                                lhs,
                                                eq_span,
                                                rhs,
                                            } => {
                                                cg.codegen_assign_stmt(
                                                    *eq_span, lhs, rhs,
                                                )?;
                                            }
                                            hir::StmtKind::Expr(expr) => {
                                                let _ =
                                                    cg.codegen_expr(expr)?;
                                            }
                                            hir::StmtKind::While {
                                                cond,
                                                body,
                                            } => {
                                                cg.codegen_while_stmt(
                                                    body_stmt.span,
                                                    cond,
                                                    body,
                                                )?;
                                            }
                                            _ => {
                                                return Err(LlvmEmitError::UnsupportedMainBody {
                                                    kind: "statement inside continuation step (while body)",
                                                    at: body_stmt.span.into(),
                                                });
                                            }
                                        }
                                    }
                                }
                                cg.env.pop_scope();
                                if !while_terminated {
                                    cg.builder.build_unconditional_branch(
                                        while_cond_bb,
                                    )?;
                                }
                                cg.builder
                                    .position_at_end(while_after_bb);
                            }
                        }
                    }

                    // Top-level tail stmts (after top_level_stmt_idx)
                    if !terminated {
                        for (idx, stmt) in
                            handle.body.stmts.iter().enumerate()
                        {
                            if terminated {
                                break;
                            }
                            if idx <= site.top_level_stmt_idx {
                                continue;
                            }
                            if let Some(intercepts) =
                                top_level_intercepts.get(&idx)
                            {
                                // Check for flat intercept first.
                                if let Some(&(next_pc, _)) = intercepts.iter().find(|(_, rp)| rp.is_empty()) {
                                    let next_site = &perform_sites[next_pc];
                                    for (slot, arg) in binder_slots.iter().zip(next_site.args.iter()) {
                                        let hir::CallArg::Positional(expr) = arg else {
                                            return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                        };
                                        let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                        let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                    }
                                    let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(next_pc as u64, false))?;
                                    cg.builder.build_unconditional_branch(intercept_bb)?;
                                    terminated = true;
                                    break;
                                }
                                // T1606e: nested intercepts — same logic as flat path.
                                let first = &intercepts[0];
                                let (intercept_pc, inner_path) = *first;
                                if !inner_path.is_empty() {
                                    match &inner_path[0] {
                                        ResumeFrame::IfThen { if_expr, then_block_stmts, resume_after_stmt: perform_stmt_idx, .. } => {
                                            if let hir::ExprKind::If { cond: if_cond, then_branch: _, else_branch } = &if_expr.kind {
                                                let cond_v = cg.codegen_expr_in_expected_context(if_cond, Some(CgTy::Bool))?;
                                                let cond_b = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody { kind: "if cond (nested tail intercept)", at: if_cond.span.into() })?;
                                                let then_bb_i = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_ntail_if_then"));
                                                let has_else = else_branch.is_some();
                                                let else_or_after = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_ntail_if_{}", if has_else { "else" } else { "after" }));
                                                let after_if_bb = if has_else { self.context.append_basic_block(step_fn, &format!("step_pc{pc}_ntail_if_after")) } else { else_or_after };
                                                cg.builder.build_conditional_branch(cond_b, then_bb_i, else_or_after)?;
                                                cg.builder.position_at_end(then_bb_i);
                                                for (ti, tstmt) in then_block_stmts.iter().enumerate() {
                                                    if ti == *perform_stmt_idx {
                                                        let is = &perform_sites[intercept_pc];
                                                        for (slot, arg) in binder_slots.iter().zip(is.args.iter()) {
                                                            let hir::CallArg::Positional(expr) = arg else {
                                                                return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                            };
                                                            let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                            let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                        }
                                                        let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(intercept_pc as u64, false))?;
                                                        cg.builder.build_unconditional_branch(intercept_bb)?;
                                                        break;
                                                    }
                                                    match &tstmt.kind {
                                                        hir::StmtKind::Empty => {}
                                                        hir::StmtKind::Val(decl) => {
                                                            if let Some(id) = decl.id {
                                                                if body_lift_ids.contains(&id) {
                                                                    let Some(init) = decl.init.as_ref() else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() }); };
                                                                    let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                    let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                    let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                    let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                                } else { cg.codegen_val_decl(decl)?; }
                                                            } else { cg.codegen_val_decl(decl)?; }
                                                        }
                                                        hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                        hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                        _ => {}
                                                    }
                                                }
                                                // Else-branch: check if also intercepted (both branches).
                                                let else_intercept = intercepts.iter().find(|(_, rp)| matches!(rp.first(), Some(ResumeFrame::IfElse { .. })));
                                                if let Some(&(else_ipc, else_rp)) = else_intercept {
                                                    if let ResumeFrame::IfElse { else_block_stmts: ebs, resume_after_stmt: epi, .. } = &else_rp[0] {
                                                        cg.builder.position_at_end(else_or_after);
                                                        for (ei, estmt) in ebs.iter().enumerate() {
                                                            if ei == *epi {
                                                                let es = &perform_sites[else_ipc];
                                                                for (slot, arg) in binder_slots.iter().zip(es.args.iter()) {
                                                                    let hir::CallArg::Positional(expr) = arg else {
                                                                        return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                                    };
                                                                    let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                    let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                                }
                                                                let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(else_ipc as u64, false))?;
                                                                cg.builder.build_unconditional_branch(intercept_bb)?;
                                                                break;
                                                            }
                                                            match &estmt.kind {
                                                                hir::StmtKind::Empty => {}
                                                                hir::StmtKind::Val(decl) => {
                                                                    if let Some(id) = decl.id {
                                                                        if body_lift_ids.contains(&id) {
                                                                            let Some(init) = decl.init.as_ref() else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() }); };
                                                                            let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                            let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                            let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                            let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                                        } else { cg.codegen_val_decl(decl)?; }
                                                                    } else { cg.codegen_val_decl(decl)?; }
                                                                }
                                                                hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                                hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                                _ => {}
                                                            }
                                                        }
                                                    }
                                                } else if let Some(else_expr) = else_branch.as_deref() {
                                                    cg.builder.position_at_end(else_or_after);
                                                    let _ = cg.codegen_expr(else_expr)?;
                                                    cg.builder.build_unconditional_branch(after_if_bb)?;
                                                }
                                                cg.builder.position_at_end(after_if_bb);
                                                continue;
                                            }
                                        }
                                        ResumeFrame::WhileBody { while_cond, while_body, resume_after_stmt: perform_body_idx, .. } => {
                                            let wc_bb = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_ntail_wc"));
                                            let wb_bb = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_ntail_wb"));
                                            let wa_bb = self.context.append_basic_block(step_fn, &format!("step_pc{pc}_ntail_wa"));
                                            cg.builder.build_unconditional_branch(wc_bb)?;
                                            cg.builder.position_at_end(wc_bb);
                                            let cv = cg.codegen_expr_in_expected_context(while_cond, Some(CgTy::Bool))?;
                                            let cb = cv.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody { kind: "while cond (nested tail intercept)", at: while_cond.span.into() })?;
                                            cg.builder.build_conditional_branch(cb, wb_bb, wa_bb)?;
                                            cg.builder.position_at_end(wb_bb);
                                            cg.env.push_scope();
                                            let mut wt = false;
                                            for (bi, bstmt) in while_body.stmts.iter().enumerate() {
                                                if wt { break; }
                                                if bi == *perform_body_idx {
                                                    let is = &perform_sites[intercept_pc];
                                                    for (slot, arg) in binder_slots.iter().zip(is.args.iter()) {
                                                        let hir::CallArg::Positional(expr) = arg else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() }); };
                                                        let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                        let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                                    }
                                                    let _ = cg.builder.build_store(intercept_next_pc_ptr, i32_ty.const_int(intercept_pc as u64, false))?;
                                                    cg.builder.build_unconditional_branch(intercept_bb)?;
                                                    wt = true;
                                                    break;
                                                }
                                                match &bstmt.kind {
                                                    hir::StmtKind::Empty => {}
                                                    hir::StmtKind::Val(decl) => {
                                                        if let Some(id) = decl.id {
                                                            if body_lift_ids.contains(&id) {
                                                                let Some(init) = decl.init.as_ref() else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() }); };
                                                                let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
                                                            } else { cg.codegen_val_decl(decl)?; }
                                                        } else { cg.codegen_val_decl(decl)?; }
                                                    }
                                                    hir::StmtKind::Assign { lhs, eq_span, rhs } => { cg.codegen_assign_stmt(*eq_span, lhs, rhs)?; }
                                                    hir::StmtKind::Expr(expr) => { let _ = cg.codegen_expr(expr)?; }
                                                    hir::StmtKind::While { cond, body } => { cg.codegen_while_stmt(bstmt.span, cond, body)?; }
                                                    _ => { return Err(LlvmEmitError::UnsupportedMainBody { kind: "stmt in while (nested tail intercept)", at: bstmt.span.into() }); }
                                                }
                                            }
                                            cg.env.pop_scope();
                                            if !wt { cg.builder.build_unconditional_branch(wc_bb)?; }
                                            cg.builder.position_at_end(wa_bb);
                                            continue;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            // Normal stmt
                            match &stmt.kind {
                                hir::StmtKind::Empty => {}
                                hir::StmtKind::Val(decl) => {
                                    if let Some(id) = decl.id {
                                        if body_lift_ids.contains(&id) {
                                            let Some(init) =
                                                decl.init.as_ref()
                                            else {
                                                return Err(LlvmEmitError::UnsupportedMainBody {
                                                    kind: "lifted local without init",
                                                    at: decl.span.into(),
                                                });
                                            };
                                            let decl_ty = cg
                                                .cg_ty_of(decl.ty)
                                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                                    kind: "lifted local type",
                                                    at: decl.span.into(),
                                                })?;
                                            let local = cg
                                                .env
                                                .get(id)
                                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                                    kind: "lifted local slot missing",
                                                    at: decl.span.into(),
                                                })?;
                                            let v = cg
                                                .codegen_expr_in_expected_context(
                                                    init,
                                                    Some(decl_ty),
                                                )?;
                                            let _stored =
                                                cg.store_local_value(
                                                    decl.span, local.ptr,
                                                    decl_ty, v,
                                                )?;
                                        } else {
                                            cg.codegen_val_decl(decl)?;
                                        }
                                    } else {
                                        cg.codegen_val_decl(decl)?;
                                    }
                                }
                                hir::StmtKind::Assign {
                                    lhs,
                                    eq_span,
                                    rhs,
                                } => {
                                    cg.codegen_assign_stmt(
                                        *eq_span, lhs, rhs,
                                    )?;
                                }
                                hir::StmtKind::Expr(expr) => {
                                    let _ = cg.codegen_expr(expr)?;
                                }
                                hir::StmtKind::While { cond, body } => {
                                    cg.codegen_while_stmt(
                                        stmt.span, cond, body,
                                    )?;
                                }
                                _ => {
                                    return Err(
                                        LlvmEmitError::UnsupportedMainBody {
                                            kind: "statement inside continuation step",
                                            at: stmt.span.into(),
                                        },
                                    );
                                }
                            }
                        }
                    }
                }

                // Completion: unpin state + return
                if !terminated {
                    let unpin = cg.declare_runtime_gc_unpin();
                    let _ = cg.builder.build_call(
                        unpin,
                        &[state_raw.into()],
                        "cont_state_unpin",
                    )?;
                    cg.builder.build_return(None)?;
                }
            }

            // --- T1606e: shared intercept_bb codegen ---
            // All perform interception points branch here after:
            //   (a) evaluating perform args → writing binder slots
            //   (b) storing next_pc into intercept_next_pc_ptr
            //
            // This block handles: write back captures → set pc → create continuation →
            // pin → detach handler → arm body → unpin → return.
            //
            // Guard: intercept_bb is only reachable if there are 2+ performs (a later
            // perform can be intercepted by an earlier pc block) or if any perform is
            // inside a while loop (same perform fires again on loop re-entry).
            let intercept_reachable = perform_sites.len() >= 2
                || perform_sites.iter().any(|s| {
                    s.resume_path
                        .iter()
                        .any(|f| matches!(f, ResumeFrame::WhileBody { .. }))
                });
            cg.builder.position_at_end(intercept_bb);

            if !intercept_reachable {
                // Dead code: no pc block ever branches here. Emit unreachable.
                cg.builder.build_unreachable()?;
            } else {

            // Write back captures (outer_captures + body_lifts) to heap state.
            for (idx, cap) in outer_captures.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "intercept_capture_gep",
                )?;
                let local =
                    cg.env
                        .get(cap.id)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "intercept: capture local not found",
                            at: span.into(),
                        })?;
                if local.ty != cap.ty {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "intercept: capture local type mismatch",
                        at: span.into(),
                    });
                }
                match cap.ty {
                    CgTy::Ref => {
                        let llvm_ty = cg.llvm_basic_type_of(span, CgTy::Ref)?;
                        let loaded = cg
                            .builder
                            .build_load(llvm_ty, local.ptr, "intercept_cap_load_ref")?;
                        let BasicValueEnum::PointerValue(ptr) = loaded else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "intercept: capture ref ptr",
                                at: span.into(),
                            });
                        };
                        let casted = cg.builder.build_pointer_cast(
                            ptr,
                            gc_i8_ptr_ty,
                            "intercept_cap_ref_i8",
                        )?;
                        let _ = cg.store_local_value(
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
                        let llvm_ty = cg.llvm_basic_type_of(span, CgTy::String)?;
                        let loaded = cg
                            .builder
                            .build_load(llvm_ty, local.ptr, "intercept_cap_load_str")?;
                        let BasicValueEnum::PointerValue(ptr) = loaded else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "intercept: capture str ptr",
                                at: span.into(),
                            });
                        };
                        let casted = cg.builder.build_pointer_cast(
                            ptr,
                            gc_i8_ptr_ty,
                            "intercept_cap_str_i8",
                        )?;
                        let _ = cg.store_local_value(
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
                        let loaded = cg
                            .builder
                            .build_load(
                                cg.llvm_basic_type_of(span, CgTy::Bool)?,
                                local.ptr,
                                "intercept_cap_load_bool",
                            )?
                            .into_int_value();
                        let extended = cg.builder.build_int_z_extend(
                            loaded,
                            i64_ty,
                            "intercept_cap_zext_bool",
                        )?;
                        let _ = cg.builder.build_store(field_ptr, extended)?;
                    }
                    CgTy::Int(int_ty_info) => {
                        if int_ty_info.bits > 64 {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "intercept: capture int width > 64",
                                at: span.into(),
                            });
                        }
                        let llvm_ty = cg.llvm_basic_type_of(span, cap.ty)?;
                        let loaded = cg
                            .builder
                            .build_load(llvm_ty, local.ptr, "intercept_cap_load_int")?
                            .into_int_value();
                        let extended = if int_ty_info.bits == 64 {
                            loaded
                        } else if int_ty_info.signed {
                            cg.builder.build_int_s_extend(
                                loaded,
                                i64_ty,
                                "intercept_cap_sext_int",
                            )?
                        } else {
                            cg.builder.build_int_z_extend(
                                loaded,
                                i64_ty,
                                "intercept_cap_zext_int",
                            )?
                        };
                        let _ = cg.builder.build_store(field_ptr, extended)?;
                    }
                    _ => unreachable!("captures filtered by type"),
                }
            }

            for (idx, cap) in body_lifts.iter().enumerate() {
                let field_idx = body_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "intercept_lift_gep",
                )?;
                let local =
                    cg.env
                        .get(cap.id)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "intercept: lift local not found",
                            at: span.into(),
                        })?;
                if local.ty != cap.ty {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "intercept: lift local type mismatch",
                        at: span.into(),
                    });
                }
                match cap.ty {
                    CgTy::Ref => {
                        let llvm_ty = cg.llvm_basic_type_of(span, CgTy::Ref)?;
                        let loaded = cg
                            .builder
                            .build_load(llvm_ty, local.ptr, "intercept_lift_load_ref")?;
                        let BasicValueEnum::PointerValue(ptr) = loaded else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "intercept: lift ref ptr",
                                at: span.into(),
                            });
                        };
                        let casted = cg.builder.build_pointer_cast(
                            ptr,
                            gc_i8_ptr_ty,
                            "intercept_lift_ref_i8",
                        )?;
                        let _ = cg.store_local_value(
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
                        let llvm_ty = cg.llvm_basic_type_of(span, CgTy::String)?;
                        let loaded = cg
                            .builder
                            .build_load(llvm_ty, local.ptr, "intercept_lift_load_str")?;
                        let BasicValueEnum::PointerValue(ptr) = loaded else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "intercept: lift str ptr",
                                at: span.into(),
                            });
                        };
                        let casted = cg.builder.build_pointer_cast(
                            ptr,
                            gc_i8_ptr_ty,
                            "intercept_lift_str_i8",
                        )?;
                        let _ = cg.store_local_value(
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
                        let loaded = cg
                            .builder
                            .build_load(
                                cg.llvm_basic_type_of(span, CgTy::Bool)?,
                                local.ptr,
                                "intercept_lift_load_bool",
                            )?
                            .into_int_value();
                        let extended = cg.builder.build_int_z_extend(
                            loaded,
                            i64_ty,
                            "intercept_lift_zext_bool",
                        )?;
                        let _ = cg.builder.build_store(field_ptr, extended)?;
                    }
                    CgTy::Int(int_ty_info) => {
                        if int_ty_info.bits > 64 {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "intercept: lift int width > 64",
                                at: span.into(),
                            });
                        }
                        let llvm_ty = cg.llvm_basic_type_of(span, cap.ty)?;
                        let loaded = cg
                            .builder
                            .build_load(llvm_ty, local.ptr, "intercept_lift_load_int")?
                            .into_int_value();
                        let extended = if int_ty_info.bits == 64 {
                            loaded
                        } else if int_ty_info.signed {
                            cg.builder.build_int_s_extend(
                                loaded,
                                i64_ty,
                                "intercept_lift_sext_int",
                            )?
                        } else {
                            cg.builder.build_int_z_extend(
                                loaded,
                                i64_ty,
                                "intercept_lift_zext_int",
                            )?
                        };
                        let _ = cg.builder.build_store(field_ptr, extended)?;
                    }
                    _ => unreachable!("captures filtered by type"),
                }
            }

            // Update pc in state from the alloca set by each interception point.
            let next_pc_val = cg
                .builder
                .build_load(i32_ty, intercept_next_pc_ptr, "intercept_load_next_pc")?;
            let pc_ptr = cg.builder.build_struct_gep(
                state_ty,
                state_ptr,
                2,
                "intercept_state_pc_gep",
            )?;
            let _ = cg.builder.build_store(pc_ptr, next_pc_val)?;

            // Create continuation.
            let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
            let step_ptr = step_fn.as_global_value().as_pointer_value();
            let call = cg.builder.build_call(
                rt_cont_alloc,
                &[state_raw.into(), step_ptr.into()],
                "intercept_cont_alloc",
            )?;
            let raw = call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "intercept: continuation alloc return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::PointerValue(k_raw) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "intercept: continuation alloc return type",
                    at: span.into(),
                });
            };

            // Pin k to avoid GC moving it before arm stores it in a root.
            let pin = cg.declare_runtime_gc_pin();
            let _ = cg.builder.build_call(
                pin,
                &[k_raw.into()],
                "intercept_k_pin",
            )?;

            // Store k into cont_ptr.
            let _stored = cg.store_local_value(
                span,
                cont_ptr,
                CgTy::Ref,
                CgValue {
                    ty: CgTy::Ref,
                    value: Some(k_raw.into()),
                },
            )?;

            // Detach handler frame from TLS handler stack (prevent self-capture).
            let handler_frame_ty = cg.llvm_effect_handler_frame_type();
            let frame_ptr = cg.builder.build_struct_gep(
                state_ty,
                state_ptr,
                1,
                "intercept_frame_gep",
            )?;
            let prev_ptr = cg.builder.build_struct_gep(
                handler_frame_ty,
                frame_ptr,
                0,
                "intercept_prev_gep",
            )?;
            let prev_raw = cg
                .builder
                .build_load(i8_ptr_ty, prev_ptr, "intercept_prev")?;
            let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
            let _ = cg.builder.build_call(
                rt_swap,
                &[prev_raw.into()],
                "intercept_detach",
            )?;

            // Execute arm body.
            cg.env.push_scope();
            for slot in &binder_slots {
                cg.env.insert(
                    slot.id,
                    CgLocal {
                        hir_ty: Some(slot.hir_ty),
                        ty: slot.ty,
                        ptr: slot.ptr,
                        mutable: false,
                    },
                );
            }
            cg.env.insert(
                continuation_symbol,
                CgLocal {
                    hir_ty: None,
                    ty: CgTy::Ref,
                    ptr: cont_ptr,
                    mutable: false,
                },
            );
            let arm_v = cg.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
            let _arm_v = if out_ty == CgTy::Unit {
                CgValue::unit()
            } else {
                cg.coerce_value(arm.body.span, arm_v, out_ty)?
            };
            cg.env.pop_scope();

            // Unpin k after arm completes.
            let llvm_ref_ty = cg.llvm_basic_type_of(span, CgTy::Ref)?;
            let k_loaded = cg
                .builder
                .build_load(llvm_ref_ty, cont_ptr, "intercept_k_unpin_load")?
                .into_pointer_value();
            let unpin = cg.declare_runtime_gc_unpin();
            let _ = cg.builder.build_call(
                unpin,
                &[k_loaded.into()],
                "intercept_k_unpin",
            )?;

            cg.builder.build_return(None)?;

            } // if intercept_reachable

            cg.env.pop_scope();
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

        // heap state：`{ header, handler_frame, pc, captures... }`
        let total_size = self.target_data.get_store_size(&state_ty);

        // 分配点统一走 typed alloc：在 runtime 内部写入对象头 `type_desc`，确保 GC 能扫描 capture fields。
        let state_desc_global_name = format!("__scoop_type_desc_cont_state__{func_name}_{seq}");
        let size_bytes = self.target_data.get_store_size(&state_ty);
        // GC trace 起点必须指向第一个 capture field（field index 3），而不是 pc:i32（field index 2）。
        // pc 是 i32，其偏移量通常不满足 pointer alignment；runtime 的
        // scoop_gc_type_descriptor_trace 在 trace_start % sizeof(void*) != 0 时直接返回 0，
        // 导致所有 capture 中的 GC 引用不可见，在 GC stress 下崩溃。
        let first_capture_field_index = 3u32; // 0=header, 1=handler_frame, 2=pc, 3=first capture
        let trace_start_offset_bytes = if outer_captures.is_empty() && body_lifts.is_empty() {
            size_bytes // 无 capture → 不需要 trace
        } else {
            self.target_data
                .offset_of_element(&state_ty, first_capture_field_index)
                .unwrap_or(size_bytes)
        };
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

        // GC 重要性：escape continuation 的 handler frame 存在于 state 对象内部，且该 frame 会被链接到
        // TLS handler stack 上。若 state 作为移动对象被搬迁，则 handler stack 中的 frame 指针会失效。
        //
        // v0 取舍（T1606c）：在 multi-perform 的生命周期内 pin 住 state，避免 moving GC 把它搬走；
        // 并在 step trampoline 走到 "body 完成（无下一次 perform）" 路径时解除 pin。
        let pin = self.declare_runtime_gc_pin();
        let _ = self.builder.build_call(pin, &[state_raw.into()], "cont_state_pin")?;

        let state_ptr_ty = state_ty.ptr_type(self.gc_address_space());
        let state_ptr =
            self.builder
                .build_pointer_cast(state_raw, state_ptr_ty, "cont_state_ptr")?;

        // typed alloc 下，GC 会按 type_desc 扫描 capture fields。
        //
        // 为避免在执行 perform 前语句（其中可能触发分配/GC）时扫描到未初始化垃圾值，
        // 这里先把所有 capture fields 置零；在 perform 点再写入实际捕获值。
        {
            // 注意：不要把 `pc_ptr`（state 内部 derived pointer）跨越任何可能触发 GC 的调用长期保活，
            // 否则 stackmap roots 里会出现 non-header roots，在 `SCOOP_GC_VERIFY_ROOTS=1` 下 fail-fast。
            let pc_ptr = self
                .builder
                .build_struct_gep(state_ty, state_ptr, 2, "cont_state_pc_gep")?;
            let _ = self.builder.build_store(pc_ptr, i32_ty.const_zero())?;
        }
        let outer_field_base = 3u32;
        let body_field_base = outer_field_base.saturating_add(outer_captures.len() as u32);
        for (idx, cap) in outer_captures.iter().enumerate() {
            let field_idx = outer_field_base.saturating_add(idx as u32);
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
        for (idx, cap) in body_lifts.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
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
        // 注意：不要把 `frame_ptr`（state 内部 derived pointer）跨越 perform 前的语句长期保活，
        // 否则会在后续 GC safepoint 的 stackmap roots 中出现 derived/non-header roots。
        let frame_ptr =
            self.builder
                .build_struct_gep(state_ty, state_ptr, 1, "cont_state_frame_gep")?;
        let frame_i8 = self.builder.build_address_space_cast(
            frame_ptr,
            i8_ptr_ty,
            "handle_escape_frame_i8",
        )?;
        // T1608：使用统一的 op_tag 分配。
        let escape_tag = self.effect_op_tag(&arm.op.op.fqn);
        let op_tag_i32 = self
            .context
            .i32_type()
            .const_int(escape_tag as u64, false);
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
                hir::StmtKind::While { cond, body } => {
                    self.codegen_while_stmt(stmt.span, cond, body)?;
                }
                hir::StmtKind::Return { .. }
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
        for (idx, cap) in outer_captures.iter().enumerate() {
            let field_idx = outer_field_base.saturating_add(idx as u32);
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
        for (idx, cap) in body_lifts.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_ptr,
                field_idx,
                "cont_state_capture_gep",
            )?;

            let Some(local) = self.env.get(cap.id) else {
                // 该 local 尚未执行到声明位置：保持 alloc 时的 0 初始化值即可。
                continue;
            };
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

        // 第一条 continuation resume 对应 pc=0（从第 1 个 perform 点之后继续）。
        {
            let pc_ptr = self
                .builder
                .build_struct_gep(state_ty, state_ptr, 2, "cont_state_pc_gep")?;
            let _ = self.builder.build_store(pc_ptr, i32_ty.const_zero())?;
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

        // GC 重要性（T1606c）：
        // - handler arm body 里可能触发分配/GC（例如 println/f-string）；
        // - 但 `k` 在 arm 内部可能尚未被存入任何"可被 GC 扫描的根"（heap field / handle / pin 等）；
        // - 因此这里先临时 pin 住，避免 `SCOOP_GC_STRESS=1` 下被提前回收/搬迁；
        // - 在 arm 结束、返回到 done_bb 前解除 pin（见下方 done_bb）。
        let pin = self.declare_runtime_gc_pin();
        let _ = self
            .builder
            .build_call(pin, &[k_raw.into()], "handle_escape_k_pin")?;

        let _stored = self.store_local_value(
            span,
            cont_ptr,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(k_raw.into()),
            },
        )?;

        // 将 handler frame 从当前线程的 handler stack 顶部"摘除"（不清理 frame 字段），以便：
        // - handler arm body 在 dispatch scope 外执行（Appendix A.4）
        // - continuation 捕获的 handler stack（frame->prev 链）保持完整（spec §5.5）
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let frame_ptr =
            self.builder
                .build_struct_gep(state_ty, state_ptr, 1, "cont_state_frame_gep")?;
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

        // 解除上面为 arm 临时 pin 的 continuation。
        let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
        let k_loaded = self
            .builder
            .build_load(llvm_ref_ty, cont_ptr, "handle_escape_k_unpin_load")?
            .into_pointer_value();
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self
            .builder
            .build_call(unpin, &[k_loaded.into()], "handle_escape_k_unpin")?;

        Ok(match out_ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
            | CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
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
        // T1607：payload 不再受限于 u64 word；支持任意类型（scalar / GC ref / compound）。
        // 实现：把值写入 continuation 的 resume_word / resume_gc_ref 槽位，再调用 scoop_continuation_resume。
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

        // T1607：获取 continuation 结构体类型，GEP 到 resume_word / resume_gc_ref 槽位。
        let cont_ty = self.llvm_continuation_struct_type();
        let cont_ptr_ty = cont_ty.ptr_type(self.gc_address_space());
        let cont_ptr =
            self.builder
                .build_pointer_cast(k_ptr, cont_ptr_ty, "cont_resume_k_typed")?;

        let i64_ty = self.context.i64_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        match value.ty {
            CgTy::Unit => {
                // Unit：写 0 到 resume_word，resume_gc_ref 置 null。
                let word_ptr = self.builder.build_struct_gep(
                    cont_ty,
                    cont_ptr,
                    6,
                    "cont_resume_word_gep",
                )?;
                let _ = self
                    .builder
                    .build_store(word_ptr, i64_ty.const_int(0, false))?;
                let ref_ptr = self.builder.build_struct_gep(
                    cont_ty,
                    cont_ptr,
                    7,
                    "cont_resume_gc_ref_gep",
                )?;
                let _ = self
                    .builder
                    .build_store(ref_ptr, i8_ptr_ty.const_null())?;
            }
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Continuation.resume bool value",
                    at: value_expr.span.into(),
                })?;
                let extended = self
                    .builder
                    .build_int_z_extend(b, i64_ty, "resume_bool_to_u64")?;
                let word_ptr = self.builder.build_struct_gep(
                    cont_ty,
                    cont_ptr,
                    6,
                    "cont_resume_word_gep",
                )?;
                let _ = self.builder.build_store(word_ptr, extended)?;
                let ref_ptr = self.builder.build_struct_gep(
                    cont_ty,
                    cont_ptr,
                    7,
                    "cont_resume_gc_ref_gep",
                )?;
                let _ = self
                    .builder
                    .build_store(ref_ptr, i8_ptr_ty.const_null())?;
            }
            CgTy::Int(_) => {
                let word = self.coerce_u64_word(value_expr.span, value)?;
                let word_ptr = self.builder.build_struct_gep(
                    cont_ty,
                    cont_ptr,
                    6,
                    "cont_resume_word_gep",
                )?;
                let _ = self.builder.build_store(word_ptr, word)?;
                let ref_ptr = self.builder.build_struct_gep(
                    cont_ty,
                    cont_ptr,
                    7,
                    "cont_resume_gc_ref_gep",
                )?;
                let _ = self
                    .builder
                    .build_store(ref_ptr, i8_ptr_ty.const_null())?;
            }
            CgTy::String | CgTy::Ref => {
                // GC reference：写 0 到 resume_word，写 GC ptr 到 resume_gc_ref（with write barrier）。
                let word_ptr = self.builder.build_struct_gep(
                    cont_ty,
                    cont_ptr,
                    6,
                    "cont_resume_word_gep",
                )?;
                let _ = self
                    .builder
                    .build_store(word_ptr, i64_ty.const_int(0, false))?;

                let Some(raw_val) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Continuation.resume gc ref value",
                        at: value_expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(gc_val) = raw_val else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Continuation.resume gc ref type",
                        at: value_expr.span.into(),
                    });
                };

                // 使用 write barrier 写入 GC 堆对象的 GC 引用槽位。
                let ref_ptr = self.builder.build_struct_gep(
                    cont_ty,
                    cont_ptr,
                    7,
                    "cont_resume_gc_ref_gep",
                )?;
                let wb = self.declare_runtime_gc_write_barrier();
                // slot_addr 必须是 addrspace(0)（见 gc.rs 写屏障约定）。
                let slot_addr_gc = self.builder.build_pointer_cast(
                    ref_ptr,
                    gc_i8_ptr_ty,
                    "cont_resume_gc_ref_slot_gc",
                )?;
                let slot_addr = self.builder.build_address_space_cast(
                    slot_addr_gc,
                    i8_ptr_ty,
                    "cont_resume_gc_ref_slot",
                )?;
                let value_i8 =
                    self.builder
                        .build_pointer_cast(gc_val, gc_i8_ptr_ty, "cont_resume_gc_val_i8")?;
                let _ = self
                    .builder
                    .build_call(wb, &[slot_addr.into(), value_i8.into()], "cont_resume_wb")?;
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                // 复合类型：box 到 GC heap，将 box ptr 写入 resume_gc_ref。
                let word_ptr = self.builder.build_struct_gep(
                    cont_ty,
                    cont_ptr,
                    6,
                    "cont_resume_word_gep",
                )?;
                let _ = self
                    .builder
                    .build_store(word_ptr, i64_ty.const_int(0, false))?;

                let Some(raw_val) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Continuation.resume compound value",
                        at: value_expr.span.into(),
                    });
                };

                // 创建 box type: { GcObjectHeader, <payload_type> }
                let payload_llvm_ty = self.llvm_basic_type_of(value_expr.span, value.ty)?;
                let header_ty = self.llvm_gc_object_header_type();
                let box_ty_name = format!("scoop.runtime.ResumePayloadBox__{}", {
                    // 使用 payload 类型的哈希作为唯一名称
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    format!("{:?}", value.ty).hash(&mut h);
                    h.finish()
                });
                let box_ty = if let Some(existing) = self.context.get_struct_type(&box_ty_name) {
                    existing
                } else {
                    let t = self.context.opaque_struct_type(&box_ty_name);
                    t.set_body(&[header_ty.into(), payload_llvm_ty], false);
                    t
                };

                // 创建 box 的 type descriptor
                let box_desc_name =
                    format!("__scoop_type_desc_resume_payload_box__{}", &box_ty_name);
                let box_size = self.target_data.get_store_size(&box_ty);
                let trace_start = self
                    .target_data
                    .offset_of_element(&box_ty, 1)
                    .unwrap_or(box_size);
                let box_desc = self.get_or_create_type_descriptor_global(
                    span,
                    &box_desc_name,
                    &box_ty_name,
                    box_ty,
                    trace_start,
                    None,
                    None,
                    None,
                )?;

                // 分配 box
                let rt_alloc = self.declare_runtime_alloc_typed();
                let size_v = self.context.i64_type().const_int(box_size, false);
                let desc_i8 = self.builder.build_pointer_cast(
                    box_desc.as_pointer_value(),
                    i8_ptr_ty,
                    "resume_box_desc_i8",
                )?;
                let alloc_call = self.builder.build_call(
                    rt_alloc,
                    &[desc_i8.into(), size_v.into()],
                    "resume_box_alloc",
                )?;
                let box_raw = alloc_call
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "resume box alloc return",
                        at: span.into(),
                    })?;
                let BasicValueEnum::PointerValue(box_gc_ptr) = box_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "resume box alloc return type",
                        at: span.into(),
                    });
                };

                // 存入 payload
                let box_ptr_ty = box_ty.ptr_type(self.gc_address_space());
                let box_typed =
                    self.builder
                        .build_pointer_cast(box_gc_ptr, box_ptr_ty, "resume_box_typed")?;
                let payload_ptr = self.builder.build_struct_gep(
                    box_ty,
                    box_typed,
                    1,
                    "resume_box_payload_gep",
                )?;
                let _ = self.builder.build_store(payload_ptr, raw_val)?;

                // 写入 continuation 的 resume_gc_ref（with write barrier）
                let ref_ptr = self.builder.build_struct_gep(
                    cont_ty,
                    cont_ptr,
                    7,
                    "cont_resume_gc_ref_gep",
                )?;
                let wb = self.declare_runtime_gc_write_barrier();
                let slot_addr_gc = self.builder.build_pointer_cast(
                    ref_ptr,
                    gc_i8_ptr_ty,
                    "cont_resume_gc_ref_slot_gc",
                )?;
                let slot_addr = self.builder.build_address_space_cast(
                    slot_addr_gc,
                    i8_ptr_ty,
                    "cont_resume_gc_ref_slot",
                )?;
                let box_i8 = self.builder.build_pointer_cast(
                    box_gc_ptr,
                    gc_i8_ptr_ty,
                    "resume_box_i8",
                )?;
                let _ = self.builder.build_call(
                    wb,
                    &[slot_addr.into(), box_i8.into()],
                    "cont_resume_box_wb",
                )?;
            }
        }

        // 调用新 ABI：payload 已在 continuation 槽位中。
        let rt_resume = self.declare_runtime_continuation_resume();
        let k_i8 =
            self.builder
                .build_pointer_cast(k_ptr, self.llvm_gc_i8_ptr_type(), "cont_k_i8")?;
        let _ = self
            .builder
            .build_call(rt_resume, &[k_i8.into()], "cont_resume")?;
        // continuation resume 可能触发 `Raise<RuntimeError>`（例如 one-shot 违规），需要按 Raise 的最小约定传播。
        self.emit_effect_unwind_if_active(span)?;

        Ok(CgValue::unit())
    }

    pub(super) fn coerce_u64_word(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        // 将一个可表示为 "word-sized u64 payload" 的值转换为 `i64`（在 ABI 层作为 `uint64_t` 使用）。
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
