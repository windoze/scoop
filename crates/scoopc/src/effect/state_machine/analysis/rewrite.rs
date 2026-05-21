//!  Resume-tail rewriting helpers: rewrite_state_*, build_resume_tail_*, expr_contains_span and the rest of the resume-slot rewriting machinery.

#![allow(dead_code)]

use super::*;

pub(crate) fn rewrite_state_op_with_resume_slot(
    op: &mut HandleStateOp,
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
) {
    match op {
        HandleStateOp::BindLocal { decl, .. } | HandleStateOp::DeclareAnonymousVal { decl, .. } => {
            if let Some(init) = decl.init.as_mut() {
                *init = rewrite_expr_with_resume_slot(init, source_expr, resume_path, resume_slot);
            }
        }
        HandleStateOp::Assign { stmt }
        | HandleStateOp::Return { stmt }
        | HandleStateOp::TodoStmt { stmt, .. }
        | HandleStateOp::StmtEmpty { stmt }
        | HandleStateOp::WhileCondHeader { stmt }
        | HandleStateOp::Break { stmt }
        | HandleStateOp::Continue { stmt } => {
            rewrite_stmt_with_resume_slot(stmt, source_expr, resume_path, resume_slot);
        }
        HandleStateOp::ExprMissing { expr }
        | HandleStateOp::Literal { expr }
        | HandleStateOp::ReadLocal { expr, .. }
        | HandleStateOp::ObjectInitAccessBoundary { expr, .. }
        | HandleStateOp::VarRef { expr }
        | HandleStateOp::StructLit { expr }
        | HandleStateOp::TupleLit { expr }
        | HandleStateOp::InterpolatedString { expr }
        | HandleStateOp::Expr { expr }
        | HandleStateOp::RuntimeRaiseBoundary { expr, .. }
        | HandleStateOp::BinaryExpr { expr }
        | HandleStateOp::WhenExpr { expr }
        | HandleStateOp::SuspendCall { expr, .. }
        | HandleStateOp::Call { expr }
        | HandleStateOp::Perform { expr, .. }
        | HandleStateOp::NestedHandleBoundary { expr, .. }
        | HandleStateOp::NestedHandle { expr, .. }
        | HandleStateOp::Closure { expr }
        | HandleStateOp::TodoExpr { expr, .. } => {
            **expr = rewrite_expr_with_resume_slot(expr, source_expr, resume_path, resume_slot);
        }
        HandleStateOp::ResumeAfterSite { .. }
        | HandleStateOp::CleanupEdgeComplete
        | HandleStateOp::ReturnToEnclosingExpression
        | HandleStateOp::LoopReentry { .. }
        | HandleStateOp::ImplicitElseUnit { .. }
        | HandleStateOp::ExecuteArmBody { .. } => {}
    }
}

pub(crate) fn build_ordinary_callee_resume_tail_block(
    body: &hir::Block,
    source_path: &SuspendSourcePath,
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
    allocate_synthetic_symbol_id: &mut dyn FnMut() -> hir::SymbolId,
) -> Option<hir::Block> {
    let start_idx = source_path.handle_body_stmt_idx()?;
    build_resume_tail_block_from_stmt_slice(
        body,
        start_idx,
        &source_path.frames,
        source_expr,
        resume_path,
        resume_slot,
        allocate_synthetic_symbol_id,
    )
}

pub(crate) fn build_resume_tail_block_from_stmt_slice(
    block: &hir::Block,
    start_idx: usize,
    frames: &[SuspendSourceFramePath],
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
    allocate_synthetic_symbol_id: &mut dyn FnMut() -> hir::SymbolId,
) -> Option<hir::Block> {
    let first_stmt = block.stmts.get(start_idx)?;
    let rebuilt_first = build_resume_tail_stmt(
        first_stmt,
        frames,
        source_expr,
        resume_path,
        resume_slot,
        allocate_synthetic_symbol_id,
    )?;
    let mut tail_stmts = vec![rebuilt_first];

    // Resume-tail rebuilding is path-specific. When the rebuilt leading stmt
    // already exits control flow (for example nested `return perform(...)`
    // rewritten to `return __resume_siteN`), sibling stmts from the original
    // enclosing block are unreachable on the resumed path and must not be
    // appended, otherwise ordinary callee resume blocks emit instructions
    // after a terminator.
    if !stmt_guarantees_control_flow_exit(&tail_stmts[0]) {
        tail_stmts.extend(block.stmts.iter().skip(start_idx + 1).cloned());
    }

    let tail_span = tail_stmts
        .first()
        .map(|stmt| stmt.span)
        .unwrap_or(block.span);
    Some(hir::Block {
        span: tail_span,
        ty: block.ty,
        stmts: tail_stmts,
    })
}

pub(crate) fn stmt_guarantees_control_flow_exit(stmt: &hir::Stmt) -> bool {
    match &stmt.kind {
        hir::StmtKind::Expr(expr) => expr_guarantees_control_flow_exit(expr),
        hir::StmtKind::Return { .. }
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. } => true,
        hir::StmtKind::Empty
        | hir::StmtKind::Val(_)
        | hir::StmtKind::Assign { .. }
        | hir::StmtKind::While { .. }
        | hir::StmtKind::Todo(_) => false,
    }
}

pub(crate) fn block_guarantees_control_flow_exit(block: &hir::Block) -> bool {
    block.stmts.iter().any(stmt_guarantees_control_flow_exit)
}

pub(crate) fn when_expr_guarantees_control_flow_exit(arms: &[hir::WhenArm]) -> bool {
    !arms.is_empty()
        && arms
            .iter()
            .all(|arm| expr_guarantees_control_flow_exit(&arm.body))
}

pub(crate) fn expr_guarantees_control_flow_exit(expr: &hir::Expr) -> bool {
    match &expr.kind {
        hir::ExprKind::Block(block) => block_guarantees_control_flow_exit(block),
        hir::ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => else_branch.as_deref().is_some_and(|else_branch| {
            expr_guarantees_control_flow_exit(then_branch)
                && expr_guarantees_control_flow_exit(else_branch)
        }),
        hir::ExprKind::When { arms, .. } => when_expr_guarantees_control_flow_exit(arms),
        _ => false,
    }
}

pub(crate) fn build_resume_tail_stmt(
    stmt: &hir::Stmt,
    frames: &[SuspendSourceFramePath],
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
    allocate_synthetic_symbol_id: &mut dyn FnMut() -> hir::SymbolId,
) -> Option<hir::Stmt> {
    if frames.is_empty() {
        let mut rewritten = stmt.clone();
        rewrite_stmt_with_resume_slot(&mut rewritten, source_expr, resume_path, resume_slot);
        return Some(rewritten);
    }

    match &stmt.kind {
        hir::StmtKind::Expr(expr) => {
            let rebuilt_expr = build_resume_tail_expr(
                expr,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )?;
            Some(hir::Stmt {
                span: stmt.span,
                ty: stmt.ty,
                kind: hir::StmtKind::Expr(rebuilt_expr),
            })
        }
        hir::StmtKind::Val(decl) => {
            let init = decl.init.as_ref()?;
            let rebuilt_init = build_resume_tail_expr(
                init,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )?;
            let mut rebuilt_decl = decl.clone();
            rebuilt_decl.init = Some(rebuilt_init);
            Some(hir::Stmt {
                span: stmt.span,
                ty: stmt.ty,
                kind: hir::StmtKind::Val(rebuilt_decl),
            })
        }
        hir::StmtKind::Assign { lhs, eq_span, rhs } => {
            if let Some(rebuilt_lhs) = build_resume_tail_expr(
                lhs,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            ) {
                return Some(hir::Stmt {
                    span: stmt.span,
                    ty: stmt.ty,
                    kind: hir::StmtKind::Assign {
                        lhs: rebuilt_lhs,
                        eq_span: *eq_span,
                        rhs: rhs.clone(),
                    },
                });
            }

            let rebuilt_rhs = build_resume_tail_expr(
                rhs,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )?;
            Some(hir::Stmt {
                span: stmt.span,
                ty: stmt.ty,
                kind: hir::StmtKind::Assign {
                    lhs: lhs.clone(),
                    eq_span: *eq_span,
                    rhs: rebuilt_rhs,
                },
            })
        }
        hir::StmtKind::While { cond, body } => {
            if let Some(SuspendSourceFramePath::WhileBody {
                while_cond_span,
                while_body_span,
                stmt_idx,
            }) = frames.first()
                && cond.span == *while_cond_span
                && body.span == *while_body_span
            {
                let current_iteration_tail = build_resume_tail_block_from_stmt_slice(
                    body,
                    *stmt_idx,
                    &frames[1..],
                    source_expr,
                    resume_path,
                    resume_slot,
                    allocate_synthetic_symbol_id,
                )?;
                return Some(build_resume_tail_while_stmt(
                    stmt,
                    cond,
                    body,
                    current_iteration_tail,
                    allocate_synthetic_symbol_id,
                ));
            }

            let rebuilt_cond = build_resume_tail_expr(
                cond,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )?;
            Some(hir::Stmt {
                span: stmt.span,
                ty: stmt.ty,
                kind: hir::StmtKind::While {
                    cond: rebuilt_cond,
                    body: body.clone(),
                },
            })
        }
        hir::StmtKind::Return { value } => {
            let expr = value.as_ref()?;
            let rebuilt = build_resume_tail_expr(
                expr,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )?;
            Some(hir::Stmt {
                span: stmt.span,
                ty: stmt.ty,
                kind: hir::StmtKind::Return {
                    value: Some(rebuilt),
                },
            })
        }
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => None,
    }
}

