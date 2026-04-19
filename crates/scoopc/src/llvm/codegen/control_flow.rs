//! 控制流 codegen（T0102d：从 `codegen/mod.rs` 拆分）。
//!
//! 说明：
//! - 该模块主要承载 `block/if/when` 等需要显式 CFG 结构的 lowering；
//! - effect/continuation/GC/statepoint 相关逻辑会在后续任务（T0102e）继续拆分。

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn codegen_block_as_exit_code(
        &mut self,
        block: &hir::Block,
        declared_return_ty: TypeId,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        // block 是表达式：若末尾是表达式语句，则它的值作为 block value。
        let mut tail_value: Option<CgValue<'ctx>> = None;
        let declared_return_cg = self.cg_ty_of(declared_return_ty).unwrap_or(CgTy::Unit);
        // main 的隐式返回只关心 `Int/Bool`（用于 exit code）；其它返回类型一律忽略，且不应把
        // “期望返回类型”强行向下传播到最后一个表达式（避免触发不必要的 coercion 失败）。
        let expected_tail_cg = match declared_return_cg {
            CgTy::Int(_) | CgTy::Bool => declared_return_cg,
            _ => CgTy::Unit,
        };

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
                    let expected = if is_last {
                        Some(expected_tail_cg)
                    } else {
                        Some(CgTy::Unit)
                    };
                    let v = self.codegen_expr_in_expected_context(expr, expected)?;
                    if is_last {
                        tail_value = Some(v);
                    } else {
                        tail_value = None;
                    }
                }
                hir::StmtKind::Return { value } => {
                    let exit = match value {
                        Some(expr) => {
                            let v = self
                                .codegen_expr_in_expected_context(expr, Some(declared_return_cg))?;
                            self.coerce_exit_code(expr.span, v)?
                        }
                        None => self.context.i32_type().const_int(0, false),
                    };

                    self.env.pop_scope();
                    return Ok(exit);
                }
                // T0141: While loops in main body.
                hir::StmtKind::While { cond, body } => {
                    self.codegen_while_stmt(stmt.span, cond, body)?;
                    tail_value = None;
                }
                // T0141: break/continue in main body (inside a loop).
                hir::StmtKind::Break { break_span } => {
                    let loop_ctx = self.loop_context_stack.last().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "break outside loop",
                            at: (*break_span).into(),
                        },
                    )?;
                    self.builder.build_unconditional_branch(loop_ctx.break_bb)?;
                    self.env.pop_scope();
                    return Ok(self.context.i32_type().const_int(0, false));
                }
                hir::StmtKind::Continue { continue_span } => {
                    let loop_ctx = self.loop_context_stack.last().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "continue outside loop",
                            at: (*continue_span).into(),
                        },
                    )?;
                    self.builder
                        .build_unconditional_branch(loop_ctx.continue_bb)?;
                    self.env.pop_scope();
                    return Ok(self.context.i32_type().const_int(0, false));
                }
                hir::StmtKind::Todo(_) => {
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

    pub(super) fn codegen_if_expr(
        &mut self,
        span: crate::span::Span,
        out_ty: TypeId,
        cond: &hir::Expr,
        then_branch: &hir::Expr,
        else_branch: Option<&hir::Expr>,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // T0142: if-without-else is always Unit — the missing else path produces no value,
        // so we force the output type to Unit regardless of the caller's expected type.
        let out_cg = if else_branch.is_none() {
            CgTy::Unit
        } else {
            expected.or_else(|| self.cg_ty_of(out_ty)).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "if output type",
                    at: span.into(),
                },
            )?
        };

        let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "if condition value",
            at: cond.span.into(),
        })?;

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

        let then_bb = self.context.append_basic_block(func, "if_then");
        let else_bb = self.context.append_basic_block(func, "if_else");
        let merge_bb = self.context.append_basic_block(func, "if_merge");

        self.builder
            .build_conditional_branch(cond_i1, then_bb, else_bb)?;

        let result_ptr = match out_cg {
            CgTy::Unit | CgTy::Never => None,
            _ => Some(self.create_entry_alloca(span, "if_result", out_cg)?),
        };

        // --- then ---
        self.builder.position_at_end(then_bb);
        let then_v = self.codegen_expr_in_expected_context(then_branch, Some(out_cg))?;
        // T0141: Check if the then-block already has a terminator (e.g., from break/continue/return).
        // If so, skip coercion, store, and merge branch — they would emit into a terminated block.
        let then_terminated = self
            .builder
            .get_insert_block()
            .is_none_or(|bb| bb.get_terminator().is_some());
        if !then_terminated {
            let then_v = if out_cg == CgTy::Unit {
                CgValue::unit()
            } else {
                self.coerce_value(then_branch.span, then_v, out_cg)?
            };
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(then_branch.span, ptr, out_cg, then_v)?;
            }
            self.builder.build_unconditional_branch(merge_bb)?;
        }

        // --- else ---
        self.builder.position_at_end(else_bb);
        let else_v = match else_branch {
            Some(expr) => self.codegen_expr_in_expected_context(expr, Some(out_cg))?,
            // T0142: else_branch is None → out_cg is guaranteed Unit (see above).
            None => CgValue::unit(),
        };
        // T0141: Same check for else branch.
        let else_terminated = self
            .builder
            .get_insert_block()
            .is_none_or(|bb| bb.get_terminator().is_some());
        if !else_terminated {
            let else_v = if out_cg == CgTy::Unit {
                CgValue::unit()
            } else {
                self.coerce_value(span, else_v, out_cg)?
            };
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(span, ptr, out_cg, else_v)?;
            }
            self.builder.build_unconditional_branch(merge_bb)?;
        }

        // --- merge ---
        self.builder.position_at_end(merge_bb);
        match out_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            // T1612: all branches diverge → merge is unreachable.
            CgTy::Never => {
                self.builder.build_unreachable()?;
                Ok(CgValue::never())
            }
            CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Enum(_) => {
                let Some(ptr) = result_ptr else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "if result slot",
                        at: span.into(),
                    });
                };
                let llvm_ty = self.llvm_basic_type_of(span, out_cg)?;
                let loaded = self.builder.build_load(llvm_ty, ptr, "if_result")?;
                self.cg_value_from_loaded(span, out_cg, loaded)
            }
        }
    }

    pub(super) fn codegen_when_expr(
        &mut self,
        span: crate::span::Span,
        subject: &hir::Expr,
        arms: &[hir::WhenArm],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if arms.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when (no arms)",
                at: span.into(),
            });
        }

        let subject_v = self.codegen_expr(subject)?;
        let subject_ty = subject_v.ty;
        let subject_raw = subject_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "when subject value",
            at: subject.span.into(),
        })?;

        // 将 subject 落到一个栈 slot：便于在各 arm 中做 payload 解构（避免跨 block 的 dominance 细节）。
        let subject_ptr = self.create_entry_alloca(span, "when_subject", subject_ty)?;
        let _ = self.store_local_value(span, subject_ptr, subject_ty, subject_v)?;

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
            .map(|i| {
                self.context
                    .append_basic_block(func, &format!("when_arm_{i}"))
            })
            .collect::<Vec<_>>();

        let expected_out_ty = expected;

        let needs_chain = arms
            .iter()
            .any(|arm| arm.guard.is_some() || self.when_pat_contains_or(&arm.pat));

        if needs_chain {
            // guard / or-pattern：用“链式判别 + guard 失败回落到下一个分支”的 CFG。
            //
            // 说明：这条路径不追求最优 CFG（TODO T0825：目标是语义正确）。
            let check_bbs = (0..arms.len())
                .map(|i| {
                    self.context
                        .append_basic_block(func, &format!("when_check_{i}"))
                })
                .collect::<Vec<_>>();
            let bind_bbs = (0..arms.len())
                .map(|i| {
                    self.context
                        .append_basic_block(func, &format!("when_bind_{i}"))
                })
                .collect::<Vec<_>>();
            let no_match_bb = self.context.append_basic_block(func, "when_no_match");

            self.builder.build_unconditional_branch(check_bbs[0])?;

            for (idx, arm) in arms.iter().enumerate() {
                self.builder.position_at_end(check_bbs[idx]);
                let cond = self.codegen_when_pat_cond(span, subject_ty, &arm.pat, subject_ptr)?;
                let else_bb = if idx + 1 < arms.len() {
                    check_bbs[idx + 1]
                } else {
                    no_match_bb
                };
                self.builder
                    .build_conditional_branch(cond, bind_bbs[idx], else_bb)?;
            }

            self.builder.position_at_end(no_match_bb);
            self.builder.build_unreachable()?;

            // 生成各 arm body，并把结果汇合到 merge。
            let mut out_ty: Option<CgTy> = expected_out_ty;
            let mut incoming: Vec<(inkwell::basic_block::BasicBlock<'ctx>, CgValue<'ctx>)> =
                Vec::new();

            for (idx, arm) in arms.iter().enumerate() {
                let else_bb = if idx + 1 < arms.len() {
                    check_bbs[idx + 1]
                } else {
                    no_match_bb
                };

                // 先在 bind block 中完成 pattern binder + guard 判定，再决定是否进入 arm body。
                self.builder.position_at_end(bind_bbs[idx]);

                self.env.push_scope();
                self.bind_when_pat(span, subject_ty, &arm.pat, subject_ptr)?;

                if let Some(guard) = &arm.guard {
                    let gv = self.codegen_expr_in_expected_context(guard, Some(CgTy::Bool))?;
                    let gb = gv.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "when guard value",
                        at: guard.span.into(),
                    })?;
                    self.builder
                        .build_conditional_branch(gb, arm_bbs[idx], else_bb)?;
                } else {
                    self.builder.build_unconditional_branch(arm_bbs[idx])?;
                }

                // arm body：在同一作用域内生成（binder 可用）。
                self.builder.position_at_end(arm_bbs[idx]);

                let mut v = match expected_out_ty {
                    Some(target) => {
                        let v = self.codegen_expr_in_expected_context(&arm.body, Some(target))?;
                        if target == CgTy::Unit {
                            CgValue::unit()
                        } else if v.ty != target {
                            self.coerce_value(arm.body.span, v, target)?
                        } else {
                            v
                        }
                    }
                    None => self.codegen_expr(&arm.body)?,
                };

                let arm_terminated = self
                    .builder
                    .get_insert_block()
                    .is_none_or(|bb| bb.get_terminator().is_some());
                if arm_terminated {
                    self.env.pop_scope();
                    continue;
                }

                if expected_out_ty.is_none() {
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
                } else {
                    // 已在 expected-context 下生成并按需 coercion：确保 `v.ty == expected_out_ty`。
                    if let Some(target) = expected_out_ty {
                        v.ty = target;
                    }
                }

                let tail_bb =
                    self.builder
                        .get_insert_block()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "when arm tail block",
                            at: arm.body.span.into(),
                        })?;
                self.builder.build_unconditional_branch(merge_bb)?;
                self.env.pop_scope();

                incoming.push((tail_bb, v));
            }

            self.builder.position_at_end(merge_bb);
            if incoming.is_empty() {
                self.builder.build_unreachable()?;
                return Ok(CgValue::never());
            }

            let out_ty = out_ty.unwrap_or(CgTy::Unit);
            return match out_ty {
                CgTy::Unit => Ok(CgValue::unit()),
                // T1612: all arms diverge → merge is unreachable.
                CgTy::Never => {
                    self.builder.build_unreachable()?;
                    Ok(CgValue::never())
                }
                CgTy::Bool
                | CgTy::Float64
                | CgTy::Float32
                | CgTy::Int(_)
                | CgTy::String
                | CgTy::Ref
                | CgTy::Tuple(_)
                | CgTy::Struct(_)
                | CgTy::Enum(_) => {
                    // T1610: use alloca/store/load to support compound types (and scalars).
                    let result_ptr = self.create_entry_alloca(span, "when_chain_result", out_ty)?;
                    for (bb, v) in incoming {
                        self.builder.position_at_end(bb);
                        // Remove the unconditional branch we already emitted, re-emit after store.
                        if let Some(term) = bb.get_terminator() {
                            term.erase_from_basic_block();
                        }
                        let _ = self.store_local_value(span, result_ptr, out_ty, v)?;
                        self.builder.build_unconditional_branch(merge_bb)?;
                    }
                    self.builder.position_at_end(merge_bb);
                    let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                    let loaded =
                        self.builder
                            .build_load(llvm_ty, result_ptr, "when_chain_result")?;
                    self.cg_value_from_loaded(span, out_ty, loaded)
                }
            };
        }

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

                // 注意：避免持有 `cg_enum_layout(...)` 的借用跨越后续 builder 调用。
                let (repr, variants) = {
                    let cg_layout = self.cg_enum_layout(span, enum_ty)?;
                    (cg_layout.repr, cg_layout.variants.clone())
                };

                let tag = match repr {
                    CgEnumRepr::TaggedUnion => {
                        let subject_struct = subject_raw.into_struct_value();
                        self.builder
                            .build_extract_value(subject_struct, 0, "when_tag")?
                            .into_int_value()
                    }
                    CgEnumRepr::Niche {
                        storage,
                        none_value,
                    } => {
                        let is_none = match storage {
                            NicheStorage::Pointer => {
                                let ptr = subject_raw.into_pointer_value();
                                if none_value != 0 {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "Option niche pointer none_value (must be NULL)",
                                        at: span.into(),
                                    });
                                }
                                self.builder.build_is_null(ptr, "option_is_none")?
                            }
                            NicheStorage::U8 => {
                                let v = subject_raw.into_int_value();
                                let expected = self.context.i8_type().const_int(none_value, false);
                                self.builder.build_int_compare(
                                    IntPredicate::EQ,
                                    v,
                                    expected,
                                    "option_is_none",
                                )?
                            }
                        };

                        let some_tag = self.context.i32_type().const_int(0, false);
                        let none_tag = self.context.i32_type().const_int(1, false);
                        self.builder
                            .build_select(is_none, none_tag, some_tag, "option_tag")?
                            .into_int_value()
                    }
                    CgEnumRepr::ValueOnly { .. } => subject_raw.into_int_value(),
                };

                let tag_ty = tag.get_type();
                let default_bb = self.context.append_basic_block(func, "when_no_match");

                let mut cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
                    Vec::with_capacity(variants.len());
                for variant in &variants {
                    let Some(target_idx) =
                        self.when_first_matching_arm_for_enum_variant(arms, &variant.name)
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "when missing enum arm",
                            at: span.into(),
                        });
                    };
                    cases.push((tag_ty.const_int(variant.tag, false), arm_bbs[target_idx]));
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
            CgTy::Int(int_ty) => {
                for arm in arms {
                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. }
                        | hir::WhenPat::IntLit { .. }
                        | hir::WhenPat::CharLit { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when pattern (int)",
                                at: arm.pat.span().into(),
                            });
                        }
                    }
                }

                let check_bbs = (0..arms.len())
                    .map(|i| {
                        self.context
                            .append_basic_block(func, &format!("when_check_{i}"))
                    })
                    .collect::<Vec<_>>();
                let no_match_bb = self.context.append_basic_block(func, "when_no_match");
                let raw = subject_raw.into_int_value();

                self.builder.build_unconditional_branch(check_bbs[0])?;

                for (idx, arm) in arms.iter().enumerate() {
                    self.builder.position_at_end(check_bbs[idx]);

                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. } => {
                            self.builder.build_unconditional_branch(arm_bbs[idx])?;
                        }
                        hir::WhenPat::IntLit { .. } | hir::WhenPat::CharLit { .. } => {
                            let cond = self.codegen_when_pat_cond_for_int_with_value(
                                span, int_ty, raw, &arm.pat,
                            )?;
                            let else_bb = if idx + 1 < arms.len() {
                                check_bbs[idx + 1]
                            } else {
                                no_match_bb
                            };
                            self.builder
                                .build_conditional_branch(cond, arm_bbs[idx], else_bb)?;
                        }
                        _ => unreachable!("int patterns validated above"),
                    }
                }

                self.builder.position_at_end(no_match_bb);
                self.builder.build_unreachable()?;
            }
            CgTy::String => {
                for arm in arms {
                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. }
                        | hir::WhenPat::StringLit { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when pattern (string)",
                                at: arm.pat.span().into(),
                            });
                        }
                    }
                }

                let check_bbs = (0..arms.len())
                    .map(|i| {
                        self.context
                            .append_basic_block(func, &format!("when_check_{i}"))
                    })
                    .collect::<Vec<_>>();
                let no_match_bb = self.context.append_basic_block(func, "when_no_match");
                let raw = subject_raw.into_pointer_value();

                self.builder.build_unconditional_branch(check_bbs[0])?;

                for (idx, arm) in arms.iter().enumerate() {
                    self.builder.position_at_end(check_bbs[idx]);

                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. } => {
                            self.builder.build_unconditional_branch(arm_bbs[idx])?;
                        }
                        hir::WhenPat::StringLit { .. } => {
                            let cond = self
                                .codegen_when_pat_cond_for_string_with_value(span, raw, &arm.pat)?;
                            let else_bb = if idx + 1 < arms.len() {
                                check_bbs[idx + 1]
                            } else {
                                no_match_bb
                            };
                            self.builder
                                .build_conditional_branch(cond, arm_bbs[idx], else_bb)?;
                        }
                        _ => unreachable!("string patterns validated above"),
                    }
                }

                self.builder.position_at_end(no_match_bb);
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
                    .map(|i| {
                        self.context
                            .append_basic_block(func, &format!("when_check_{i}"))
                    })
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
        let mut out_ty: Option<CgTy> = expected_out_ty;
        let mut incoming: Vec<(inkwell::basic_block::BasicBlock<'ctx>, CgValue<'ctx>)> = Vec::new();

        for (idx, arm) in arms.iter().enumerate() {
            self.builder.position_at_end(arm_bbs[idx]);

            self.env.push_scope();
            self.bind_when_pat(span, subject_ty, &arm.pat, subject_ptr)?;

            let mut v = match expected_out_ty {
                Some(target) => {
                    let v = self.codegen_expr_in_expected_context(&arm.body, Some(target))?;
                    if target == CgTy::Unit {
                        CgValue::unit()
                    } else if v.ty != target {
                        self.coerce_value(arm.body.span, v, target)?
                    } else {
                        v
                    }
                }
                None => self.codegen_expr(&arm.body)?,
            };

            let arm_terminated = self
                .builder
                .get_insert_block()
                .is_none_or(|bb| bb.get_terminator().is_some());
            if arm_terminated {
                self.env.pop_scope();
                continue;
            }

            if expected_out_ty.is_none() {
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
            } else {
                if let Some(target) = expected_out_ty {
                    v.ty = target;
                }
            }

            let tail_bb =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "when arm tail block",
                        at: arm.body.span.into(),
                    })?;
            self.builder.build_unconditional_branch(merge_bb)?;
            self.env.pop_scope();

            incoming.push((tail_bb, v));
        }

        self.builder.position_at_end(merge_bb);
        if incoming.is_empty() {
            self.builder.build_unreachable()?;
            return Ok(CgValue::never());
        }

        let out_ty = out_ty.unwrap_or(CgTy::Unit);
        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            // T1612: all arms diverge → merge is unreachable.
            CgTy::Never => {
                self.builder.build_unreachable()?;
                Ok(CgValue::never())
            }
            CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Enum(_) => {
                // T1610: use alloca/store/load to support compound types (and scalars).
                let result_ptr = self.create_entry_alloca(span, "when_result", out_ty)?;
                for (bb, v) in incoming {
                    self.builder.position_at_end(bb);
                    if let Some(term) = bb.get_terminator() {
                        term.erase_from_basic_block();
                    }
                    let _ = self.store_local_value(span, result_ptr, out_ty, v)?;
                    self.builder.build_unconditional_branch(merge_bb)?;
                }
                self.builder.position_at_end(merge_bb);
                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, result_ptr, "when_result")?;
                self.cg_value_from_loaded(span, out_ty, loaded)
            }
        }
    }

    pub(super) fn bind_when_pat(
        &mut self,
        at: crate::span::Span,
        subject_ty: CgTy,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Or { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Rest { .. }
            | hir::WhenPat::Is { .. }
            | hir::WhenPat::IntLit { .. }
            | hir::WhenPat::CharLit { .. }
            | hir::WhenPat::StringLit { .. }
            | hir::WhenPat::BoolLit { .. } => Ok(()),
            hir::WhenPat::Bind { id, name, .. } => {
                // `x -> ...`：绑定整个 subject。
                let ptr = self.create_entry_alloca(at, name, subject_ty)?;
                let llvm_ty = self.llvm_basic_type_of(at, subject_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, subject_ptr, "bind_subject")?;
                let v = CgValue {
                    ty: subject_ty,
                    value: Some(loaded),
                };
                let _ = self.store_local_value(at, ptr, subject_ty, v)?;
                let hir_ty = self.when_pat_binding_hir_ty(pat.span())?;
                self.env.insert(
                    *id,
                    CgLocal {
                        hir_ty,
                        call_may_suspend: self.local_call_may_suspend_from_hir_ty(hir_ty),
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

                let (repr, variant) = {
                    let cg_layout = self.cg_enum_layout(at, enum_ty)?;
                    let repr = cg_layout.repr;
                    let variant = cg_layout
                        .variants
                        .iter()
                        .find(|v| v.name == *name)
                        .cloned()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "when unknown enum variant",
                            at: pat.span().into(),
                        })?;
                    (repr, variant)
                };

                // 解析 `..`：parser/typecheck 已保证它最多出现一次且必须出现在最后一个位置。
                let (prefix_pats, has_rest) = match args.last() {
                    Some(hir::WhenPat::Rest { .. }) => {
                        (&args[..args.len().saturating_sub(1)], true)
                    }
                    _ => (args.as_slice(), false),
                };

                let expected_arity = variant.fields.len();
                let found_arity = prefix_pats.len();
                if (!has_rest && expected_arity != found_arity)
                    || (has_rest && found_arity > expected_arity)
                {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when variant arity mismatch",
                        at: pat.span().into(),
                    });
                }

                if prefix_pats.is_empty() {
                    return Ok(());
                }

                // boxed variant：payload 是指向“payload struct”的指针（存放所有字段）。
                if variant.boxed {
                    let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?.into_struct_type();
                    let loaded =
                        self.builder
                            .build_load(llvm_enum_ty, subject_ptr, "load_when_subject")?;
                    let raw_struct = loaded.into_struct_value();

                    let payload_struct_ty =
                        self.llvm_enum_boxed_payload_struct_type(at, enum_ty, &variant)?;
                    let payload_ptr = self
                        .builder
                        .build_extract_value(raw_struct, 2, "when_payload_ptr")?
                        .into_pointer_value();

                    let payload_obj_ty =
                        self.llvm_enum_boxed_payload_object_type(at, enum_ty, &variant)?;
                    let payload_obj_ptr = self.builder.build_pointer_cast(
                        payload_ptr,
                        self.llvm_ptr_type(self.gc_address_space()),
                        "when_payload_obj_ptr",
                    )?;
                    let payload_gep = self.builder.build_struct_gep(
                        payload_obj_ty,
                        payload_obj_ptr,
                        1,
                        "when_payload_gep",
                    )?;
                    let payload_loaded = self
                        .builder
                        .build_load(payload_struct_ty, payload_gep, "load_when_payload")?
                        .into_struct_value();

                    for (idx, arg_pat) in prefix_pats.iter().enumerate() {
                        let field_cg =
                            *variant
                                .fields
                                .get(idx)
                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                    kind: "when boxed payload field index",
                                    at: arg_pat.span().into(),
                                })?;

                        match arg_pat {
                            hir::WhenPat::Bind { id, name, .. } => {
                                let raw = self.builder.build_extract_value(
                                    payload_loaded,
                                    idx as u32,
                                    "when_payload_field",
                                )?;
                                let extracted =
                                    self.cg_value_from_loaded(arg_pat.span(), field_cg, raw)?;

                                let ptr = self.create_entry_alloca(at, name, field_cg)?;
                                let _ = self.store_local_value(at, ptr, field_cg, extracted)?;
                                let hir_ty = self.when_pat_binding_hir_ty(arg_pat.span())?;
                                self.env.insert(
                                    *id,
                                    CgLocal {
                                        hir_ty,
                                        call_may_suspend: self
                                            .local_call_may_suspend_from_hir_ty(hir_ty),
                                        ty: field_cg,
                                        ptr,
                                        mutable: false,
                                    },
                                );
                            }
                            hir::WhenPat::Wildcard { .. } => {}
                            hir::WhenPat::Rest { .. } => break,
                            _ => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "when variant arg pattern",
                                    at: arg_pat.span().into(),
                                });
                            }
                        }
                    }

                    return Ok(());
                }

                // niche enum（当前仅 Option<T>）：payload 就是 enum 本身。
                if matches!(repr, CgEnumRepr::Niche { .. }) {
                    if variant.fields.len() != 1 {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "niche enum variant arity",
                            at: pat.span().into(),
                        });
                    }

                    let field_cg = variant.fields[0];
                    let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?;
                    let loaded =
                        self.builder
                            .build_load(llvm_enum_ty, subject_ptr, "load_when_subject")?;

                    // 存储类型可能与字段类型不同（例如 `Option<Bool>` 存储为 u8）。
                    let extracted = match field_cg {
                        CgTy::Bool => {
                            let b = self.builder.build_int_truncate(
                                loaded.into_int_value(),
                                self.context.bool_type(),
                                "option_bool_from_u8",
                            )?;
                            CgValue::bool(b)
                        }
                        CgTy::String => CgValue {
                            ty: CgTy::String,
                            value: Some(loaded.into_pointer_value().into()),
                        },
                        CgTy::Ref => CgValue {
                            ty: CgTy::Ref,
                            value: Some(loaded.into_pointer_value().into()),
                        },
                        CgTy::Never
                        | CgTy::Unit
                        | CgTy::Float64
                        | CgTy::Float32
                        | CgTy::Int(_)
                        | CgTy::Tuple(_)
                        | CgTy::Struct(_)
                        | CgTy::Enum(_) => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "niche enum payload type",
                                at: pat.span().into(),
                            });
                        }
                    };

                    // niche enum 的 binder 只能绑定第一个字段（且 rest 可能忽略其余）。
                    let Some(first_pat) = prefix_pats.first() else {
                        return Ok(());
                    };
                    match first_pat {
                        hir::WhenPat::Bind { id, name, .. } => {
                            let ptr = self.create_entry_alloca(at, name, field_cg)?;
                            let _ = self.store_local_value(at, ptr, field_cg, extracted)?;
                            let hir_ty = self.when_pat_binding_hir_ty(first_pat.span())?;
                            self.env.insert(
                                *id,
                                CgLocal {
                                    hir_ty,
                                    call_may_suspend: self
                                        .local_call_may_suspend_from_hir_ty(hir_ty),
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
                                at: first_pat.span().into(),
                            });
                        }
                    }

                    return Ok(());
                }

                // inline tagged union：仍只支持 “小 payload”（单字段标量）。
                if variant.fields.len() != 1 || prefix_pats.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when variant payload (inline, unsupported arity)",
                        at: pat.span().into(),
                    });
                }

                let field_cg = variant.fields[0];
                let arg_pat = &prefix_pats[0];

                let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?.into_struct_type();
                let loaded =
                    self.builder
                        .build_load(llvm_enum_ty, subject_ptr, "load_when_subject")?;
                let raw_struct = loaded.into_struct_value();
                let payload_word = self
                    .builder
                    .build_extract_value(raw_struct, 1, "when_payload_word")?
                    .into_int_value();
                let payload_ptr = self
                    .builder
                    .build_extract_value(raw_struct, 2, "when_payload_ptr")?
                    .into_pointer_value();

                // 当前阶段 tagged union payload 分成两路：
                // - word：承载 Bool/Int 等标量
                // - gc ptr：承载 Ref/String 等 GC-managed 指针
                let extracted = match field_cg {
                    CgTy::Never => CgValue::never(),
                    CgTy::Unit => CgValue::unit(),
                    CgTy::Bool => {
                        let b = self.builder.build_int_truncate(
                            payload_word,
                            self.context.bool_type(),
                            "payload_to_bool",
                        )?;
                        CgValue::bool(b)
                    }
                    CgTy::Int(int_ty) => {
                        let from = self.enum_payload_ty();
                        let casted = self.cast_int(payload_word, from, int_ty)?;
                        CgValue::int(casted, int_ty)
                    }
                    CgTy::Float64 => {
                        let raw = self
                            .builder
                            .build_bit_cast(
                                payload_word,
                                self.context.f64_type(),
                                "payload_to_f64",
                            )?
                            .into_float_value();
                        CgValue::float(raw, CgTy::Float64)
                    }
                    CgTy::Float32 => {
                        let bits32 = self.builder.build_int_truncate(
                            payload_word,
                            self.context.i32_type(),
                            "payload_to_f32_bits",
                        )?;
                        let raw = self
                            .builder
                            .build_bit_cast(bits32, self.context.f32_type(), "payload_to_f32")?
                            .into_float_value();
                        CgValue::float(raw, CgTy::Float32)
                    }
                    CgTy::String => {
                        let ptr = self.builder.build_pointer_cast(
                            payload_ptr,
                            self.llvm_scoop_string_ptr_type(),
                            "payload_to_str",
                        )?;
                        CgValue {
                            ty: CgTy::String,
                            value: Some(ptr.into()),
                        }
                    }
                    CgTy::Ref => CgValue {
                        ty: CgTy::Ref,
                        value: Some(payload_ptr.into()),
                    },
                    CgTy::Enum(nested_enum_ty) => {
                        let repr = self.cg_enum_layout(at, nested_enum_ty)?.repr;
                        match repr {
                            CgEnumRepr::Niche {
                                storage,
                                none_value,
                            } => match storage {
                                NicheStorage::Pointer => {
                                    if none_value != 0 {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "when payload nested niche pointer none_value (must be NULL)",
                                            at: arg_pat.span().into(),
                                        });
                                    }

                                    let llvm_nested =
                                        self.llvm_enum_value_type(at, nested_enum_ty)?;
                                    let BasicTypeEnum::PointerType(ptr_ty) = llvm_nested else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "when payload nested niche storage (non-pointer)",
                                            at: arg_pat.span().into(),
                                        });
                                    };

                                    let casted = self.builder.build_pointer_cast(
                                        payload_ptr,
                                        ptr_ty,
                                        "when_payload_nested_niche_ptr",
                                    )?;
                                    CgValue {
                                        ty: CgTy::Enum(nested_enum_ty),
                                        value: Some(casted.into()),
                                    }
                                }
                                NicheStorage::U8 => {
                                    let llvm_nested =
                                        self.llvm_enum_value_type(at, nested_enum_ty)?;
                                    let BasicTypeEnum::IntType(int_ty) = llvm_nested else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "when payload nested niche storage (non-int)",
                                            at: arg_pat.span().into(),
                                        });
                                    };

                                    let v = self.builder.build_int_truncate(
                                        payload_word,
                                        int_ty,
                                        "when_payload_nested_niche_u8",
                                    )?;
                                    CgValue {
                                        ty: CgTy::Enum(nested_enum_ty),
                                        value: Some(v.into()),
                                    }
                                }
                            },
                            _ => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "when payload (nested enum, unsupported repr)",
                                    at: arg_pat.span().into(),
                                });
                            }
                        }
                    }
                    CgTy::Tuple(_) | CgTy::Struct(_) => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "when payload (non-scalar)",
                            at: arg_pat.span().into(),
                        });
                    }
                };

                match arg_pat {
                    hir::WhenPat::Bind { id, name, .. } => {
                        let ptr = self.create_entry_alloca(at, name, field_cg)?;
                        let _ = self.store_local_value(at, ptr, field_cg, extracted)?;
                        let hir_ty = self.when_pat_binding_hir_ty(arg_pat.span())?;
                        self.env.insert(
                            *id,
                            CgLocal {
                                hir_ty,
                                call_may_suspend: self.local_call_may_suspend_from_hir_ty(hir_ty),
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

                if (!has_rest && pat_arity != tuple_elems.len())
                    || (has_rest && pat_arity > tuple_elems.len())
                {
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
                        let raw = self.builder.build_extract_value(
                            tuple_v,
                            idx as u32,
                            "when_tuple_elem",
                        )?;
                        self.cg_value_from_loaded(elem_pat.span(), elem_ty, raw)?
                    };

                    match elem_pat {
                        hir::WhenPat::Bind { .. } => {
                            // 直接把元素作为 subject 绑定（避免额外临时 slot）。
                            let hir::WhenPat::Bind { id, name, .. } = elem_pat else {
                                unreachable!()
                            };
                            let ptr = self.create_entry_alloca(at, name, elem_ty)?;
                            let _ = self.store_local_value(at, ptr, elem_ty, extracted_v)?;
                            let hir_ty = self.when_pat_binding_hir_ty(elem_pat.span())?;
                            self.env.insert(
                                *id,
                                CgLocal {
                                    hir_ty,
                                    call_may_suspend: self
                                        .local_call_may_suspend_from_hir_ty(hir_ty),
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
                            let _ = self.store_local_value(at, tmp_ptr, elem_ty, extracted_v)?;
                            self.bind_when_pat(at, elem_ty, elem_pat, tmp_ptr)?;
                        }
                        _ => {}
                    }
                }

                Ok(())
            }
        }
    }

    pub(super) fn when_pat_contains_or(&self, pat: &hir::WhenPat) -> bool {
        match pat {
            hir::WhenPat::Or { .. } => true,
            hir::WhenPat::Tuple { elements, .. } => {
                elements.iter().any(|p| self.when_pat_contains_or(p))
            }
            hir::WhenPat::Variant { args, .. } => args.iter().any(|p| self.when_pat_contains_or(p)),
            _ => false,
        }
    }

    pub(super) fn codegen_when_pat_cond(
        &mut self,
        at: crate::span::Span,
        subject_ty: CgTy,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match subject_ty {
            CgTy::Enum(enum_ty) => {
                self.codegen_when_pat_cond_for_enum(at, enum_ty, pat, subject_ptr)
            }
            CgTy::Bool => self.codegen_when_pat_cond_for_bool(at, pat, subject_ptr),
            CgTy::Int(int_ty) => self.codegen_when_pat_cond_for_int(at, int_ty, pat, subject_ptr),
            CgTy::String => self.codegen_when_pat_cond_for_string(at, pat, subject_ptr),
            CgTy::Tuple(tuple_ty) => {
                self.codegen_when_pat_cond_for_tuple(at, tuple_ty, pat, subject_ptr)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when subject type",
                at: at.into(),
            }),
        }
    }

    pub(super) fn codegen_when_pat_cond_for_enum(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        // 注意：避免持有 `cg_enum_layout(...)` 的借用跨越后续 builder 调用。
        let (repr, variants) = {
            let cg_layout = self.cg_enum_layout(at, enum_ty)?;
            (cg_layout.repr, cg_layout.variants.clone())
        };
        let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?;
        let loaded = self
            .builder
            .build_load(llvm_enum_ty, subject_ptr, "load_when_subject")?;

        let tag = match repr {
            CgEnumRepr::TaggedUnion => {
                let raw_struct = loaded.into_struct_value();
                self.builder
                    .build_extract_value(raw_struct, 0, "when_tag")?
                    .into_int_value()
            }
            CgEnumRepr::Niche {
                storage,
                none_value,
            } => {
                let is_none = match storage {
                    NicheStorage::Pointer => {
                        let ptr = loaded.into_pointer_value();
                        if none_value != 0 {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "Option niche pointer none_value (must be NULL)",
                                at: at.into(),
                            });
                        }
                        self.builder.build_is_null(ptr, "option_is_none")?
                    }
                    NicheStorage::U8 => {
                        let v = loaded.into_int_value();
                        let expected = self.context.i8_type().const_int(none_value, false);
                        self.builder.build_int_compare(
                            IntPredicate::EQ,
                            v,
                            expected,
                            "option_is_none",
                        )?
                    }
                };

                let some_tag = self.context.i32_type().const_int(0, false);
                let none_tag = self.context.i32_type().const_int(1, false);
                self.builder
                    .build_select(is_none, none_tag, some_tag, "option_tag")?
                    .into_int_value()
            }
            CgEnumRepr::ValueOnly { .. } => loaded.into_int_value(),
        };

        self.codegen_when_pat_cond_for_enum_with_tag(at, &variants, tag, pat)
    }

    pub(super) fn codegen_when_pat_cond_for_enum_with_tag(
        &self,
        _at: crate::span::Span,
        variants: &[CgEnumVariant],
        tag: IntValue<'ctx>,
        pat: &hir::WhenPat,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Bind { .. } => Ok(self.context.bool_type().const_int(1, false)),
            hir::WhenPat::Variant { name, args, .. } => {
                let Some(variant) = variants.iter().find(|v| v.name == *name) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when unknown enum variant",
                        at: pat.span().into(),
                    });
                };
                let _ = args;

                let expected = tag.get_type().const_int(variant.tag, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    tag,
                    expected,
                    "when_enum_tag_eq",
                )?)
            }
            hir::WhenPat::Or { pats, .. } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for p in pats {
                    let c = self.codegen_when_pat_cond_for_enum_with_tag(_at, variants, tag, p)?;
                    cond = self.builder.build_or(cond, c, "when_or")?;
                }
                Ok(cond)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when pattern (enum)",
                at: pat.span().into(),
            }),
        }
    }

    pub(super) fn codegen_when_pat_cond_for_bool(
        &mut self,
        at: crate::span::Span,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let loaded = self
            .builder
            .build_load(self.context.bool_type(), subject_ptr, "load_when_bool")?
            .into_int_value();
        self.codegen_when_pat_cond_for_bool_with_value(at, loaded, pat)
    }

    pub(super) fn codegen_when_pat_cond_for_bool_with_value(
        &self,
        _at: crate::span::Span,
        value: IntValue<'ctx>,
        pat: &hir::WhenPat,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Bind { .. } => Ok(self.context.bool_type().const_int(1, false)),
            hir::WhenPat::BoolLit {
                value: expected, ..
            } => {
                let expected = self.context.bool_type().const_int(*expected as u64, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    expected,
                    "when_bool_eq",
                )?)
            }
            hir::WhenPat::Or { pats, .. } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for p in pats {
                    let c = self.codegen_when_pat_cond_for_bool_with_value(_at, value, p)?;
                    cond = self.builder.build_or(cond, c, "when_or")?;
                }
                Ok(cond)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when pattern (bool)",
                at: pat.span().into(),
            }),
        }
    }

    pub(super) fn codegen_when_pat_cond_for_int(
        &mut self,
        at: crate::span::Span,
        int_ty: IntTy,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let loaded = self
            .builder
            .build_load(self.int_type(int_ty), subject_ptr, "load_when_int")?
            .into_int_value();
        self.codegen_when_pat_cond_for_int_with_value(at, int_ty, loaded, pat)
    }

    pub(super) fn codegen_when_pat_cond_for_int_with_value(
        &self,
        _at: crate::span::Span,
        int_ty: IntTy,
        value: IntValue<'ctx>,
        pat: &hir::WhenPat,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Bind { .. } => Ok(self.context.bool_type().const_int(1, false)),
            hir::WhenPat::IntLit { .. } => {
                let expected_raw = self.int_literal_bits_for_ty(pat.span(), int_ty)?;
                let expected = self.int_type(int_ty).const_int(expected_raw, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    expected,
                    "when_int_eq",
                )?)
            }
            hir::WhenPat::CharLit {
                value: expected, ..
            } => {
                let expected = self.int_type(int_ty).const_int(*expected as u64, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    expected,
                    "when_char_eq",
                )?)
            }
            hir::WhenPat::Or { pats, .. } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for p in pats {
                    let c = self.codegen_when_pat_cond_for_int_with_value(_at, int_ty, value, p)?;
                    cond = self.builder.build_or(cond, c, "when_or")?;
                }
                Ok(cond)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when pattern (int)",
                at: pat.span().into(),
            }),
        }
    }

    pub(super) fn codegen_when_pat_cond_for_string(
        &mut self,
        at: crate::span::Span,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let loaded = self
            .builder
            .build_load(
                self.llvm_scoop_string_ptr_type(),
                subject_ptr,
                "load_when_string",
            )?
            .into_pointer_value();
        self.codegen_when_pat_cond_for_string_with_value(at, loaded, pat)
    }

    pub(super) fn codegen_when_pat_cond_for_string_with_value(
        &mut self,
        at: crate::span::Span,
        value: PointerValue<'ctx>,
        pat: &hir::WhenPat,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Bind { .. } => Ok(self.context.bool_type().const_int(1, false)),
            hir::WhenPat::StringLit { span } => {
                let expected = self.codegen_string_literal(*span)?;
                let Some(BasicValueEnum::PointerValue(expected_ptr)) = expected.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when string pattern literal value",
                        at: (*span).into(),
                    });
                };
                let fn_val = self.declare_runtime_string_equals();
                let call = self.builder.build_call(
                    fn_val,
                    &[value.into(), expected_ptr.into()],
                    "when_str_eq",
                )?;
                let raw_result = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "when string equals return value",
                        at: at.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(eq_i64) = raw_result else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when string equals return type",
                        at: at.into(),
                    });
                };
                Ok(self.builder.build_int_compare(
                    IntPredicate::NE,
                    eq_i64,
                    self.context.i64_type().const_zero(),
                    "when_str_eq_bool",
                )?)
            }
            hir::WhenPat::Or { pats, .. } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for p in pats {
                    let c = self.codegen_when_pat_cond_for_string_with_value(at, value, p)?;
                    cond = self.builder.build_or(cond, c, "when_or")?;
                }
                Ok(cond)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when pattern (string)",
                at: pat.span().into(),
            }),
        }
    }

    pub(super) fn codegen_when_pat_cond_for_tuple(
        &mut self,
        at: crate::span::Span,
        tuple_ty: TypeId,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Bind { .. } => Ok(self.context.bool_type().const_int(1, false)),
            hir::WhenPat::Tuple { elements, .. } => {
                self.codegen_when_tuple_pat_cond(at, tuple_ty, elements, subject_ptr)
            }
            hir::WhenPat::Or { pats, .. } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for p in pats {
                    let c = self.codegen_when_pat_cond_for_tuple(at, tuple_ty, p, subject_ptr)?;
                    cond = self.builder.build_or(cond, c, "when_or")?;
                }
                Ok(cond)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when pattern (tuple)",
                at: pat.span().into(),
            }),
        }
    }

    pub(super) fn when_first_matching_arm_for_enum_variant(
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

    pub(super) fn when_first_matching_arm_for_bool(
        &self,
        arms: &[hir::WhenArm],
        value: bool,
    ) -> Option<usize> {
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

    pub(super) fn codegen_when_tuple_pat_cond(
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

        if let Some(rest) = rest_idx
            && rest + 1 != elements.len()
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when tuple pattern rest position",
                at: elements[rest].span().into(),
            });
        }

        let pat_arity = rest_idx.unwrap_or(elements.len());
        if (rest_idx.is_none() && pat_arity != tuple_elems.len())
            || (rest_idx.is_some() && pat_arity > tuple_elems.len())
        {
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
            let elem_cond = self.codegen_when_pat_cond_for_tuple_elem(
                at, tuple_ty, idx, elem_ty, tuple_v, elem_pat,
            )?;
            cond = self.builder.build_and(cond, elem_cond, "when_tuple_and")?;
        }
        Ok(cond)
    }

    pub(super) fn codegen_when_pat_cond_for_tuple_elem(
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
            hir::WhenPat::IntLit { .. } => {
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
                let value = self.int_literal_bits_for_ty(pat.span(), int_ty)?;
                let expected = self.int_type(int_ty).const_int(value, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    raw,
                    expected,
                    "when_tuple_int_eq",
                )?)
            }
            hir::WhenPat::CharLit { value, .. } => {
                let CgTy::Int(int_ty) = elem_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple elem char pattern type",
                        at: pat.span().into(),
                    });
                };
                let raw = self
                    .builder
                    .build_extract_value(tuple_v, elem_idx as u32, "when_tuple_elem")?
                    .into_int_value();
                let expected = self.int_type(int_ty).const_int(*value as u64, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    raw,
                    expected,
                    "when_tuple_char_eq",
                )?)
            }
            hir::WhenPat::StringLit { span } => {
                let CgTy::String = elem_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple elem string pattern type",
                        at: pat.span().into(),
                    });
                };
                let raw = self
                    .builder
                    .build_extract_value(tuple_v, elem_idx as u32, "when_tuple_elem")?
                    .into_pointer_value();
                self.codegen_when_pat_cond_for_string_with_value(*span, raw, pat)
            }
            hir::WhenPat::Tuple { elements, .. } => {
                let CgTy::Tuple(nested_tuple_ty) = elem_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple elem tuple pattern type",
                        at: pat.span().into(),
                    });
                };

                let TypeKind::Value(ValueTypeKind::Tuple(_)) = self.types.kind(nested_tuple_ty)
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "tuple type id",
                        at: pat.span().into(),
                    });
                };

                // 由于 extractvalue 返回的是一个“by-value tuple struct”，我们先把它落到临时 slot，
                // 再复用 `codegen_when_tuple_pat_cond` 的逻辑生成递归比较。
                let nested_raw = self.builder.build_extract_value(
                    tuple_v,
                    elem_idx as u32,
                    "when_tuple_elem",
                )?;
                let nested_value = self.cg_value_from_loaded(pat.span(), elem_ty, nested_raw)?;
                let tmp_name = format!("when_tuple_nested_{}_{}", tuple_ty.as_u32(), elem_idx);
                let tmp_ptr = self.create_entry_alloca(at, &tmp_name, elem_ty)?;
                let _ = self.store_local_value(at, tmp_ptr, elem_ty, nested_value)?;
                self.codegen_when_tuple_pat_cond(at, nested_tuple_ty, elements, tmp_ptr)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when tuple pattern",
                at: pat.span().into(),
            }),
        }
    }

    pub(super) fn codegen_block_as_return_value(
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
                    let expected = if is_last {
                        Some(declared_return_ty)
                    } else {
                        Some(CgTy::Unit)
                    };
                    let v = self.codegen_expr_in_expected_context(expr, expected)?;
                    if v.ty == CgTy::Never {
                        self.env.pop_scope();
                        return Ok(CgValue::never());
                    }
                    tail_value = if is_last { Some(v) } else { None };
                }
                hir::StmtKind::Return { value } => {
                    // T0141: If we have a return context (early return from nested block),
                    // use it. Otherwise, fall back to old behavior (return value directly).
                    if self.return_context.is_some() {
                        self.codegen_early_return(stmt.span, value.as_ref())?;
                        self.env.pop_scope();
                        // After branch, return a dummy — the normal path won't use it.
                        return self.default_value(stmt.span, declared_return_ty);
                    }
                    let out = match value {
                        Some(expr) => {
                            let v = self
                                .codegen_expr_in_expected_context(expr, Some(declared_return_ty))?;
                            if declared_return_ty == CgTy::Unit {
                                CgValue::unit()
                            } else {
                                self.coerce_value(expr.span, v, declared_return_ty)?
                            }
                        }
                        None => self.default_value(stmt.span, declared_return_ty)?,
                    };

                    self.env.pop_scope();
                    return Ok(out);
                }
                hir::StmtKind::While { cond, body } => {
                    self.codegen_while_stmt(stmt.span, cond, body)?;
                    tail_value = None;
                }
                // T0141: break/continue in function-level block (inside a loop).
                hir::StmtKind::Break { break_span } => {
                    let loop_ctx = self.loop_context_stack.last().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "break outside loop",
                            at: (*break_span).into(),
                        },
                    )?;
                    self.builder.build_unconditional_branch(loop_ctx.break_bb)?;
                    self.env.pop_scope();
                    return self.default_value(*break_span, declared_return_ty);
                }
                hir::StmtKind::Continue { continue_span } => {
                    let loop_ctx = self.loop_context_stack.last().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "continue outside loop",
                            at: (*continue_span).into(),
                        },
                    )?;
                    self.builder
                        .build_unconditional_branch(loop_ctx.continue_bb)?;
                    self.env.pop_scope();
                    return self.default_value(*continue_span, declared_return_ty);
                }
                hir::StmtKind::Todo(_) => {
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
            self.default_value(block.span, declared_return_ty)?
        };

        self.env.pop_scope();
        Ok(out)
    }

    pub(super) fn codegen_block_value_with_local_return_context(
        &mut self,
        block: &hir::Block,
        declared_return_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: block.span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: block.span.into(),
            })?;

        let return_bb = self.context.append_basic_block(func, "block_return");
        let return_alloca = match declared_return_ty {
            CgTy::Unit | CgTy::Never => None,
            _ => Some(self.create_entry_alloca(
                block.span,
                "block_return_val",
                declared_return_ty,
            )?),
        };

        let saved_return_ctx = self.return_context;
        let saved_return_ty = self.current_fun_return_ty;
        self.return_context = Some(ReturnContext {
            return_bb,
            return_alloca,
        });
        self.current_fun_return_ty = Some(declared_return_ty);

        let result = (|| -> Result<CgValue<'ctx>, LlvmEmitError> {
            let tail = self.codegen_block_as_return_value(block, declared_return_ty)?;

            if self
                .builder
                .get_insert_block()
                .is_some_and(|bb| bb.get_terminator().is_none())
            {
                let tail = if declared_return_ty == CgTy::Unit {
                    CgValue::unit()
                } else {
                    self.coerce_value(block.span, tail, declared_return_ty)?
                };
                if let Some(alloca) = return_alloca
                    && let Some(raw) = tail.value
                {
                    self.builder.build_store(alloca, raw)?;
                }
                self.builder.build_unconditional_branch(return_bb)?;
            }

            self.builder.position_at_end(return_bb);
            match declared_return_ty {
                CgTy::Unit => Ok(CgValue::unit()),
                CgTy::Never => Ok(CgValue::never()),
                _ => {
                    let alloca = return_alloca.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "block return alloca",
                        at: block.span.into(),
                    })?;
                    let llvm_ty = self.llvm_basic_type_of(block.span, declared_return_ty)?;
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, alloca, "block_return_load")?;
                    self.cg_value_from_loaded(block.span, declared_return_ty, loaded)
                }
            }
        })();

        self.current_fun_return_ty = saved_return_ty;
        self.return_context = saved_return_ctx;
        result
    }

    pub(super) fn codegen_block_value(
        &mut self,
        block: &hir::Block,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_block_value_in_expected_context(block, None)
    }

    pub(super) fn codegen_block_value_in_expected_context(
        &mut self,
        block: &hir::Block,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.env.push_scope();
        let expected_block_ty = expected.or_else(|| {
            if matches!(
                self.types.kind(block.ty),
                crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Any)
            ) {
                None
            } else {
                self.cg_ty_of(block.ty)
            }
        });

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
                    let expected = if is_last {
                        expected_block_ty
                    } else {
                        Some(CgTy::Unit)
                    };
                    let v = self.codegen_expr_in_expected_context(expr, expected)?;
                    if v.ty == CgTy::Never {
                        self.env.pop_scope();
                        return Ok(CgValue::never());
                    }
                    value = if is_last { v } else { CgValue::unit() };
                }
                hir::StmtKind::While { cond, body } => {
                    self.codegen_while_stmt(stmt.span, cond, body)?;
                    value = CgValue::unit();
                }
                // T0141: return inside block expression — use early return context.
                hir::StmtKind::Return { value } => {
                    if self.return_context.is_some() {
                        self.codegen_early_return(stmt.span, value.as_ref())?;
                        self.env.pop_scope();
                        let dead_path_ty = expected_block_ty.unwrap_or(CgTy::Unit);
                        return self.default_value(stmt.span, dead_path_ty);
                    }
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "`return` inside block expression",
                        at: stmt.span.into(),
                    });
                }
                // T0141: break/continue inside block expression (must be inside a loop).
                hir::StmtKind::Break { break_span } => {
                    let loop_ctx = self.loop_context_stack.last().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "break outside loop",
                            at: (*break_span).into(),
                        },
                    )?;
                    self.builder.build_unconditional_branch(loop_ctx.break_bb)?;
                    self.env.pop_scope();
                    let dead_path_ty = expected_block_ty.unwrap_or(CgTy::Unit);
                    return self.default_value(*break_span, dead_path_ty);
                }
                hir::StmtKind::Continue { continue_span } => {
                    let loop_ctx = self.loop_context_stack.last().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "continue outside loop",
                            at: (*continue_span).into(),
                        },
                    )?;
                    self.builder
                        .build_unconditional_branch(loop_ctx.continue_bb)?;
                    self.env.pop_scope();
                    let dead_path_ty = expected_block_ty.unwrap_or(CgTy::Unit);
                    return self.default_value(*continue_span, dead_path_ty);
                }
                hir::StmtKind::Todo(_) => {
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
}
