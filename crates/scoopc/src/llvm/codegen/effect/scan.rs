#[allow(dead_code)]
trait ScanStmtPathFrame {
    fn set_stmt_idx(&mut self, idx: usize);
}

impl<'hir> ScanStmtPathFrame for ImmediateResumeFrame<'hir> {
    fn set_stmt_idx(&mut self, idx: usize) {
        ImmediateResumeFrame::set_stmt_idx(self, idx);
    }
}

impl<'hir> ScanStmtPathFrame for MixedEscapeDirectFrame<'hir> {
    fn set_stmt_idx(&mut self, idx: usize) {
        MixedEscapeDirectFrame::set_stmt_idx(self, idx);
    }
}

#[allow(dead_code)]
trait PathScanState<Frame> {
    fn path_mut(&mut self) -> &mut Vec<Frame>;
}

#[allow(dead_code)]
fn scan_stmt_slice_with_state<'hir, Frame, State, F>(
    state: &mut State,
    stmts: &'hir [hir::Stmt],
    mut visit: F,
) -> Result<(), LlvmEmitError>
where
    Frame: ScanStmtPathFrame,
    State: PathScanState<Frame>,
    F: FnMut(&mut State, usize, &'hir hir::Stmt) -> Result<(), LlvmEmitError>,
{
    for (idx, stmt) in stmts.iter().enumerate() {
        if let Some(frame) = state.path_mut().last_mut() {
            frame.set_stmt_idx(idx);
        }
        visit(state, idx, stmt)?;
    }
    Ok(())
}

#[allow(dead_code)]
fn with_scoped_scan_frame<State, Frame, T, F>(
    state: &mut State,
    frame: Frame,
    visit: F,
) -> Result<T, LlvmEmitError>
where
    State: PathScanState<Frame>,
    F: FnOnce(&mut State) -> Result<T, LlvmEmitError>,
{
    state.path_mut().push(frame);
    let result = visit(state);
    state.path_mut().pop();
    result
}

#[allow(dead_code)]
impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn immediate_resume_expr_contains_matching_direct_perform(
        &self,
        expr: &hir::Expr,
        arm_op_fqn: &str,
    ) -> bool {
        match &expr.kind {
            hir::ExprKind::Perform { op, .. } => op.fqn == arm_op_fqn,
            hir::ExprKind::Block(block) => {
                self.immediate_resume_block_contains_matching_direct_perform(block, arm_op_fqn)
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.immediate_resume_expr_contains_matching_direct_perform(cond, arm_op_fqn)
                    || self.immediate_resume_expr_contains_matching_direct_perform(
                        then_branch,
                        arm_op_fqn,
                    )
                    || else_branch.as_ref().is_some_and(|expr| {
                        self.immediate_resume_expr_contains_matching_direct_perform(
                            expr, arm_op_fqn,
                        )
                    })
            }
            hir::ExprKind::Call { callee, args } => {
                if self.immediate_resume_expr_contains_matching_direct_perform(callee, arm_op_fqn) {
                    return true;
                }
                args.iter().any(|arg| match arg {
                    hir::CallArg::Positional(expr) => self
                        .immediate_resume_expr_contains_matching_direct_perform(expr, arm_op_fqn),
                    hir::CallArg::Named { value, .. } => self
                        .immediate_resume_expr_contains_matching_direct_perform(value, arm_op_fqn),
                })
            }
            hir::ExprKind::StructLit { fields, .. } => fields.iter().any(|field| {
                self.immediate_resume_expr_contains_matching_direct_perform(
                    &field.value,
                    arm_op_fqn,
                )
            }),
            hir::ExprKind::TupleLit { elements } => elements.iter().any(|expr| {
                self.immediate_resume_expr_contains_matching_direct_perform(expr, arm_op_fqn)
            }),
            hir::ExprKind::InterpolatedString { parts, .. } => {
                parts.iter().any(|part| match part {
                    hir::InterpolatedStringPart::Text { .. } => false,
                    hir::InterpolatedStringPart::Expr { expr } => self
                        .immediate_resume_expr_contains_matching_direct_perform(expr, arm_op_fqn),
                })
            }
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::Cast { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. }
            | hir::ExprKind::MemberAccess {
                receiver: inner, ..
            } => self.immediate_resume_expr_contains_matching_direct_perform(inner, arm_op_fqn),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.immediate_resume_expr_contains_matching_direct_perform(lhs, arm_op_fqn)
                    || self.immediate_resume_expr_contains_matching_direct_perform(rhs, arm_op_fqn)
            }
            hir::ExprKind::When { subject, arms } => {
                self.immediate_resume_expr_contains_matching_direct_perform(subject, arm_op_fqn)
                    || arms.iter().any(|arm| {
                        arm.guard.as_ref().is_some_and(|guard| {
                            self.immediate_resume_expr_contains_matching_direct_perform(
                                guard, arm_op_fqn,
                            )
                        }) || self.immediate_resume_expr_contains_matching_direct_perform(
                            &arm.body, arm_op_fqn,
                        )
                    })
            }
            hir::ExprKind::Closure(_) => false,
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Handle(_)
            | hir::ExprKind::Todo(_) => false,
        }
    }

    fn immediate_resume_stmt_contains_matching_direct_perform(
        &self,
        stmt: &hir::Stmt,
        arm_op_fqn: &str,
    ) -> bool {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => false,
            hir::StmtKind::Expr(expr) => {
                self.immediate_resume_expr_contains_matching_direct_perform(expr, arm_op_fqn)
            }
            hir::StmtKind::Val(decl) => decl.init.as_ref().is_some_and(|init| {
                self.immediate_resume_expr_contains_matching_direct_perform(init, arm_op_fqn)
            }),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.immediate_resume_expr_contains_matching_direct_perform(lhs, arm_op_fqn)
                    || self.immediate_resume_expr_contains_matching_direct_perform(rhs, arm_op_fqn)
            }
            hir::StmtKind::Return { value } => value.as_ref().is_some_and(|expr| {
                self.immediate_resume_expr_contains_matching_direct_perform(expr, arm_op_fqn)
            }),
            hir::StmtKind::While { cond, body } => {
                self.immediate_resume_expr_contains_matching_direct_perform(cond, arm_op_fqn)
                    || self
                        .immediate_resume_block_contains_matching_direct_perform(body, arm_op_fqn)
            }
        }
    }

    fn immediate_resume_block_contains_matching_direct_perform(
        &self,
        block: &hir::Block,
        arm_op_fqn: &str,
    ) -> bool {
        block.stmts.iter().any(|stmt| {
            self.immediate_resume_stmt_contains_matching_direct_perform(stmt, arm_op_fqn)
        })
    }

    fn scan_immediate_resume_site<'hir>(
        &self,
        handle: &'hir hir::HandleExpr,
        arm_op_fqn: &str,
    ) -> Result<Option<ImmediateResumeSite<'hir>>, LlvmEmitError> {
        struct ScanState<'hir> {
            path: Vec<ImmediateResumeFrame<'hir>>,
            site: Option<ImmediateResumeSite<'hir>>,
        }

        impl<'hir> PathScanState<ImmediateResumeFrame<'hir>> for ScanState<'hir> {
            fn path_mut(&mut self) -> &mut Vec<ImmediateResumeFrame<'hir>> {
                &mut self.path
            }
        }

        impl<'hir> ScanState<'hir> {
            fn record_site(
                &mut self,
                decl: &'hir hir::ValDecl,
                op: &'hir hir::EffectOpRef,
                args: &'hir [hir::CallArg],
                top_level_stmt_idx: usize,
            ) -> Result<(), LlvmEmitError> {
                if self.site.is_some() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle resume body (multiple perform points)",
                        at: decl.span.into(),
                    });
                }
                let Some(id) = decl.id else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle resume perform binding id",
                        at: decl.span.into(),
                    });
                };
                self.site = Some(ImmediateResumeSite {
                    top_level_stmt_idx,
                    decl,
                    op,
                    args,
                    id,
                    resume_path: self.path.clone(),
                });
                Ok(())
            }

            fn scan_stmts(
                &mut self,
                cg: &MainCodegen<'_, '_>,
                stmts: &'hir [hir::Stmt],
                arm_op_fqn: &str,
                top_level_stmt_idx: usize,
            ) -> Result<(), LlvmEmitError> {
                scan_stmt_slice_with_state(self, stmts, |state, _, stmt| match &stmt.kind {
                    hir::StmtKind::Empty => Ok(()),
                    hir::StmtKind::Val(decl) => {
                        let Some(init) = decl.init.as_ref() else {
                            return Ok(());
                        };
                        if let hir::ExprKind::Perform { op, args } = &init.kind
                            && op.fqn == arm_op_fqn
                        {
                            state.record_site(decl, op, args.as_slice(), top_level_stmt_idx)?;
                            return Ok(());
                        }
                        if cg.immediate_resume_expr_contains_matching_direct_perform(
                            init, arm_op_fqn,
                        ) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle resume body (nested value expression not yet supported)",
                                at: init.span.into(),
                            });
                        }
                        Ok(())
                    }
                    hir::StmtKind::Assign { lhs, rhs, .. } => {
                        if cg.immediate_resume_expr_contains_matching_direct_perform(
                            lhs, arm_op_fqn,
                        ) || cg.immediate_resume_expr_contains_matching_direct_perform(
                            rhs, arm_op_fqn,
                        ) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle resume body (nested value expression not yet supported)",
                                at: stmt.span.into(),
                            });
                        }
                        Ok(())
                    }
                    hir::StmtKind::Expr(expr) => {
                        state.scan_expr_stmt(cg, expr, arm_op_fqn, top_level_stmt_idx)
                    }
                    hir::StmtKind::While { cond, body } => {
                        if cg.immediate_resume_expr_contains_matching_direct_perform(
                            cond, arm_op_fqn,
                        ) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle resume body (while condition with perform not yet supported)",
                                at: stmt.span.into(),
                            });
                        }
                        for (body_idx, body_stmt) in body.stmts.iter().enumerate() {
                            match &body_stmt.kind {
                                hir::StmtKind::Empty => {}
                                hir::StmtKind::Val(decl) => {
                                    let Some(init) = decl.init.as_ref() else {
                                        continue;
                                    };
                                    if let hir::ExprKind::Perform { op, args } = &init.kind
                                        && op.fqn == arm_op_fqn
                                    {
                                        with_scoped_scan_frame(
                                            state,
                                            ImmediateResumeFrame::WhileBody {
                                                while_cond: cond,
                                                while_body: body,
                                                stmt_idx: body_idx,
                                            },
                                            |state| {
                                                state.record_site(
                                                    decl,
                                                    op,
                                                    args.as_slice(),
                                                    top_level_stmt_idx,
                                                )
                                            },
                                        )?;
                                        continue;
                                    }
                                    if cg.immediate_resume_expr_contains_matching_direct_perform(
                                        init, arm_op_fqn,
                                    ) {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle resume body (nested perform in while body not yet supported)",
                                            at: init.span.into(),
                                        });
                                    }
                                }
                                hir::StmtKind::Assign { lhs, rhs, .. } => {
                                    if cg.immediate_resume_expr_contains_matching_direct_perform(
                                        lhs, arm_op_fqn,
                                    ) || cg.immediate_resume_expr_contains_matching_direct_perform(
                                        rhs, arm_op_fqn,
                                    ) {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle resume body (nested perform in while body not yet supported)",
                                            at: body_stmt.span.into(),
                                        });
                                    }
                                }
                                hir::StmtKind::Expr(expr) => {
                                    if let hir::ExprKind::Perform { op, .. } = &expr.kind
                                        && op.fqn == arm_op_fqn
                                    {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle resume body (perform must be bound to val)",
                                            at: expr.span.into(),
                                        });
                                    }
                                    if cg.immediate_resume_expr_contains_matching_direct_perform(
                                        expr, arm_op_fqn,
                                    ) {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle resume body (nested perform in while body not yet supported)",
                                            at: expr.span.into(),
                                        });
                                    }
                                }
                                hir::StmtKind::While { cond, body } => {
                                    if cg.immediate_resume_expr_contains_matching_direct_perform(
                                        cond, arm_op_fqn,
                                    ) || cg.immediate_resume_block_contains_matching_direct_perform(
                                        body, arm_op_fqn,
                                    ) {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle resume body (nested perform in while body not yet supported)",
                                            at: body_stmt.span.into(),
                                        });
                                    }
                                }
                                hir::StmtKind::Return { value } => {
                                    if value.as_ref().is_some_and(|expr| {
                                        cg.immediate_resume_expr_contains_matching_direct_perform(
                                            expr, arm_op_fqn,
                                        )
                                    }) {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle resume body (`return` with perform not yet supported)",
                                            at: body_stmt.span.into(),
                                        });
                                    }
                                }
                                hir::StmtKind::Break { .. }
                                | hir::StmtKind::Continue { .. }
                                | hir::StmtKind::Todo(_) => {}
                            }
                        }
                        Ok(())
                    }
                    hir::StmtKind::Return { value } => {
                        if value.as_ref().is_some_and(|expr| {
                            cg.immediate_resume_expr_contains_matching_direct_perform(
                                expr, arm_op_fqn,
                            )
                        }) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle resume body (`return` with perform not yet supported)",
                                at: stmt.span.into(),
                            });
                        }
                        Ok(())
                    }
                    hir::StmtKind::Break { .. }
                    | hir::StmtKind::Continue { .. }
                    | hir::StmtKind::Todo(_) => Ok(()),
                })
            }

            fn scan_expr_stmt(
                &mut self,
                cg: &MainCodegen<'_, '_>,
                expr: &'hir hir::Expr,
                arm_op_fqn: &str,
                top_level_stmt_idx: usize,
            ) -> Result<(), LlvmEmitError> {
                match &expr.kind {
                    hir::ExprKind::Perform { op, .. } if op.fqn == arm_op_fqn => {
                        Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (perform must be bound to val)",
                            at: expr.span.into(),
                        })
                    }
                    hir::ExprKind::Block(block) => with_scoped_scan_frame(
                        self,
                        ImmediateResumeFrame::Block { block, stmt_idx: 0 },
                        |state| state.scan_stmts(cg, &block.stmts, arm_op_fqn, top_level_stmt_idx),
                    ),
                    hir::ExprKind::If {
                        cond,
                        then_branch,
                        else_branch,
                    } => {
                        if cg.immediate_resume_expr_contains_matching_direct_perform(
                            cond, arm_op_fqn,
                        ) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle resume body (if condition with perform not yet supported)",
                                at: cond.span.into(),
                            });
                        }

                        if let hir::ExprKind::Block(block) = &then_branch.kind {
                            if cg.immediate_resume_block_contains_matching_direct_perform(
                                block, arm_op_fqn,
                            ) {
                                with_scoped_scan_frame(
                                    self,
                                    ImmediateResumeFrame::IfThen {
                                        if_expr: expr,
                                        then_block: block,
                                        stmt_idx: 0,
                                    },
                                    |state| {
                                        state.scan_stmts(
                                            cg,
                                            &block.stmts,
                                            arm_op_fqn,
                                            top_level_stmt_idx,
                                        )
                                    },
                                )?;
                            }
                        } else if cg.immediate_resume_expr_contains_matching_direct_perform(
                            then_branch,
                            arm_op_fqn,
                        ) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle resume body (if branch value with perform not yet supported)",
                                at: then_branch.span.into(),
                            });
                        }

                        if let Some(else_expr) = else_branch.as_deref() {
                            if let hir::ExprKind::Block(block) = &else_expr.kind {
                                if cg.immediate_resume_block_contains_matching_direct_perform(
                                    block, arm_op_fqn,
                                ) {
                                    with_scoped_scan_frame(
                                        self,
                                        ImmediateResumeFrame::IfElse {
                                            if_expr: expr,
                                            else_block: block,
                                            stmt_idx: 0,
                                        },
                                        |state| {
                                            state.scan_stmts(
                                                cg,
                                                &block.stmts,
                                                arm_op_fqn,
                                                top_level_stmt_idx,
                                            )
                                        },
                                    )?;
                                }
                            } else if cg.immediate_resume_expr_contains_matching_direct_perform(
                                else_expr, arm_op_fqn,
                            ) {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle resume body (if branch value with perform not yet supported)",
                                    at: else_expr.span.into(),
                                });
                            }
                        }

                        Ok(())
                    }
                    _ => {
                        if cg.immediate_resume_expr_contains_matching_direct_perform(
                            expr, arm_op_fqn,
                        ) {
                            Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle resume body (nested value expression not yet supported)",
                                at: expr.span.into(),
                            })
                        } else {
                            Ok(())
                        }
                    }
                }
            }
        }

        let mut state = ScanState {
            path: Vec::new(),
            site: None,
        };
        for (top_idx, stmt) in handle.body.stmts.iter().enumerate() {
            state.scan_stmts(self, std::slice::from_ref(stmt), arm_op_fqn, top_idx)?;
        }
        Ok(state.site)
    }

    fn scan_mixed_escape_direct_sites<'hir>(
        &self,
        handle: &'hir hir::HandleExpr,
        arm_op_fqn: &str,
    ) -> Result<Vec<MixedEscapeDirectSite<'hir>>, LlvmEmitError> {
        struct ScanState<'hir> {
            path: Vec<MixedEscapeDirectFrame<'hir>>,
            sites: Vec<MixedEscapeDirectSite<'hir>>,
        }

        impl<'hir> PathScanState<MixedEscapeDirectFrame<'hir>> for ScanState<'hir> {
            fn path_mut(&mut self) -> &mut Vec<MixedEscapeDirectFrame<'hir>> {
                &mut self.path
            }
        }

        impl<'hir> ScanState<'hir> {
            fn record_site(
                &mut self,
                decl: &'hir hir::ValDecl,
                _op: &'hir hir::EffectOpRef,
                args: &'hir [hir::CallArg],
                top_level_stmt_idx: usize,
            ) -> Result<(), LlvmEmitError> {
                let Some(id) = decl.id else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation perform binding id",
                        at: decl.span.into(),
                    });
                };
                self.sites.push(MixedEscapeDirectSite {
                    top_level_stmt_idx,
                    decl,
                    args,
                    id,
                    resume_path: self.path.clone(),
                });
                Ok(())
            }

            fn scan_stmts(
                &mut self,
                cg: &MainCodegen<'_, '_>,
                stmts: &'hir [hir::Stmt],
                arm_op_fqn: &str,
                top_level_stmt_idx: usize,
            ) -> Result<(), LlvmEmitError> {
                scan_stmt_slice_with_state(self, stmts, |state, _, stmt| match &stmt.kind {
                    hir::StmtKind::Empty
                    | hir::StmtKind::Break { .. }
                    | hir::StmtKind::Continue { .. }
                    | hir::StmtKind::Todo(_) => Ok(()),
                    hir::StmtKind::Val(decl) => {
                        let Some(init) = decl.init.as_ref() else {
                            return Ok(());
                        };
                        if let hir::ExprKind::Perform { op, args } = &init.kind
                            && op.fqn == arm_op_fqn
                        {
                            state.record_site(decl, op, args.as_slice(), top_level_stmt_idx)?;
                            return Ok(());
                        }
                        if cg.immediate_resume_expr_contains_matching_direct_perform(
                            init, arm_op_fqn,
                        ) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (only top-level val-bound direct perform supported)",
                                at: init.span.into(),
                            });
                        }
                        Ok(())
                    }
                    hir::StmtKind::Assign { lhs, rhs, .. } => {
                        if cg.immediate_resume_expr_contains_matching_direct_perform(
                            lhs, arm_op_fqn,
                        ) || cg.immediate_resume_expr_contains_matching_direct_perform(
                            rhs, arm_op_fqn,
                        ) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (only top-level val-bound direct perform supported)",
                                at: stmt.span.into(),
                            });
                        }
                        Ok(())
                    }
                    hir::StmtKind::Expr(expr) => {
                        state.scan_expr_stmt(cg, expr, arm_op_fqn, top_level_stmt_idx)
                    }
                    hir::StmtKind::While { cond, body } => {
                        if cg.immediate_resume_expr_contains_matching_direct_perform(
                            cond, arm_op_fqn,
                        ) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (while condition with direct perform not yet supported)",
                                at: cond.span.into(),
                            });
                        }
                        if cg.immediate_resume_block_contains_matching_direct_perform(
                            body, arm_op_fqn,
                        ) {
                            with_scoped_scan_frame(
                                state,
                                MixedEscapeDirectFrame::WhileBody {
                                    while_cond: cond,
                                    while_body: body,
                                    stmt_idx: 0,
                                },
                                |state| {
                                    state.scan_stmts(
                                        cg,
                                        &body.stmts,
                                        arm_op_fqn,
                                        top_level_stmt_idx,
                                    )
                                },
                            )?;
                        }
                        Ok(())
                    }
                    hir::StmtKind::Return { value } => {
                        if value.as_ref().is_some_and(|expr| {
                            cg.immediate_resume_expr_contains_matching_direct_perform(
                                expr, arm_op_fqn,
                            )
                        }) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (only top-level val-bound direct perform supported)",
                                at: stmt.span.into(),
                            });
                        }
                        Ok(())
                    }
                })
            }

            fn scan_expr_stmt(
                &mut self,
                cg: &MainCodegen<'_, '_>,
                expr: &'hir hir::Expr,
                arm_op_fqn: &str,
                top_level_stmt_idx: usize,
            ) -> Result<(), LlvmEmitError> {
                match &expr.kind {
                    hir::ExprKind::Perform { op, .. } if op.fqn == arm_op_fqn => {
                        Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (perform must be bound to val)",
                            at: expr.span.into(),
                        })
                    }
                    hir::ExprKind::Block(block) => with_scoped_scan_frame(
                        self,
                        MixedEscapeDirectFrame::Block { block, stmt_idx: 0 },
                        |state| state.scan_stmts(cg, &block.stmts, arm_op_fqn, top_level_stmt_idx),
                    ),
                    hir::ExprKind::If {
                        cond,
                        then_branch,
                        else_branch,
                    } => {
                        if cg.immediate_resume_expr_contains_matching_direct_perform(
                            cond, arm_op_fqn,
                        ) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (if condition with direct perform not yet supported)",
                                at: cond.span.into(),
                            });
                        }

                        if let hir::ExprKind::Block(block) = &then_branch.kind {
                            if cg.immediate_resume_block_contains_matching_direct_perform(
                                block, arm_op_fqn,
                            ) {
                                with_scoped_scan_frame(
                                    self,
                                    MixedEscapeDirectFrame::IfThen {
                                        if_expr: expr,
                                        then_block: block,
                                        stmt_idx: 0,
                                    },
                                    |state| {
                                        state.scan_stmts(
                                            cg,
                                            &block.stmts,
                                            arm_op_fqn,
                                            top_level_stmt_idx,
                                        )
                                    },
                                )?;
                            }
                        } else if cg.immediate_resume_expr_contains_matching_direct_perform(
                            then_branch,
                            arm_op_fqn,
                        ) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (if branch value with direct perform not yet supported)",
                                at: then_branch.span.into(),
                            });
                        }

                        if let Some(else_expr) = else_branch.as_deref() {
                            if let hir::ExprKind::Block(block) = &else_expr.kind {
                                if cg.immediate_resume_block_contains_matching_direct_perform(
                                    block, arm_op_fqn,
                                ) {
                                    with_scoped_scan_frame(
                                        self,
                                        MixedEscapeDirectFrame::IfElse {
                                            if_expr: expr,
                                            else_block: block,
                                            stmt_idx: 0,
                                        },
                                        |state| {
                                            state.scan_stmts(
                                                cg,
                                                &block.stmts,
                                                arm_op_fqn,
                                                top_level_stmt_idx,
                                            )
                                        },
                                    )?;
                                }
                            } else if cg.immediate_resume_expr_contains_matching_direct_perform(
                                else_expr, arm_op_fqn,
                            ) {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (if branch value with direct perform not yet supported)",
                                    at: else_expr.span.into(),
                                });
                            }
                        }

                        Ok(())
                    }
                    _ => {
                        if cg.immediate_resume_expr_contains_matching_direct_perform(
                            expr, arm_op_fqn,
                        ) {
                            Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (only top-level val-bound direct perform supported)",
                                at: expr.span.into(),
                            })
                        } else {
                            Ok(())
                        }
                    }
                }
            }
        }

        let mut state = ScanState {
            path: Vec::new(),
            sites: Vec::new(),
        };
        for (top_idx, stmt) in handle.body.stmts.iter().enumerate() {
            state.scan_stmts(self, std::slice::from_ref(stmt), arm_op_fqn, top_idx)?;
        }

        let mut seen_nested_block_stmt_idx: HashSet<usize> = HashSet::new();
        let mut seen_if_then_stmt_idx: HashSet<usize> = HashSet::new();
        let mut seen_if_else_stmt_idx: HashSet<usize> = HashSet::new();
        let mut seen_while_stmt_idx: HashSet<usize> = HashSet::new();
        for site in &state.sites {
            let Some(first_frame) = site.resume_path.first() else {
                continue;
            };
            match first_frame {
                MixedEscapeDirectFrame::Block { .. } => {
                    if !seen_nested_block_stmt_idx.insert(site.top_level_stmt_idx) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple nested block direct sites per top-level statement not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
                MixedEscapeDirectFrame::IfThen { .. } => {
                    if !seen_if_then_stmt_idx.insert(site.top_level_stmt_idx) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple direct sites in the same if-then branch not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
                MixedEscapeDirectFrame::IfElse { .. } => {
                    if !seen_if_else_stmt_idx.insert(site.top_level_stmt_idx) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple direct sites in the same if-else branch not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
                MixedEscapeDirectFrame::WhileBody { .. } => {
                    if !Self::mixed_escape_while_nested_path_supported(&site.resume_path) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (deeper nested direct site in while body not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                    if !seen_while_stmt_idx.insert(site.top_level_stmt_idx) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple direct sites in the same while body not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
            }
        }

        Ok(state.sites)
    }

    fn expr_is_indirect_perform_call_site_candidate(&self, expr: &hir::Expr) -> bool {
        if let hir::ExprKind::Call { callee, .. } = &expr.kind
            && let Some(fqn) = self.try_extract_callee_fqn(callee)
            && let Some(fun) = self.fun_index.get(fqn)
        {
            return !self.fun_ty_effects_is_pure(fun.ty).unwrap_or(false);
        }
        false
    }

    fn expr_contains_nested_indirect_perform_call_site(&self, expr: &hir::Expr) -> bool {
        match &expr.kind {
            hir::ExprKind::Call { .. } => self.expr_is_indirect_perform_call_site_candidate(expr),
            hir::ExprKind::Block(block) => {
                self.block_contains_nested_indirect_perform_call_site(block)
            }
            hir::ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.expr_contains_nested_indirect_perform_call_site(then_branch)
                    || else_branch.as_deref().is_some_and(|else_expr| {
                        self.expr_contains_nested_indirect_perform_call_site(else_expr)
                    })
            }
            _ => false,
        }
    }

    fn stmt_contains_nested_indirect_perform_call_site(&self, stmt: &hir::Stmt) -> bool {
        match &stmt.kind {
            hir::StmtKind::Val(decl) => decl.init.as_ref().is_some_and(|init| {
                self.expr_is_indirect_perform_call_site_candidate(init)
                    || self.expr_contains_nested_indirect_perform_call_site(init)
            }),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.expr_contains_nested_indirect_perform_call_site(lhs)
                    || self.expr_contains_nested_indirect_perform_call_site(rhs)
            }
            hir::StmtKind::Expr(expr) => self.expr_contains_nested_indirect_perform_call_site(expr),
            hir::StmtKind::While { body, .. } => {
                self.block_contains_nested_indirect_perform_call_site(body)
            }
            hir::StmtKind::Return { value } => value
                .as_ref()
                .is_some_and(|expr| self.expr_contains_nested_indirect_perform_call_site(expr)),
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => false,
        }
    }

    fn block_contains_nested_indirect_perform_call_site(&self, block: &hir::Block) -> bool {
        block
            .stmts
            .iter()
            .any(|stmt| self.stmt_contains_nested_indirect_perform_call_site(stmt))
    }

    fn scan_mixed_escape_indirect_sites<'hir>(
        &self,
        handle: &'hir hir::HandleExpr,
    ) -> Result<Vec<MixedEscapeIndirectSite<'hir>>, LlvmEmitError> {
        struct ScanState<'hir> {
            path: Vec<MixedEscapeDirectFrame<'hir>>,
            sites: Vec<MixedEscapeIndirectSite<'hir>>,
        }

        impl<'hir> PathScanState<MixedEscapeDirectFrame<'hir>> for ScanState<'hir> {
            fn path_mut(&mut self) -> &mut Vec<MixedEscapeDirectFrame<'hir>> {
                &mut self.path
            }
        }

        impl<'hir> ScanState<'hir> {
            fn record_site(
                &mut self,
                decl: &'hir hir::ValDecl,
                init: &'hir hir::Expr,
                top_level_stmt_idx: usize,
            ) -> Result<(), LlvmEmitError> {
                let Some(id) = decl.id else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation perform binding id",
                        at: decl.span.into(),
                    });
                };
                self.sites.push(MixedEscapeIndirectSite {
                    top_level_stmt_idx,
                    decl,
                    init,
                    id,
                    resume_path: self.path.clone(),
                });
                Ok(())
            }

            fn scan_stmts(
                &mut self,
                cg: &MainCodegen<'_, '_>,
                stmts: &'hir [hir::Stmt],
                top_level_stmt_idx: usize,
            ) -> Result<(), LlvmEmitError> {
                scan_stmt_slice_with_state(self, stmts, |state, _, stmt| match &stmt.kind {
                    hir::StmtKind::Empty
                    | hir::StmtKind::Break { .. }
                    | hir::StmtKind::Continue { .. }
                    | hir::StmtKind::Todo(_) => Ok(()),
                    hir::StmtKind::Val(decl) => {
                        let Some(init) = decl.init.as_ref() else {
                            return Ok(());
                        };
                        if cg.expr_is_indirect_perform_call_site_candidate(init) {
                            state.record_site(decl, init, top_level_stmt_idx)?;
                            return Ok(());
                        }
                        if cg.expr_contains_nested_indirect_perform_call_site(init) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (only statement-position nested block indirect call site supported)",
                                at: init.span.into(),
                            });
                        }
                        Ok(())
                    }
                    hir::StmtKind::Assign { lhs, rhs, .. } => {
                        if cg.expr_contains_nested_indirect_perform_call_site(lhs)
                            || cg.expr_contains_nested_indirect_perform_call_site(rhs)
                        {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (only statement-position nested block indirect call site supported)",
                                at: stmt.span.into(),
                            });
                        }
                        Ok(())
                    }
                    hir::StmtKind::Expr(expr) => state.scan_expr_stmt(cg, expr, top_level_stmt_idx),
                    hir::StmtKind::While { cond, body } => {
                        if cg.expr_contains_nested_indirect_perform_call_site(cond) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (while condition with indirect call site not yet supported)",
                                at: cond.span.into(),
                            });
                        }
                        if cg.block_contains_nested_indirect_perform_call_site(body) {
                            with_scoped_scan_frame(
                                state,
                                MixedEscapeDirectFrame::WhileBody {
                                    while_cond: cond,
                                    while_body: body,
                                    stmt_idx: 0,
                                },
                                |state| state.scan_stmts(cg, &body.stmts, top_level_stmt_idx),
                            )?;
                        }
                        Ok(())
                    }
                    hir::StmtKind::Return { value } => {
                        if value.as_ref().is_some_and(|expr| {
                            cg.expr_contains_nested_indirect_perform_call_site(expr)
                        }) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (only statement-position nested block indirect call site supported)",
                                at: stmt.span.into(),
                            });
                        }
                        Ok(())
                    }
                })
            }

            fn scan_expr_stmt(
                &mut self,
                cg: &MainCodegen<'_, '_>,
                expr: &'hir hir::Expr,
                top_level_stmt_idx: usize,
            ) -> Result<(), LlvmEmitError> {
                match &expr.kind {
                    hir::ExprKind::Call { .. }
                        if cg.expr_is_indirect_perform_call_site_candidate(expr) =>
                    {
                        Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (indirect site must be val-bound)",
                            at: expr.span.into(),
                        })
                    }
                    hir::ExprKind::Block(block) => with_scoped_scan_frame(
                        self,
                        MixedEscapeDirectFrame::Block { block, stmt_idx: 0 },
                        |state| state.scan_stmts(cg, &block.stmts, top_level_stmt_idx),
                    ),
                    hir::ExprKind::If {
                        cond,
                        then_branch,
                        else_branch,
                        ..
                    } => {
                        if cg.expr_contains_nested_indirect_perform_call_site(cond) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (if condition with indirect call site not yet supported)",
                                at: cond.span.into(),
                            });
                        }

                        if let hir::ExprKind::Block(block) = &then_branch.kind {
                            if cg.block_contains_nested_indirect_perform_call_site(block) {
                                with_scoped_scan_frame(
                                    self,
                                    MixedEscapeDirectFrame::IfThen {
                                        if_expr: expr,
                                        then_block: block,
                                        stmt_idx: 0,
                                    },
                                    |state| state.scan_stmts(cg, &block.stmts, top_level_stmt_idx),
                                )?;
                            }
                        } else if cg.expr_contains_nested_indirect_perform_call_site(then_branch) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (if branch value with indirect call site not yet supported)",
                                at: then_branch.span.into(),
                            });
                        }

                        if let Some(else_expr) = else_branch.as_deref() {
                            if let hir::ExprKind::Block(block) = &else_expr.kind {
                                if cg.block_contains_nested_indirect_perform_call_site(block) {
                                    with_scoped_scan_frame(
                                        self,
                                        MixedEscapeDirectFrame::IfElse {
                                            if_expr: expr,
                                            else_block: block,
                                            stmt_idx: 0,
                                        },
                                        |state| {
                                            state.scan_stmts(cg, &block.stmts, top_level_stmt_idx)
                                        },
                                    )?;
                                }
                            } else if cg.expr_contains_nested_indirect_perform_call_site(else_expr)
                            {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (if branch value with indirect call site not yet supported)",
                                    at: else_expr.span.into(),
                                });
                            }
                        }

                        Ok(())
                    }
                    _ => {
                        if cg.expr_contains_nested_indirect_perform_call_site(expr) {
                            Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (only statement-position nested block indirect call site supported)",
                                at: expr.span.into(),
                            })
                        } else {
                            Ok(())
                        }
                    }
                }
            }
        }

        let mut state = ScanState {
            path: Vec::new(),
            sites: Vec::new(),
        };
        for (top_idx, stmt) in handle.body.stmts.iter().enumerate() {
            state.scan_stmts(self, std::slice::from_ref(stmt), top_idx)?;
        }

        let mut seen_nested_block_stmt_idx: HashSet<usize> = HashSet::new();
        let mut seen_if_then_stmt_idx: HashSet<usize> = HashSet::new();
        let mut seen_if_else_stmt_idx: HashSet<usize> = HashSet::new();
        let mut seen_while_stmt_idx: HashSet<usize> = HashSet::new();
        for site in &state.sites {
            let Some(first_frame) = site.resume_path.first() else {
                continue;
            };
            match first_frame {
                MixedEscapeDirectFrame::Block { .. } => {
                    if !site
                        .resume_path
                        .iter()
                        .all(|frame| matches!(frame, MixedEscapeDirectFrame::Block { .. }))
                    {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (only statement-position nested block indirect call site supported)",
                            at: site.decl.span.into(),
                        });
                    }
                    if !seen_nested_block_stmt_idx.insert(site.top_level_stmt_idx) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple nested block indirect sites per top-level statement not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
                MixedEscapeDirectFrame::IfThen { .. } => {
                    if !seen_if_then_stmt_idx.insert(site.top_level_stmt_idx) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple indirect sites in the same if-then branch not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
                MixedEscapeDirectFrame::IfElse { .. } => {
                    if !seen_if_else_stmt_idx.insert(site.top_level_stmt_idx) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple indirect sites in the same if-else branch not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
                MixedEscapeDirectFrame::WhileBody { .. } => {
                    if !Self::mixed_escape_while_nested_path_supported(&site.resume_path) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (deeper nested indirect site in while body not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                    if !seen_while_stmt_idx.insert(site.top_level_stmt_idx) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple indirect sites in the same while body not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
            }
        }

        Ok(state.sites)
    }

    fn mixed_escape_while_nested_path_supported<'hir>(
        resume_path: &[MixedEscapeDirectFrame<'hir>],
    ) -> bool {
        let Some((first, tail)) = resume_path.split_first() else {
            return false;
        };
        if !matches!(first, MixedEscapeDirectFrame::WhileBody { .. }) {
            return false;
        }
        let Some((second, rest)) = tail.split_first() else {
            return true;
        };
        match second {
            MixedEscapeDirectFrame::Block { .. } => rest
                .iter()
                .all(|frame| matches!(frame, MixedEscapeDirectFrame::Block { .. })),
            MixedEscapeDirectFrame::IfThen { .. } | MixedEscapeDirectFrame::IfElse { .. } => rest
                .iter()
                .all(|frame| matches!(frame, MixedEscapeDirectFrame::Block { .. })),
            MixedEscapeDirectFrame::WhileBody { .. } => false,
        }
    }

    fn mixed_escape_block_only_path_supported<'hir>(
        resume_path: &[MixedEscapeDirectFrame<'hir>],
    ) -> bool {
        !resume_path.is_empty()
            && resume_path
                .iter()
                .all(|frame| matches!(frame, MixedEscapeDirectFrame::Block { .. }))
    }

    fn mixed_escape_block_frames_same<'hir>(
        lhs: &MixedEscapeDirectFrame<'hir>,
        rhs: &MixedEscapeDirectFrame<'hir>,
    ) -> bool {
        match (lhs, rhs) {
            (
                MixedEscapeDirectFrame::Block {
                    block: lhs_block, ..
                },
                MixedEscapeDirectFrame::Block {
                    block: rhs_block, ..
                },
            ) => std::ptr::eq(*lhs_block, *rhs_block),
            _ => false,
        }
    }

    fn mixed_escape_if_branch_path_supported<'hir>(
        resume_path: &[MixedEscapeDirectFrame<'hir>],
    ) -> bool {
        matches!(
            resume_path.first(),
            Some(MixedEscapeDirectFrame::IfThen { .. } | MixedEscapeDirectFrame::IfElse { .. })
        ) && resume_path[1..]
            .iter()
            .all(|frame| matches!(frame, MixedEscapeDirectFrame::Block { .. }))
    }

    fn mixed_escape_if_frames_same<'hir>(
        lhs: &MixedEscapeDirectFrame<'hir>,
        rhs: &MixedEscapeDirectFrame<'hir>,
    ) -> bool {
        match (lhs, rhs) {
            (
                MixedEscapeDirectFrame::IfThen {
                    if_expr: lhs_if,
                    then_block: lhs_block,
                    ..
                },
                MixedEscapeDirectFrame::IfThen {
                    if_expr: rhs_if,
                    then_block: rhs_block,
                    ..
                },
            ) => std::ptr::eq(*lhs_if, *rhs_if) && std::ptr::eq(*lhs_block, *rhs_block),
            (
                MixedEscapeDirectFrame::IfElse {
                    if_expr: lhs_if,
                    else_block: lhs_block,
                    ..
                },
                MixedEscapeDirectFrame::IfElse {
                    if_expr: rhs_if,
                    else_block: rhs_block,
                    ..
                },
            ) => std::ptr::eq(*lhs_if, *rhs_if) && std::ptr::eq(*lhs_block, *rhs_block),
            _ => false,
        }
    }

    fn mixed_escape_while_frames_same<'hir>(
        lhs: &MixedEscapeDirectFrame<'hir>,
        rhs: &MixedEscapeDirectFrame<'hir>,
    ) -> bool {
        match (lhs, rhs) {
            (
                MixedEscapeDirectFrame::WhileBody {
                    while_cond: lhs_cond,
                    while_body: lhs_body,
                    ..
                },
                MixedEscapeDirectFrame::WhileBody {
                    while_cond: rhs_cond,
                    while_body: rhs_body,
                    ..
                },
            ) => std::ptr::eq(*lhs_cond, *rhs_cond) && std::ptr::eq(*lhs_body, *rhs_body),
            _ => false,
        }
    }

    fn mixed_escape_while_same_stmt_mixed_path_supported<'hir>(
        lhs: &[MixedEscapeDirectFrame<'hir>],
        rhs: &[MixedEscapeDirectFrame<'hir>],
    ) -> bool {
        if !Self::mixed_escape_while_nested_path_supported(lhs)
            || !Self::mixed_escape_while_nested_path_supported(rhs)
        {
            return false;
        }
        let Some(lhs_first) = lhs.first() else {
            return false;
        };
        let Some(rhs_first) = rhs.first() else {
            return false;
        };
        if !Self::mixed_escape_while_frames_same(lhs_first, rhs_first)
            || lhs_first.stmt_idx() != rhs_first.stmt_idx()
        {
            return false;
        }

        match (lhs.get(1), rhs.get(1)) {
            (
                Some(MixedEscapeDirectFrame::Block { .. }),
                Some(MixedEscapeDirectFrame::Block { .. }),
            ) => {
                Self::mixed_escape_block_only_path_supported(&lhs[1..])
                    && Self::mixed_escape_block_only_path_supported(&rhs[1..])
            }
            (
                Some(MixedEscapeDirectFrame::IfThen { .. } | MixedEscapeDirectFrame::IfElse { .. }),
                Some(MixedEscapeDirectFrame::IfThen { .. } | MixedEscapeDirectFrame::IfElse { .. }),
            ) => {
                Self::mixed_escape_if_branch_path_supported(&lhs[1..])
                    && Self::mixed_escape_if_branch_path_supported(&rhs[1..])
                    && Self::mixed_escape_if_frames_same(&lhs[1], &rhs[1])
            }
            _ => false,
        }
    }

    fn mixed_escape_while_separate_stmt_order_supported<'hir>(
        lhs: &[MixedEscapeDirectFrame<'hir>],
        rhs: &[MixedEscapeDirectFrame<'hir>],
    ) -> bool {
        if !Self::mixed_escape_while_nested_path_supported(lhs)
            || !Self::mixed_escape_while_nested_path_supported(rhs)
        {
            return false;
        }
        let Some(lhs_first) = lhs.first() else {
            return false;
        };
        let Some(rhs_first) = rhs.first() else {
            return false;
        };
        Self::mixed_escape_while_frames_same(lhs_first, rhs_first)
            && lhs_first.stmt_idx() < rhs_first.stmt_idx()
    }


    // ── T1606f-2: Indirect perform support for escape continuations ──

    /// Scan handle body stmts for `val x = f(...)` where f may perform (non-pure).
    fn scan_for_indirect_perform_call_sites(
        &self,
        body: &hir::Block,
        _arm_op_fqn: &str,
    ) -> Vec<IndirectPerformCallSite> {
        let mut sites = Vec::new();
        for (idx, stmt) in body.stmts.iter().enumerate() {
            if let hir::StmtKind::Val(decl) = &stmt.kind
                && let Some(init) = &decl.init
                && let hir::ExprKind::Call { callee, .. } = &init.kind
                && let Some(fqn) = self.try_extract_callee_fqn(callee)
                && let Some(fun) = self.fun_index.get(fqn)
            {
                let is_pure = self.fun_ty_effects_is_pure(fun.ty).unwrap_or(false);
                if !is_pure && let Some(id) = decl.id {
                    sites.push(IndirectPerformCallSite {
                        stmt_idx: idx,
                        _result_id: id,
                        result_ty: decl.ty,
                    });
                }
            }
        }
        sites
    }

    /// Helper: collect used locals in a block (static, no codegen state needed).
    fn collect_used_locals_in_block_static(block: &hir::Block, out: &mut HashSet<hir::SymbolId>) {
        for stmt in &block.stmts {
            Self::collect_used_locals_in_stmt_static(stmt, out);
        }
    }

    fn collect_used_locals_in_call_args_static(
        args: &[hir::CallArg],
        out: &mut HashSet<hir::SymbolId>,
    ) {
        for arg in args {
            match arg {
                hir::CallArg::Positional(expr) => {
                    Self::collect_used_locals_in_expr_static(expr, out);
                }
                hir::CallArg::Named { value, .. } => {
                    Self::collect_used_locals_in_expr_static(value, out);
                }
            }
        }
    }

    fn collect_used_locals_in_handle_static(
        handle: &hir::HandleExpr,
        out: &mut HashSet<hir::SymbolId>,
    ) {
        Self::collect_used_locals_in_block_static(&handle.body, out);
        for arm in &handle.arms {
            Self::collect_used_locals_in_expr_static(&arm.body, out);
        }
        if let Some(finally) = &handle.finally {
            Self::collect_used_locals_in_block_static(finally, out);
        }
    }

    /// Helper: collect used locals in a stmt (static, no codegen state needed).
    fn collect_used_locals_in_stmt_static(stmt: &hir::Stmt, out: &mut HashSet<hir::SymbolId>) {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
            hir::StmtKind::Expr(expr) => Self::collect_used_locals_in_expr_static(expr, out),
            hir::StmtKind::Val(decl) => {
                if let Some(init) = &decl.init {
                    Self::collect_used_locals_in_expr_static(init, out);
                }
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                Self::collect_used_locals_in_expr_static(lhs, out);
                Self::collect_used_locals_in_expr_static(rhs, out);
            }
            hir::StmtKind::Return { value } => {
                if let Some(expr) = value {
                    Self::collect_used_locals_in_expr_static(expr, out);
                }
            }
            hir::StmtKind::While { cond, body } => {
                Self::collect_used_locals_in_expr_static(cond, out);
                Self::collect_used_locals_in_block_static(body, out);
            }
        }
    }

    /// Helper: collect used locals in an expression (static).
    fn collect_used_locals_in_expr_static(expr: &hir::Expr, out: &mut HashSet<hir::SymbolId>) {
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Todo(_) => {}
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                out.insert(*id);
            }
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { .. }) => {}
            hir::ExprKind::Call { callee, args } => {
                Self::collect_used_locals_in_expr_static(callee, out);
                Self::collect_used_locals_in_call_args_static(args, out);
            }
            hir::ExprKind::Perform { args, .. } => {
                Self::collect_used_locals_in_call_args_static(args, out);
            }
            hir::ExprKind::Block(block) => {
                Self::collect_used_locals_in_block_static(block, out);
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                Self::collect_used_locals_in_expr_static(cond, out);
                Self::collect_used_locals_in_expr_static(then_branch, out);
                if let Some(e) = else_branch {
                    Self::collect_used_locals_in_expr_static(e, out);
                }
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                Self::collect_used_locals_in_expr_static(lhs, out);
                Self::collect_used_locals_in_expr_static(rhs, out);
            }
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::Cast { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. }
            | hir::ExprKind::MemberAccess {
                receiver: inner, ..
            } => {
                Self::collect_used_locals_in_expr_static(inner, out);
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let hir::InterpolatedStringPart::Expr { expr } = part {
                        Self::collect_used_locals_in_expr_static(expr, out);
                    }
                }
            }
            hir::ExprKind::StructLit { fields, .. } => {
                for f in fields {
                    Self::collect_used_locals_in_expr_static(&f.value, out);
                }
            }
            hir::ExprKind::TupleLit { elements } => {
                for e in elements {
                    Self::collect_used_locals_in_expr_static(e, out);
                }
            }
            hir::ExprKind::When { subject, arms } => {
                Self::collect_used_locals_in_expr_static(subject, out);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        Self::collect_used_locals_in_expr_static(g, out);
                    }
                    Self::collect_used_locals_in_expr_static(&arm.body, out);
                }
            }
            hir::ExprKind::Closure(closure) => {
                // 闭包 body 里引用到的外层 locals 会经由 captures 显式列出，
                // 这里一并收集，避免 body-lift / capture 分析漏算。
                for cap in &closure.captures {
                    out.insert(cap.id);
                }
                Self::collect_used_locals_in_expr_static(&closure.body, out);
            }
            hir::ExprKind::Handle(handle) => {
                Self::collect_used_locals_in_handle_static(handle, out);
            }
        }
    }
}