pub(crate) fn build_resume_tail_expr(
    expr: &hir::Expr,
    frames: &[SuspendSourceFramePath],
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
    allocate_synthetic_symbol_id: &mut dyn FnMut() -> hir::SymbolId,
) -> Option<hir::Expr> {
    if frames.is_empty() {
        return Some(rewrite_expr_with_resume_slot(
            expr,
            source_expr,
            resume_path,
            resume_slot,
        ));
    }

    if let Some(frame) = frames.first() {
        match frame {
            SuspendSourceFramePath::Block {
                block_span,
                stmt_idx,
            } => {
                if let hir::ExprKind::Block(block) = &expr.kind
                    && block.span == *block_span
                {
                    let rebuilt_block = build_resume_tail_block_from_stmt_slice(
                        block,
                        *stmt_idx,
                        &frames[1..],
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )?;
                    return Some(make_block_expr_with_original_span(expr, rebuilt_block));
                }
            }
            SuspendSourceFramePath::IfThen {
                if_span,
                then_span,
                stmt_idx,
            } => {
                if let hir::ExprKind::If { then_branch, .. } = &expr.kind
                    && expr.span == *if_span
                    && let hir::ExprKind::Block(block) = &then_branch.kind
                    && block.span == *then_span
                {
                    let rebuilt_block = build_resume_tail_block_from_stmt_slice(
                        block,
                        *stmt_idx,
                        &frames[1..],
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )?;
                    return Some(make_block_expr_with_original_span(expr, rebuilt_block));
                }
            }
            SuspendSourceFramePath::IfElse {
                if_span,
                else_span,
                stmt_idx,
            } => {
                if let hir::ExprKind::If {
                    else_branch: Some(else_branch),
                    ..
                } = &expr.kind
                    && expr.span == *if_span
                    && let hir::ExprKind::Block(block) = &else_branch.kind
                    && block.span == *else_span
                {
                    let rebuilt_block = build_resume_tail_block_from_stmt_slice(
                        block,
                        *stmt_idx,
                        &frames[1..],
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )?;
                    return Some(make_block_expr_with_original_span(expr, rebuilt_block));
                }
            }
            SuspendSourceFramePath::WhenArm {
                when_span,
                arm_index,
                arm_span,
                stmt_idx,
            } => {
                if let hir::ExprKind::When { arms, .. } = &expr.kind
                    && expr.span == *when_span
                    && let Some(arm) = arms.get(*arm_index)
                    && let hir::ExprKind::Block(block) = &arm.body.kind
                    && block.span == *arm_span
                {
                    let rebuilt_block = build_resume_tail_block_from_stmt_slice(
                        block,
                        *stmt_idx,
                        &frames[1..],
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )?;
                    return Some(make_block_expr_with_original_span(expr, rebuilt_block));
                }
            }
            SuspendSourceFramePath::WhileBody { .. } => {}
        }
    }

    match &expr.kind {
        hir::ExprKind::StructLit { fields, ty } => {
            for (field_index, field) in fields.iter().enumerate() {
                let Some(rebuilt_value) = build_resume_tail_expr(
                    &field.value,
                    frames,
                    source_expr,
                    resume_path,
                    resume_slot,
                    allocate_synthetic_symbol_id,
                ) else {
                    continue;
                };
                let mut rebuilt_fields = fields.clone();
                rebuilt_fields[field_index].value = rebuilt_value;
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::StructLit {
                        ty: *ty,
                        fields: rebuilt_fields,
                    },
                });
            }
            None
        }
        hir::ExprKind::TupleLit { elements } => {
            for (element_index, element) in elements.iter().enumerate() {
                let Some(rebuilt_element) = build_resume_tail_expr(
                    element,
                    frames,
                    source_expr,
                    resume_path,
                    resume_slot,
                    allocate_synthetic_symbol_id,
                ) else {
                    continue;
                };
                let mut rebuilt_elements = elements.clone();
                rebuilt_elements[element_index] = rebuilt_element;
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::TupleLit {
                        elements: rebuilt_elements,
                    },
                });
            }
            None
        }
        hir::ExprKind::InterpolatedString { raw, parts } => {
            for (part_index, part) in parts.iter().enumerate() {
                let hir::InterpolatedStringPart::Expr { expr: part_expr } = part else {
                    continue;
                };
                let Some(rebuilt_expr) = build_resume_tail_expr(
                    part_expr,
                    frames,
                    source_expr,
                    resume_path,
                    resume_slot,
                    allocate_synthetic_symbol_id,
                ) else {
                    continue;
                };
                let mut rebuilt_parts = parts.clone();
                rebuilt_parts[part_index] =
                    hir::InterpolatedStringPart::Expr { expr: rebuilt_expr };
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::InterpolatedString {
                        raw: *raw,
                        parts: rebuilt_parts,
                    },
                });
            }
            None
        }
        hir::ExprKind::Unary {
            op,
            op_span,
            expr: inner,
        } => build_resume_tail_expr(
            inner,
            frames,
            source_expr,
            resume_path,
            resume_slot,
            allocate_synthetic_symbol_id,
        )
        .map(|rewritten_inner| hir::Expr {
            span: expr.span,
            ty: expr.ty,
            kind: hir::ExprKind::Unary {
                op: *op,
                op_span: *op_span,
                expr: Box::new(rewritten_inner),
            },
        }),
        hir::ExprKind::TypeCheck {
            expr: inner,
            op,
            op_span,
            target_ty,
        } => build_resume_tail_expr(
            inner,
            frames,
            source_expr,
            resume_path,
            resume_slot,
            allocate_synthetic_symbol_id,
        )
        .map(|rewritten_inner| hir::Expr {
            span: expr.span,
            ty: expr.ty,
            kind: hir::ExprKind::TypeCheck {
                expr: Box::new(rewritten_inner),
                op: *op,
                op_span: *op_span,
                target_ty: *target_ty,
            },
        }),
        hir::ExprKind::Cast {
            expr: inner,
            op,
            op_span,
            target_ty,
        } => build_resume_tail_expr(
            inner,
            frames,
            source_expr,
            resume_path,
            resume_slot,
            allocate_synthetic_symbol_id,
        )
        .map(|rewritten_inner| hir::Expr {
            span: expr.span,
            ty: expr.ty,
            kind: hir::ExprKind::Cast {
                expr: Box::new(rewritten_inner),
                op: *op,
                op_span: *op_span,
                target_ty: *target_ty,
            },
        }),
        hir::ExprKind::Binary {
            lhs,
            op,
            op_span,
            rhs,
        } => {
            if let Some(rewritten_lhs) = build_resume_tail_expr(
                lhs,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            ) {
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::Binary {
                        lhs: Box::new(rewritten_lhs),
                        op: *op,
                        op_span: *op_span,
                        rhs: rhs.clone(),
                    },
                });
            }

            build_resume_tail_expr(
                rhs,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )
            .map(|rewritten_rhs| hir::Expr {
                span: expr.span,
                ty: expr.ty,
                kind: hir::ExprKind::Binary {
                    lhs: lhs.clone(),
                    op: *op,
                    op_span: *op_span,
                    rhs: Box::new(rewritten_rhs),
                },
            })
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            if let Some(rewritten_cond) = build_resume_tail_expr(
                cond,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            ) {
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::If {
                        cond: Box::new(rewritten_cond),
                        then_branch: then_branch.clone(),
                        else_branch: else_branch.clone(),
                    },
                });
            }

            if let Some(rewritten_then) = build_resume_tail_expr(
                then_branch,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            ) {
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::If {
                        cond: cond.clone(),
                        then_branch: Box::new(rewritten_then),
                        else_branch: else_branch.clone(),
                    },
                });
            }

            let else_branch_expr = else_branch.as_deref()?;
            let rewritten_else = build_resume_tail_expr(
                else_branch_expr,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )?;
            Some(hir::Expr {
                span: expr.span,
                ty: expr.ty,
                kind: hir::ExprKind::If {
                    cond: cond.clone(),
                    then_branch: then_branch.clone(),
                    else_branch: Some(Box::new(rewritten_else)),
                },
            })
        }
        hir::ExprKind::When { subject, arms } => {
            if let Some(rewritten_subject) = build_resume_tail_expr(
                subject,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            ) {
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::When {
                        subject: Box::new(rewritten_subject),
                        arms: arms.clone(),
                    },
                });
            }

            for (arm_index, arm) in arms.iter().enumerate() {
                if let Some(guard) = arm.guard.as_ref()
                    && let Some(rewritten_guard) = build_resume_tail_expr(
                        guard,
                        frames,
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )
                {
                    let mut rebuilt_arms = arms.clone();
                    rebuilt_arms[arm_index].guard = Some(rewritten_guard);
                    return Some(hir::Expr {
                        span: expr.span,
                        ty: expr.ty,
                        kind: hir::ExprKind::When {
                            subject: subject.clone(),
                            arms: rebuilt_arms,
                        },
                    });
                }

                if let Some(rewritten_body) = build_resume_tail_expr(
                    &arm.body,
                    frames,
                    source_expr,
                    resume_path,
                    resume_slot,
                    allocate_synthetic_symbol_id,
                ) {
                    let mut rebuilt_arms = arms.clone();
                    rebuilt_arms[arm_index].body = rewritten_body;
                    return Some(hir::Expr {
                        span: expr.span,
                        ty: expr.ty,
                        kind: hir::ExprKind::When {
                            subject: subject.clone(),
                            arms: rebuilt_arms,
                        },
                    });
                }
            }
            None
        }
        hir::ExprKind::MemberAccess { receiver, member } => build_resume_tail_expr(
            receiver,
            frames,
            source_expr,
            resume_path,
            resume_slot,
            allocate_synthetic_symbol_id,
        )
        .map(|rewritten_receiver| hir::Expr {
            span: expr.span,
            ty: expr.ty,
            kind: hir::ExprKind::MemberAccess {
                receiver: Box::new(rewritten_receiver),
                member: member.clone(),
            },
        }),
        hir::ExprKind::Call { callee, args } => {
            if let Some(rewritten_callee) = build_resume_tail_expr(
                callee,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            ) {
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::Call {
                        callee: Box::new(rewritten_callee),
                        args: args.clone(),
                    },
                });
            }

            for (arg_index, arg) in args.iter().enumerate() {
                let rebuilt = match arg {
                    hir::CallArg::Positional(arg_expr) => build_resume_tail_expr(
                        arg_expr,
                        frames,
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )
                    .map(hir::CallArg::Positional),
                    hir::CallArg::Named {
                        name,
                        name_span,
                        value,
                    } => build_resume_tail_expr(
                        value,
                        frames,
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )
                    .map(|rewritten_value| hir::CallArg::Named {
                        name: name.clone(),
                        name_span: *name_span,
                        value: rewritten_value,
                    }),
                };
                let Some(rewritten_arg) = rebuilt else {
                    continue;
                };
                let mut rebuilt_args = args.clone();
                rebuilt_args[arg_index] = rewritten_arg;
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::Call {
                        callee: callee.clone(),
                        args: rebuilt_args,
                    },
                });
            }
            None
        }
        hir::ExprKind::Perform {
            effect_ty,
            op,
            args,
        } => {
            for (arg_index, arg) in args.iter().enumerate() {
                let rebuilt = match arg {
                    hir::CallArg::Positional(arg_expr) => build_resume_tail_expr(
                        arg_expr,
                        frames,
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )
                    .map(hir::CallArg::Positional),
                    hir::CallArg::Named {
                        name,
                        name_span,
                        value,
                    } => build_resume_tail_expr(
                        value,
                        frames,
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )
                    .map(|rewritten_value| hir::CallArg::Named {
                        name: name.clone(),
                        name_span: *name_span,
                        value: rewritten_value,
                    }),
                };
                let Some(rewritten_arg) = rebuilt else {
                    continue;
                };
                let mut rebuilt_args = args.clone();
                rebuilt_args[arg_index] = rewritten_arg;
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::Perform {
                        effect_ty: *effect_ty,
                        op: op.clone(),
                        args: rebuilt_args,
                    },
                });
            }
            None
        }
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Block(_)
        | hir::ExprKind::Closure(_)
        | hir::ExprKind::Handle(_)
        | hir::ExprKind::Todo(_) => None,
    }
}

