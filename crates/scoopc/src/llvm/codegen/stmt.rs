//! 语句 codegen（T0102d：从 `codegen/mod.rs` 拆分）。

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn codegen_val_decl(&mut self, decl: &hir::ValDecl) -> Result<(), LlvmEmitError> {
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
            Some(expr) => self.codegen_initializer_expr(expr, target_ty, decl.ty)?,
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
        let _stored = self.store_local_value(decl.span, ptr, target_ty, init)?;

        self.env.insert(
            id,
            CgLocal {
                hir_ty: Some(decl.ty),
                ty: target_ty,
                ptr,
                mutable: decl.mutable,
            },
        );
        Ok(())
    }

    pub(super) fn codegen_assign_stmt(
        &mut self,
        eq_span: crate::span::Span,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<(), LlvmEmitError> {
        match &lhs.kind {
            hir::ExprKind::VarRef(vref) => match vref {
                hir::ValueRef::Local { id, .. } => {
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

                    let rhs_v = self.codegen_expr_in_expected_context(rhs, Some(local.ty))?;
                    let _stored = self.store_local_value(eq_span, local.ptr, local.ty, rhs_v)?;
                    Ok(())
                }
                hir::ValueRef::TopLevel { fqn, .. } => {
                    let Some(var) = self.top_level_vars.get(fqn) else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "assignment to non-local",
                            at: lhs.span.into(),
                        });
                    };

                    let cg_ty =
                        self.cg_ty_of(var.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "top-level var type",
                                at: var.span.into(),
                            })?;

                    let gv = self.declare_top_level_var_global(var)?;
                    let rhs_v = self.codegen_expr_in_expected_context(rhs, Some(cg_ty))?;
                    let _stored =
                        self.store_local_value(eq_span, gv.as_pointer_value(), cg_ty, rhs_v)?;
                    Ok(())
                }
            },
            hir::ExprKind::MemberAccess { receiver, member } => {
                let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref() else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "assignment lhs member target",
                        at: lhs.span.into(),
                    });
                };

                // T0125：同 codegen_member_access，使用局部变量的 hir_ty 获取精确泛型类型。
                let receiver_hir_ty = if let hir::ExprKind::VarRef(hir::ValueRef::Local {
                    id,
                    ..
                }) = &receiver.kind
                {
                    self.env
                        .get(*id)
                        .and_then(|local| local.hir_ty)
                        .unwrap_or(receiver.ty)
                } else {
                    receiver.ty
                };

                let Some((class, field_idx, field_cg)) =
                    self.lookup_class_field_by_fqn(fqn, member.span, Some(receiver_hir_ty))?
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "assignment lhs",
                        at: lhs.span.into(),
                    });
                };

                let field = class.fields.get(field_idx as usize).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "assignment class field index",
                        at: lhs.span.into(),
                    },
                )?;
                if !field.mutable {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "assignment to immutable class field",
                        at: eq_span.into(),
                    });
                }

                let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::Ref))?;
                let recv = self.coerce_value(receiver.span, recv, CgTy::Ref)?;
                let Some(raw) = recv.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "assignment class receiver value",
                        at: receiver.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(obj_ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "assignment class receiver type",
                        at: receiver.span.into(),
                    });
                };

                let rhs_v = self.codegen_expr_in_expected_context(rhs, Some(field_cg))?;
                let field_ptr =
                    self.codegen_class_field_ptr(eq_span, &class, obj_ptr, field_idx)?;
                let _stored = self.store_local_value(eq_span, field_ptr, field_cg, rhs_v)?;
                Ok(())
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "assignment lhs",
                at: lhs.span.into(),
            }),
        }
    }

    pub(super) fn codegen_block_stmt(&mut self, block: &hir::Block) -> Result<(), LlvmEmitError> {
        self.env.push_scope();

        for stmt in &block.stmts {
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => self.codegen_val_decl(decl)?,
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                }
                hir::StmtKind::Expr(expr) => {
                    let _ = self.codegen_expr_in_expected_context(expr, Some(CgTy::Unit))?;
                }
                hir::StmtKind::While { cond, body } => {
                    self.codegen_while_stmt(stmt.span, cond, body)?;
                }
                // T0141: return inside loop body — branch to the function return BB.
                hir::StmtKind::Return { value } => {
                    self.codegen_early_return(stmt.span, value.as_ref())?;
                    // After an unconditional branch, stop processing further stmts.
                    self.env.pop_scope();
                    return Ok(());
                }
                // T0141: break — branch to the innermost loop's after-BB.
                hir::StmtKind::Break { break_span } => {
                    let loop_ctx = self.loop_context_stack.last().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "break outside loop",
                            at: (*break_span).into(),
                        },
                    )?;
                    self.builder.build_unconditional_branch(loop_ctx.break_bb)?;
                    self.env.pop_scope();
                    return Ok(());
                }
                // T0141: continue — branch to the innermost loop's cond-BB.
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
                    return Ok(());
                }
                hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        self.env.pop_scope();
        Ok(())
    }

    pub(super) fn codegen_while_stmt(
        &mut self,
        at: crate::span::Span,
        cond: &hir::Expr,
        body: &hir::Block,
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

        let cond_bb = self.context.append_basic_block(func, "while_cond");
        let body_bb = self.context.append_basic_block(func, "while_body");
        let after_bb = self.context.append_basic_block(func, "while_after");

        self.builder.build_unconditional_branch(cond_bb)?;

        self.builder.position_at_end(cond_bb);
        let cv = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
        let cb = cv.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "while cond value",
            at: cond.span.into(),
        })?;
        self.builder
            .build_conditional_branch(cb, body_bb, after_bb)?;

        // T0141: Push loop context so break/continue can find their targets.
        self.loop_context_stack.push(super::LoopContext {
            break_bb: after_bb,
            continue_bb: cond_bb,
        });

        self.builder.position_at_end(body_bb);
        self.codegen_block_stmt(body)?;

        self.loop_context_stack.pop();

        let body_end =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        if body_end.get_terminator().is_none() {
            self.builder.build_unconditional_branch(cond_bb)?;
        }

        self.builder.position_at_end(after_bb);
        Ok(())
    }
}