pub(crate) fn make_block_expr_with_original_span(
    original_expr: &hir::Expr,
    block: hir::Block,
) -> hir::Expr {
    hir::Expr {
        span: original_expr.span,
        ty: original_expr.ty,
        kind: hir::ExprKind::Block(block),
    }
}

pub(crate) fn make_block_expr(span: Span, ty: TypeId, block: hir::Block) -> hir::Expr {
    hir::Expr {
        span,
        ty,
        kind: hir::ExprKind::Block(block),
    }
}

pub(crate) fn make_local_var_expr(
    span: Span,
    ty: TypeId,
    id: hir::SymbolId,
    name: &str,
) -> hir::Expr {
    hir::Expr {
        span,
        ty,
        kind: hir::ExprKind::VarRef(hir::ValueRef::Local {
            id,
            name: name.to_string(),
            decl_span: span,
        }),
    }
}

pub(crate) fn make_bool_literal_expr(span: Span, ty: TypeId, value: bool) -> hir::Expr {
    hir::Expr {
        span,
        ty,
        kind: hir::ExprKind::Literal(hir::LiteralKind::Bool(value)),
    }
}

pub(crate) fn make_assign_stmt(
    span: Span,
    ty: TypeId,
    lhs: hir::Expr,
    rhs: hir::Expr,
) -> hir::Stmt {
    hir::Stmt {
        span,
        ty,
        kind: hir::StmtKind::Assign {
            lhs,
            eq_span: span,
            rhs,
        },
    }
}

// 对 `while` body 内部的 suspend source，resume 后必须先完成当前迭代尾部，
// 然后才回到原 loop 的后续迭代；不能重新从 cond 之前开始，也不能丢掉
// `break/continue` 对当前 loop 的控制流语义。
pub(crate) fn build_resume_tail_while_stmt(
    original_stmt: &hir::Stmt,
    cond: &hir::Expr,
    body: &hir::Block,
    current_iteration_tail: hir::Block,
    allocate_synthetic_symbol_id: &mut dyn FnMut() -> hir::SymbolId,
) -> hir::Stmt {
    let resume_first_id = allocate_synthetic_symbol_id();
    let resume_first_name = format!("__resume_loop_first{}", resume_first_id.as_u32());
    let bool_ty = cond.ty;

    let resume_first_decl = hir::Stmt {
        span: original_stmt.span,
        ty: original_stmt.ty,
        kind: hir::StmtKind::Val(hir::ValDecl {
            span: original_stmt.span,
            id: Some(resume_first_id),
            name: Some(resume_first_name.clone()),
            mutable: true,
            ty: bool_ty,
            init: Some(make_bool_literal_expr(original_stmt.span, bool_ty, true)),
        }),
    };

    let resume_first_var = make_local_var_expr(
        original_stmt.span,
        bool_ty,
        resume_first_id,
        &resume_first_name,
    );
    let loop_cond = hir::Expr {
        span: cond.span,
        ty: bool_ty,
        kind: hir::ExprKind::If {
            cond: Box::new(resume_first_var.clone()),
            then_branch: Box::new(make_bool_literal_expr(cond.span, bool_ty, true)),
            else_branch: Some(Box::new(cond.clone())),
        },
    };

    let clear_resume_first = make_assign_stmt(
        original_stmt.span,
        original_stmt.ty,
        resume_first_var.clone(),
        make_bool_literal_expr(original_stmt.span, bool_ty, false),
    );

    let mut first_iteration_stmts = vec![clear_resume_first];
    first_iteration_stmts.extend(current_iteration_tail.stmts);
    let first_iteration_block = hir::Block {
        span: current_iteration_tail.span,
        ty: body.ty,
        stmts: first_iteration_stmts,
    };

    let loop_body_if = hir::Expr {
        span: original_stmt.span,
        ty: body.ty,
        kind: hir::ExprKind::If {
            cond: Box::new(resume_first_var),
            then_branch: Box::new(make_block_expr(
                first_iteration_block.span,
                body.ty,
                first_iteration_block,
            )),
            else_branch: Some(Box::new(make_block_expr(body.span, body.ty, body.clone()))),
        },
    };
    let loop_body = hir::Block {
        span: body.span,
        ty: body.ty,
        stmts: vec![hir::Stmt {
            span: original_stmt.span,
            ty: body.ty,
            kind: hir::StmtKind::Expr(loop_body_if),
        }],
    };
    let resumed_loop = hir::Stmt {
        span: original_stmt.span,
        ty: original_stmt.ty,
        kind: hir::StmtKind::While {
            cond: loop_cond,
            body: loop_body,
        },
    };

    let wrapper_block = hir::Block {
        span: original_stmt.span,
        ty: original_stmt.ty,
        stmts: vec![resume_first_decl, resumed_loop],
    };
    hir::Stmt {
        span: original_stmt.span,
        ty: original_stmt.ty,
        kind: hir::StmtKind::Expr(make_block_expr(
            original_stmt.span,
            original_stmt.ty,
            wrapper_block,
        )),
    }
}

pub(crate) fn rewrite_state_terminator_with_resume_slot(
    terminator: &mut StateTerminator,
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
) {
    if let StateTerminator::Branch { condition, .. } = terminator {
        rewrite_branch_condition_with_resume_slot(condition, source_expr, resume_path, resume_slot);
    }
}

pub(crate) struct MaterializedWhenResumeRewrite {
    pub(crate) when_span: Span,
    pub(crate) when_index: usize,
    pub(crate) consumer_action_indices: Vec<usize>,
    pub(crate) rewrite_terminator: bool,
    pub(crate) replacement_expr: Option<hir::Expr>,
}

pub(crate) struct MaterializedWhenResumeInput<'a> {
    pub(crate) source_path: &'a SuspendSourcePath,
    pub(crate) source_expr: &'a hir::Expr,
    pub(crate) resume_path: &'a SuspendResumePath,
    pub(crate) resume_slot: &'a FrameSlot,
    pub(crate) allocate_synthetic_symbol_id: &'a mut dyn FnMut() -> hir::SymbolId,
}

pub(crate) fn prepare_materialized_when_resume_rewrite(
    actions: &[HandleStateOp],
    resume_after_index: usize,
    terminator: &StateTerminator,
    input: &mut MaterializedWhenResumeInput<'_>,
) -> Option<MaterializedWhenResumeRewrite> {
    let (when_frame_index, when_span) = input
        .source_path
        .frames
        .iter()
        .enumerate()
        .rev()
        .find_map(|(idx, op)| match op {
            SuspendSourceFramePath::WhenArm { when_span, .. } => Some((idx, *when_span)),
            _ => None,
        })?;

    let when_index = actions
        .iter()
        .enumerate()
        .skip(resume_after_index + 1)
        .find_map(|(idx, op)| match op {
            HandleStateOp::WhenExpr { expr } if expr.span == when_span => Some(idx),
            _ => None,
        })?;

    let consumer_action_indices = actions[when_index + 1..]
        .iter()
        .enumerate()
        .filter_map(|(offset, op)| {
            state_op_contains_expr_span(op, when_span).then_some(when_index + 1 + offset)
        })
        .collect::<Vec<_>>();
    let rewrite_terminator = state_terminator_contains_expr_span(terminator, when_span);

    if consumer_action_indices.is_empty() && !rewrite_terminator {
        return Some(MaterializedWhenResumeRewrite {
            when_span,
            when_index,
            consumer_action_indices,
            rewrite_terminator,
            replacement_expr: None,
        });
    }

    let HandleStateOp::WhenExpr { expr: when_expr } = &actions[when_index] else {
        return None;
    };
    let replacement_expr = build_resume_tail_expr(
        when_expr,
        &input.source_path.frames[when_frame_index..],
        input.source_expr,
        input.resume_path,
        input.resume_slot,
        input.allocate_synthetic_symbol_id,
    )?;

    debug_assert!(
        consumer_action_indices.len() + usize::from(rewrite_terminator) <= 1,
        "materialized when resume rewrite unexpectedly found multiple live consumers for span {:?}",
        when_span
    );

    Some(MaterializedWhenResumeRewrite {
        when_span,
        when_index,
        consumer_action_indices,
        rewrite_terminator,
        replacement_expr: Some(replacement_expr),
    })
}

pub(crate) fn resume_rewrite_candidate_spans(
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
) -> Vec<Span> {
    let mut spans = vec![source_expr.span];
    for frame in &resume_path.expr_frames {
        let span = frame.expr_span();
        if !spans.contains(&span) {
            spans.push(span);
        }
    }
    spans
}

pub(crate) fn suspend_site_kind_matches_source_path_expr_kind(
    site_kind: &SuspendSiteKind,
    expr_kind: &hir::ExprKind,
) -> bool {
    matches!(
        (site_kind, expr_kind),
        (
            SuspendSiteKind::Perform { .. },
            hir::ExprKind::Perform { .. }
        ) | (
            SuspendSiteKind::CallMaySuspend { .. }
                | SuspendSiteKind::CallStateMachineCallee { .. }
                | SuspendSiteKind::ClassCtorInit { .. },
            hir::ExprKind::Call { .. },
        ) | (
            SuspendSiteKind::NestedHandleBoundary { .. },
            hir::ExprKind::Handle(_),
        )
    )
}

pub(crate) fn suspend_site_kind_matches_resume_path_expr_kind(
    site_kind: &SuspendSiteKind,
    expr_kind: &hir::ExprKind,
) -> bool {
    matches!(
        (site_kind, expr_kind),
        (
            SuspendSiteKind::Perform { .. },
            hir::ExprKind::Perform { .. }
        ) | (
            SuspendSiteKind::CallMaySuspend { .. }
                | SuspendSiteKind::CallStateMachineCallee { .. }
                | SuspendSiteKind::ClassCtorInit { .. }
                | SuspendSiteKind::RuntimeRaise { .. },
            hir::ExprKind::Call { .. },
        ) | (
            SuspendSiteKind::NestedHandleBoundary { .. },
            hir::ExprKind::Handle(_),
        )
    )
}

pub(crate) fn state_contains_any_expr_span(state: &PlanState, candidate_spans: &[Span]) -> bool {
    candidate_spans.iter().copied().any(|expr_span| {
        state
            .actions
            .iter()
            .any(|op| state_op_contains_expr_span(op, expr_span))
            || state_terminator_contains_expr_span(&state.terminator, expr_span)
    })
}

pub(crate) fn state_op_contains_expr_span(op: &HandleStateOp, expr_span: Span) -> bool {
    match op {
        HandleStateOp::BindLocal { decl, .. } | HandleStateOp::DeclareAnonymousVal { decl, .. } => {
            decl.init
                .as_ref()
                .is_some_and(|init| expr_contains_span(init, expr_span))
        }
        HandleStateOp::Assign { stmt }
        | HandleStateOp::Return { stmt }
        | HandleStateOp::TodoStmt { stmt, .. }
        | HandleStateOp::StmtEmpty { stmt }
        | HandleStateOp::WhileCondHeader { stmt }
        | HandleStateOp::Break { stmt }
        | HandleStateOp::Continue { stmt } => stmt_contains_expr_span(stmt, expr_span),
        HandleStateOp::ExprMissing { expr }
        | HandleStateOp::Literal { expr }
        | HandleStateOp::ReadLocal { expr, .. }
        | HandleStateOp::ObjectInitAccessBoundary { expr, .. }
        | HandleStateOp::VarRef { expr }
        | HandleStateOp::StructLit { expr }
        | HandleStateOp::TupleLit { expr }
        | HandleStateOp::InterpolatedString { expr }
        | HandleStateOp::Expr { expr }
        | HandleStateOp::RuntimeRaiseBoundary { expr, .. }
        | HandleStateOp::BinaryExpr { expr }
        | HandleStateOp::WhenExpr { expr }
        | HandleStateOp::SuspendCall { expr, .. }
        | HandleStateOp::Call { expr }
        | HandleStateOp::Perform { expr, .. }
        | HandleStateOp::NestedHandleBoundary { expr, .. }
        | HandleStateOp::NestedHandle { expr, .. }
        | HandleStateOp::Closure { expr }
        | HandleStateOp::TodoExpr { expr, .. } => expr_contains_span(expr, expr_span),
        HandleStateOp::ResumeAfterSite { source_span, .. } => *source_span == expr_span,
        HandleStateOp::CleanupEdgeComplete
        | HandleStateOp::ReturnToEnclosingExpression
        | HandleStateOp::LoopReentry { .. }
        | HandleStateOp::ImplicitElseUnit { .. }
        | HandleStateOp::ExecuteArmBody { .. } => false,
    }
}

pub(crate) fn state_op_within_span(op: &HandleStateOp, container_span: Span) -> bool {
    let span_within_container =
        |span: Span| span.start >= container_span.start && span.end <= container_span.end;

    match op {
        HandleStateOp::BindLocal { decl, .. } | HandleStateOp::DeclareAnonymousVal { decl, .. } => {
            span_within_container(decl.span)
        }
        HandleStateOp::Assign { stmt }
        | HandleStateOp::Return { stmt }
        | HandleStateOp::TodoStmt { stmt, .. }
        | HandleStateOp::StmtEmpty { stmt }
        | HandleStateOp::WhileCondHeader { stmt }
        | HandleStateOp::Break { stmt }
        | HandleStateOp::Continue { stmt } => span_within_container(stmt.span),
        HandleStateOp::ExprMissing { expr }
        | HandleStateOp::Literal { expr }
        | HandleStateOp::ReadLocal { expr, .. }
        | HandleStateOp::ObjectInitAccessBoundary { expr, .. }
        | HandleStateOp::VarRef { expr }
        | HandleStateOp::StructLit { expr }
        | HandleStateOp::TupleLit { expr }
        | HandleStateOp::InterpolatedString { expr }
        | HandleStateOp::Expr { expr }
        | HandleStateOp::RuntimeRaiseBoundary { expr, .. }
        | HandleStateOp::BinaryExpr { expr }
        | HandleStateOp::WhenExpr { expr }
        | HandleStateOp::SuspendCall { expr, .. }
        | HandleStateOp::Call { expr }
        | HandleStateOp::Perform { expr, .. }
        | HandleStateOp::NestedHandleBoundary { expr, .. }
        | HandleStateOp::NestedHandle { expr, .. }
        | HandleStateOp::Closure { expr }
        | HandleStateOp::TodoExpr { expr, .. } => span_within_container(expr.span),
        HandleStateOp::ResumeAfterSite { source_span, .. } => span_within_container(*source_span),
        HandleStateOp::ImplicitElseUnit { span } => span_within_container(*span),
        HandleStateOp::ExecuteArmBody { arm, .. } => span_within_container(arm.span),
        HandleStateOp::CleanupEdgeComplete
        | HandleStateOp::ReturnToEnclosingExpression
        | HandleStateOp::LoopReentry { .. } => false,
    }
}

pub(crate) fn stmt_contains_expr_span(stmt: &hir::Stmt, expr_span: Span) -> bool {
    match &stmt.kind {
        hir::StmtKind::Expr(expr) => expr_contains_span(expr, expr_span),
        hir::StmtKind::Val(decl) => decl
            .init
            .as_ref()
            .is_some_and(|init| expr_contains_span(init, expr_span)),
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            expr_contains_span(lhs, expr_span) || expr_contains_span(rhs, expr_span)
        }
        hir::StmtKind::While { cond, body } => {
            expr_contains_span(cond, expr_span)
                || body
                    .stmts
                    .iter()
                    .any(|stmt| stmt_contains_expr_span(stmt, expr_span))
        }
        hir::StmtKind::Return { value } => value
            .as_ref()
            .is_some_and(|expr| expr_contains_span(expr, expr_span)),
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => false,
    }
}

pub(crate) fn expr_contains_span(expr: &hir::Expr, expr_span: Span) -> bool {
    if expr.span == expr_span {
        return true;
    }

    match &expr.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Closure(_)
        | hir::ExprKind::Todo(_) => false,
        hir::ExprKind::StructLit { fields, .. } => fields
            .iter()
            .any(|field| expr_contains_span(&field.value, expr_span)),
        hir::ExprKind::TupleLit { elements } => elements
            .iter()
            .any(|element| expr_contains_span(element, expr_span)),
        hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().any(|part| {
            matches!(
                part,
                hir::InterpolatedStringPart::Expr { expr }
                    if expr_contains_span(expr, expr_span)
            )
        }),
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. } => expr_contains_span(inner, expr_span),
        hir::ExprKind::Block(block) => block
            .stmts
            .iter()
            .any(|stmt| stmt_contains_expr_span(stmt, expr_span)),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            expr_contains_span(lhs, expr_span) || expr_contains_span(rhs, expr_span)
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            expr_contains_span(cond, expr_span)
                || expr_contains_span(then_branch, expr_span)
                || else_branch
                    .as_deref()
                    .is_some_and(|else_branch| expr_contains_span(else_branch, expr_span))
        }
        hir::ExprKind::When { subject, arms } => {
            expr_contains_span(subject, expr_span)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| expr_contains_span(guard, expr_span))
                        || expr_contains_span(&arm.body, expr_span)
                })
        }
        hir::ExprKind::MemberAccess { receiver, .. } => expr_contains_span(receiver, expr_span),
        hir::ExprKind::Call { callee, args } => {
            expr_contains_span(callee, expr_span)
                || args.iter().any(|arg| match arg {
                    hir::CallArg::Positional(arg_expr) => expr_contains_span(arg_expr, expr_span),
                    hir::CallArg::Named { value, .. } => expr_contains_span(value, expr_span),
                })
        }
        hir::ExprKind::Perform { args, .. } => args.iter().any(|arg| match arg {
            hir::CallArg::Positional(arg_expr) => expr_contains_span(arg_expr, expr_span),
            hir::CallArg::Named { value, .. } => expr_contains_span(value, expr_span),
        }),
        hir::ExprKind::Handle(handle) => {
            handle
                .body
                .stmts
                .iter()
                .any(|stmt| stmt_contains_expr_span(stmt, expr_span))
                || handle
                    .arms
                    .iter()
                    .any(|arm| expr_contains_span(&arm.body, expr_span))
                || handle.finally.as_ref().is_some_and(|finally_block| {
                    finally_block
                        .stmts
                        .iter()
                        .any(|stmt| stmt_contains_expr_span(stmt, expr_span))
                })
        }
    }
}

pub(crate) fn state_terminator_contains_expr_span(
    terminator: &StateTerminator,
    expr_span: Span,
) -> bool {
    match terminator {
        StateTerminator::Branch { condition, .. } => match condition {
            HandleBranchCondition::WhileCond { condition }
            | HandleBranchCondition::IfCond { condition } => {
                expr_contains_span(condition, expr_span)
            }
        },
        StateTerminator::Goto(_)
        | StateTerminator::Suspend { .. }
        | StateTerminator::ReturnHandle
        | StateTerminator::ReturnFromFunction
        | StateTerminator::CleanupEnter { .. }
        | StateTerminator::ArmExit(_) => false,
    }
}

pub(crate) fn rewrite_state_op_replacing_expr_span(
    op: &mut HandleStateOp,
    target_span: Span,
    replacement_expr: &hir::Expr,
) {
    match op {
        HandleStateOp::BindLocal { decl, .. } | HandleStateOp::DeclareAnonymousVal { decl, .. } => {
            if let Some(init) = decl.init.as_mut() {
                *init = rewrite_expr_replacing_span(init, target_span, replacement_expr);
            }
        }
        HandleStateOp::Assign { stmt }
        | HandleStateOp::Return { stmt }
        | HandleStateOp::TodoStmt { stmt, .. }
        | HandleStateOp::StmtEmpty { stmt }
        | HandleStateOp::WhileCondHeader { stmt }
        | HandleStateOp::Break { stmt }
        | HandleStateOp::Continue { stmt } => {
            rewrite_stmt_replacing_expr_span(stmt, target_span, replacement_expr);
        }
        HandleStateOp::ExprMissing { expr }
        | HandleStateOp::Literal { expr }
        | HandleStateOp::ReadLocal { expr, .. }
        | HandleStateOp::ObjectInitAccessBoundary { expr, .. }
        | HandleStateOp::VarRef { expr }
        | HandleStateOp::StructLit { expr }
        | HandleStateOp::TupleLit { expr }
        | HandleStateOp::InterpolatedString { expr }
        | HandleStateOp::Expr { expr }
        | HandleStateOp::RuntimeRaiseBoundary { expr, .. }
        | HandleStateOp::BinaryExpr { expr }
        | HandleStateOp::WhenExpr { expr }
        | HandleStateOp::SuspendCall { expr, .. }
        | HandleStateOp::Call { expr }
        | HandleStateOp::Perform { expr, .. }
        | HandleStateOp::NestedHandleBoundary { expr, .. }
        | HandleStateOp::NestedHandle { expr, .. }
        | HandleStateOp::Closure { expr }
        | HandleStateOp::TodoExpr { expr, .. } => {
            **expr = rewrite_expr_replacing_span(expr, target_span, replacement_expr);
        }
        HandleStateOp::ResumeAfterSite { .. }
        | HandleStateOp::CleanupEdgeComplete
        | HandleStateOp::ReturnToEnclosingExpression
        | HandleStateOp::LoopReentry { .. }
        | HandleStateOp::ImplicitElseUnit { .. }
        | HandleStateOp::ExecuteArmBody { .. } => {}
    }
}

pub(crate) fn rewrite_state_terminator_replacing_expr_span(
    terminator: &mut StateTerminator,
    target_span: Span,
    replacement_expr: &hir::Expr,
) {
    if let StateTerminator::Branch { condition, .. } = terminator {
        rewrite_branch_condition_replacing_expr_span(condition, target_span, replacement_expr);
    }
}

pub(crate) fn rewrite_stmt_replacing_expr_span(
    stmt: &mut hir::Stmt,
    target_span: Span,
    replacement_expr: &hir::Expr,
) {
    match &mut stmt.kind {
        hir::StmtKind::Expr(expr) => {
            *expr = rewrite_expr_replacing_span(expr, target_span, replacement_expr);
        }
        hir::StmtKind::Val(decl) => {
            if let Some(init) = decl.init.as_mut() {
                *init = rewrite_expr_replacing_span(init, target_span, replacement_expr);
            }
        }
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            *lhs = rewrite_expr_replacing_span(lhs, target_span, replacement_expr);
            *rhs = rewrite_expr_replacing_span(rhs, target_span, replacement_expr);
        }
        hir::StmtKind::While { cond, body } => {
            *cond = rewrite_expr_replacing_span(cond, target_span, replacement_expr);
            for stmt in &mut body.stmts {
                rewrite_stmt_replacing_expr_span(stmt, target_span, replacement_expr);
            }
        }
        hir::StmtKind::Return { value } => {
            if let Some(expr) = value.as_mut() {
                *expr = rewrite_expr_replacing_span(expr, target_span, replacement_expr);
            }
        }
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => {}
    }
}

pub(crate) fn rewrite_branch_condition_replacing_expr_span(
    condition: &mut HandleBranchCondition,
    target_span: Span,
    replacement_expr: &hir::Expr,
) {
    match condition {
        HandleBranchCondition::WhileCond { condition }
        | HandleBranchCondition::IfCond { condition } => {
            **condition = rewrite_expr_replacing_span(condition, target_span, replacement_expr);
        }
    }
}

pub(crate) fn rewrite_expr_replacing_span(
    expr: &hir::Expr,
    target_span: Span,
    replacement_expr: &hir::Expr,
) -> hir::Expr {
    if expr.span == target_span {
        return replacement_expr.clone();
    }

    let mut rewritten = expr.clone();
    match &mut rewritten.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Closure(_)
        | hir::ExprKind::Todo(_) => {}
        hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                field.value =
                    rewrite_expr_replacing_span(&field.value, target_span, replacement_expr);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                *element = rewrite_expr_replacing_span(element, target_span, replacement_expr);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    *expr = rewrite_expr_replacing_span(expr, target_span, replacement_expr);
                }
            }
        }
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. } => {
            **inner = rewrite_expr_replacing_span(inner, target_span, replacement_expr);
        }
        hir::ExprKind::Block(block) => {
            for stmt in &mut block.stmts {
                rewrite_stmt_replacing_expr_span(stmt, target_span, replacement_expr);
            }
        }
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            **lhs = rewrite_expr_replacing_span(lhs, target_span, replacement_expr);
            **rhs = rewrite_expr_replacing_span(rhs, target_span, replacement_expr);
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            **cond = rewrite_expr_replacing_span(cond, target_span, replacement_expr);
            **then_branch = rewrite_expr_replacing_span(then_branch, target_span, replacement_expr);
            if let Some(else_branch) = else_branch.as_mut() {
                **else_branch =
                    rewrite_expr_replacing_span(else_branch, target_span, replacement_expr);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            **subject = rewrite_expr_replacing_span(subject, target_span, replacement_expr);
            for arm in arms {
                if let Some(guard) = arm.guard.as_mut() {
                    *guard = rewrite_expr_replacing_span(guard, target_span, replacement_expr);
                }
                arm.body = rewrite_expr_replacing_span(&arm.body, target_span, replacement_expr);
            }
        }
        hir::ExprKind::MemberAccess { receiver, .. } => {
            **receiver = rewrite_expr_replacing_span(receiver, target_span, replacement_expr);
        }
        hir::ExprKind::Call { callee, args } => {
            **callee = rewrite_expr_replacing_span(callee, target_span, replacement_expr);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(arg_expr) => {
                        *arg_expr =
                            rewrite_expr_replacing_span(arg_expr, target_span, replacement_expr);
                    }
                    hir::CallArg::Named { value, .. } => {
                        *value = rewrite_expr_replacing_span(value, target_span, replacement_expr);
                    }
                }
            }
        }
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(arg_expr) => {
                        *arg_expr =
                            rewrite_expr_replacing_span(arg_expr, target_span, replacement_expr);
                    }
                    hir::CallArg::Named { value, .. } => {
                        *value = rewrite_expr_replacing_span(value, target_span, replacement_expr);
                    }
                }
            }
        }
        hir::ExprKind::Handle(handle) => {
            for stmt in &mut handle.body.stmts {
                rewrite_stmt_replacing_expr_span(stmt, target_span, replacement_expr);
            }
            for arm in &mut handle.arms {
                arm.body = rewrite_expr_replacing_span(&arm.body, target_span, replacement_expr);
            }
            if let Some(finally_block) = handle.finally.as_mut() {
                for stmt in &mut finally_block.stmts {
                    rewrite_stmt_replacing_expr_span(stmt, target_span, replacement_expr);
                }
            }
        }
    }

    rewritten
}

pub(crate) fn rewrite_stmt_with_resume_slot(
    stmt: &mut hir::Stmt,
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
) {
    match &mut stmt.kind {
        hir::StmtKind::Expr(expr) => {
            *expr = rewrite_expr_with_resume_slot(expr, source_expr, resume_path, resume_slot);
        }
        hir::StmtKind::Val(decl) => {
            if let Some(init) = decl.init.as_mut() {
                *init = rewrite_expr_with_resume_slot(init, source_expr, resume_path, resume_slot);
            }
        }
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            *lhs = rewrite_expr_with_resume_slot(lhs, source_expr, resume_path, resume_slot);
            *rhs = rewrite_expr_with_resume_slot(rhs, source_expr, resume_path, resume_slot);
        }
        hir::StmtKind::While { cond, body } => {
            *cond = rewrite_expr_with_resume_slot(cond, source_expr, resume_path, resume_slot);
            for stmt in &mut body.stmts {
                rewrite_stmt_with_resume_slot(stmt, source_expr, resume_path, resume_slot);
            }
        }
        hir::StmtKind::Return { value } => {
            if let Some(expr) = value.as_mut() {
                *expr = rewrite_expr_with_resume_slot(expr, source_expr, resume_path, resume_slot);
            }
        }
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => {}
    }
}

pub(crate) fn rewrite_branch_condition_with_resume_slot(
    condition: &mut HandleBranchCondition,
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
) {
    match condition {
        HandleBranchCondition::WhileCond { condition }
        | HandleBranchCondition::IfCond { condition } => {
            **condition =
                rewrite_expr_with_resume_slot(condition, source_expr, resume_path, resume_slot);
        }
    }
}

pub(crate) fn rewrite_expr_with_resume_slot(
    expr: &hir::Expr,
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
) -> hir::Expr {
    if expr.span == source_expr.span {
        return make_resume_slot_var_expr(source_expr, resume_slot);
    }

    for start in 0..resume_path.expr_frames.len() {
        if resume_path_frame_matches_expr(&resume_path.expr_frames[start], expr) {
            return rewrite_expr_from_resume_path(
                expr,
                source_expr,
                &resume_path.expr_frames[start..],
                resume_slot,
            );
        }
    }

    expr.clone()
}

pub(crate) fn rewrite_expr_from_resume_path(
    expr: &hir::Expr,
    source_expr: &hir::Expr,
    expr_frames: &[SuspendResumeExprFrame],
    resume_slot: &FrameSlot,
) -> hir::Expr {
    if expr.span == source_expr.span {
        return make_resume_slot_var_expr(source_expr, resume_slot);
    }
    let Some(frame) = expr_frames.first() else {
        return expr.clone();
    };

    let mut rewritten = expr.clone();
    match (frame, &mut rewritten.kind) {
        (SuspendResumeExprFrame::CallCallee { call_span }, hir::ExprKind::Call { callee, .. })
            if rewritten.span == *call_span =>
        {
            **callee =
                rewrite_expr_from_resume_path(callee, source_expr, &expr_frames[1..], resume_slot);
        }
        (
            SuspendResumeExprFrame::CallArg {
                call_span,
                arg_index,
            },
            hir::ExprKind::Call { args, .. },
        ) if rewritten.span == *call_span => {
            if let Some(hir::CallArg::Positional(arg_expr)) = args.get_mut(*arg_index) {
                *arg_expr = rewrite_expr_from_resume_path(
                    arg_expr,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        (
            SuspendResumeExprFrame::NamedArgValue {
                call_span,
                arg_index,
                name_span,
            },
            hir::ExprKind::Call { args, .. },
        ) if rewritten.span == *call_span => {
            if let Some(hir::CallArg::Named {
                name_span: arg_name_span,
                value,
                ..
            }) = args.get_mut(*arg_index)
                && *arg_name_span == *name_span
            {
                *value = rewrite_expr_from_resume_path(
                    value,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        (
            SuspendResumeExprFrame::PerformArg {
                perform_span,
                arg_index,
            },
            hir::ExprKind::Perform { args, .. },
        ) if rewritten.span == *perform_span => {
            if let Some(arg) = args.get_mut(*arg_index) {
                match arg {
                    hir::CallArg::Positional(arg_expr) => {
                        *arg_expr = rewrite_expr_from_resume_path(
                            arg_expr,
                            source_expr,
                            &expr_frames[1..],
                            resume_slot,
                        );
                    }
                    hir::CallArg::Named { value, .. } => {
                        *value = rewrite_expr_from_resume_path(
                            value,
                            source_expr,
                            &expr_frames[1..],
                            resume_slot,
                        );
                    }
                }
            }
        }
        (
            SuspendResumeExprFrame::MemberReceiver { access_span },
            hir::ExprKind::MemberAccess { receiver, .. },
        ) if rewritten.span == *access_span => {
            **receiver = rewrite_expr_from_resume_path(
                receiver,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
        }
        (SuspendResumeExprFrame::BinaryLhs { binary_span }, hir::ExprKind::Binary { lhs, .. })
            if rewritten.span == *binary_span =>
        {
            **lhs = rewrite_expr_from_resume_path(lhs, source_expr, &expr_frames[1..], resume_slot);
        }
        (SuspendResumeExprFrame::BinaryRhs { binary_span }, hir::ExprKind::Binary { rhs, .. })
            if rewritten.span == *binary_span =>
        {
            **rhs = rewrite_expr_from_resume_path(rhs, source_expr, &expr_frames[1..], resume_slot);
        }
        (
            SuspendResumeExprFrame::StructField {
                struct_span,
                field_name,
            },
            hir::ExprKind::StructLit { fields, .. },
        ) if rewritten.span == *struct_span => {
            if let Some(field) = fields.iter_mut().find(|field| field.name == *field_name) {
                field.value = rewrite_expr_from_resume_path(
                    &field.value,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        (
            SuspendResumeExprFrame::TupleElement {
                tuple_span,
                element_index,
            },
            hir::ExprKind::TupleLit { elements },
        ) if rewritten.span == *tuple_span => {
            if let Some(element) = elements.get_mut(*element_index) {
                *element = rewrite_expr_from_resume_path(
                    element,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        (
            SuspendResumeExprFrame::InterpolatedExpr {
                string_span,
                part_index,
            },
            hir::ExprKind::InterpolatedString { parts, .. },
        ) if rewritten.span == *string_span => {
            if let Some(hir::InterpolatedStringPart::Expr { expr: part_expr }) =
                parts.get_mut(*part_index)
            {
                *part_expr = rewrite_expr_from_resume_path(
                    part_expr,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        (
            SuspendResumeExprFrame::UnaryOperand { expr_span },
            hir::ExprKind::Unary { expr: inner, .. },
        ) if rewritten.span == *expr_span => {
            **inner =
                rewrite_expr_from_resume_path(inner, source_expr, &expr_frames[1..], resume_slot);
        }
        (
            SuspendResumeExprFrame::CastOperand { expr_span },
            hir::ExprKind::Cast { expr: inner, .. },
        ) if rewritten.span == *expr_span => {
            **inner =
                rewrite_expr_from_resume_path(inner, source_expr, &expr_frames[1..], resume_slot);
        }
        (
            SuspendResumeExprFrame::TypeCheckOperand { expr_span },
            hir::ExprKind::TypeCheck { expr: inner, .. },
        ) if rewritten.span == *expr_span => {
            **inner =
                rewrite_expr_from_resume_path(inner, source_expr, &expr_frames[1..], resume_slot);
        }
        (SuspendResumeExprFrame::IfCond { if_span }, hir::ExprKind::If { cond, .. })
            if rewritten.span == *if_span =>
        {
            **cond =
                rewrite_expr_from_resume_path(cond, source_expr, &expr_frames[1..], resume_slot);
        }
        (SuspendResumeExprFrame::IfThenExpr { if_span }, hir::ExprKind::If { then_branch, .. })
            if rewritten.span == *if_span =>
        {
            **then_branch = rewrite_expr_from_resume_path(
                then_branch,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
        }
        (
            SuspendResumeExprFrame::IfElseExpr { if_span },
            hir::ExprKind::If {
                else_branch: Some(else_branch),
                ..
            },
        ) if rewritten.span == *if_span => {
            **else_branch = rewrite_expr_from_resume_path(
                else_branch,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
        }
        (
            SuspendResumeExprFrame::WhenSubject { when_span },
            hir::ExprKind::When { subject, .. },
        ) if rewritten.span == *when_span => {
            **subject =
                rewrite_expr_from_resume_path(subject, source_expr, &expr_frames[1..], resume_slot);
        }
        (
            SuspendResumeExprFrame::WhenArmGuard {
                when_span,
                arm_index,
            },
            hir::ExprKind::When { arms, .. },
        ) if rewritten.span == *when_span => {
            if let Some(arm) = arms.get_mut(*arm_index)
                && let Some(guard) = arm.guard.as_mut()
            {
                *guard = rewrite_expr_from_resume_path(
                    guard,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        (
            SuspendResumeExprFrame::WhenArmBody {
                when_span,
                arm_index,
            },
            hir::ExprKind::When { arms, .. },
        ) if rewritten.span == *when_span => {
            if let Some(arm) = arms.get_mut(*arm_index) {
                arm.body = rewrite_expr_from_resume_path(
                    &arm.body,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        _ => {}
    }

    rewritten
}

pub(crate) fn resume_path_frame_matches_expr(
    frame: &SuspendResumeExprFrame,
    expr: &hir::Expr,
) -> bool {
    match (frame, &expr.kind) {
        (SuspendResumeExprFrame::CallCallee { call_span }, hir::ExprKind::Call { .. })
        | (SuspendResumeExprFrame::CallArg { call_span, .. }, hir::ExprKind::Call { .. })
        | (SuspendResumeExprFrame::NamedArgValue { call_span, .. }, hir::ExprKind::Call { .. }) => {
            expr.span == *call_span
        }
        (
            SuspendResumeExprFrame::PerformArg { perform_span, .. },
            hir::ExprKind::Perform { .. },
        ) => expr.span == *perform_span,
        (
            SuspendResumeExprFrame::MemberReceiver { access_span },
            hir::ExprKind::MemberAccess { .. },
        ) => expr.span == *access_span,
        (SuspendResumeExprFrame::BinaryLhs { binary_span }, hir::ExprKind::Binary { .. })
        | (SuspendResumeExprFrame::BinaryRhs { binary_span }, hir::ExprKind::Binary { .. }) => {
            expr.span == *binary_span
        }
        (
            SuspendResumeExprFrame::StructField { struct_span, .. },
            hir::ExprKind::StructLit { .. },
        ) => expr.span == *struct_span,
        (
            SuspendResumeExprFrame::TupleElement { tuple_span, .. },
            hir::ExprKind::TupleLit { .. },
        ) => expr.span == *tuple_span,
        (
            SuspendResumeExprFrame::InterpolatedExpr { string_span, .. },
            hir::ExprKind::InterpolatedString { .. },
        ) => expr.span == *string_span,
        (SuspendResumeExprFrame::UnaryOperand { expr_span }, hir::ExprKind::Unary { .. })
        | (SuspendResumeExprFrame::CastOperand { expr_span }, hir::ExprKind::Cast { .. })
        | (
            SuspendResumeExprFrame::TypeCheckOperand { expr_span },
            hir::ExprKind::TypeCheck { .. },
        ) => expr.span == *expr_span,
        (SuspendResumeExprFrame::IfCond { if_span }, hir::ExprKind::If { .. })
        | (SuspendResumeExprFrame::IfThenExpr { if_span }, hir::ExprKind::If { .. })
        | (SuspendResumeExprFrame::IfElseExpr { if_span }, hir::ExprKind::If { .. }) => {
            expr.span == *if_span
        }
        (SuspendResumeExprFrame::WhenSubject { when_span }, hir::ExprKind::When { .. })
        | (SuspendResumeExprFrame::WhenArmGuard { when_span, .. }, hir::ExprKind::When { .. })
        | (SuspendResumeExprFrame::WhenArmBody { when_span, .. }, hir::ExprKind::When { .. }) => {
            expr.span == *when_span
        }
        _ => false,
    }
}

pub(crate) fn make_resume_slot_var_expr(
    source_expr: &hir::Expr,
    resume_slot: &FrameSlot,
) -> hir::Expr {
    hir::Expr {
        span: source_expr.span,
        ty: source_expr.ty,
        kind: hir::ExprKind::VarRef(hir::ValueRef::Local {
            id: resume_slot.id,
            name: resume_slot.name.clone(),
            decl_span: source_expr.span,
        }),
    }
}

pub(crate) fn next_synthetic_symbol_seed(
    handle: &hir::HandleExpr,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
) -> u32 {
    let mut ids = known_local_metadata.keys().copied().collect::<HashSet<_>>();
    for stmt in &handle.body.stmts {
        collect_declared_local_ids_in_stmt(stmt, &mut ids);
        collect_used_locals_in_stmt_static(stmt, &mut ids);
    }

    for arm in &handle.arms {
        for binder in &arm.op.binders {
            ids.insert(binder.id);
        }
        match arm.kind {
            hir::HandleArmKind::NonResuming => {}
            hir::HandleArmKind::EscapeContinuation { continuation } => {
                ids.insert(continuation);
            }
        }
        collect_declared_local_ids_in_expr(&arm.body, &mut ids);
        collect_used_locals_in_expr_static(&arm.body, &mut ids);
    }

    if let Some(finally_block) = handle.finally.as_ref() {
        for stmt in &finally_block.stmts {
            collect_declared_local_ids_in_stmt(stmt, &mut ids);
            collect_used_locals_in_stmt_static(stmt, &mut ids);
        }
    }

    ids.into_iter()
        .map(hir::SymbolId::as_u32)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

pub(crate) fn matching_arms(arms: &[ArmPlan], kind: &SuspendSiteKind) -> Vec<ArmPlanId> {
    match kind {
        SuspendSiteKind::Perform { op_fqn } => arms
            .iter()
            .filter(|arm| arm.op_fqn == *op_fqn)
            .map(|arm| arm.id)
            .collect(),
        SuspendSiteKind::RuntimeRaise { .. } => arms
            .iter()
            .filter(|arm| arm.op_fqn == "scoop.core.Raise.raise")
            .map(|arm| arm.id)
            .collect(),
        SuspendSiteKind::CallMaySuspend { .. }
        | SuspendSiteKind::CallStateMachineCallee { .. }
        | SuspendSiteKind::ObjectInitAccess { .. }
        | SuspendSiteKind::TopLevelValueInitAccess { .. }
        | SuspendSiteKind::ClassCtorInit { .. }
        | SuspendSiteKind::NestedHandleBoundary { .. } => Vec::new(),
    }
}

pub(crate) fn build_successor_map(states: &[PlanState]) -> HashMap<PlanStateId, Vec<PlanStateId>> {
    states
        .iter()
        .map(|state| {
            let succs = match &state.terminator {
                StateTerminator::Goto(next) => vec![*next],
                StateTerminator::Branch {
                    then_state,
                    else_state,
                    ..
                } => vec![*then_state, *else_state],
                StateTerminator::CleanupEnter { next_state, .. } => vec![*next_state],
                StateTerminator::Suspend { site_id } => {
                    let _ = site_id;
                    Vec::new()
                }
                StateTerminator::ReturnHandle
                | StateTerminator::ReturnFromFunction
                | StateTerminator::ArmExit(_) => Vec::new(),
            };
            (state.id, succs)
        })
        .collect()
}

pub(crate) fn reachable_states(
    start: PlanStateId,
    successors: &HashMap<PlanStateId, Vec<PlanStateId>>,
) -> HashSet<PlanStateId> {
    let mut seen = HashSet::new();
    let mut stack = vec![start];
    while let Some(state) = stack.pop() {
        if !seen.insert(state) {
            continue;
        }
        if let Some(nexts) = successors.get(&state) {
            stack.extend(nexts.iter().copied());
        }
    }
    seen
}

pub(crate) fn extract_tail_resume_payload_expr(
    expr: &hir::Expr,
    continuation_symbol: hir::SymbolId,
) -> Option<&hir::Expr> {
    let hir::ExprKind::Call { callee, args } = &expr.kind else {
        return None;
    };
    let hir::ExprKind::MemberAccess { receiver, member } = &callee.kind else {
        return None;
    };
    let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind else {
        return None;
    };
    if *id != continuation_symbol || member.name != "resume" {
        return None;
    }

    match args.as_slice() {
        [hir::CallArg::Positional(payload)] => Some(payload),
        [hir::CallArg::Named { value, .. }] => Some(value),
        _ => None,
    }
}

pub(crate) fn tail_resume_arm_matches_static(
    expr: &hir::Expr,
    continuation_symbol: hir::SymbolId,
) -> bool {
    if extract_tail_resume_payload_expr(expr, continuation_symbol).is_some() {
        return true;
    }

    match &expr.kind {
        hir::ExprKind::Block(block) => block
            .stmts
            .last()
            .is_some_and(|stmt| matches!(&stmt.kind, hir::StmtKind::Expr(expr) if tail_resume_arm_matches_static(expr, continuation_symbol))),
        hir::ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            tail_resume_arm_matches_static(then_branch, continuation_symbol)
                && else_branch
                    .as_deref()
                    .is_some_and(|expr| tail_resume_arm_matches_static(expr, continuation_symbol))
        }
        hir::ExprKind::When { arms, .. } => {
            !arms.is_empty()
                && arms
                    .iter()
                    .all(|arm| tail_resume_arm_matches_static(&arm.body, continuation_symbol))
        }
        _ => false,
    }
}

pub(crate) fn try_extract_callee_fqn(callee: &hir::Expr) -> Option<String> {
    match &callee.kind {
        hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => Some(fqn.clone()),
        hir::ExprKind::MemberAccess { member, .. } => match member.resolved.as_ref()? {
            hir::MemberRef::Fun { fqn, .. } | hir::MemberRef::ExtensionFun { fqn, .. } => {
                Some(fqn.clone())
            }
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn resolve_plan_expr_concrete_type(
    context: &HandlePlanContext,
    types: &TypeStore,
    expr: &hir::Expr,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
) -> Option<TypeId> {
    ExprFactResolver::new(types, context.hir_facts.as_ref(), |id| {
        known_local_metadata.get(&id).map(|metadata| metadata.ty)
    })
    .resolve_expr_concrete_type(expr)
}

pub(crate) fn collect_outer_scope_slots(
    handle: &hir::HandleExpr,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
) -> Vec<FrameSlot> {
    let mut declared = HashSet::new();
    for stmt in &handle.body.stmts {
        collect_declared_local_ids_in_stmt(stmt, &mut declared);
    }
    for arm in &handle.arms {
        for binder in &arm.op.binders {
            declared.insert(binder.id);
        }
        match arm.kind {
            hir::HandleArmKind::NonResuming => {}
            hir::HandleArmKind::EscapeContinuation { continuation } => {
                declared.insert(continuation);
            }
        }
        collect_declared_local_ids_in_expr(&arm.body, &mut declared);
    }
    if let Some(finally_block) = handle.finally.as_ref() {
        for stmt in &finally_block.stmts {
            collect_declared_local_ids_in_stmt(stmt, &mut declared);
        }
    }

    let mut used = HashMap::new();
    for stmt in &handle.body.stmts {
        collect_local_refs_in_stmt(stmt, &mut used);
    }
    for arm in &handle.arms {
        collect_local_refs_in_expr(&arm.body, &mut used);
    }
    if let Some(finally_block) = handle.finally.as_ref() {
        for stmt in &finally_block.stmts {
            collect_local_refs_in_stmt(stmt, &mut used);
        }
    }

    let mut slots = used
        .into_iter()
        .filter(|(id, _)| !declared.contains(id))
        .map(|(id, (name, ty))| {
            let metadata = known_local_metadata.get(&id).copied();
            FrameSlot {
                id,
                name,
                ty: metadata.map_or(ty, |meta| meta.ty),
                mutable: metadata.is_some_and(|meta| meta.mutable),
                seed_from_outer_scope: true,
                owner_arm: None,
            }
        })
        .collect::<Vec<_>>();
    slots.sort_by_key(|slot| slot.id.as_u32());
    slots
}
