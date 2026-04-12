impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn mixed_escape_matrix_site_resume_path<'hir>(
        site: &'hir MatrixEscapeSite<'hir>,
    ) -> &'hir [MixedEscapeDirectFrame<'hir>] {
        match &site.kind {
            MatrixEscapeSiteKind::Direct { site } => site.resume_path.as_slice(),
            MatrixEscapeSiteKind::Indirect { site } => site.resume_path.as_slice(),
        }
    }

    fn mixed_escape_matrix_stmt_path_cmp<'hir>(
        lhs: &[MixedEscapeDirectFrame<'hir>],
        rhs: &[MixedEscapeDirectFrame<'hir>],
    ) -> std::cmp::Ordering {
        let mut lhs_iter = lhs.iter().map(MixedEscapeDirectFrame::stmt_idx);
        let mut rhs_iter = rhs.iter().map(MixedEscapeDirectFrame::stmt_idx);
        loop {
            match (lhs_iter.next(), rhs_iter.next()) {
                (Some(lhs_idx), Some(rhs_idx)) => match lhs_idx.cmp(&rhs_idx) {
                    std::cmp::Ordering::Equal => {}
                    order => return order,
                },
                (Some(_), None) => return std::cmp::Ordering::Greater,
                (None, Some(_)) => return std::cmp::Ordering::Less,
                (None, None) => return std::cmp::Ordering::Equal,
            }
        }
    }

    fn collect_mixed_escape_matrix_body_decls(
        &self,
        stmts: &[hir::Stmt],
        body_decl_all: &mut HashMap<hir::SymbolId, EscapeCaptureMeta>,
        body_decl_spans: &mut HashMap<hir::SymbolId, crate::span::Span>,
        body_decl_order: &mut HashMap<hir::SymbolId, usize>,
        next_decl_order: &mut usize,
    ) -> Result<(), LlvmEmitError> {
        for stmt in stmts {
            match &stmt.kind {
                hir::StmtKind::Val(decl) => {
                    if let Some(id) = decl.id {
                        let ty =
                            self.cg_ty_of(decl.ty)
                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape capture local type",
                                    at: decl.span.into(),
                                })?;
                        let meta = EscapeCaptureMeta {
                            id,
                            hir_ty: Some(decl.ty),
                            ty,
                            mutable: decl.mutable,
                        };
                        body_decl_all.insert(id, meta);
                        body_decl_spans.insert(id, decl.span);
                        body_decl_order.insert(id, *next_decl_order);
                        *next_decl_order += 1;
                    }
                }
                hir::StmtKind::Expr(expr) => {
                    if let hir::ExprKind::Block(block) = &expr.kind {
                        self.collect_mixed_escape_matrix_body_decls(
                            &block.stmts,
                            body_decl_all,
                            body_decl_spans,
                            body_decl_order,
                            next_decl_order,
                        )?;
                    } else if let hir::ExprKind::If {
                        then_branch,
                        else_branch,
                        ..
                    } = &expr.kind
                    {
                        if let hir::ExprKind::Block(block) = &then_branch.kind {
                            self.collect_mixed_escape_matrix_body_decls(
                                &block.stmts,
                                body_decl_all,
                                body_decl_spans,
                                body_decl_order,
                                next_decl_order,
                            )?;
                        }
                        if let Some(else_expr) = else_branch.as_deref()
                            && let hir::ExprKind::Block(block) = &else_expr.kind
                        {
                            self.collect_mixed_escape_matrix_body_decls(
                                &block.stmts,
                                body_decl_all,
                                body_decl_spans,
                                body_decl_order,
                                next_decl_order,
                            )?;
                        }
                    }
                }
                hir::StmtKind::While { body, .. } => {
                    self.collect_mixed_escape_matrix_body_decls(
                        &body.stmts,
                        body_decl_all,
                        body_decl_spans,
                        body_decl_order,
                        next_decl_order,
                    )?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn collect_mixed_escape_used_after_site(
        site: &MixedEscapeDirectSite<'_>,
        top_level_stmts: &[hir::Stmt],
        used_after: &mut HashSet<hir::SymbolId>,
    ) {
        for frame in &site.resume_path {
            match frame {
                MixedEscapeDirectFrame::Block { block, stmt_idx } => {
                    for stmt in block.stmts.iter().skip(*stmt_idx + 1) {
                        Self::collect_used_locals_in_stmt_static(stmt, used_after);
                    }
                }
                MixedEscapeDirectFrame::IfThen {
                    then_block,
                    stmt_idx,
                    ..
                } => {
                    for stmt in then_block.stmts.iter().skip(*stmt_idx + 1) {
                        Self::collect_used_locals_in_stmt_static(stmt, used_after);
                    }
                }
                MixedEscapeDirectFrame::IfElse {
                    else_block,
                    stmt_idx,
                    ..
                } => {
                    for stmt in else_block.stmts.iter().skip(*stmt_idx + 1) {
                        Self::collect_used_locals_in_stmt_static(stmt, used_after);
                    }
                }
                MixedEscapeDirectFrame::WhileBody {
                    while_cond,
                    while_body,
                    stmt_idx,
                } => {
                    for stmt in while_body.stmts.iter().skip(*stmt_idx + 1) {
                        Self::collect_used_locals_in_stmt_static(stmt, used_after);
                    }
                    for stmt in &while_body.stmts {
                        Self::collect_used_locals_in_stmt_static(stmt, used_after);
                    }
                    Self::collect_used_locals_in_expr_static(while_cond, used_after);
                }
            }
        }
        for stmt in top_level_stmts.iter().skip(site.top_level_stmt_idx + 1) {
            Self::collect_used_locals_in_stmt_static(stmt, used_after);
        }
    }

    fn collect_mixed_escape_used_after_indirect_site(
        site: &MixedEscapeIndirectSite<'_>,
        top_level_stmts: &[hir::Stmt],
        used_after: &mut HashSet<hir::SymbolId>,
    ) {
        Self::collect_used_locals_in_expr_static(site.init, used_after);
        used_after.remove(&site.id);
        for frame in &site.resume_path {
            match frame {
                MixedEscapeDirectFrame::Block { block, stmt_idx } => {
                    for stmt in block.stmts.iter().skip(*stmt_idx + 1) {
                        Self::collect_used_locals_in_stmt_static(stmt, used_after);
                    }
                }
                MixedEscapeDirectFrame::IfThen {
                    then_block,
                    stmt_idx,
                    ..
                } => {
                    for stmt in then_block.stmts.iter().skip(*stmt_idx + 1) {
                        Self::collect_used_locals_in_stmt_static(stmt, used_after);
                    }
                }
                MixedEscapeDirectFrame::IfElse {
                    else_block,
                    stmt_idx,
                    ..
                } => {
                    for stmt in else_block.stmts.iter().skip(*stmt_idx + 1) {
                        Self::collect_used_locals_in_stmt_static(stmt, used_after);
                    }
                }
                MixedEscapeDirectFrame::WhileBody {
                    while_cond,
                    while_body,
                    stmt_idx,
                } => {
                    for stmt in while_body.stmts.iter().skip(*stmt_idx + 1) {
                        Self::collect_used_locals_in_stmt_static(stmt, used_after);
                    }
                    for stmt in &while_body.stmts {
                        Self::collect_used_locals_in_stmt_static(stmt, used_after);
                    }
                    Self::collect_used_locals_in_expr_static(while_cond, used_after);
                }
            }
        }
        for stmt in top_level_stmts.iter().skip(site.top_level_stmt_idx + 1) {
            Self::collect_used_locals_in_stmt_static(stmt, used_after);
        }
    }

    fn collect_mixed_escape_used_before_block_site_from_idx<'hir>(
        block_level: usize,
        block: &'hir hir::Block,
        start_idx: usize,
        next_path: &[MixedEscapeDirectFrame<'hir>],
        used_before: &mut HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        let Some(MixedEscapeDirectFrame::Block {
            block: expected_block,
            stmt_idx: target_stmt_idx,
        }) = next_path.get(block_level)
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (missing block path)",
                at: block.span.into(),
            });
        };
        if !std::ptr::eq(block, *expected_block) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (block path mismatch)",
                at: block.span.into(),
            });
        }

        for (idx, stmt) in block.stmts.iter().enumerate().skip(start_idx) {
            if idx < *target_stmt_idx {
                Self::collect_used_locals_in_stmt_static(stmt, used_before);
                continue;
            }

            if block_level + 1 == next_path.len() {
                return Ok(());
            }

            let hir::StmtKind::Expr(expr) = &stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected block statement)",
                    at: stmt.span.into(),
                });
            };
            let hir::ExprKind::Block(next_block) = &expr.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected block statement)",
                    at: expr.span.into(),
                });
            };
            let Some(MixedEscapeDirectFrame::Block {
                block: expected_next_block,
                ..
            }) = next_path.get(block_level + 1)
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (missing block path)",
                    at: block.span.into(),
                });
            };
            if !std::ptr::eq(next_block, *expected_next_block) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (block path mismatch)",
                    at: expr.span.into(),
                });
            }
            return Self::collect_mixed_escape_used_before_block_site_from_idx(
                block_level + 1,
                next_block,
                0,
                next_path,
                used_before,
            );
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (perform site missing)",
            at: block.span.into(),
        })
    }

    fn collect_mixed_escape_used_between_block_sites<'hir>(
        current_site: &MixedEscapeDirectSite<'hir>,
        next_site: &MixedEscapeIndirectSite<'hir>,
        used_between: &mut HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        let current_path = current_site.resume_path.as_slice();
        let next_path = next_site.resume_path.as_slice();
        if !Self::mixed_escape_block_only_path_supported(current_path)
            || !Self::mixed_escape_block_only_path_supported(next_path)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only statement-position nested block direct / indirect coexistence supported)",
                at: next_site.decl.span.into(),
            });
        }

        let mut common = 0usize;
        while common < current_path.len()
            && common < next_path.len()
            && Self::mixed_escape_block_frames_same(&current_path[common], &next_path[common])
        {
            common += 1;
        }
        if common == 0 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (block path mismatch)",
                at: next_site.decl.span.into(),
            });
        }

        for frame in current_path[common..].iter().rev() {
            let MixedEscapeDirectFrame::Block { block, stmt_idx } = frame else {
                unreachable!("validated block-only current path");
            };
            for stmt in block.stmts.iter().skip(*stmt_idx + 1) {
                Self::collect_used_locals_in_stmt_static(stmt, used_between);
            }
        }

        let MixedEscapeDirectFrame::Block {
            block: common_block,
            stmt_idx,
        } = &current_path[common - 1]
        else {
            unreachable!("validated block-only current path");
        };
        Self::collect_mixed_escape_used_before_block_site_from_idx(
            common - 1,
            common_block,
            *stmt_idx + 1,
            next_path,
            used_between,
        )
    }

    fn collect_mixed_escape_used_before_if_site_from_idx<'hir>(
        branch_block: &'hir hir::Block,
        start_idx: usize,
        next_path: &[MixedEscapeDirectFrame<'hir>],
        used_before: &mut HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        let Some(first_frame) = next_path.first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (missing if branch path)",
                at: branch_block.span.into(),
            });
        };
        let target_stmt_idx = first_frame.stmt_idx();
        for (idx, stmt) in branch_block.stmts.iter().enumerate().skip(start_idx) {
            if idx < target_stmt_idx {
                Self::collect_used_locals_in_stmt_static(stmt, used_before);
                continue;
            }

            if next_path.len() == 1 {
                return Ok(());
            }

            let hir::StmtKind::Expr(expr) = &stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected block statement)",
                    at: stmt.span.into(),
                });
            };
            let hir::ExprKind::Block(next_block) = &expr.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (richer if-branch mixed sites not yet supported)",
                    at: expr.span.into(),
                });
            };
            let Some(MixedEscapeDirectFrame::Block {
                block: expected_next_block,
                ..
            }) = next_path.get(1)
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (richer if-branch mixed sites not yet supported)",
                    at: expr.span.into(),
                });
            };
            if !std::ptr::eq(next_block, *expected_next_block) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (if path mismatch)",
                    at: expr.span.into(),
                });
            }

            return Self::collect_mixed_escape_used_before_block_site_from_idx(
                1,
                next_block,
                0,
                next_path,
                used_before,
            );
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (perform site missing)",
            at: branch_block.span.into(),
        })
    }

    fn collect_mixed_escape_used_between_if_sites<'hir>(
        current_site: &MixedEscapeDirectSite<'hir>,
        next_site: &MixedEscapeIndirectSite<'hir>,
        used_between: &mut HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        let current_path = current_site.resume_path.as_slice();
        let next_path = next_site.resume_path.as_slice();
        if !Self::mixed_escape_if_branch_path_supported(current_path)
            || !Self::mixed_escape_if_branch_path_supported(next_path)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only same-branch direct / indirect coexistence in if statement supported)",
                at: next_site.decl.span.into(),
            });
        }

        let Some(current_first) = current_path.first() else {
            unreachable!("validated if path for current site");
        };
        let Some(next_first) = next_path.first() else {
            unreachable!("validated if path for next site");
        };
        if !Self::mixed_escape_if_frames_same(current_first, next_first) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (if path mismatch)",
                at: next_site.decl.span.into(),
            });
        }

        for frame in current_path[1..].iter().rev() {
            let MixedEscapeDirectFrame::Block { block, stmt_idx } = frame else {
                unreachable!("validated block-only tail for if mixed path");
            };
            for stmt in block.stmts.iter().skip(*stmt_idx + 1) {
                Self::collect_used_locals_in_stmt_static(stmt, used_between);
            }
        }

        match current_first {
            MixedEscapeDirectFrame::IfThen {
                then_block,
                stmt_idx,
                ..
            } => Self::collect_mixed_escape_used_before_if_site_from_idx(
                then_block,
                *stmt_idx + 1,
                next_path,
                used_between,
            ),
            MixedEscapeDirectFrame::IfElse {
                else_block,
                stmt_idx,
                ..
            } => Self::collect_mixed_escape_used_before_if_site_from_idx(
                else_block,
                *stmt_idx + 1,
                next_path,
                used_between,
            ),
            MixedEscapeDirectFrame::Block { .. } | MixedEscapeDirectFrame::WhileBody { .. } => {
                unreachable!("validated if path for current site")
            }
        }
    }

    fn collect_mixed_escape_used_between_while_sites<'hir>(
        current_site: &MixedEscapeDirectSite<'hir>,
        next_site: &MixedEscapeIndirectSite<'hir>,
        used_between: &mut HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        let current_path = current_site.resume_path.as_slice();
        let next_path = next_site.resume_path.as_slice();
        if !Self::mixed_escape_while_same_stmt_mixed_path_supported(current_path, next_path) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only same-body-stmt direct / indirect coexistence in while body supported)",
                at: next_site.decl.span.into(),
            });
        }

        let nested_current = MixedEscapeDirectSite {
            top_level_stmt_idx: current_site.top_level_stmt_idx,
            decl: current_site.decl,
            args: current_site.args,
            id: current_site.id,
            resume_path: current_path[1..].to_vec(),
        };
        let nested_next = MixedEscapeIndirectSite {
            top_level_stmt_idx: next_site.top_level_stmt_idx,
            decl: next_site.decl,
            init: next_site.init,
            id: next_site.id,
            resume_path: next_path[1..].to_vec(),
        };

        match (
            nested_current.resume_path.first(),
            nested_next.resume_path.first(),
        ) {
            (
                Some(MixedEscapeDirectFrame::Block { .. }),
                Some(MixedEscapeDirectFrame::Block { .. }),
            ) => Self::collect_mixed_escape_used_between_block_sites(
                &nested_current,
                &nested_next,
                used_between,
            ),
            (
                Some(MixedEscapeDirectFrame::IfThen { .. } | MixedEscapeDirectFrame::IfElse { .. }),
                Some(MixedEscapeDirectFrame::IfThen { .. } | MixedEscapeDirectFrame::IfElse { .. }),
            ) => Self::collect_mixed_escape_used_between_if_sites(
                &nested_current,
                &nested_next,
                used_between,
            ),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only same-body-stmt direct / indirect coexistence in while body supported)",
                at: next_site.decl.span.into(),
            }),
        }
    }

    fn codegen_mixed_escape_matrix_replay_stmt(
        &mut self,
        stmt: &hir::Stmt,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        match &stmt.kind {
            hir::StmtKind::Empty => Ok(()),
            hir::StmtKind::Val(decl) => {
                if let Some(id) = decl.id
                    && body_lift_ids.contains(&id)
                {
                    let Some(init) = decl.init.as_ref() else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "lifted local without init",
                            at: decl.span.into(),
                        });
                    };
                    let decl_ty =
                        self.cg_ty_of(decl.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "lifted local type",
                                at: decl.span.into(),
                            })?;
                    let target_ptr = if let Some(local) = self.env.get(id) {
                        if local.ty != decl_ty {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "lifted local type",
                                at: decl.span.into(),
                            });
                        }
                        local.ptr
                    } else {
                        let name = decl.name.as_deref().unwrap_or("v");
                        let ptr = self.create_entry_alloca(decl.span, name, decl_ty)?;
                        self.env.insert(
                            id,
                            CgLocal {
                                hir_ty: Some(decl.ty),
                                ty: decl_ty,
                                ptr,
                                mutable: decl.mutable,
                            },
                        );
                        ptr
                    };
                    let v = self.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                    let _stored = self.store_local_value(decl.span, target_ptr, decl_ty, v)?;
                    Ok(())
                } else {
                    self.codegen_val_decl(decl)
                }
            }
            hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                self.codegen_assign_stmt(*eq_span, lhs, rhs)
            }
            hir::StmtKind::Expr(expr) => {
                let _ = self.codegen_expr(expr)?;
                Ok(())
            }
            hir::StmtKind::While { cond, body } => self.codegen_while_stmt(stmt.span, cond, body),
            hir::StmtKind::Return { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "`return` inside mixed-arm escape continuation step",
                at: stmt.span.into(),
            }),
            hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "statement inside mixed-arm escape continuation step",
                at: stmt.span.into(),
            }),
        }
    }

    fn codegen_mixed_escape_matrix_prefix_from_stmts<'hir>(
        &mut self,
        site: &MixedEscapeDirectSite<'hir>,
        depth: usize,
        stmts: &'hir [hir::Stmt],
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        let target_stmt_idx = site.resume_path[depth].stmt_idx();
        for (idx, stmt) in stmts.iter().enumerate() {
            if idx < target_stmt_idx {
                self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
                continue;
            }
            if depth + 1 == site.resume_path.len() {
                let hir::StmtKind::Val(decl) = &stmt.kind else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected perform binding)",
                        at: stmt.span.into(),
                    });
                };
                if !std::ptr::eq(decl, site.decl) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (block path mismatch)",
                        at: decl.span.into(),
                    });
                }
                return Ok(());
            }

            let hir::StmtKind::Expr(expr) = &stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected block statement)",
                    at: stmt.span.into(),
                });
            };
            let hir::ExprKind::Block(block) = &expr.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected block statement)",
                    at: expr.span.into(),
                });
            };
            let MixedEscapeDirectFrame::Block {
                block: expected_block,
                ..
            } = &site.resume_path[depth + 1]
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (nested if path after branch not yet supported)",
                    at: expr.span.into(),
                });
            };
            if !std::ptr::eq(block, *expected_block) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (block path mismatch)",
                    at: expr.span.into(),
                });
            }

            self.env.push_scope();
            return self.codegen_mixed_escape_matrix_prefix_from_stmts(
                site,
                depth + 1,
                &block.stmts,
                body_lift_ids,
            );
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (perform site missing)",
            at: site.decl.span.into(),
        })
    }

    fn codegen_mixed_escape_matrix_nested_block_prefix_to_site<'hir>(
        &mut self,
        site: &MixedEscapeDirectSite<'hir>,
        top_stmt: &'hir hir::Stmt,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        let Some(first_frame) = site.resume_path.first() else {
            return Ok(());
        };
        let MixedEscapeDirectFrame::Block {
            block: expected_block,
            ..
        } = first_frame
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected block statement)",
                at: site.decl.span.into(),
            });
        };
        let hir::StmtKind::Expr(expr) = &top_stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected block statement)",
                at: top_stmt.span.into(),
            });
        };
        let hir::ExprKind::Block(block) = &expr.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected block statement)",
                at: expr.span.into(),
            });
        };
        if !std::ptr::eq(block, *expected_block) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (block path mismatch)",
                at: expr.span.into(),
            });
        }

        self.env.push_scope();
        self.codegen_mixed_escape_matrix_prefix_from_stmts(site, 0, &block.stmts, body_lift_ids)
    }

    fn codegen_mixed_escape_matrix_if_branch_prefix_to_site<'hir>(
        &mut self,
        site: &MixedEscapeDirectSite<'hir>,
        top_stmt: &'hir hir::Stmt,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        let Some(first_frame) = site.resume_path.first() else {
            return Ok(());
        };
        let hir::StmtKind::Expr(expr) = &top_stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected if statement)",
                at: top_stmt.span.into(),
            });
        };
        let hir::ExprKind::If { .. } = &expr.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected if statement)",
                at: expr.span.into(),
            });
        };

        let branch_stmts = match first_frame {
            MixedEscapeDirectFrame::IfThen {
                if_expr,
                then_block,
                ..
            } => {
                if !std::ptr::eq(*if_expr, expr) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (if path mismatch)",
                        at: expr.span.into(),
                    });
                }
                &then_block.stmts
            }
            MixedEscapeDirectFrame::IfElse {
                if_expr,
                else_block,
                ..
            } => {
                if !std::ptr::eq(*if_expr, expr) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (if path mismatch)",
                        at: expr.span.into(),
                    });
                }
                &else_block.stmts
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected if branch site)",
                    at: site.decl.span.into(),
                });
            }
        };

        self.env.push_scope();
        self.codegen_mixed_escape_matrix_prefix_from_stmts(site, 0, branch_stmts, body_lift_ids)
    }

    fn codegen_mixed_escape_matrix_prefix_to_indirect_site_from_stmts<'hir>(
        &mut self,
        site: &MixedEscapeIndirectSite<'hir>,
        depth: usize,
        stmts: &'hir [hir::Stmt],
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        let target_stmt_idx = site.resume_path[depth].stmt_idx();
        for (idx, stmt) in stmts.iter().enumerate() {
            if idx < target_stmt_idx {
                self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
                continue;
            }
            if depth + 1 == site.resume_path.len() {
                let hir::StmtKind::Val(decl) = &stmt.kind else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected indirect call binding)",
                        at: stmt.span.into(),
                    });
                };
                if !std::ptr::eq(decl, site.decl) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (block path mismatch)",
                        at: decl.span.into(),
                    });
                }
                return Ok(());
            }

            let hir::StmtKind::Expr(expr) = &stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected block statement)",
                    at: stmt.span.into(),
                });
            };
            let hir::ExprKind::Block(block) = &expr.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected block statement)",
                    at: expr.span.into(),
                });
            };
            let MixedEscapeDirectFrame::Block {
                block: expected_block,
                ..
            } = &site.resume_path[depth + 1]
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (only statement-position nested block indirect call site supported)",
                    at: expr.span.into(),
                });
            };
            if !std::ptr::eq(block, *expected_block) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (block path mismatch)",
                    at: expr.span.into(),
                });
            }

            self.env.push_scope();
            return self.codegen_mixed_escape_matrix_prefix_to_indirect_site_from_stmts(
                site,
                depth + 1,
                &block.stmts,
                body_lift_ids,
            );
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (indirect site missing)",
            at: site.decl.span.into(),
        })
    }

    fn codegen_mixed_escape_matrix_nested_block_prefix_to_indirect_site<'hir>(
        &mut self,
        site: &MixedEscapeIndirectSite<'hir>,
        top_stmt: &'hir hir::Stmt,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        let Some(first_frame) = site.resume_path.first() else {
            return Ok(());
        };
        let MixedEscapeDirectFrame::Block {
            block: expected_block,
            ..
        } = first_frame
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected block statement)",
                at: site.decl.span.into(),
            });
        };
        let hir::StmtKind::Expr(expr) = &top_stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected block statement)",
                at: top_stmt.span.into(),
            });
        };
        let hir::ExprKind::Block(block) = &expr.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected block statement)",
                at: expr.span.into(),
            });
        };
        if !std::ptr::eq(block, *expected_block) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (block path mismatch)",
                at: expr.span.into(),
            });
        }

        self.env.push_scope();
        self.codegen_mixed_escape_matrix_prefix_to_indirect_site_from_stmts(
            site,
            0,
            &block.stmts,
            body_lift_ids,
        )
    }

    fn codegen_mixed_escape_matrix_nested_block_continue_after_indirect_site<'hir>(
        &mut self,
        site: &MixedEscapeIndirectSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        self.codegen_mixed_escape_matrix_nested_tail_after_indirect_site_from_depth(
            site,
            0,
            body_lift_ids,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mixed_escape_matrix_scan_if_branch_to_site_from_idx<'hir, FDirect, FIndirect>(
        &mut self,
        branch_block: &'hir hir::Block,
        start_idx: usize,
        next_pc: usize,
        next_site: &MatrixEscapeSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        emit_direct: &mut FDirect,
        emit_indirect: &mut FIndirect,
    ) -> Result<(), LlvmEmitError>
    where
        FDirect: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
        FIndirect:
            FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let next_path = Self::mixed_escape_matrix_site_resume_path(next_site);
        let Some(first_frame) = next_path.first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (missing if branch path)",
                at: next_site.decl.span.into(),
            });
        };
        let target_stmt_idx = first_frame.stmt_idx();
        for (idx, stmt) in branch_block.stmts.iter().enumerate().skip(start_idx) {
            if idx < target_stmt_idx {
                self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
                continue;
            }

            if next_path.len() == 1 {
                let hir::StmtKind::Val(decl) = &stmt.kind else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected perform binding)",
                        at: stmt.span.into(),
                    });
                };
                if !std::ptr::eq(decl, next_site.decl) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (if path mismatch)",
                        at: decl.span.into(),
                    });
                }
                match &next_site.kind {
                    MatrixEscapeSiteKind::Direct { site } => emit_direct(self, next_pc, site)?,
                    MatrixEscapeSiteKind::Indirect { site } => emit_indirect(self, next_pc, site)?,
                }
                return Ok(());
            }

            let hir::StmtKind::Expr(expr) = &stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected block statement)",
                    at: stmt.span.into(),
                });
            };
            let hir::ExprKind::Block(next_block) = &expr.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (richer if-branch mixed sites not yet supported)",
                    at: expr.span.into(),
                });
            };
            let Some(MixedEscapeDirectFrame::Block {
                block: expected_next_block,
                ..
            }) = next_path.get(1)
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (richer if-branch mixed sites not yet supported)",
                    at: expr.span.into(),
                });
            };
            if !std::ptr::eq(next_block, *expected_next_block) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (if path mismatch)",
                    at: expr.span.into(),
                });
            }

            self.env.push_scope();
            return self.codegen_mixed_escape_matrix_scan_block_to_site_from_idx(
                1,
                next_block,
                0,
                next_pc,
                next_site,
                body_lift_ids,
                emit_direct,
                emit_indirect,
            );
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (perform site missing)",
            at: next_site.decl.span.into(),
        })
    }

    fn codegen_mixed_escape_matrix_continue_to_next_if_site_after_direct<'hir, FDirect, FIndirect>(
        &mut self,
        current_site: &MixedEscapeDirectSite<'hir>,
        next_pc: usize,
        next_site: &MatrixEscapeSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        emit_direct: &mut FDirect,
        emit_indirect: &mut FIndirect,
    ) -> Result<(), LlvmEmitError>
    where
        FDirect: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
        FIndirect:
            FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let current_path = current_site.resume_path.as_slice();
        let next_path = Self::mixed_escape_matrix_site_resume_path(next_site);
        if !Self::mixed_escape_if_branch_path_supported(current_path)
            || !Self::mixed_escape_if_branch_path_supported(next_path)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only same-branch direct / indirect coexistence in if statement supported)",
                at: next_site.decl.span.into(),
            });
        }

        let Some(current_first) = current_path.first() else {
            unreachable!("validated if path for current site");
        };
        let Some(next_first) = next_path.first() else {
            unreachable!("validated if path for next site");
        };
        if !Self::mixed_escape_if_frames_same(current_first, next_first) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (if path mismatch)",
                at: next_site.decl.span.into(),
            });
        }

        if current_path.len() > 1 {
            self.codegen_mixed_escape_matrix_nested_tail_after_site_from_depth(
                current_site,
                1,
                body_lift_ids,
            )?;
        }
        self.env.push_scope();

        match current_first {
            MixedEscapeDirectFrame::IfThen {
                then_block,
                stmt_idx,
                ..
            } => self.codegen_mixed_escape_matrix_scan_if_branch_to_site_from_idx(
                then_block,
                *stmt_idx + 1,
                next_pc,
                next_site,
                body_lift_ids,
                emit_direct,
                emit_indirect,
            ),
            MixedEscapeDirectFrame::IfElse {
                else_block,
                stmt_idx,
                ..
            } => self.codegen_mixed_escape_matrix_scan_if_branch_to_site_from_idx(
                else_block,
                *stmt_idx + 1,
                next_pc,
                next_site,
                body_lift_ids,
                emit_direct,
                emit_indirect,
            ),
            MixedEscapeDirectFrame::Block { .. } | MixedEscapeDirectFrame::WhileBody { .. } => {
                unreachable!("validated if path for current site")
            }
        }
    }

    fn codegen_mixed_escape_matrix_continue_to_next_if_site_after_indirect<
        'hir,
        FDirect,
        FIndirect,
    >(
        &mut self,
        current_site: &MixedEscapeIndirectSite<'hir>,
        next_pc: usize,
        next_site: &MatrixEscapeSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        emit_direct: &mut FDirect,
        emit_indirect: &mut FIndirect,
    ) -> Result<(), LlvmEmitError>
    where
        FDirect: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
        FIndirect:
            FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let current_path = current_site.resume_path.as_slice();
        let next_path = Self::mixed_escape_matrix_site_resume_path(next_site);
        if !Self::mixed_escape_if_branch_path_supported(current_path)
            || !Self::mixed_escape_if_branch_path_supported(next_path)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only same-branch direct / indirect coexistence in if statement supported)",
                at: next_site.decl.span.into(),
            });
        }

        let Some(current_first) = current_path.first() else {
            unreachable!("validated if path for current site");
        };
        let Some(next_first) = next_path.first() else {
            unreachable!("validated if path for next site");
        };
        if !Self::mixed_escape_if_frames_same(current_first, next_first) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (if path mismatch)",
                at: next_site.decl.span.into(),
            });
        }

        if current_path.len() > 1 {
            self.codegen_mixed_escape_matrix_nested_tail_after_indirect_site_from_depth(
                current_site,
                1,
                body_lift_ids,
            )?;
        }
        self.env.push_scope();

        match current_first {
            MixedEscapeDirectFrame::IfThen {
                then_block,
                stmt_idx,
                ..
            } => self.codegen_mixed_escape_matrix_scan_if_branch_to_site_from_idx(
                then_block,
                *stmt_idx + 1,
                next_pc,
                next_site,
                body_lift_ids,
                emit_direct,
                emit_indirect,
            ),
            MixedEscapeDirectFrame::IfElse {
                else_block,
                stmt_idx,
                ..
            } => self.codegen_mixed_escape_matrix_scan_if_branch_to_site_from_idx(
                else_block,
                *stmt_idx + 1,
                next_pc,
                next_site,
                body_lift_ids,
                emit_direct,
                emit_indirect,
            ),
            MixedEscapeDirectFrame::Block { .. } | MixedEscapeDirectFrame::WhileBody { .. } => {
                unreachable!("validated if path for current site")
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mixed_escape_matrix_scan_block_to_site_from_idx<'hir, FDirect, FIndirect>(
        &mut self,
        block_level: usize,
        block: &'hir hir::Block,
        start_idx: usize,
        next_pc: usize,
        next_site: &MatrixEscapeSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        emit_direct: &mut FDirect,
        emit_indirect: &mut FIndirect,
    ) -> Result<(), LlvmEmitError>
    where
        FDirect: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
        FIndirect:
            FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let next_path = Self::mixed_escape_matrix_site_resume_path(next_site);
        let Some(MixedEscapeDirectFrame::Block {
            block: expected_block,
            stmt_idx: target_stmt_idx,
        }) = next_path.get(block_level)
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (missing block path)",
                at: next_site.decl.span.into(),
            });
        };
        if !std::ptr::eq(block, *expected_block) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (block path mismatch)",
                at: next_site.decl.span.into(),
            });
        }

        for (idx, stmt) in block.stmts.iter().enumerate().skip(start_idx) {
            if idx < *target_stmt_idx {
                self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
                continue;
            }

            if block_level + 1 == next_path.len() {
                match &next_site.kind {
                    MatrixEscapeSiteKind::Direct { site } => emit_direct(self, next_pc, site)?,
                    MatrixEscapeSiteKind::Indirect { site } => emit_indirect(self, next_pc, site)?,
                }
                return Ok(());
            }

            let hir::StmtKind::Expr(expr) = &stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected block statement)",
                    at: stmt.span.into(),
                });
            };
            let hir::ExprKind::Block(next_block) = &expr.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected block statement)",
                    at: expr.span.into(),
                });
            };
            let Some(MixedEscapeDirectFrame::Block {
                block: expected_next_block,
                ..
            }) = next_path.get(block_level + 1)
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (missing block path)",
                    at: next_site.decl.span.into(),
                });
            };
            if !std::ptr::eq(next_block, *expected_next_block) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (block path mismatch)",
                    at: next_site.decl.span.into(),
                });
            }

            self.env.push_scope();
            return self.codegen_mixed_escape_matrix_scan_block_to_site_from_idx(
                block_level + 1,
                next_block,
                0,
                next_pc,
                next_site,
                body_lift_ids,
                emit_direct,
                emit_indirect,
            );
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (perform site missing)",
            at: next_site.decl.span.into(),
        })
    }

    fn codegen_mixed_escape_matrix_continue_to_next_block_site_after_direct<
        'hir,
        FDirect,
        FIndirect,
    >(
        &mut self,
        current_site: &MixedEscapeDirectSite<'hir>,
        next_pc: usize,
        next_site: &MatrixEscapeSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        emit_direct: &mut FDirect,
        emit_indirect: &mut FIndirect,
    ) -> Result<(), LlvmEmitError>
    where
        FDirect: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
        FIndirect:
            FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let current_path = current_site.resume_path.as_slice();
        let next_path = Self::mixed_escape_matrix_site_resume_path(next_site);
        if !Self::mixed_escape_block_only_path_supported(current_path)
            || !Self::mixed_escape_block_only_path_supported(next_path)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only statement-position nested block direct / indirect coexistence supported)",
                at: next_site.decl.span.into(),
            });
        }

        let mut common = 0usize;
        while common < current_path.len()
            && common < next_path.len()
            && Self::mixed_escape_block_frames_same(&current_path[common], &next_path[common])
        {
            common += 1;
        }
        if common == 0 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (block path mismatch)",
                at: next_site.decl.span.into(),
            });
        }

        for frame in current_path[common..].iter().rev() {
            let MixedEscapeDirectFrame::Block { block, stmt_idx } = frame else {
                unreachable!("validated block-only current path");
            };
            self.env.push_scope();
            for stmt in block.stmts.iter().skip(*stmt_idx + 1) {
                self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
            }
            self.env.pop_scope();
        }

        let MixedEscapeDirectFrame::Block {
            block: common_block,
            stmt_idx,
        } = &current_path[common - 1]
        else {
            unreachable!("validated block-only current path");
        };
        self.env.push_scope();
        self.codegen_mixed_escape_matrix_scan_block_to_site_from_idx(
            common - 1,
            common_block,
            *stmt_idx + 1,
            next_pc,
            next_site,
            body_lift_ids,
            emit_direct,
            emit_indirect,
        )
    }

    fn codegen_mixed_escape_matrix_continue_to_next_block_site_after_indirect<
        'hir,
        FDirect,
        FIndirect,
    >(
        &mut self,
        current_site: &MixedEscapeIndirectSite<'hir>,
        next_pc: usize,
        next_site: &MatrixEscapeSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        emit_direct: &mut FDirect,
        emit_indirect: &mut FIndirect,
    ) -> Result<(), LlvmEmitError>
    where
        FDirect: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
        FIndirect:
            FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let current_path = current_site.resume_path.as_slice();
        let next_path = Self::mixed_escape_matrix_site_resume_path(next_site);
        if !Self::mixed_escape_block_only_path_supported(current_path)
            || !Self::mixed_escape_block_only_path_supported(next_path)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only statement-position nested block direct / indirect coexistence supported)",
                at: next_site.decl.span.into(),
            });
        }

        let mut common = 0usize;
        while common < current_path.len()
            && common < next_path.len()
            && Self::mixed_escape_block_frames_same(&current_path[common], &next_path[common])
        {
            common += 1;
        }
        if common == 0 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (block path mismatch)",
                at: next_site.decl.span.into(),
            });
        }

        for frame in current_path[common..].iter().rev() {
            let MixedEscapeDirectFrame::Block { block, stmt_idx } = frame else {
                unreachable!("validated block-only current path");
            };
            for stmt in block.stmts.iter().skip(*stmt_idx + 1) {
                self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
            }
            self.env.pop_scope();
        }

        let MixedEscapeDirectFrame::Block {
            block: common_block,
            stmt_idx,
        } = &current_path[common - 1]
        else {
            unreachable!("validated block-only current path");
        };
        self.codegen_mixed_escape_matrix_scan_block_to_site_from_idx(
            common - 1,
            common_block,
            *stmt_idx + 1,
            next_pc,
            next_site,
            body_lift_ids,
            emit_direct,
            emit_indirect,
        )
    }

    fn codegen_mixed_escape_matrix_continue_to_next_while_site_after_direct<
        'hir,
        FDirect,
        FIndirect,
    >(
        &mut self,
        current_site: &MixedEscapeDirectSite<'hir>,
        next_pc: usize,
        next_site: &MatrixEscapeSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        emit_direct: &mut FDirect,
        emit_indirect: &mut FIndirect,
    ) -> Result<(), LlvmEmitError>
    where
        FDirect: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
        FIndirect:
            FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let current_path = current_site.resume_path.as_slice();
        let next_path = Self::mixed_escape_matrix_site_resume_path(next_site);
        if !Self::mixed_escape_while_same_stmt_mixed_path_supported(current_path, next_path) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only same-body-stmt direct / indirect coexistence in while body supported)",
                at: next_site.decl.span.into(),
            });
        }

        let nested_current = MixedEscapeDirectSite {
            top_level_stmt_idx: current_site.top_level_stmt_idx,
            decl: current_site.decl,
            args: current_site.args,
            id: current_site.id,
            resume_path: current_path[1..].to_vec(),
        };
        let nested_next = match &next_site.kind {
            MatrixEscapeSiteKind::Direct { site } => MatrixEscapeSite {
                stmt_idx: 0,
                decl: site.decl,
                id: site.id,
                kind: MatrixEscapeSiteKind::Direct {
                    site: MixedEscapeDirectSite {
                        top_level_stmt_idx: site.top_level_stmt_idx,
                        decl: site.decl,
                        args: site.args,
                        id: site.id,
                        resume_path: site.resume_path[1..].to_vec(),
                    },
                },
            },
            MatrixEscapeSiteKind::Indirect { site } => MatrixEscapeSite {
                stmt_idx: 0,
                decl: site.decl,
                id: site.id,
                kind: MatrixEscapeSiteKind::Indirect {
                    site: MixedEscapeIndirectSite {
                        top_level_stmt_idx: site.top_level_stmt_idx,
                        decl: site.decl,
                        init: site.init,
                        id: site.id,
                        resume_path: site.resume_path[1..].to_vec(),
                    },
                },
            },
        };

        match nested_current.resume_path.first() {
            Some(MixedEscapeDirectFrame::Block { .. }) => self
                .codegen_mixed_escape_matrix_continue_to_next_block_site_after_direct(
                    &nested_current,
                    next_pc,
                    &nested_next,
                    body_lift_ids,
                    emit_direct,
                    emit_indirect,
                ),
            Some(MixedEscapeDirectFrame::IfThen { .. } | MixedEscapeDirectFrame::IfElse { .. }) => {
                self.codegen_mixed_escape_matrix_continue_to_next_if_site_after_direct(
                    &nested_current,
                    next_pc,
                    &nested_next,
                    body_lift_ids,
                    emit_direct,
                    emit_indirect,
                )
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only same-body-stmt direct / indirect coexistence in while body supported)",
                at: next_site.decl.span.into(),
            }),
        }
    }

    fn codegen_mixed_escape_matrix_continue_to_next_while_site_after_indirect<
        'hir,
        FDirect,
        FIndirect,
    >(
        &mut self,
        current_site: &MixedEscapeIndirectSite<'hir>,
        next_pc: usize,
        next_site: &MatrixEscapeSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        emit_direct: &mut FDirect,
        emit_indirect: &mut FIndirect,
    ) -> Result<(), LlvmEmitError>
    where
        FDirect: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
        FIndirect:
            FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let current_path = current_site.resume_path.as_slice();
        let next_path = Self::mixed_escape_matrix_site_resume_path(next_site);
        if !Self::mixed_escape_while_same_stmt_mixed_path_supported(current_path, next_path) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only same-body-stmt direct / indirect coexistence in while body supported)",
                at: next_site.decl.span.into(),
            });
        }

        let nested_current = MixedEscapeIndirectSite {
            top_level_stmt_idx: current_site.top_level_stmt_idx,
            decl: current_site.decl,
            init: current_site.init,
            id: current_site.id,
            resume_path: current_path[1..].to_vec(),
        };
        let nested_next = match &next_site.kind {
            MatrixEscapeSiteKind::Direct { site } => MatrixEscapeSite {
                stmt_idx: 0,
                decl: site.decl,
                id: site.id,
                kind: MatrixEscapeSiteKind::Direct {
                    site: MixedEscapeDirectSite {
                        top_level_stmt_idx: site.top_level_stmt_idx,
                        decl: site.decl,
                        args: site.args,
                        id: site.id,
                        resume_path: site.resume_path[1..].to_vec(),
                    },
                },
            },
            MatrixEscapeSiteKind::Indirect { site } => MatrixEscapeSite {
                stmt_idx: 0,
                decl: site.decl,
                id: site.id,
                kind: MatrixEscapeSiteKind::Indirect {
                    site: MixedEscapeIndirectSite {
                        top_level_stmt_idx: site.top_level_stmt_idx,
                        decl: site.decl,
                        init: site.init,
                        id: site.id,
                        resume_path: site.resume_path[1..].to_vec(),
                    },
                },
            },
        };

        match nested_current.resume_path.first() {
            Some(MixedEscapeDirectFrame::Block { .. }) => self
                .codegen_mixed_escape_matrix_continue_to_next_block_site_after_indirect(
                    &nested_current,
                    next_pc,
                    &nested_next,
                    body_lift_ids,
                    emit_direct,
                    emit_indirect,
                ),
            Some(MixedEscapeDirectFrame::IfThen { .. } | MixedEscapeDirectFrame::IfElse { .. }) => {
                self.codegen_mixed_escape_matrix_continue_to_next_if_site_after_indirect(
                    &nested_current,
                    next_pc,
                    &nested_next,
                    body_lift_ids,
                    emit_direct,
                    emit_indirect,
                )
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only same-body-stmt direct / indirect coexistence in while body supported)",
                at: next_site.decl.span.into(),
            }),
        }
    }

    fn codegen_mixed_escape_matrix_nested_tail_after_indirect_site_from_depth<'hir>(
        &mut self,
        site: &MixedEscapeIndirectSite<'hir>,
        start_depth: usize,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        for frame in site.resume_path[start_depth..].iter().rev() {
            match frame {
                MixedEscapeDirectFrame::Block { block, stmt_idx } => {
                    for stmt in block.stmts.iter().skip(*stmt_idx + 1) {
                        self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
                    }
                    self.env.pop_scope();
                }
                MixedEscapeDirectFrame::IfThen {
                    then_block,
                    stmt_idx,
                    ..
                } => {
                    for stmt in then_block.stmts.iter().skip(*stmt_idx + 1) {
                        self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
                    }
                    self.env.pop_scope();
                }
                MixedEscapeDirectFrame::IfElse {
                    else_block,
                    stmt_idx,
                    ..
                } => {
                    for stmt in else_block.stmts.iter().skip(*stmt_idx + 1) {
                        self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
                    }
                    self.env.pop_scope();
                }
                MixedEscapeDirectFrame::WhileBody { while_body, .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (while tail needs dedicated lowering)",
                        at: while_body.span.into(),
                    });
                }
            }
        }
        Ok(())
    }

    fn codegen_mixed_escape_matrix_if_branch_prefix_to_indirect_site<'hir>(
        &mut self,
        site: &MixedEscapeIndirectSite<'hir>,
        top_stmt: &'hir hir::Stmt,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        let Some(first_frame) = site.resume_path.first() else {
            return Ok(());
        };
        let hir::StmtKind::Expr(expr) = &top_stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected if statement)",
                at: top_stmt.span.into(),
            });
        };
        let hir::ExprKind::If { .. } = &expr.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected if statement)",
                at: expr.span.into(),
            });
        };

        let branch_stmts = match first_frame {
            MixedEscapeDirectFrame::IfThen {
                if_expr,
                then_block,
                ..
            } => {
                if !std::ptr::eq(*if_expr, expr) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (if path mismatch)",
                        at: expr.span.into(),
                    });
                }
                &then_block.stmts
            }
            MixedEscapeDirectFrame::IfElse {
                if_expr,
                else_block,
                ..
            } => {
                if !std::ptr::eq(*if_expr, expr) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (if path mismatch)",
                        at: expr.span.into(),
                    });
                }
                &else_block.stmts
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected if branch site)",
                    at: site.decl.span.into(),
                });
            }
        };

        self.env.push_scope();
        self.codegen_mixed_escape_matrix_prefix_to_indirect_site_from_stmts(
            site,
            0,
            branch_stmts,
            body_lift_ids,
        )
    }

    fn codegen_mixed_escape_matrix_if_continue_after_indirect_site<'hir>(
        &mut self,
        site: &MixedEscapeIndirectSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        if site.resume_path.len() > 1 {
            self.codegen_mixed_escape_matrix_nested_tail_after_indirect_site_from_depth(
                site,
                1,
                body_lift_ids,
            )?;
        }

        match site.resume_path.first() {
            Some(MixedEscapeDirectFrame::IfThen {
                then_block,
                stmt_idx,
                ..
            }) => {
                for stmt in then_block.stmts.iter().skip(*stmt_idx + 1) {
                    self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
                }
                self.env.pop_scope();
                Ok(())
            }
            Some(MixedEscapeDirectFrame::IfElse {
                else_block,
                stmt_idx,
                ..
            }) => {
                for stmt in else_block.stmts.iter().skip(*stmt_idx + 1) {
                    self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
                }
                self.env.pop_scope();
                Ok(())
            }
            Some(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected if branch site)",
                at: site.decl.span.into(),
            }),
            None => Ok(()),
        }
    }

    fn codegen_mixed_escape_matrix_while_prefix_to_indirect_site<'hir>(
        &mut self,
        site: &MixedEscapeIndirectSite<'hir>,
        top_stmt: &'hir hir::Stmt,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        let Some(first_frame) = site.resume_path.first() else {
            return Ok(());
        };
        let MixedEscapeDirectFrame::WhileBody {
            while_cond: expected_cond,
            while_body: expected_body,
            stmt_idx: perform_stmt_idx,
        } = first_frame
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected while site)",
                at: site.decl.span.into(),
            });
        };
        if !Self::mixed_escape_while_nested_path_supported(&site.resume_path) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (deeper nested indirect site in while body not yet supported)",
                at: site.decl.span.into(),
            });
        }

        let hir::StmtKind::While { cond, body } = &top_stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected while statement)",
                at: top_stmt.span.into(),
            });
        };
        if !std::ptr::eq(cond, *expected_cond) || !std::ptr::eq(body, *expected_body) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (while path mismatch)",
                at: top_stmt.span.into(),
            });
        }

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: top_stmt.span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: top_stmt.span.into(),
            })?;
        let cond_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_while_prefix_cond");
        let body_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_while_prefix_body");
        let after_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_while_prefix_after");

        self.builder.build_unconditional_branch(cond_bb)?;

        self.builder.position_at_end(after_bb);
        self.builder.build_unreachable()?;

        self.builder.position_at_end(cond_bb);
        let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (while condition value)",
            at: cond.span.into(),
        })?;
        self.builder
            .build_conditional_branch(cond_i1, body_bb, after_bb)?;

        let base_env = self.env.clone();
        self.builder.position_at_end(body_bb);
        self.env = base_env;
        self.env.push_scope();

        for (idx, body_stmt) in body.stmts.iter().enumerate() {
            if idx < *perform_stmt_idx {
                self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
                continue;
            }

            if site.resume_path.len() == 1 {
                let hir::StmtKind::Val(decl) = &body_stmt.kind else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected indirect call binding)",
                        at: body_stmt.span.into(),
                    });
                };
                if !std::ptr::eq(decl, site.decl) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (while path mismatch)",
                        at: decl.span.into(),
                    });
                }
                return Ok(());
            }

            match site.resume_path[1] {
                MixedEscapeDirectFrame::Block {
                    block: expected_block,
                    ..
                } => {
                    let hir::StmtKind::Expr(expr) = &body_stmt.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (expected block statement)",
                            at: body_stmt.span.into(),
                        });
                    };
                    let hir::ExprKind::Block(block) = &expr.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (expected block statement)",
                            at: expr.span.into(),
                        });
                    };
                    if !std::ptr::eq(block, expected_block) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (while path mismatch)",
                            at: expr.span.into(),
                        });
                    }

                    self.env.push_scope();
                    return self.codegen_mixed_escape_matrix_prefix_to_indirect_site_from_stmts(
                        site,
                        1,
                        &block.stmts,
                        body_lift_ids,
                    );
                }
                MixedEscapeDirectFrame::IfThen { .. } | MixedEscapeDirectFrame::IfElse { .. } => {
                    let nested_site = MixedEscapeIndirectSite {
                        top_level_stmt_idx: site.top_level_stmt_idx,
                        decl: site.decl,
                        init: site.init,
                        id: site.id,
                        resume_path: site.resume_path[1..].to_vec(),
                    };
                    return self.codegen_mixed_escape_matrix_if_branch_prefix_to_indirect_site(
                        &nested_site,
                        body_stmt,
                        body_lift_ids,
                    );
                }
                MixedEscapeDirectFrame::WhileBody { .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (deeper nested indirect site in while body not yet supported)",
                        at: site.decl.span.into(),
                    });
                }
            }
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (indirect site missing)",
            at: site.decl.span.into(),
        })
    }

    fn codegen_mixed_escape_matrix_prefix_to_indirect_site<'hir>(
        &mut self,
        site: &MixedEscapeIndirectSite<'hir>,
        top_stmt: &'hir hir::Stmt,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        match site.resume_path.first() {
            None => Ok(()),
            Some(MixedEscapeDirectFrame::Block { .. }) => self
                .codegen_mixed_escape_matrix_nested_block_prefix_to_indirect_site(
                    site,
                    top_stmt,
                    body_lift_ids,
                ),
            Some(MixedEscapeDirectFrame::IfThen { .. } | MixedEscapeDirectFrame::IfElse { .. }) => {
                self.codegen_mixed_escape_matrix_if_branch_prefix_to_indirect_site(
                    site,
                    top_stmt,
                    body_lift_ids,
                )
            }
            Some(MixedEscapeDirectFrame::WhileBody { .. }) => self
                .codegen_mixed_escape_matrix_while_prefix_to_indirect_site(
                    site,
                    top_stmt,
                    body_lift_ids,
                ),
        }
    }

    fn codegen_mixed_escape_matrix_continue_after_indirect_site<'hir>(
        &mut self,
        site: &MixedEscapeIndirectSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        match site.resume_path.first() {
            None => Ok(()),
            Some(MixedEscapeDirectFrame::Block { .. }) => self
                .codegen_mixed_escape_matrix_nested_block_continue_after_indirect_site(
                    site,
                    body_lift_ids,
                ),
            Some(MixedEscapeDirectFrame::IfThen { .. } | MixedEscapeDirectFrame::IfElse { .. }) => {
                self.codegen_mixed_escape_matrix_if_continue_after_indirect_site(
                    site,
                    body_lift_ids,
                )
            }
            Some(MixedEscapeDirectFrame::WhileBody { .. }) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (while body indirect re-entry needs dedicated lowering)",
                    at: site.decl.span.into(),
                })
            }
        }
    }

    fn codegen_mixed_escape_matrix_emit_indirect_site_binding<'hir>(
        &mut self,
        site: &MixedEscapeIndirectSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        if body_lift_ids.contains(&site.id) {
            let decl_ty =
                self.cg_ty_of(site.decl.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "lifted local type",
                        at: site.decl.span.into(),
                    })?;
            let target_ptr = if let Some(local) = self.env.get(site.id) {
                if local.ty != decl_ty {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "lifted local type",
                        at: site.decl.span.into(),
                    });
                }
                local.ptr
            } else {
                let name = site.decl.name.as_deref().unwrap_or("v");
                let ptr = self.create_entry_alloca(site.decl.span, name, decl_ty)?;
                self.env.insert(
                    site.id,
                    CgLocal {
                        hir_ty: Some(site.decl.ty),
                        ty: decl_ty,
                        ptr,
                        mutable: site.decl.mutable,
                    },
                );
                ptr
            };
            let v = self.codegen_expr_in_expected_context(site.init, Some(decl_ty))?;
            let _ = self.store_local_value(site.decl.span, target_ptr, decl_ty, v)?;
        } else {
            self.codegen_val_decl(site.decl)?;
        }
        Ok(())
    }

    fn codegen_mixed_escape_matrix_while_site_stmt<'hir, F>(
        &mut self,
        body_stmt: &'hir hir::Stmt,
        site_pc: usize,
        site: &MixedEscapeDirectSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        emit_intercept: &mut F,
    ) -> Result<(), LlvmEmitError>
    where
        F: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        if site.resume_path.len() == 1 {
            let hir::StmtKind::Val(decl) = &body_stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected perform binding)",
                    at: body_stmt.span.into(),
                });
            };
            if !std::ptr::eq(decl, site.decl) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (while path mismatch)",
                    at: decl.span.into(),
                });
            }
            emit_intercept(self, site_pc, site)?;
            return Ok(());
        }

        match site.resume_path[1] {
            MixedEscapeDirectFrame::Block {
                block: expected_block,
                ..
            } => {
                let hir::StmtKind::Expr(expr) = &body_stmt.kind else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected block statement)",
                        at: body_stmt.span.into(),
                    });
                };
                let hir::ExprKind::Block(block) = &expr.kind else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected block statement)",
                        at: expr.span.into(),
                    });
                };
                if !std::ptr::eq(block, expected_block) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (while path mismatch)",
                        at: expr.span.into(),
                    });
                }

                self.env.push_scope();
                self.codegen_mixed_escape_matrix_prefix_from_stmts(
                    site,
                    1,
                    &block.stmts,
                    body_lift_ids,
                )?;
                emit_intercept(self, site_pc, site)?;
                Ok(())
            }
            MixedEscapeDirectFrame::IfThen { .. } | MixedEscapeDirectFrame::IfElse { .. } => {
                let nested_site = MixedEscapeDirectSite {
                    top_level_stmt_idx: site.top_level_stmt_idx,
                    decl: site.decl,
                    args: site.args,
                    id: site.id,
                    resume_path: site.resume_path[1..].to_vec(),
                };
                let nested_escape_sites = [MatrixEscapeSite {
                    stmt_idx: 0,
                    decl: site.decl,
                    id: site.id,
                    kind: MatrixEscapeSiteKind::Direct { site: nested_site },
                }];
                self.codegen_mixed_escape_matrix_if_stmt_direct_sites(
                    body_stmt,
                    &[0],
                    &nested_escape_sites,
                    body_lift_ids,
                    |cg, _nested_pc, _nested_site| emit_intercept(cg, site_pc, site),
                )
            }
            MixedEscapeDirectFrame::WhileBody { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (nested while direct site in while body not yet supported)",
                at: site.decl.span.into(),
            }),
        }
    }

    fn codegen_mixed_escape_matrix_while_indirect_site_stmt<'hir, F>(
        &mut self,
        body_stmt: &'hir hir::Stmt,
        site_pc: usize,
        site: &MixedEscapeIndirectSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        emit_site: &mut F,
    ) -> Result<(), LlvmEmitError>
    where
        F: FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        if site.resume_path.len() == 1 {
            let hir::StmtKind::Val(decl) = &body_stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected indirect call binding)",
                    at: body_stmt.span.into(),
                });
            };
            if !std::ptr::eq(decl, site.decl) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (while path mismatch)",
                    at: decl.span.into(),
                });
            }
            emit_site(self, site_pc, site)?;
            return Ok(());
        }

        match site.resume_path[1] {
            MixedEscapeDirectFrame::Block {
                block: expected_block,
                ..
            } => {
                let hir::StmtKind::Expr(expr) = &body_stmt.kind else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected block statement)",
                        at: body_stmt.span.into(),
                    });
                };
                let hir::ExprKind::Block(block) = &expr.kind else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected block statement)",
                        at: expr.span.into(),
                    });
                };
                if !std::ptr::eq(block, expected_block) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (while path mismatch)",
                        at: expr.span.into(),
                    });
                }

                self.env.push_scope();
                self.codegen_mixed_escape_matrix_prefix_to_indirect_site_from_stmts(
                    site,
                    1,
                    &block.stmts,
                    body_lift_ids,
                )?;
                emit_site(self, site_pc, site)?;
                Ok(())
            }
            MixedEscapeDirectFrame::IfThen { .. } | MixedEscapeDirectFrame::IfElse { .. } => {
                let nested_site = MixedEscapeIndirectSite {
                    top_level_stmt_idx: site.top_level_stmt_idx,
                    decl: site.decl,
                    init: site.init,
                    id: site.id,
                    resume_path: site.resume_path[1..].to_vec(),
                };
                let nested_escape_sites = [MatrixEscapeSite {
                    stmt_idx: 0,
                    decl: site.decl,
                    id: site.id,
                    kind: MatrixEscapeSiteKind::Indirect { site: nested_site },
                }];
                self.codegen_mixed_escape_matrix_if_stmt_indirect_sites(
                    body_stmt,
                    &[0],
                    &nested_escape_sites,
                    body_lift_ids,
                    |cg, _nested_pc, _nested_site| emit_site(cg, site_pc, site),
                )
            }
            MixedEscapeDirectFrame::WhileBody { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (deeper nested indirect site in while body not yet supported)",
                at: site.decl.span.into(),
            }),
        }
    }

    fn codegen_mixed_escape_matrix_while_stmt_indirect_site<'hir, F>(
        &mut self,
        stmt: &'hir hir::Stmt,
        site_pc: usize,
        site: &MixedEscapeIndirectSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        mut emit_site: F,
    ) -> Result<(), LlvmEmitError>
    where
        F: FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let Some(first_frame) = site.resume_path.first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (missing while path)",
                at: site.decl.span.into(),
            });
        };
        let MixedEscapeDirectFrame::WhileBody {
            while_cond: expected_cond,
            while_body: expected_body,
            stmt_idx: perform_stmt_idx,
        } = first_frame
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected while site)",
                at: site.decl.span.into(),
            });
        };
        if !Self::mixed_escape_while_nested_path_supported(&site.resume_path) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (deeper nested indirect site in while body not yet supported)",
                at: site.decl.span.into(),
            });
        }

        let hir::StmtKind::While { cond, body } = &stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected while statement)",
                at: stmt.span.into(),
            });
        };
        if !std::ptr::eq(cond, *expected_cond) || !std::ptr::eq(body, *expected_body) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (while path mismatch)",
                at: stmt.span.into(),
            });
        }

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: stmt.span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: stmt.span.into(),
            })?;
        let cond_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_while_cond");
        let body_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_while_body");
        let after_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_while_after");

        self.builder.build_unconditional_branch(cond_bb)?;

        self.builder.position_at_end(cond_bb);
        let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (while condition value)",
            at: cond.span.into(),
        })?;
        self.builder
            .build_conditional_branch(cond_i1, body_bb, after_bb)?;

        let base_env = self.env.clone();

        self.builder.position_at_end(body_bb);
        self.env = base_env.clone();
        self.env.push_scope();

        for (idx, body_stmt) in body.stmts.iter().enumerate() {
            if idx < *perform_stmt_idx {
                self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
                continue;
            }

            self.codegen_mixed_escape_matrix_while_indirect_site_stmt(
                body_stmt,
                site_pc,
                site,
                body_lift_ids,
                &mut emit_site,
            )?;
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                if matches!(
                    site.resume_path.get(1),
                    Some(MixedEscapeDirectFrame::Block { .. })
                ) {
                    self.codegen_mixed_escape_matrix_nested_tail_after_indirect_site_from_depth(
                        site,
                        1,
                        body_lift_ids,
                    )?;
                }
                for body_stmt in body.stmts.iter().skip(*perform_stmt_idx + 1) {
                    self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
                }
                self.env.pop_scope();
                self.builder.build_unconditional_branch(cond_bb)?;
            }
            self.env = base_env;
            self.builder.position_at_end(after_bb);
            return Ok(());
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (indirect site missing)",
            at: site.decl.span.into(),
        })
    }

    fn codegen_mixed_escape_matrix_if_stmt_direct_sites<'hir, F>(
        &mut self,
        stmt: &'hir hir::Stmt,
        direct_site_pcs: &[usize],
        escape_sites: &[MatrixEscapeSite<'hir>],
        body_lift_ids: &HashSet<hir::SymbolId>,
        mut emit_intercept: F,
    ) -> Result<(), LlvmEmitError>
    where
        F: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let hir::StmtKind::Expr(expr) = &stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected if statement)",
                at: stmt.span.into(),
            });
        };
        let hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } = &expr.kind
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected if statement)",
                at: expr.span.into(),
            });
        };

        let mut then_site: Option<(usize, &MixedEscapeDirectSite<'hir>)> = None;
        let mut else_site: Option<(usize, &MixedEscapeDirectSite<'hir>)> = None;
        for &site_pc in direct_site_pcs {
            let MatrixEscapeSiteKind::Direct { site } = &escape_sites[site_pc].kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected direct site)",
                    at: stmt.span.into(),
                });
            };
            let Some(first_frame) = site.resume_path.first() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (missing if branch path)",
                    at: site.decl.span.into(),
                });
            };
            match first_frame {
                MixedEscapeDirectFrame::IfThen { if_expr, .. } => {
                    if !std::ptr::eq(*if_expr, expr) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (if path mismatch)",
                            at: site.decl.span.into(),
                        });
                    }
                    if then_site.replace((site_pc, site)).is_some() {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple direct sites in the same if-then branch not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
                MixedEscapeDirectFrame::IfElse { if_expr, .. } => {
                    if !std::ptr::eq(*if_expr, expr) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (if path mismatch)",
                            at: site.decl.span.into(),
                        });
                    }
                    if else_site.replace((site_pc, site)).is_some() {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple direct sites in the same if-else branch not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
                MixedEscapeDirectFrame::Block { .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected if branch site)",
                        at: site.decl.span.into(),
                    });
                }
                MixedEscapeDirectFrame::WhileBody { .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected if branch site)",
                        at: site.decl.span.into(),
                    });
                }
            }
        }

        let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (if condition value)",
            at: cond.span.into(),
        })?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: expr.span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: expr.span.into(),
            })?;
        let then_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_if_then");
        let after_if_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_if_after");
        let else_bb = if else_branch.is_some() || else_site.is_some() {
            Some(
                self.context
                    .append_basic_block(func, "mixed_escape_matrix_if_else"),
            )
        } else {
            None
        };
        self.builder
            .build_conditional_branch(cond_i1, then_bb, else_bb.unwrap_or(after_if_bb))?;

        let base_env = self.env.clone();

        self.builder.position_at_end(then_bb);
        self.env = base_env.clone();
        if let Some((site_pc, site)) = then_site {
            let MixedEscapeDirectFrame::IfThen { then_block, .. } = site.resume_path[0] else {
                unreachable!("validated if-then site");
            };
            self.env.push_scope();
            self.codegen_mixed_escape_matrix_prefix_from_stmts(
                site,
                0,
                &then_block.stmts,
                body_lift_ids,
            )?;
            emit_intercept(self, site_pc, site)?;
        } else {
            let _ = self.codegen_expr(then_branch)?;
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                self.builder.build_unconditional_branch(after_if_bb)?;
            }
        }

        if let Some(else_bb) = else_bb {
            self.builder.position_at_end(else_bb);
            self.env = base_env.clone();
            if let Some((site_pc, site)) = else_site {
                let MixedEscapeDirectFrame::IfElse { else_block, .. } = site.resume_path[0] else {
                    unreachable!("validated if-else site");
                };
                self.env.push_scope();
                self.codegen_mixed_escape_matrix_prefix_from_stmts(
                    site,
                    0,
                    &else_block.stmts,
                    body_lift_ids,
                )?;
                emit_intercept(self, site_pc, site)?;
            } else if let Some(else_expr) = else_branch.as_deref() {
                let _ = self.codegen_expr(else_expr)?;
                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    self.builder.build_unconditional_branch(after_if_bb)?;
                }
            } else {
                self.builder.build_unconditional_branch(after_if_bb)?;
            }
        }

        self.env = base_env;
        self.builder.position_at_end(after_if_bb);
        Ok(())
    }

    fn codegen_mixed_escape_matrix_if_stmt_indirect_sites<'hir, F>(
        &mut self,
        stmt: &'hir hir::Stmt,
        indirect_site_pcs: &[usize],
        escape_sites: &[MatrixEscapeSite<'hir>],
        body_lift_ids: &HashSet<hir::SymbolId>,
        mut emit_site: F,
    ) -> Result<(), LlvmEmitError>
    where
        F: FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let hir::StmtKind::Expr(expr) = &stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected if statement)",
                at: stmt.span.into(),
            });
        };
        let hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } = &expr.kind
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected if statement)",
                at: expr.span.into(),
            });
        };

        let mut then_site: Option<(usize, &MixedEscapeIndirectSite<'hir>)> = None;
        let mut else_site: Option<(usize, &MixedEscapeIndirectSite<'hir>)> = None;
        for &site_pc in indirect_site_pcs {
            let MatrixEscapeSiteKind::Indirect { site } = &escape_sites[site_pc].kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected indirect site)",
                    at: stmt.span.into(),
                });
            };
            let Some(first_frame) = site.resume_path.first() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (missing if branch path)",
                    at: site.decl.span.into(),
                });
            };
            match first_frame {
                MixedEscapeDirectFrame::IfThen { if_expr, .. } => {
                    if !std::ptr::eq(*if_expr, expr) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (if path mismatch)",
                            at: site.decl.span.into(),
                        });
                    }
                    if then_site.replace((site_pc, site)).is_some() {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple indirect sites in the same if-then branch not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
                MixedEscapeDirectFrame::IfElse { if_expr, .. } => {
                    if !std::ptr::eq(*if_expr, expr) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (if path mismatch)",
                            at: site.decl.span.into(),
                        });
                    }
                    if else_site.replace((site_pc, site)).is_some() {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple indirect sites in the same if-else branch not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
                MixedEscapeDirectFrame::Block { .. } | MixedEscapeDirectFrame::WhileBody { .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected if branch site)",
                        at: site.decl.span.into(),
                    });
                }
            }
        }

        let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (if condition value)",
            at: cond.span.into(),
        })?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: expr.span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: expr.span.into(),
            })?;
        let then_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_if_then");
        let after_if_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_if_after");
        let else_bb = if else_branch.is_some() || else_site.is_some() {
            Some(
                self.context
                    .append_basic_block(func, "mixed_escape_matrix_if_else"),
            )
        } else {
            None
        };
        self.builder
            .build_conditional_branch(cond_i1, then_bb, else_bb.unwrap_or(after_if_bb))?;

        let base_env = self.env.clone();

        self.builder.position_at_end(then_bb);
        self.env = base_env.clone();
        if let Some((site_pc, site)) = then_site {
            self.codegen_mixed_escape_matrix_if_branch_prefix_to_indirect_site(
                site,
                stmt,
                body_lift_ids,
            )?;
            emit_site(self, site_pc, site)?;
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                self.codegen_mixed_escape_matrix_if_continue_after_indirect_site(
                    site,
                    body_lift_ids,
                )?;
                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    self.builder.build_unconditional_branch(after_if_bb)?;
                }
            }
        } else {
            let _ = self.codegen_expr(then_branch)?;
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                self.builder.build_unconditional_branch(after_if_bb)?;
            }
        }

        if let Some(else_bb) = else_bb {
            self.builder.position_at_end(else_bb);
            self.env = base_env.clone();
            if let Some((site_pc, site)) = else_site {
                self.codegen_mixed_escape_matrix_if_branch_prefix_to_indirect_site(
                    site,
                    stmt,
                    body_lift_ids,
                )?;
                emit_site(self, site_pc, site)?;
                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    self.codegen_mixed_escape_matrix_if_continue_after_indirect_site(
                        site,
                        body_lift_ids,
                    )?;
                    if let Some(bb) = self.builder.get_insert_block()
                        && bb.get_terminator().is_none()
                    {
                        self.builder.build_unconditional_branch(after_if_bb)?;
                    }
                }
            } else if let Some(else_expr) = else_branch.as_deref() {
                let _ = self.codegen_expr(else_expr)?;
                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    self.builder.build_unconditional_branch(after_if_bb)?;
                }
            } else {
                self.builder.build_unconditional_branch(after_if_bb)?;
            }
        }

        self.env = base_env;
        self.builder.position_at_end(after_if_bb);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mixed_escape_matrix_if_stmt_mixed_sites<'hir, FDirect, FIndirect>(
        &mut self,
        stmt: &'hir hir::Stmt,
        site_pcs: &[usize],
        escape_sites: &[MatrixEscapeSite<'hir>],
        if_next_site_pc_by_pc: &HashMap<usize, usize>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        mut emit_direct: FDirect,
        mut emit_indirect: FIndirect,
    ) -> Result<(), LlvmEmitError>
    where
        FDirect: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
        FIndirect:
            FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let hir::StmtKind::Expr(expr) = &stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected if statement)",
                at: stmt.span.into(),
            });
        };
        let hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } = &expr.kind
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected if statement)",
                at: expr.span.into(),
            });
        };

        let mut then_direct: Option<(usize, &MixedEscapeDirectSite<'hir>)> = None;
        let mut then_indirect: Option<(usize, &MixedEscapeIndirectSite<'hir>)> = None;
        let mut else_direct: Option<(usize, &MixedEscapeDirectSite<'hir>)> = None;
        let mut else_indirect: Option<(usize, &MixedEscapeIndirectSite<'hir>)> = None;

        for &site_pc in site_pcs {
            match &escape_sites[site_pc].kind {
                MatrixEscapeSiteKind::Direct { site } => {
                    if !Self::mixed_escape_if_branch_path_supported(&site.resume_path) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (richer if-branch mixed sites not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                    let Some(first_frame) = site.resume_path.first() else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (missing if branch path)",
                            at: site.decl.span.into(),
                        });
                    };
                    match first_frame {
                        MixedEscapeDirectFrame::IfThen { if_expr, .. } => {
                            if !std::ptr::eq(*if_expr, expr) {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (if path mismatch)",
                                    at: site.decl.span.into(),
                                });
                            }
                            if then_direct.replace((site_pc, site)).is_some() {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (multiple direct sites in the same if-then branch not yet supported)",
                                    at: site.decl.span.into(),
                                });
                            }
                        }
                        MixedEscapeDirectFrame::IfElse { if_expr, .. } => {
                            if !std::ptr::eq(*if_expr, expr) {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (if path mismatch)",
                                    at: site.decl.span.into(),
                                });
                            }
                            if else_direct.replace((site_pc, site)).is_some() {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (multiple direct sites in the same if-else branch not yet supported)",
                                    at: site.decl.span.into(),
                                });
                            }
                        }
                        MixedEscapeDirectFrame::Block { .. }
                        | MixedEscapeDirectFrame::WhileBody { .. } => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (expected if branch site)",
                                at: site.decl.span.into(),
                            });
                        }
                    }
                }
                MatrixEscapeSiteKind::Indirect { site } => {
                    if !Self::mixed_escape_if_branch_path_supported(&site.resume_path) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (richer if-branch mixed sites not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                    let Some(first_frame) = site.resume_path.first() else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (missing if branch path)",
                            at: site.decl.span.into(),
                        });
                    };
                    match first_frame {
                        MixedEscapeDirectFrame::IfThen { if_expr, .. } => {
                            if !std::ptr::eq(*if_expr, expr) {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (if path mismatch)",
                                    at: site.decl.span.into(),
                                });
                            }
                            if then_indirect.replace((site_pc, site)).is_some() {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (multiple indirect sites in the same if-then branch not yet supported)",
                                    at: site.decl.span.into(),
                                });
                            }
                        }
                        MixedEscapeDirectFrame::IfElse { if_expr, .. } => {
                            if !std::ptr::eq(*if_expr, expr) {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (if path mismatch)",
                                    at: site.decl.span.into(),
                                });
                            }
                            if else_indirect.replace((site_pc, site)).is_some() {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (multiple indirect sites in the same if-else branch not yet supported)",
                                    at: site.decl.span.into(),
                                });
                            }
                        }
                        MixedEscapeDirectFrame::Block { .. }
                        | MixedEscapeDirectFrame::WhileBody { .. } => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (expected if branch site)",
                                at: site.decl.span.into(),
                            });
                        }
                    }
                }
            }
        }

        let choose_first = |direct: Option<(usize, &MixedEscapeDirectSite<'hir>)>,
                            indirect: Option<(usize, &MixedEscapeIndirectSite<'hir>)>|
         -> Result<Option<usize>, LlvmEmitError> {
            match (direct, indirect) {
                (Some((direct_pc, direct_site)), Some((indirect_pc, indirect_site))) => {
                    match Self::mixed_escape_matrix_stmt_path_cmp(
                        &direct_site.resume_path,
                        &indirect_site.resume_path,
                    ) {
                        std::cmp::Ordering::Less => Ok(Some(direct_pc)),
                        std::cmp::Ordering::Greater => Ok(Some(indirect_pc)),
                        std::cmp::Ordering::Equal => Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (if mixed site order ambiguous)",
                            at: direct_site.decl.span.into(),
                        }),
                    }
                }
                (Some((direct_pc, _)), None) => Ok(Some(direct_pc)),
                (None, Some((indirect_pc, _))) => Ok(Some(indirect_pc)),
                (None, None) => Ok(None),
            }
        };

        let then_first_pc = choose_first(then_direct, then_indirect)?;
        let else_first_pc = choose_first(else_direct, else_indirect)?;

        let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (if condition value)",
            at: cond.span.into(),
        })?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: expr.span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: expr.span.into(),
            })?;
        let then_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_if_then");
        let after_if_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_if_after");
        let else_bb = if else_branch.is_some() || else_first_pc.is_some() {
            Some(
                self.context
                    .append_basic_block(func, "mixed_escape_matrix_if_else"),
            )
        } else {
            None
        };
        self.builder
            .build_conditional_branch(cond_i1, then_bb, else_bb.unwrap_or(after_if_bb))?;

        let base_env = self.env.clone();

        self.builder.position_at_end(then_bb);
        self.env = base_env.clone();
        if let Some(site_pc) = then_first_pc {
            match &escape_sites[site_pc].kind {
                MatrixEscapeSiteKind::Direct { site } => {
                    self.codegen_mixed_escape_matrix_if_branch_prefix_to_site(
                        site,
                        stmt,
                        body_lift_ids,
                    )?;
                    emit_direct(self, site_pc, site)?;
                }
                MatrixEscapeSiteKind::Indirect { site } => {
                    self.codegen_mixed_escape_matrix_if_branch_prefix_to_indirect_site(
                        site,
                        stmt,
                        body_lift_ids,
                    )?;
                    emit_indirect(self, site_pc, site)?;
                    if let Some(bb) = self.builder.get_insert_block()
                        && bb.get_terminator().is_none()
                    {
                        if let Some(&next_pc) = if_next_site_pc_by_pc.get(&site_pc) {
                            let next_site = &escape_sites[next_pc];
                            self.codegen_mixed_escape_matrix_continue_to_next_if_site_after_indirect(
                                site,
                                next_pc,
                                next_site,
                                body_lift_ids,
                                &mut emit_direct,
                                &mut emit_indirect,
                            )?;
                            if let MatrixEscapeSiteKind::Indirect {
                                site: next_indirect_site,
                            } = &next_site.kind
                                && let Some(bb) = self.builder.get_insert_block()
                                && bb.get_terminator().is_none()
                            {
                                self.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                    next_indirect_site,
                                    body_lift_ids,
                                )?;
                            }
                        } else {
                            self.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                site,
                                body_lift_ids,
                            )?;
                        }
                        if let Some(bb) = self.builder.get_insert_block()
                            && bb.get_terminator().is_none()
                        {
                            self.builder.build_unconditional_branch(after_if_bb)?;
                        }
                    }
                }
            }
        } else {
            let _ = self.codegen_expr(then_branch)?;
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                self.builder.build_unconditional_branch(after_if_bb)?;
            }
        }

        if let Some(else_bb) = else_bb {
            self.builder.position_at_end(else_bb);
            self.env = base_env.clone();
            if let Some(site_pc) = else_first_pc {
                match &escape_sites[site_pc].kind {
                    MatrixEscapeSiteKind::Direct { site } => {
                        self.codegen_mixed_escape_matrix_if_branch_prefix_to_site(
                            site,
                            stmt,
                            body_lift_ids,
                        )?;
                        emit_direct(self, site_pc, site)?;
                    }
                    MatrixEscapeSiteKind::Indirect { site } => {
                        self.codegen_mixed_escape_matrix_if_branch_prefix_to_indirect_site(
                            site,
                            stmt,
                            body_lift_ids,
                        )?;
                        emit_indirect(self, site_pc, site)?;
                        if let Some(bb) = self.builder.get_insert_block()
                            && bb.get_terminator().is_none()
                        {
                            if let Some(&next_pc) = if_next_site_pc_by_pc.get(&site_pc) {
                                let next_site = &escape_sites[next_pc];
                                self.codegen_mixed_escape_matrix_continue_to_next_if_site_after_indirect(
                                    site,
                                    next_pc,
                                    next_site,
                                    body_lift_ids,
                                    &mut emit_direct,
                                    &mut emit_indirect,
                                )?;
                                if let MatrixEscapeSiteKind::Indirect {
                                    site: next_indirect_site,
                                } = &next_site.kind
                                    && let Some(bb) = self.builder.get_insert_block()
                                    && bb.get_terminator().is_none()
                                {
                                    self.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                        next_indirect_site,
                                        body_lift_ids,
                                    )?;
                                }
                            } else {
                                self.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                    site,
                                    body_lift_ids,
                                )?;
                            }
                            if let Some(bb) = self.builder.get_insert_block()
                                && bb.get_terminator().is_none()
                            {
                                self.builder.build_unconditional_branch(after_if_bb)?;
                            }
                        }
                    }
                }
            } else if let Some(else_expr) = else_branch.as_deref() {
                let _ = self.codegen_expr(else_expr)?;
                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    self.builder.build_unconditional_branch(after_if_bb)?;
                }
            } else {
                self.builder.build_unconditional_branch(after_if_bb)?;
            }
        }

        self.env = base_env;
        self.builder.position_at_end(after_if_bb);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mixed_escape_matrix_block_stmt_mixed_sites<'hir, FDirect, FIndirect>(
        &mut self,
        stmt: &'hir hir::Stmt,
        direct_pc: usize,
        direct_site: &MixedEscapeDirectSite<'hir>,
        indirect_pc: usize,
        indirect_site: &MixedEscapeIndirectSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        emit_direct: &mut FDirect,
        emit_indirect: &mut FIndirect,
    ) -> Result<(), LlvmEmitError>
    where
        FDirect: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
        FIndirect:
            FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let direct_nested = MixedEscapeDirectSite {
            top_level_stmt_idx: direct_site.top_level_stmt_idx,
            decl: direct_site.decl,
            args: direct_site.args,
            id: direct_site.id,
            resume_path: direct_site.resume_path[1..].to_vec(),
        };
        let indirect_nested = MixedEscapeIndirectSite {
            top_level_stmt_idx: indirect_site.top_level_stmt_idx,
            decl: indirect_site.decl,
            init: indirect_site.init,
            id: indirect_site.id,
            resume_path: indirect_site.resume_path[1..].to_vec(),
        };
        if !Self::mixed_escape_block_only_path_supported(&direct_nested.resume_path)
            || !Self::mixed_escape_block_only_path_supported(&indirect_nested.resume_path)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only same-body-stmt direct / indirect coexistence in while body supported)",
                at: indirect_site.decl.span.into(),
            });
        }

        let local_sites = [
            MatrixEscapeSite {
                stmt_idx: 0,
                decl: direct_nested.decl,
                id: direct_nested.id,
                kind: MatrixEscapeSiteKind::Direct {
                    site: direct_nested.clone(),
                },
            },
            MatrixEscapeSite {
                stmt_idx: 0,
                decl: indirect_nested.decl,
                id: indirect_nested.id,
                kind: MatrixEscapeSiteKind::Indirect {
                    site: indirect_nested.clone(),
                },
            },
        ];

        match Self::mixed_escape_matrix_stmt_path_cmp(
            &direct_site.resume_path,
            &indirect_site.resume_path,
        ) {
            std::cmp::Ordering::Less => {
                self.codegen_mixed_escape_matrix_nested_block_prefix_to_site(
                    &direct_nested,
                    stmt,
                    body_lift_ids,
                )?;
                emit_direct(self, direct_pc, direct_site)?;
                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    let mut unexpected_direct =
                        |_cg: &mut Self,
                         _next_pc: usize,
                         _next_direct: &MixedEscapeDirectSite<'hir>| {
                            Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (unexpected direct site while continuing mixed block)",
                                at: indirect_site.decl.span.into(),
                            })
                        };
                    let mut emit_expected_indirect =
                        |cg: &mut Self,
                         _next_pc: usize,
                         _next_indirect: &MixedEscapeIndirectSite<'hir>| {
                            emit_indirect(cg, indirect_pc, indirect_site)
                        };
                    self.codegen_mixed_escape_matrix_continue_to_next_block_site_after_direct(
                        &direct_nested,
                        1,
                        &local_sites[1],
                        body_lift_ids,
                        &mut unexpected_direct,
                        &mut emit_expected_indirect,
                    )?;
                    if let MatrixEscapeSiteKind::Indirect {
                        site: next_indirect_site,
                    } = &local_sites[1].kind
                        && let Some(bb) = self.builder.get_insert_block()
                        && bb.get_terminator().is_none()
                    {
                        self.codegen_mixed_escape_matrix_nested_block_continue_after_indirect_site(
                            next_indirect_site,
                            body_lift_ids,
                        )?;
                    }
                }
            }
            std::cmp::Ordering::Greater => {
                self.codegen_mixed_escape_matrix_nested_block_prefix_to_indirect_site(
                    &indirect_nested,
                    stmt,
                    body_lift_ids,
                )?;
                emit_indirect(self, indirect_pc, indirect_site)?;
                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    let mut emit_expected_direct =
                        |cg: &mut Self,
                         _next_pc: usize,
                         _next_direct: &MixedEscapeDirectSite<'hir>| {
                            emit_direct(cg, direct_pc, direct_site)
                        };
                    let mut unexpected_indirect = |_cg: &mut Self,
                                                   _next_pc: usize,
                                                   _next_indirect: &MixedEscapeIndirectSite<
                        'hir,
                    >| {
                        Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (unexpected indirect site while continuing mixed block)",
                            at: direct_site.decl.span.into(),
                        })
                    };
                    self.codegen_mixed_escape_matrix_continue_to_next_block_site_after_indirect(
                        &indirect_nested,
                        0,
                        &local_sites[0],
                        body_lift_ids,
                        &mut emit_expected_direct,
                        &mut unexpected_indirect,
                    )?;
                    if let MatrixEscapeSiteKind::Direct {
                        site: next_direct_site,
                    } = &local_sites[0].kind
                        && let Some(bb) = self.builder.get_insert_block()
                        && bb.get_terminator().is_none()
                    {
                        self.codegen_mixed_escape_matrix_nested_block_tail_after_site(
                            next_direct_site,
                            body_lift_ids,
                        )?;
                    }
                }
            }
            std::cmp::Ordering::Equal => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (while mixed site order ambiguous)",
                    at: direct_site.decl.span.into(),
                });
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_mixed_escape_matrix_while_stmt_mixed_sites<'hir, FDirect, FIndirect>(
        &mut self,
        stmt: &'hir hir::Stmt,
        site_pcs: &[usize],
        escape_sites: &[MatrixEscapeSite<'hir>],
        body_lift_ids: &HashSet<hir::SymbolId>,
        mut emit_direct: FDirect,
        mut emit_indirect: FIndirect,
    ) -> Result<(), LlvmEmitError>
    where
        FDirect: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
        FIndirect:
            FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        if site_pcs.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only same-body-stmt direct / indirect coexistence in while body supported)",
                at: stmt.span.into(),
            });
        }

        let mut direct: Option<(usize, &MixedEscapeDirectSite<'hir>)> = None;
        let mut indirect: Option<(usize, &MixedEscapeIndirectSite<'hir>)> = None;
        for &site_pc in site_pcs {
            match &escape_sites[site_pc].kind {
                MatrixEscapeSiteKind::Direct { site } => {
                    if direct.replace((site_pc, site)).is_some() {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple direct sites in the same while body not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
                MatrixEscapeSiteKind::Indirect { site } => {
                    if indirect.replace((site_pc, site)).is_some() {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple indirect sites in the same while body not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                }
            }
        }

        let Some((direct_pc, direct_site)) = direct else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected direct site)",
                at: stmt.span.into(),
            });
        };
        let Some((indirect_pc, indirect_site)) = indirect else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected indirect site)",
                at: stmt.span.into(),
            });
        };
        if !Self::mixed_escape_while_same_stmt_mixed_path_supported(
            &direct_site.resume_path,
            &indirect_site.resume_path,
        ) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only same-body-stmt direct / indirect coexistence in while body supported)",
                at: stmt.span.into(),
            });
        }

        let Some(first_frame) = direct_site.resume_path.first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (missing while path)",
                at: direct_site.decl.span.into(),
            });
        };
        let MixedEscapeDirectFrame::WhileBody {
            while_cond: expected_cond,
            while_body: expected_body,
            stmt_idx: mixed_stmt_idx,
        } = first_frame
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected while site)",
                at: direct_site.decl.span.into(),
            });
        };
        let hir::StmtKind::While { cond, body } = &stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected while statement)",
                at: stmt.span.into(),
            });
        };
        if !std::ptr::eq(cond, *expected_cond) || !std::ptr::eq(body, *expected_body) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (while path mismatch)",
                at: stmt.span.into(),
            });
        }

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: stmt.span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: stmt.span.into(),
            })?;
        let cond_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_while_mixed_cond");
        let body_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_while_mixed_body");
        let after_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_while_mixed_after");

        self.builder.build_unconditional_branch(cond_bb)?;

        self.builder.position_at_end(cond_bb);
        let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (while condition value)",
            at: cond.span.into(),
        })?;
        self.builder
            .build_conditional_branch(cond_i1, body_bb, after_bb)?;

        let base_env = self.env.clone();

        self.builder.position_at_end(body_bb);
        self.env = base_env.clone();
        self.env.push_scope();

        for (idx, body_stmt) in body.stmts.iter().enumerate() {
            if idx < *mixed_stmt_idx {
                self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
                continue;
            }

            match (
                direct_site.resume_path.get(1),
                indirect_site.resume_path.get(1),
            ) {
                (
                    Some(MixedEscapeDirectFrame::Block { .. }),
                    Some(MixedEscapeDirectFrame::Block { .. }),
                ) => {
                    self.codegen_mixed_escape_matrix_block_stmt_mixed_sites(
                        body_stmt,
                        direct_pc,
                        direct_site,
                        indirect_pc,
                        indirect_site,
                        body_lift_ids,
                        &mut emit_direct,
                        &mut emit_indirect,
                    )?;
                }
                (
                    Some(
                        MixedEscapeDirectFrame::IfThen { .. }
                        | MixedEscapeDirectFrame::IfElse { .. },
                    ),
                    Some(
                        MixedEscapeDirectFrame::IfThen { .. }
                        | MixedEscapeDirectFrame::IfElse { .. },
                    ),
                ) => {
                    let direct_nested = MixedEscapeDirectSite {
                        top_level_stmt_idx: direct_site.top_level_stmt_idx,
                        decl: direct_site.decl,
                        args: direct_site.args,
                        id: direct_site.id,
                        resume_path: direct_site.resume_path[1..].to_vec(),
                    };
                    let indirect_nested = MixedEscapeIndirectSite {
                        top_level_stmt_idx: indirect_site.top_level_stmt_idx,
                        decl: indirect_site.decl,
                        init: indirect_site.init,
                        id: indirect_site.id,
                        resume_path: indirect_site.resume_path[1..].to_vec(),
                    };
                    let local_sites = [
                        MatrixEscapeSite {
                            stmt_idx: 0,
                            decl: direct_nested.decl,
                            id: direct_nested.id,
                            kind: MatrixEscapeSiteKind::Direct {
                                site: direct_nested,
                            },
                        },
                        MatrixEscapeSite {
                            stmt_idx: 0,
                            decl: indirect_nested.decl,
                            id: indirect_nested.id,
                            kind: MatrixEscapeSiteKind::Indirect {
                                site: indirect_nested,
                            },
                        },
                    ];
                    let local_site_pcs = [0usize, 1usize];
                    let mut local_if_next_site_pc_by_pc: HashMap<usize, usize> = HashMap::new();
                    match Self::mixed_escape_matrix_stmt_path_cmp(
                        &direct_site.resume_path,
                        &indirect_site.resume_path,
                    ) {
                        std::cmp::Ordering::Less => {
                            local_if_next_site_pc_by_pc.insert(0, 1);
                        }
                        std::cmp::Ordering::Greater => {
                            local_if_next_site_pc_by_pc.insert(1, 0);
                        }
                        std::cmp::Ordering::Equal => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (while mixed site order ambiguous)",
                                at: direct_site.decl.span.into(),
                            });
                        }
                    }
                    self.codegen_mixed_escape_matrix_if_stmt_mixed_sites(
                        body_stmt,
                        &local_site_pcs,
                        &local_sites,
                        &local_if_next_site_pc_by_pc,
                        body_lift_ids,
                        |cg, _local_pc, _local_direct| emit_direct(cg, direct_pc, direct_site),
                        |cg, _local_pc, _local_indirect| {
                            emit_indirect(cg, indirect_pc, indirect_site)
                        },
                    )?;
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (only same-body-stmt direct / indirect coexistence in while body supported)",
                        at: stmt.span.into(),
                    });
                }
            }

            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                for body_stmt in body.stmts.iter().skip(*mixed_stmt_idx + 1) {
                    self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
                }
                self.env.pop_scope();
                self.builder.build_unconditional_branch(cond_bb)?;
            }
            self.env = base_env;
            self.builder.position_at_end(after_bb);
            return Ok(());
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (perform site missing)",
            at: stmt.span.into(),
        })
    }

    fn codegen_mixed_escape_matrix_while_stmt_direct_site<'hir, F>(
        &mut self,
        stmt: &'hir hir::Stmt,
        site_pc: usize,
        site: &MixedEscapeDirectSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        mut emit_intercept: F,
    ) -> Result<(), LlvmEmitError>
    where
        F: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let Some(first_frame) = site.resume_path.first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (missing while path)",
                at: site.decl.span.into(),
            });
        };
        let MixedEscapeDirectFrame::WhileBody {
            while_cond: expected_cond,
            while_body: expected_body,
            stmt_idx: perform_stmt_idx,
        } = first_frame
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected while site)",
                at: site.decl.span.into(),
            });
        };
        if !Self::mixed_escape_while_nested_path_supported(&site.resume_path) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (deeper nested direct site in while body not yet supported)",
                at: site.decl.span.into(),
            });
        }

        let hir::StmtKind::While { cond, body } = &stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected while statement)",
                at: stmt.span.into(),
            });
        };
        if !std::ptr::eq(cond, *expected_cond) || !std::ptr::eq(body, *expected_body) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (while path mismatch)",
                at: stmt.span.into(),
            });
        }

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: stmt.span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: stmt.span.into(),
            })?;
        let cond_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_while_cond");
        let body_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_while_body");
        let after_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_while_after");

        self.builder.build_unconditional_branch(cond_bb)?;

        self.builder.position_at_end(cond_bb);
        let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (while condition value)",
            at: cond.span.into(),
        })?;
        self.builder
            .build_conditional_branch(cond_i1, body_bb, after_bb)?;

        let base_env = self.env.clone();

        self.builder.position_at_end(body_bb);
        self.env = base_env.clone();
        self.env.push_scope();

        for (idx, body_stmt) in body.stmts.iter().enumerate() {
            if idx < *perform_stmt_idx {
                self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
                continue;
            }

            self.codegen_mixed_escape_matrix_while_site_stmt(
                body_stmt,
                site_pc,
                site,
                body_lift_ids,
                &mut emit_intercept,
            )?;
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                for body_stmt in body.stmts.iter().skip(*perform_stmt_idx + 1) {
                    self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
                }
                self.env.pop_scope();
                self.builder.build_unconditional_branch(cond_bb)?;
            }
            self.env = base_env;
            self.builder.position_at_end(after_bb);
            return Ok(());
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (perform site missing)",
            at: site.decl.span.into(),
        })
    }

    fn codegen_mixed_escape_matrix_while_tail_after_site<'hir, F>(
        &mut self,
        stmt: &'hir hir::Stmt,
        site_pc: usize,
        site: &MixedEscapeDirectSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        mut emit_intercept: F,
    ) -> Result<(), LlvmEmitError>
    where
        F: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let Some(first_frame) = site.resume_path.first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (missing while path)",
                at: site.decl.span.into(),
            });
        };
        let MixedEscapeDirectFrame::WhileBody {
            while_cond: expected_cond,
            while_body: expected_body,
            stmt_idx: perform_stmt_idx,
        } = first_frame
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected while site)",
                at: site.decl.span.into(),
            });
        };
        if !Self::mixed_escape_while_nested_path_supported(&site.resume_path) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (deeper nested direct site in while body not yet supported)",
                at: site.decl.span.into(),
            });
        }

        let hir::StmtKind::While { cond, body } = &stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected while statement)",
                at: stmt.span.into(),
            });
        };
        if !std::ptr::eq(cond, *expected_cond) || !std::ptr::eq(body, *expected_body) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (while path mismatch)",
                at: stmt.span.into(),
            });
        }

        self.env.push_scope();
        if site.resume_path.len() > 1 {
            self.codegen_mixed_escape_matrix_nested_tail_after_site_from_depth(
                site,
                1,
                body_lift_ids,
            )?;
        }
        for body_stmt in body.stmts.iter().skip(*perform_stmt_idx + 1) {
            self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
        }
        self.env.pop_scope();

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: stmt.span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: stmt.span.into(),
            })?;
        let cond_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_tail_while_cond");
        let body_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_tail_while_body");
        let after_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_tail_while_after");

        self.builder.build_unconditional_branch(cond_bb)?;

        self.builder.position_at_end(cond_bb);
        let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (while condition value)",
            at: cond.span.into(),
        })?;
        self.builder
            .build_conditional_branch(cond_i1, body_bb, after_bb)?;

        let base_env = self.env.clone();

        self.builder.position_at_end(body_bb);
        self.env = base_env.clone();
        self.env.push_scope();

        for (idx, body_stmt) in body.stmts.iter().enumerate() {
            if idx < *perform_stmt_idx {
                self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
                continue;
            }

            self.codegen_mixed_escape_matrix_while_site_stmt(
                body_stmt,
                site_pc,
                site,
                body_lift_ids,
                &mut emit_intercept,
            )?;
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                for body_stmt in body.stmts.iter().skip(*perform_stmt_idx + 1) {
                    self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
                }
                self.env.pop_scope();
                self.builder.build_unconditional_branch(cond_bb)?;
            }
            self.env = base_env;
            self.builder.position_at_end(after_bb);
            return Ok(());
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (perform site missing)",
            at: site.decl.span.into(),
        })
    }

    fn codegen_mixed_escape_matrix_while_tail_after_indirect_site<'hir, F>(
        &mut self,
        site_pc: usize,
        site: &MixedEscapeIndirectSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        mut emit_site: F,
    ) -> Result<(), LlvmEmitError>
    where
        F: FnMut(&mut Self, usize, &MixedEscapeIndirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let Some(first_frame) = site.resume_path.first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (missing while path)",
                at: site.decl.span.into(),
            });
        };
        let MixedEscapeDirectFrame::WhileBody {
            while_cond,
            while_body,
            stmt_idx: perform_stmt_idx,
        } = first_frame
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected while site)",
                at: site.decl.span.into(),
            });
        };
        if !Self::mixed_escape_while_nested_path_supported(&site.resume_path) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (deeper nested indirect site in while body not yet supported)",
                at: site.decl.span.into(),
            });
        }

        if site.resume_path.len() > 1 {
            self.codegen_mixed_escape_matrix_nested_tail_after_indirect_site_from_depth(
                site,
                1,
                body_lift_ids,
            )?;
        }
        for body_stmt in while_body.stmts.iter().skip(*perform_stmt_idx + 1) {
            self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
        }
        self.env.pop_scope();

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: site.decl.span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: site.decl.span.into(),
            })?;
        let cond_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_tail_while_cond");
        let body_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_tail_while_body");
        let after_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_tail_while_after");

        self.builder.build_unconditional_branch(cond_bb)?;

        self.builder.position_at_end(cond_bb);
        let cond_v = self.codegen_expr_in_expected_context(while_cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(while_cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (while condition value)",
            at: while_cond.span.into(),
        })?;
        self.builder
            .build_conditional_branch(cond_i1, body_bb, after_bb)?;

        let base_env = self.env.clone();

        self.builder.position_at_end(body_bb);
        self.env = base_env.clone();
        self.env.push_scope();

        for (idx, body_stmt) in while_body.stmts.iter().enumerate() {
            if idx < *perform_stmt_idx {
                self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
                continue;
            }

            self.codegen_mixed_escape_matrix_while_indirect_site_stmt(
                body_stmt,
                site_pc,
                site,
                body_lift_ids,
                &mut emit_site,
            )?;
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                if matches!(
                    site.resume_path.get(1),
                    Some(MixedEscapeDirectFrame::Block { .. })
                ) {
                    self.codegen_mixed_escape_matrix_nested_tail_after_indirect_site_from_depth(
                        site,
                        1,
                        body_lift_ids,
                    )?;
                }
                for body_stmt in while_body.stmts.iter().skip(*perform_stmt_idx + 1) {
                    self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
                }
                self.env.pop_scope();
                self.builder.build_unconditional_branch(cond_bb)?;
            }
            self.env = base_env;
            self.builder.position_at_end(after_bb);
            return Ok(());
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (indirect site missing)",
            at: site.decl.span.into(),
        })
    }

    fn codegen_mixed_escape_matrix_while_tail_after_mixed_indirect_site<'hir, F>(
        &mut self,
        current_indirect: &MixedEscapeIndirectSite<'hir>,
        future_direct_pc: usize,
        future_direct: &MixedEscapeDirectSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
        mut emit_intercept: F,
    ) -> Result<(), LlvmEmitError>
    where
        F: FnMut(&mut Self, usize, &MixedEscapeDirectSite<'hir>) -> Result<(), LlvmEmitError>,
    {
        let Some(first_frame) = current_indirect.resume_path.first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (missing while path)",
                at: current_indirect.decl.span.into(),
            });
        };
        let MixedEscapeDirectFrame::WhileBody {
            while_cond,
            while_body,
            stmt_idx: perform_stmt_idx,
        } = first_frame
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (expected while site)",
                at: current_indirect.decl.span.into(),
            });
        };
        if !Self::mixed_escape_while_same_stmt_mixed_path_supported(
            &future_direct.resume_path,
            &current_indirect.resume_path,
        ) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (only same-body-stmt direct / indirect coexistence in while body supported)",
                at: future_direct.decl.span.into(),
            });
        }
        if !Self::mixed_escape_while_nested_path_supported(&current_indirect.resume_path) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (deeper nested indirect site in while body not yet supported)",
                at: current_indirect.decl.span.into(),
            });
        }

        self.env.push_scope();
        if current_indirect.resume_path.len() > 1 {
            self.codegen_mixed_escape_matrix_nested_tail_after_indirect_site_from_depth(
                current_indirect,
                1,
                body_lift_ids,
            )?;
        }
        for body_stmt in while_body.stmts.iter().skip(*perform_stmt_idx + 1) {
            self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
        }
        self.env.pop_scope();

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: current_indirect.decl.span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: current_indirect.decl.span.into(),
            })?;
        let cond_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_tail_while_mixed_cond");
        let body_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_tail_while_mixed_body");
        let after_bb = self
            .context
            .append_basic_block(func, "mixed_escape_matrix_tail_while_mixed_after");

        self.builder.build_unconditional_branch(cond_bb)?;

        self.builder.position_at_end(cond_bb);
        let cond_v = self.codegen_expr_in_expected_context(while_cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(while_cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (while condition value)",
            at: while_cond.span.into(),
        })?;
        self.builder
            .build_conditional_branch(cond_i1, body_bb, after_bb)?;

        let base_env = self.env.clone();

        self.builder.position_at_end(body_bb);
        self.env = base_env.clone();
        self.env.push_scope();

        for (idx, body_stmt) in while_body.stmts.iter().enumerate() {
            if idx < *perform_stmt_idx {
                self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
                continue;
            }

            self.codegen_mixed_escape_matrix_while_site_stmt(
                body_stmt,
                future_direct_pc,
                future_direct,
                body_lift_ids,
                &mut emit_intercept,
            )?;
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                for body_stmt in while_body.stmts.iter().skip(*perform_stmt_idx + 1) {
                    self.codegen_mixed_escape_matrix_replay_stmt(body_stmt, body_lift_ids)?;
                }
                self.env.pop_scope();
                self.builder.build_unconditional_branch(cond_bb)?;
            }
            self.env = base_env;
            self.builder.position_at_end(after_bb);
            return Ok(());
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle mixed-arm escape continuation (perform site missing)",
            at: future_direct.decl.span.into(),
        })
    }

    fn codegen_mixed_escape_matrix_nested_tail_after_site_from_depth<'hir>(
        &mut self,
        site: &MixedEscapeDirectSite<'hir>,
        start_depth: usize,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        for frame in site.resume_path[start_depth..].iter().rev() {
            match frame {
                MixedEscapeDirectFrame::Block { block, stmt_idx } => {
                    self.env.push_scope();
                    for stmt in block.stmts.iter().skip(*stmt_idx + 1) {
                        self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
                    }
                    self.env.pop_scope();
                }
                MixedEscapeDirectFrame::IfThen {
                    then_block,
                    stmt_idx,
                    ..
                } => {
                    self.env.push_scope();
                    for stmt in then_block.stmts.iter().skip(*stmt_idx + 1) {
                        self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
                    }
                    self.env.pop_scope();
                }
                MixedEscapeDirectFrame::IfElse {
                    else_block,
                    stmt_idx,
                    ..
                } => {
                    self.env.push_scope();
                    for stmt in else_block.stmts.iter().skip(*stmt_idx + 1) {
                        self.codegen_mixed_escape_matrix_replay_stmt(stmt, body_lift_ids)?;
                    }
                    self.env.pop_scope();
                }
                MixedEscapeDirectFrame::WhileBody { while_body, .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (while tail needs dedicated lowering)",
                        at: while_body.span.into(),
                    });
                }
            }
        }
        Ok(())
    }

    fn codegen_mixed_escape_matrix_nested_block_tail_after_site<'hir>(
        &mut self,
        site: &MixedEscapeDirectSite<'hir>,
        body_lift_ids: &HashSet<hir::SymbolId>,
    ) -> Result<(), LlvmEmitError> {
        self.codegen_mixed_escape_matrix_nested_tail_after_site_from_depth(site, 0, body_lift_ids)
    }


    /// T2003c0b2b2 / T2003c0b2b3：mixed-arm immediate-resume + sibling
    /// escape-continuation 的 top-level site matrix。
    ///
    /// 当前支持：
    /// - 一个 immediate-resume arm + 一个 escape-continuation arm；
    /// - immediate site 是 top-level `val = perform`；
    /// - escape sites 可位于 immediate site 之前或之后，且允许 top-level direct / indirect 混合；
    /// - indirect path 仍只支持单 binder payload；nested site 继续稳定诊断。
    fn codegen_handle_expr_immediate_resume_with_escape_sibling_site_matrix<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        immediate: (&'hir hir::HandleArm, hir::SymbolId),
        escape: (&'hir hir::HandleArm, hir::SymbolId),
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        #[derive(Clone, Copy)]
        struct CustomSiblingArm<'hir> {
            arm: &'hir hir::HandleArm,
            op_tag: u32,
        }

        let (immediate_arm, resume_symbol) = immediate;
        let (escape_arm, continuation_symbol) = escape;
        let mut raise_sibling: Option<&'hir hir::HandleArm> = None;
        let mut custom_siblings: Vec<CustomSiblingArm<'hir>> = Vec::new();
        for arm in sibling_nonresuming_arms {
            if arm.op.binders.len() != 1 {
                let kind = if arm.op.op.fqn == "scoop.core.Raise.raise" {
                    "handle binder count (only 1 supported)"
                } else {
                    "handle binder count (custom non-resuming, only single payload supported)"
                };
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind,
                    at: arm.op.span.into(),
                });
            }
            if arm.op.op.fqn == "scoop.core.Raise.raise" {
                if raise_sibling.is_some() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed Raise arms (only 1 supported)",
                        at: arm.span.into(),
                    });
                }
                raise_sibling = Some(*arm);
                continue;
            }
            custom_siblings.push(CustomSiblingArm {
                arm,
                op_tag: self.effect_op_tag(&arm.op.op.fqn),
            });
        }
        let has_sibling_nonresuming = raise_sibling.is_some() || !custom_siblings.is_empty();

        let Some(perform_site) =
            self.scan_immediate_resume_site(handle, &immediate_arm.op.op.fqn)?
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm immediate-resume body (missing direct perform)",
                at: span.into(),
            });
        };
        if !perform_site.resume_path.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (nested immediate-resume site not yet supported)",
                at: perform_site.decl.span.into(),
            });
        }

        let perform_idx = perform_site.top_level_stmt_idx;
        let perform_decl = perform_site.decl;
        let perform_op = perform_site.op;
        let perform_args = perform_site.args;

        if perform_op.fqn != immediate_arm.op.op.fqn {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume op mismatch",
                at: perform_op.span.into(),
            });
        }

        let resume_value_ty =
            self.cg_ty_of(perform_decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume perform value type",
                    at: perform_decl.span.into(),
                })?;

        if immediate_arm.op.binders.len() != perform_args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume binder arity mismatch",
                at: immediate_arm.op.span.into(),
            });
        }

        let indirect_sites = self.scan_mixed_escape_indirect_sites(handle)?;
        let mut indirect_sites_by_stmt_idx: HashMap<usize, Vec<&MixedEscapeIndirectSite<'hir>>> =
            HashMap::new();
        for site in &indirect_sites {
            indirect_sites_by_stmt_idx
                .entry(site.top_level_stmt_idx)
                .or_default()
                .push(site);
        }
        let direct_sites = self.scan_mixed_escape_direct_sites(handle, &escape_arm.op.op.fqn)?;
        let mut direct_sites_by_stmt_idx: HashMap<usize, Vec<&MixedEscapeDirectSite<'hir>>> =
            HashMap::new();
        for site in &direct_sites {
            direct_sites_by_stmt_idx
                .entry(site.top_level_stmt_idx)
                .or_default()
                .push(site);
        }

        let mut body_decl_all: HashMap<hir::SymbolId, EscapeCaptureMeta> = HashMap::new();
        let mut body_decl_spans: HashMap<hir::SymbolId, crate::span::Span> = HashMap::new();
        let mut body_decl_order: HashMap<hir::SymbolId, usize> = HashMap::new();
        let mut next_decl_order = 0usize;
        self.collect_mixed_escape_matrix_body_decls(
            &handle.body.stmts,
            &mut body_decl_all,
            &mut body_decl_spans,
            &mut body_decl_order,
            &mut next_decl_order,
        )?;

        let mut escape_sites: Vec<MatrixEscapeSite<'hir>> = Vec::new();
        for (idx, _) in handle.body.stmts.iter().enumerate() {
            let mut stmt_sites: Vec<MatrixEscapeSite<'hir>> = Vec::new();
            if let Some(direct_sites_for_stmt) = direct_sites_by_stmt_idx.get(&idx) {
                for direct_site in direct_sites_for_stmt {
                    stmt_sites.push(MatrixEscapeSite {
                        stmt_idx: idx,
                        decl: direct_site.decl,
                        id: direct_site.id,
                        kind: MatrixEscapeSiteKind::Direct {
                            site: (*direct_site).clone(),
                        },
                    });
                }
            }
            if let Some(indirect_sites_for_stmt) = indirect_sites_by_stmt_idx.get(&idx) {
                for indirect_site in indirect_sites_for_stmt {
                    stmt_sites.push(MatrixEscapeSite {
                        stmt_idx: indirect_site.top_level_stmt_idx,
                        decl: indirect_site.decl,
                        id: indirect_site.id,
                        kind: MatrixEscapeSiteKind::Indirect {
                            site: (*indirect_site).clone(),
                        },
                    });
                }
            }
            stmt_sites.sort_by_key(|site| site.decl.span.start);
            escape_sites.extend(stmt_sites);
        }

        let Some(first_escape_site) = escape_sites.first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (top-level site matrix required)",
                at: escape_arm.span.into(),
            });
        };

        let mut escape_site_pcs_by_stmt_idx: HashMap<usize, Vec<usize>> = HashMap::new();
        for (pc, site) in escape_sites.iter().enumerate() {
            escape_site_pcs_by_stmt_idx
                .entry(site.stmt_idx)
                .or_default()
                .push(pc);
        }
        let mut if_mixed_site_pcs_by_stmt_idx: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut if_direct_site_pcs_by_stmt_idx: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut if_indirect_site_pcs_by_stmt_idx: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut while_mixed_site_pcs_by_stmt_idx: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut while_direct_site_pc_by_stmt_idx: HashMap<usize, usize> = HashMap::new();
        let mut while_indirect_site_pc_by_stmt_idx: HashMap<usize, usize> = HashMap::new();
        let mut simple_escape_site_pc_by_stmt_idx: HashMap<usize, usize> = HashMap::new();
        let mut if_next_site_pc_by_pc: HashMap<usize, usize> = HashMap::new();
        let mut if_prev_site_pc_by_pc: HashMap<usize, usize> = HashMap::new();
        let mut block_next_site_pc_by_pc: HashMap<usize, usize> = HashMap::new();
        let mut block_prev_site_pc_by_pc: HashMap<usize, usize> = HashMap::new();
        let mut while_next_site_pc_by_pc: HashMap<usize, usize> = HashMap::new();
        let mut while_prev_site_pc_by_pc: HashMap<usize, usize> = HashMap::new();
        for (stmt_idx, site_pcs) in &escape_site_pcs_by_stmt_idx {
            let mut if_direct_sites: Vec<usize> = Vec::new();
            let mut if_indirect_sites: Vec<usize> = Vec::new();
            let mut while_direct_sites: Vec<usize> = Vec::new();
            let mut while_indirect_sites: Vec<usize> = Vec::new();
            let mut block_sites: Vec<usize> = Vec::new();
            for &pc in site_pcs {
                if let MatrixEscapeSiteKind::Direct { site } = &escape_sites[pc].kind
                    && matches!(
                        site.resume_path.first(),
                        Some(
                            MixedEscapeDirectFrame::IfThen { .. }
                                | MixedEscapeDirectFrame::IfElse { .. }
                        )
                    )
                {
                    if_direct_sites.push(pc);
                } else if let MatrixEscapeSiteKind::Direct { site } = &escape_sites[pc].kind
                    && matches!(
                        site.resume_path.first(),
                        Some(MixedEscapeDirectFrame::WhileBody { .. })
                    )
                {
                    while_direct_sites.push(pc);
                } else if let MatrixEscapeSiteKind::Indirect { site } = &escape_sites[pc].kind
                    && matches!(
                        site.resume_path.first(),
                        Some(MixedEscapeDirectFrame::WhileBody { .. })
                    )
                {
                    while_indirect_sites.push(pc);
                } else if let MatrixEscapeSiteKind::Indirect { site } = &escape_sites[pc].kind
                    && matches!(
                        site.resume_path.first(),
                        Some(
                            MixedEscapeDirectFrame::IfThen { .. }
                                | MixedEscapeDirectFrame::IfElse { .. }
                        )
                    )
                {
                    if_indirect_sites.push(pc);
                } else if Self::mixed_escape_block_only_path_supported(
                    Self::mixed_escape_matrix_site_resume_path(&escape_sites[pc]),
                ) {
                    block_sites.push(pc);
                }
            }
            if !if_direct_sites.is_empty() {
                if !if_indirect_sites.is_empty() {
                    if !while_direct_sites.is_empty()
                        || !while_indirect_sites.is_empty()
                        || !block_sites.is_empty()
                    {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple sites per top-level statement not yet supported)",
                            at: handle.body.stmts[*stmt_idx].span.into(),
                        });
                    }

                    let mut then_direct_pc: Option<usize> = None;
                    let mut then_indirect_pc: Option<usize> = None;
                    let mut else_direct_pc: Option<usize> = None;
                    let mut else_indirect_pc: Option<usize> = None;

                    for &pc in &if_direct_sites {
                        let MatrixEscapeSiteKind::Direct { site } = &escape_sites[pc].kind else {
                            unreachable!("classified direct if site");
                        };
                        if !Self::mixed_escape_if_branch_path_supported(&site.resume_path) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (richer if-branch mixed sites not yet supported)",
                                at: site.decl.span.into(),
                            });
                        }
                        match site.resume_path.first() {
                            Some(MixedEscapeDirectFrame::IfThen { .. }) => {
                                if then_direct_pc.replace(pc).is_some() {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle mixed-arm escape continuation (multiple direct sites in the same if-then branch not yet supported)",
                                        at: site.decl.span.into(),
                                    });
                                }
                            }
                            Some(MixedEscapeDirectFrame::IfElse { .. }) => {
                                if else_direct_pc.replace(pc).is_some() {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle mixed-arm escape continuation (multiple direct sites in the same if-else branch not yet supported)",
                                        at: site.decl.span.into(),
                                    });
                                }
                            }
                            _ => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (expected if branch site)",
                                    at: site.decl.span.into(),
                                });
                            }
                        }
                    }

                    for &pc in &if_indirect_sites {
                        let MatrixEscapeSiteKind::Indirect { site } = &escape_sites[pc].kind else {
                            unreachable!("classified indirect if site");
                        };
                        if !Self::mixed_escape_if_branch_path_supported(&site.resume_path) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (richer if-branch mixed sites not yet supported)",
                                at: site.decl.span.into(),
                            });
                        }
                        match site.resume_path.first() {
                            Some(MixedEscapeDirectFrame::IfThen { .. }) => {
                                if then_indirect_pc.replace(pc).is_some() {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle mixed-arm escape continuation (multiple indirect sites in the same if-then branch not yet supported)",
                                        at: site.decl.span.into(),
                                    });
                                }
                            }
                            Some(MixedEscapeDirectFrame::IfElse { .. }) => {
                                if else_indirect_pc.replace(pc).is_some() {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle mixed-arm escape continuation (multiple indirect sites in the same if-else branch not yet supported)",
                                        at: site.decl.span.into(),
                                    });
                                }
                            }
                            _ => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (expected if branch site)",
                                    at: site.decl.span.into(),
                                });
                            }
                        }
                    }

                    for (direct_pc, indirect_pc) in [
                        then_direct_pc.zip(then_indirect_pc),
                        else_direct_pc.zip(else_indirect_pc),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        let MatrixEscapeSiteKind::Direct { site: direct_site } =
                            &escape_sites[direct_pc].kind
                        else {
                            unreachable!("stored direct if site");
                        };
                        let MatrixEscapeSiteKind::Indirect {
                            site: indirect_site,
                        } = &escape_sites[indirect_pc].kind
                        else {
                            unreachable!("stored indirect if site");
                        };
                        match Self::mixed_escape_matrix_stmt_path_cmp(
                            &direct_site.resume_path,
                            &indirect_site.resume_path,
                        ) {
                            std::cmp::Ordering::Less => {
                                if_next_site_pc_by_pc.insert(direct_pc, indirect_pc);
                                if_prev_site_pc_by_pc.insert(indirect_pc, direct_pc);
                            }
                            std::cmp::Ordering::Greater => {
                                if_next_site_pc_by_pc.insert(indirect_pc, direct_pc);
                                if_prev_site_pc_by_pc.insert(direct_pc, indirect_pc);
                            }
                            std::cmp::Ordering::Equal => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (if mixed site order ambiguous)",
                                    at: direct_site.decl.span.into(),
                                });
                            }
                        }
                    }

                    let mut mixed_sites = if_direct_sites.clone();
                    mixed_sites.extend(if_indirect_sites.iter().copied());
                    if_mixed_site_pcs_by_stmt_idx.insert(*stmt_idx, mixed_sites);
                    continue;
                }
                if !while_direct_sites.is_empty()
                    || !while_indirect_sites.is_empty()
                    || !block_sites.is_empty()
                {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (multiple sites per top-level statement not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
                if_direct_site_pcs_by_stmt_idx.insert(*stmt_idx, if_direct_sites);
                continue;
            }
            if !if_indirect_sites.is_empty() {
                if !while_direct_sites.is_empty()
                    || !while_indirect_sites.is_empty()
                    || !block_sites.is_empty()
                {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (multiple sites per top-level statement not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
                if_indirect_site_pcs_by_stmt_idx.insert(*stmt_idx, if_indirect_sites);
                continue;
            }
            if !block_sites.is_empty() {
                if block_sites.len() != site_pcs.len()
                    || !while_direct_sites.is_empty()
                    || !while_indirect_sites.is_empty()
                {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (multiple sites per top-level statement not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
                if block_sites.len() > 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (multiple nested block mixed sites per top-level statement not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
                let direct_count = block_sites
                    .iter()
                    .filter(|&&pc| {
                        matches!(escape_sites[pc].kind, MatrixEscapeSiteKind::Direct { .. })
                    })
                    .count();
                let indirect_count = block_sites
                    .iter()
                    .filter(|&&pc| {
                        matches!(escape_sites[pc].kind, MatrixEscapeSiteKind::Indirect { .. })
                    })
                    .count();
                if block_sites.len() == 2 && (direct_count != 1 || indirect_count != 1) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (multiple nested block mixed sites per top-level statement not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
                simple_escape_site_pc_by_stmt_idx.insert(*stmt_idx, block_sites[0]);
                if block_sites.len() == 2 {
                    block_next_site_pc_by_pc.insert(block_sites[0], block_sites[1]);
                    block_prev_site_pc_by_pc.insert(block_sites[1], block_sites[0]);
                }
                continue;
            }
            if !while_direct_sites.is_empty() {
                if !while_indirect_sites.is_empty() {
                    if site_pcs.len() != 2
                        || while_direct_sites.len() != 1
                        || while_indirect_sites.len() != 1
                    {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (multiple sites in the same while body not yet supported)",
                            at: handle.body.stmts[*stmt_idx].span.into(),
                        });
                    }

                    let direct_pc = while_direct_sites[0];
                    let indirect_pc = while_indirect_sites[0];
                    let MatrixEscapeSiteKind::Direct { site: direct_site } =
                        &escape_sites[direct_pc].kind
                    else {
                        unreachable!("classified direct while site");
                    };
                    let MatrixEscapeSiteKind::Indirect {
                        site: indirect_site,
                    } = &escape_sites[indirect_pc].kind
                    else {
                        unreachable!("classified indirect while site");
                    };
                    if !Self::mixed_escape_while_same_stmt_mixed_path_supported(
                        &direct_site.resume_path,
                        &indirect_site.resume_path,
                    ) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (only same-body-stmt direct / indirect coexistence in while body supported)",
                            at: handle.body.stmts[*stmt_idx].span.into(),
                        });
                    }
                    match Self::mixed_escape_matrix_stmt_path_cmp(
                        &direct_site.resume_path,
                        &indirect_site.resume_path,
                    ) {
                        std::cmp::Ordering::Less => {
                            while_next_site_pc_by_pc.insert(direct_pc, indirect_pc);
                            while_prev_site_pc_by_pc.insert(indirect_pc, direct_pc);
                        }
                        std::cmp::Ordering::Greater => {
                            while_next_site_pc_by_pc.insert(indirect_pc, direct_pc);
                            while_prev_site_pc_by_pc.insert(direct_pc, indirect_pc);
                        }
                        std::cmp::Ordering::Equal => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (while mixed site order ambiguous)",
                                at: direct_site.decl.span.into(),
                            });
                        }
                    }
                    let mut mixed_sites = while_direct_sites.clone();
                    mixed_sites.extend(while_indirect_sites.iter().copied());
                    while_mixed_site_pcs_by_stmt_idx.insert(*stmt_idx, mixed_sites);
                    continue;
                }
                if site_pcs.len() > 1 || while_direct_sites.len() > 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (multiple sites in the same while body not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
                while_direct_site_pc_by_stmt_idx.insert(*stmt_idx, while_direct_sites[0]);
                continue;
            }
            if !while_indirect_sites.is_empty() {
                if site_pcs.len() > 1 || while_indirect_sites.len() > 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (multiple sites in the same while body not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
                while_indirect_site_pc_by_stmt_idx.insert(*stmt_idx, while_indirect_sites[0]);
                continue;
            }
            if site_pcs.len() > 1 {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (multiple sites per top-level statement not yet supported)",
                    at: handle.body.stmts[*stmt_idx].span.into(),
                });
            }
            simple_escape_site_pc_by_stmt_idx.insert(*stmt_idx, site_pcs[0]);
        }

        let has_indirect_escape_site = escape_sites
            .iter()
            .any(|site| matches!(site.kind, MatrixEscapeSiteKind::Indirect { .. }));
        if has_indirect_escape_site && escape_arm.op.binders.len() > 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape binder count (indirect, only 1 supported)",
                at: escape_arm.op.span.into(),
            });
        }
        for site in &escape_sites {
            if let MatrixEscapeSiteKind::Direct {
                site: ref direct_site,
            } = site.kind
                && escape_arm.op.binders.len() != direct_site.args.len()
            {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape binder arity mismatch",
                    at: escape_arm.op.span.into(),
                });
            }
        }

        let escape_resume_value_ty =
            self.cg_ty_of(first_escape_site.decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape perform value type",
                    at: first_escape_site.decl.span.into(),
                })?;
        for site in escape_sites.iter().skip(1) {
            let site_ty =
                self.cg_ty_of(site.decl.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape perform value type",
                        at: site.decl.span.into(),
                    })?;
            if site_ty != escape_resume_value_ty {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape perform value type mismatch",
                    at: site.decl.span.into(),
                });
            }
        }

        let mut outer_visible_supported: Vec<EscapeCaptureMeta> = Vec::new();
        let mut outer_visible_all: HashMap<hir::SymbolId, EscapeCaptureMeta> = HashMap::new();
        let mut seen_outer: HashSet<hir::SymbolId> = HashSet::new();
        let mut visible_outer: Vec<(hir::SymbolId, CgLocal<'ctx>)> = Vec::new();
        for scope in self.env.scopes.iter().rev() {
            for (&id, &local) in scope.iter() {
                if !seen_outer.insert(id) {
                    continue;
                }
                visible_outer.push((id, local));
            }
        }
        for (id, local) in visible_outer {
            let meta = EscapeCaptureMeta {
                id,
                hir_ty: local.hir_ty,
                ty: local.ty,
                mutable: local.mutable,
            };
            outer_visible_all.insert(id, meta);
            if self.escape_capture_storage_kind(span, local.ty)?.is_some() {
                outer_visible_supported.push(meta);
            }
        }
        outer_visible_supported.sort_by_key(|meta| meta.id.as_u32());

        let mut body_lift_ids: HashSet<hir::SymbolId> = HashSet::new();
        for (site_pc, site) in escape_sites.iter().enumerate() {
            let Some(&site_order) = body_decl_order.get(&site.id) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation perform binding id",
                    at: site.decl.span.into(),
                });
            };

            let mut used_after: HashSet<hir::SymbolId> = HashSet::new();
            match &site.kind {
                MatrixEscapeSiteKind::Direct { site: direct_site } => {
                    Self::collect_mixed_escape_used_after_site(
                        direct_site,
                        &handle.body.stmts,
                        &mut used_after,
                    );
                }
                MatrixEscapeSiteKind::Indirect {
                    site: indirect_site,
                } => {
                    Self::collect_mixed_escape_used_after_indirect_site(
                        indirect_site,
                        &handle.body.stmts,
                        &mut used_after,
                    );
                    if let Some(&prev_pc) = block_prev_site_pc_by_pc.get(&site_pc) {
                        let MatrixEscapeSiteKind::Direct {
                            site: prev_direct_site,
                        } = &escape_sites[prev_pc].kind
                        else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (expected previous direct site)",
                                at: site.decl.span.into(),
                            });
                        };
                        Self::collect_mixed_escape_used_between_block_sites(
                            prev_direct_site,
                            indirect_site,
                            &mut used_after,
                        )?;
                    } else if let Some(&prev_pc) = if_prev_site_pc_by_pc.get(&site_pc) {
                        let MatrixEscapeSiteKind::Direct {
                            site: prev_direct_site,
                        } = &escape_sites[prev_pc].kind
                        else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (expected previous direct site)",
                                at: site.decl.span.into(),
                            });
                        };
                        Self::collect_mixed_escape_used_between_if_sites(
                            prev_direct_site,
                            indirect_site,
                            &mut used_after,
                        )?;
                    } else if let Some(&prev_pc) = while_prev_site_pc_by_pc.get(&site_pc) {
                        let MatrixEscapeSiteKind::Direct {
                            site: prev_direct_site,
                        } = &escape_sites[prev_pc].kind
                        else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (expected previous direct site)",
                                at: site.decl.span.into(),
                            });
                        };
                        Self::collect_mixed_escape_used_between_while_sites(
                            prev_direct_site,
                            indirect_site,
                            &mut used_after,
                        )?;
                    }
                }
            }

            for id in used_after {
                if let Some(meta) = body_decl_all.get(&id) {
                    let at = body_decl_spans.get(&id).copied().unwrap_or(site.decl.span);
                    if self.escape_capture_storage_kind(at, meta.ty)?.is_none() {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape capture local type",
                            at: at.into(),
                        });
                    }
                    if let Some(&decl_order) = body_decl_order.get(&id)
                        && decl_order < site_order
                    {
                        body_lift_ids.insert(id);
                    }
                    continue;
                }
                if let Some(meta) = outer_visible_all.get(&id) {
                    if self
                        .escape_capture_storage_kind(site.decl.span, meta.ty)?
                        .is_none()
                    {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape capture local type",
                            at: site.decl.span.into(),
                        });
                    }
                    continue;
                }
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape capture local missing",
                    at: site.decl.span.into(),
                });
            }
        }

        let mut body_visible_supported: Vec<EscapeCaptureMeta> = Vec::new();
        for &id in &body_lift_ids {
            let Some(meta) = body_decl_all.get(&id).copied() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape capture local missing",
                    at: first_escape_site.decl.span.into(),
                });
            };
            body_visible_supported.push(meta);
        }
        body_visible_supported.sort_by_key(|meta| meta.id.as_u32());

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
        let outer_raise_target = self.current_raise_target();

        let func_name = func.get_name().to_str().unwrap_or("anonymous").to_string();
        let func_name = sanitize_llvm_ident(&func_name);
        let seq = self.escape_continuation_seq;
        self.escape_continuation_seq = self.escape_continuation_seq.saturating_add(1);

        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let handler_frame_ty = self.llvm_effect_handler_frame_type();

        let state_ty_name = format!("scoop.runtime.MixedEscapeMatrixState__{func_name}_{seq}");
        let state_ty = if let Some(existing) = self.context.get_struct_type(&state_ty_name) {
            existing
        } else {
            let ty = self.context.opaque_struct_type(&state_ty_name);
            let header_ty = self.llvm_gc_object_header_type();
            let mut fields: Vec<BasicTypeEnum<'ctx>> =
                vec![header_ty.into(), handler_frame_ty.into(), i32_ty.into()];
            for cap in &outer_visible_supported {
                fields.push(match self.escape_capture_storage_kind(span, cap.ty)? {
                    Some(EscapeCaptureStorageKind::Word) => i64_ty.into(),
                    Some(EscapeCaptureStorageKind::GcRef) => gc_i8_ptr_ty.into(),
                    None => unreachable!("captures filtered by type"),
                });
            }
            for cap in &body_visible_supported {
                fields.push(match self.escape_capture_storage_kind(span, cap.ty)? {
                    Some(EscapeCaptureStorageKind::Word) => i64_ty.into(),
                    Some(EscapeCaptureStorageKind::GcRef) => gc_i8_ptr_ty.into(),
                    None => unreachable!("captures filtered by type"),
                });
            }
            ty.set_body(&fields, false);
            ty
        };

        let step_name = format!("__scoop_mixed_escape_matrix_step__{func_name}_{seq}");
        let step_fn_ty = self.context.void_type().fn_type(
            &[gc_i8_ptr_ty.into(), i64_ty.into(), gc_i8_ptr_ty.into()],
            false,
        );
        let step_fn = self.module.add_function(&step_name, step_fn_ty, None);
        step_fn.set_linkage(Linkage::Internal);
        step_fn.set_gc(super::super::LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE);

        let saved_block = insert_block;
        let outer_field_base = 3u32;
        let body_field_base = outer_field_base.saturating_add(outer_visible_supported.len() as u32);
        {
            let mut cg = MainCodegen::new(MainCodegenInputs {
                context: self.context,
                module: self.module,
                builder: self.builder,
                target_data: self.target_data,
                host: self.host,
                source_map: self.source_map,
                entry_source_id: self.entry_source_id,
                types: self.types,
                struct_layouts: self.struct_layouts,
                enum_layouts: self.enum_layouts,
                top_level_vars: self.top_level_vars,
                top_level_consts: self.top_level_consts,
                object_inits: self.object_inits,
                class_inits: self.class_inits,
                class_vtables: self.class_vtables,
                interfaces: self.interfaces,
                class_itables: self.class_itables,
                ctor_call_sites: self.ctor_call_sites,
                extern_funs: self.extern_funs,
                fun_index: self.fun_index,
                effect_op_tags: Rc::clone(&self.effect_op_tags),
            });

            let entry = self.context.append_basic_block(step_fn, "entry");
            cg.builder.position_at_end(entry);
            cg.current_fun_return_ty = Some(CgTy::Unit);
            cg.env.push_scope();

            let state_raw = step_fn
                .get_nth_param(0)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape matrix step state param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_ptr_ty = cg.llvm_ptr_type(cg.gc_address_space());
            let state_ptr = cg.builder.build_pointer_cast(
                state_raw,
                state_ptr_ty,
                "mixed_escape_matrix_step_state_ptr",
            )?;

            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape matrix step resume_word param",
                    at: span.into(),
                })?
                .into_int_value();
            let resume_gc_ref = step_fn
                .get_nth_param(2)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape matrix step resume_gc_ref param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let step_dispatch_pc_ptr = cg.builder.build_struct_gep(
                state_ty,
                state_ptr,
                2,
                "mixed_escape_matrix_step_pc_gep",
            )?;

            for (idx, cap) in outer_visible_supported.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "mixed_escape_matrix_step_outer_gep",
                )?;
                let name = format!("mixed_escape_matrix_outer_{}", cap.id.as_u32());
                let ptr =
                    cg.restore_escape_capture_local_from_state(span, field_ptr, cap.ty, &name)?;
                cg.env.insert(
                    cap.id,
                    CgLocal {
                        hir_ty: cap.hir_ty,
                        ty: cap.ty,
                        ptr,
                        mutable: cap.mutable,
                    },
                );
            }

            for (idx, cap) in body_visible_supported.iter().enumerate() {
                let field_idx = body_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "mixed_escape_matrix_step_body_gep",
                )?;
                let name = format!("mixed_escape_matrix_body_{}", cap.id.as_u32());
                let ptr =
                    cg.restore_escape_capture_local_from_state(span, field_ptr, cap.ty, &name)?;
                cg.env.insert(
                    cap.id,
                    CgLocal {
                        hir_ty: cap.hir_ty,
                        ty: cap.ty,
                        ptr,
                        mutable: cap.mutable,
                    },
                );
            }

            let mut step_escape_binder_slots: Vec<ImmediateResumeBinderSlot<'ctx>> = Vec::new();
            for binder in &escape_arm.op.binders {
                let binder_ty = if has_indirect_escape_site {
                    match cg.cg_ty_of(binder.ty) {
                        Some(CgTy::Int(int_ty)) => CgTy::Int(int_ty),
                        Some(_) | None => CgTy::Int(IntTy {
                            bits: cg.host.word_bit_width(),
                            signed: true,
                        }),
                    }
                } else {
                    cg.cg_ty_of(binder.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape binder type",
                            at: binder.span.into(),
                        })?
                };
                let ptr = cg.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
                step_escape_binder_slots.push(ImmediateResumeBinderSlot {
                    id: binder.id,
                    hir_ty: binder.ty,
                    ty: binder_ty,
                    ptr,
                });
            }
            let step_immediate_state_ptr = cg.create_entry_alloca_raw(
                span,
                "handle_mixed_escape_matrix_step_immediate_state",
                i32_ty.into(),
            )?;
            let step_immediate_resume_used_ptr = cg.create_entry_alloca_raw(
                span,
                "handle_mixed_escape_matrix_step_immediate_resume_used",
                cg.context.bool_type().into(),
            )?;
            let step_immediate_resume_value_ptr = if resume_value_ty == CgTy::Unit {
                None
            } else {
                Some(cg.create_entry_alloca(
                    span,
                    "handle_mixed_escape_matrix_step_immediate_resume_value",
                    resume_value_ty,
                )?)
            };
            let step_immediate_target_ptr = cg.create_entry_alloca(
                perform_decl.span,
                perform_decl
                    .name
                    .as_deref()
                    .unwrap_or("mixed_escape_matrix_step_immediate"),
                resume_value_ty,
            )?;
            let mut step_immediate_binder_slots: Vec<ImmediateResumeBinderSlot<'ctx>> = Vec::new();
            for binder in &immediate_arm.op.binders {
                let binder_ty =
                    cg.cg_ty_of(binder.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume binder type",
                            at: binder.span.into(),
                        })?;
                let ptr = cg.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
                step_immediate_binder_slots.push(ImmediateResumeBinderSlot {
                    id: binder.id,
                    hir_ty: binder.ty,
                    ty: binder_ty,
                    ptr,
                });
            }
            let cont_ptr = cg.create_entry_alloca(
                span,
                &format!("handle_mixed_escape_matrix_k_{seq}"),
                CgTy::Ref,
            )?;
            let _ = cg
                .builder
                .build_store(cont_ptr, cg.llvm_gc_i8_ptr_type().const_null())?;
            let llvm_ref_ty = cg.llvm_basic_type_of(span, CgTy::Ref)?;

            let frame_ptr = cg.builder.build_struct_gep(
                state_ty,
                state_ptr,
                1,
                "mixed_escape_matrix_step_frame_gep",
            )?;
            let prev_ptr = cg.builder.build_struct_gep(
                handler_frame_ty,
                frame_ptr,
                0,
                "mixed_escape_matrix_step_prev_gep",
            )?;
            let prev_raw = cg
                .builder
                .build_load(i8_ptr_ty, prev_ptr, "mixed_escape_matrix_step_prev")?
                .into_pointer_value();
            let frame_i8 = cg.builder.build_address_space_cast(
                frame_ptr,
                i8_ptr_ty,
                "mixed_escape_matrix_step_frame_i8",
            )?;
            let step_effect_dispatch_bb = if has_sibling_nonresuming {
                Some(
                    self.context
                        .append_basic_block(step_fn, "mixed_escape_matrix_step_effect_dispatch"),
                )
            } else {
                None
            };
            let step_effect_dispatch_nomatch_bb = if has_sibling_nonresuming {
                Some(self.context.append_basic_block(
                    step_fn,
                    "mixed_escape_matrix_step_effect_dispatch_nomatch",
                ))
            } else {
                None
            };
            let step_raise_catch_bb = if raise_sibling.is_some() {
                Some(
                    self.context
                        .append_basic_block(step_fn, "mixed_escape_matrix_step_raise_catch"),
                )
            } else {
                None
            };
            let mut step_custom_catch_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            for (idx, _) in custom_siblings.iter().enumerate() {
                step_custom_catch_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("mixed_escape_matrix_step_custom_catch_{idx}"),
                ));
            }

            let dispatch_bb = self
                .context
                .append_basic_block(step_fn, "mixed_escape_matrix_step_dispatch");
            let bad_state_bb = self
                .context
                .append_basic_block(step_fn, "mixed_escape_matrix_step_bad_pc");
            let escape_dispatch_bb = self
                .context
                .append_basic_block(step_fn, "mixed_escape_matrix_step_escape_dispatch");
            let escape_dispatch_nomatch_bb = self
                .context
                .append_basic_block(step_fn, "mixed_escape_matrix_step_escape_dispatch_nomatch");
            let escape_arm_bb = self
                .context
                .append_basic_block(step_fn, "mixed_escape_matrix_step_escape_arm");
            let mut state_bbs = Vec::new();
            for pc in 0..escape_sites.len() {
                state_bbs.push(
                    self.context
                        .append_basic_block(step_fn, &format!("mixed_escape_matrix_step_pc_{pc}")),
                );
            }
            let restore_step_raise_target = |cg: &mut Self| {
                if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                    cg.push_raise_target(step_effect_dispatch_bb);
                }
            };

            cg.builder.build_unconditional_branch(dispatch_bb)?;

            cg.builder.position_at_end(dispatch_bb);
            let pc = cg
                .builder
                .build_load(i32_ty, step_dispatch_pc_ptr, "mixed_escape_matrix_step_pc")?
                .into_int_value();
            let mut cases = Vec::new();
            for (pc, bb) in state_bbs.iter().enumerate() {
                cases.push((i32_ty.const_int(pc as u64, false), *bb));
            }
            cg.builder.build_switch(pc, bad_state_bb, &cases)?;

            cg.builder.position_at_end(bad_state_bb);
            cg.emit_exit_with_code(span, 3)?;

            for (site_pc, state_bb) in state_bbs.iter().enumerate() {
                let site = &escape_sites[site_pc];
                cg.builder.position_at_end(*state_bb);
                let mut current_site_escaped = false;
                if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                    for (idx, custom) in custom_siblings.iter().enumerate() {
                        cg.push_effect_unwind_target(
                            &custom.arm.op.op.fqn,
                            step_custom_catch_bbs[idx],
                        );
                    }
                    cg.push_raise_target(step_effect_dispatch_bb);
                }

                match &site.kind {
                    MatrixEscapeSiteKind::Direct { .. } => {
                        let target_ptr = if let Some(local) = cg.env.get(site.id) {
                            if local.ty != escape_resume_value_ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape perform value type mismatch",
                                    at: site.decl.span.into(),
                                });
                            }
                            local.ptr
                        } else {
                            let name = site
                                .decl
                                .name
                                .as_deref()
                                .unwrap_or("mixed_escape_matrix_resume_value");
                            let ptr = cg.create_entry_alloca(
                                site.decl.span,
                                name,
                                escape_resume_value_ty,
                            )?;
                            cg.env.insert(
                                site.id,
                                CgLocal {
                                    hir_ty: Some(site.decl.ty),
                                    ty: escape_resume_value_ty,
                                    ptr,
                                    mutable: site.decl.mutable,
                                },
                            );
                            ptr
                        };

                        let resume_value = cg.decode_abi_payload_transport(
                            site.decl.span,
                            resume_word,
                            resume_gc_ref,
                            escape_resume_value_ty,
                        )?;
                        let _stored = cg.store_local_value(
                            site.decl.span,
                            target_ptr,
                            escape_resume_value_ty,
                            resume_value,
                        )?;
                        if let MatrixEscapeSiteKind::Direct { site: direct_site } = &site.kind {
                            if matches!(
                                direct_site.resume_path.first(),
                                Some(MixedEscapeDirectFrame::WhileBody { .. })
                            ) {
                                if let Some(next_pc) =
                                    while_next_site_pc_by_pc.get(&site_pc).copied()
                                {
                                    let next_site = &escape_sites[next_pc];
                                    let mut unexpected_direct = |_cg: &mut Self,
                                                                 _next_pc: usize,
                                                                 _next_direct: &MixedEscapeDirectSite<
                                        'hir,
                                    >| {
                                        Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle mixed-arm escape continuation (unexpected direct site while continuing mixed while)",
                                            at: direct_site.decl.span.into(),
                                        })
                                    };
                                    let mut emit_next_indirect =
                                        |cg: &mut Self,
                                         next_pc: usize,
                                         indirect_site: &MixedEscapeIndirectSite<'hir>| {
                                            for (field_idx, cap) in
                                                outer_visible_supported.iter().enumerate()
                                            {
                                                let field_ptr = cg.builder.build_struct_gep(
                                                    state_ty,
                                                    state_ptr,
                                                    outer_field_base
                                                        .saturating_add(field_idx as u32),
                                                    "mixed_escape_matrix_step_capture_outer_gep",
                                                )?;
                                                let local = cg.env.get(cap.id).ok_or(
                                                    LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local not found",
                                                        at: indirect_site.decl.span.into(),
                                                    },
                                                )?;
                                                if local.ty != cap.ty {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local type mismatch",
                                                        at: indirect_site.decl.span.into(),
                                                    });
                                                }
                                                cg.write_escape_capture_local_to_state(
                                                    span, field_ptr, local.ptr, cap.ty,
                                                )?;
                                            }

                                            for (field_idx, cap) in
                                                body_visible_supported.iter().enumerate()
                                            {
                                                let field_ptr = cg.builder.build_struct_gep(
                                                    state_ty,
                                                    state_ptr,
                                                    body_field_base
                                                        .saturating_add(field_idx as u32),
                                                    "mixed_escape_matrix_step_capture_body_gep",
                                                )?;
                                                let Some(local) = cg.env.get(cap.id) else {
                                                    continue;
                                                };
                                                if local.ty != cap.ty {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local type mismatch",
                                                        at: indirect_site.decl.span.into(),
                                                    });
                                                }
                                                cg.write_escape_capture_local_to_state(
                                                    span, field_ptr, local.ptr, cap.ty,
                                                )?;
                                            }

                                            let pc_ptr = cg.builder.build_struct_gep(
                                                state_ty,
                                                state_ptr,
                                                2,
                                                "mixed_escape_matrix_step_pc_store_gep",
                                            )?;
                                            let _ = cg.builder.build_store(
                                                pc_ptr,
                                                i32_ty.const_int(next_pc as u64, false),
                                            )?;

                                            cg.push_raise_target(escape_dispatch_bb);
                                            cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                                indirect_site,
                                                &body_lift_ids,
                                            )?;
                                            cg.pop_raise_target();
                                            restore_step_raise_target(cg);
                                            Ok(())
                                        };
                                    cg.codegen_mixed_escape_matrix_continue_to_next_while_site_after_direct(
                                        direct_site,
                                        next_pc,
                                        next_site,
                                        &body_lift_ids,
                                        &mut unexpected_direct,
                                        &mut emit_next_indirect,
                                    )?;
                                    if let MatrixEscapeSiteKind::Indirect {
                                        site: next_indirect_site,
                                    } = &next_site.kind
                                        && let Some(bb) = cg.builder.get_insert_block()
                                        && bb.get_terminator().is_none()
                                    {
                                        cg.codegen_mixed_escape_matrix_while_tail_after_mixed_indirect_site(
                                            next_indirect_site,
                                            site_pc,
                                            direct_site,
                                            &body_lift_ids,
                                            |cg, reenter_pc, future_direct_site| {
                                                for (field_idx, cap) in
                                                    outer_visible_supported.iter().enumerate()
                                                {
                                                    let field_ptr = cg.builder.build_struct_gep(
                                                        state_ty,
                                                        state_ptr,
                                                        outer_field_base
                                                            .saturating_add(field_idx as u32),
                                                        "mixed_escape_matrix_step_capture_outer_gep",
                                                    )?;
                                                    let local = cg.env.get(cap.id).ok_or(
                                                        LlvmEmitError::UnsupportedMainBody {
                                                            kind: "mixed escape capture local not found",
                                                            at: future_direct_site.decl.span.into(),
                                                        },
                                                    )?;
                                                    if local.ty != cap.ty {
                                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                                            kind: "mixed escape capture local type mismatch",
                                                            at: future_direct_site.decl.span.into(),
                                                        });
                                                    }
                                                    cg.write_escape_capture_local_to_state(
                                                        span, field_ptr, local.ptr, cap.ty,
                                                    )?;
                                                }

                                                for (field_idx, cap) in
                                                    body_visible_supported.iter().enumerate()
                                                {
                                                    let field_ptr = cg.builder.build_struct_gep(
                                                        state_ty,
                                                        state_ptr,
                                                        body_field_base
                                                            .saturating_add(field_idx as u32),
                                                        "mixed_escape_matrix_step_capture_body_gep",
                                                    )?;
                                                    let Some(local) = cg.env.get(cap.id) else {
                                                        continue;
                                                    };
                                                    if local.ty != cap.ty {
                                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                                            kind: "mixed escape capture local type mismatch",
                                                            at: future_direct_site.decl.span.into(),
                                                        });
                                                    }
                                                    cg.write_escape_capture_local_to_state(
                                                        span, field_ptr, local.ptr, cap.ty,
                                                    )?;
                                                }

                                                let pc_ptr = cg.builder.build_struct_gep(
                                                    state_ty,
                                                    state_ptr,
                                                    2,
                                                    "mixed_escape_matrix_step_pc_store_gep",
                                                )?;
                                                let _ = cg.builder.build_store(
                                                    pc_ptr,
                                                    i32_ty.const_int(reenter_pc as u64, false),
                                                )?;

                                                for (slot, arg) in step_escape_binder_slots
                                                    .iter()
                                                    .zip(future_direct_site.args.iter())
                                                {
                                                    let hir::CallArg::Positional(expr) = arg else {
                                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                                            kind: "handle mixed-arm escape named perform arg",
                                                            at: future_direct_site.decl.span.into(),
                                                        });
                                                    };
                                                    let v = cg.codegen_expr_in_expected_context(
                                                        expr,
                                                        Some(slot.ty),
                                                    )?;
                                                    let _stored = cg.store_local_value(
                                                        expr.span,
                                                        slot.ptr,
                                                        slot.ty,
                                                        v,
                                                    )?;
                                                }

                                                let rt_cont_alloc =
                                                    cg.declare_runtime_continuation_alloc();
                                                let step_ptr =
                                                    step_fn.as_global_value().as_pointer_value();
                                                let cont_call = cg.builder.build_call(
                                                    rt_cont_alloc,
                                                    &[state_raw.into(), step_ptr.into()],
                                                    "mixed_escape_matrix_step_cont_alloc",
                                                )?;
                                                let cont_raw =
                                                    cont_call.try_as_basic_value().basic().ok_or(
                                                        LlvmEmitError::UnsupportedMainBody {
                                                            kind: "mixed escape continuation alloc return value",
                                                            at: future_direct_site.decl.span.into(),
                                                        },
                                                    )?;
                                                let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape continuation alloc return type",
                                                        at: future_direct_site.decl.span.into(),
                                                    });
                                                };
                                                let pin = cg.declare_runtime_gc_pin();
                                                let _ = cg.builder.build_call(
                                                    pin,
                                                    &[k_raw.into()],
                                                    "mixed_escape_matrix_step_k_pin",
                                                )?;
                                                let _stored = cg.store_local_value(
                                                    span,
                                                    cont_ptr,
                                                    CgTy::Ref,
                                                    CgValue {
                                                        ty: CgTy::Ref,
                                                        value: Some(k_raw.into()),
                                                    },
                                                )?;

                                                let rt_swap =
                                                    cg.declare_runtime_effect_handler_stack_swap_top();
                                                let _ = cg.builder.build_call(
                                                    rt_swap,
                                                    &[prev_raw.into()],
                                                    "mixed_escape_matrix_step_detach_for_direct",
                                                )?;

                                                cg.env.push_scope();
                                                for slot in &step_escape_binder_slots {
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
                                                let arm_v = cg.codegen_expr_in_expected_context(
                                                    &escape_arm.body,
                                                    Some(out_ty),
                                                )?;
                                                let _arm_v = if out_ty == CgTy::Unit {
                                                    CgValue::unit()
                                                } else {
                                                    cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
                                                };
                                                cg.env.pop_scope();

                                                let k_loaded = cg
                                                    .builder
                                                    .build_load(
                                                        llvm_ref_ty,
                                                        cont_ptr,
                                                        "mixed_escape_matrix_step_k_unpin_load",
                                                    )?
                                                    .into_pointer_value();
                                                let unpin = cg.declare_runtime_gc_unpin();
                                                let _ = cg.builder.build_call(
                                                    unpin,
                                                    &[k_loaded.into()],
                                                    "mixed_escape_matrix_step_k_unpin",
                                                )?;
                                                cg.builder.build_return(None)?;
                                                Ok(())
                                            },
                                        )?;
                                    }
                                } else {
                                    cg.codegen_mixed_escape_matrix_while_tail_after_site(
                                        &handle.body.stmts[site.stmt_idx],
                                        site_pc,
                                        direct_site,
                                        &body_lift_ids,
                                        |cg, next_pc, direct_site| {
                                            for (field_idx, cap) in
                                                outer_visible_supported.iter().enumerate()
                                            {
                                                let field_ptr = cg.builder.build_struct_gep(
                                                    state_ty,
                                                    state_ptr,
                                                    outer_field_base
                                                        .saturating_add(field_idx as u32),
                                                    "mixed_escape_matrix_step_capture_outer_gep",
                                                )?;
                                                let local = cg.env.get(cap.id).ok_or(
                                                    LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local not found",
                                                        at: direct_site.decl.span.into(),
                                                    },
                                                )?;
                                                if local.ty != cap.ty {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local type mismatch",
                                                        at: direct_site.decl.span.into(),
                                                    });
                                                }
                                                cg.write_escape_capture_local_to_state(
                                                    span, field_ptr, local.ptr, cap.ty,
                                                )?;
                                            }

                                            for (field_idx, cap) in
                                                body_visible_supported.iter().enumerate()
                                            {
                                                let field_ptr = cg.builder.build_struct_gep(
                                                    state_ty,
                                                    state_ptr,
                                                    body_field_base
                                                        .saturating_add(field_idx as u32),
                                                    "mixed_escape_matrix_step_capture_body_gep",
                                                )?;
                                                let Some(local) = cg.env.get(cap.id) else {
                                                    continue;
                                                };
                                                if local.ty != cap.ty {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local type mismatch",
                                                        at: direct_site.decl.span.into(),
                                                    });
                                                }
                                                cg.write_escape_capture_local_to_state(
                                                    span, field_ptr, local.ptr, cap.ty,
                                                )?;
                                            }

                                            let pc_ptr = cg.builder.build_struct_gep(
                                                state_ty,
                                                state_ptr,
                                                2,
                                                "mixed_escape_matrix_step_pc_store_gep",
                                            )?;
                                            let _ = cg.builder.build_store(
                                                pc_ptr,
                                                i32_ty.const_int(next_pc as u64, false),
                                            )?;

                                            for (slot, arg) in step_escape_binder_slots
                                                .iter()
                                                .zip(direct_site.args.iter())
                                            {
                                                let hir::CallArg::Positional(expr) = arg else {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "handle mixed-arm escape named perform arg",
                                                        at: direct_site.decl.span.into(),
                                                    });
                                                };
                                                let v = cg.codegen_expr_in_expected_context(
                                                    expr,
                                                    Some(slot.ty),
                                                )?;
                                                let _stored = cg.store_local_value(
                                                    expr.span,
                                                    slot.ptr,
                                                    slot.ty,
                                                    v,
                                                )?;
                                            }

                                            let rt_cont_alloc =
                                                cg.declare_runtime_continuation_alloc();
                                            let step_ptr =
                                                step_fn.as_global_value().as_pointer_value();
                                            let cont_call = cg.builder.build_call(
                                                rt_cont_alloc,
                                                &[state_raw.into(), step_ptr.into()],
                                                "mixed_escape_matrix_step_cont_alloc",
                                            )?;
                                            let cont_raw =
                                                cont_call.try_as_basic_value().basic().ok_or(
                                                    LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape continuation alloc return value",
                                                        at: direct_site.decl.span.into(),
                                                    },
                                                )?;
                                            let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                                return Err(LlvmEmitError::UnsupportedMainBody {
                                                    kind: "mixed escape continuation alloc return type",
                                                    at: direct_site.decl.span.into(),
                                                });
                                            };
                                            let pin = cg.declare_runtime_gc_pin();
                                            let _ = cg.builder.build_call(
                                                pin,
                                                &[k_raw.into()],
                                                "mixed_escape_matrix_step_k_pin",
                                            )?;
                                            let _stored = cg.store_local_value(
                                                span,
                                                cont_ptr,
                                                CgTy::Ref,
                                                CgValue {
                                                    ty: CgTy::Ref,
                                                    value: Some(k_raw.into()),
                                                },
                                            )?;

                                            let rt_swap =
                                                cg.declare_runtime_effect_handler_stack_swap_top();
                                            let _ = cg.builder.build_call(
                                                rt_swap,
                                                &[prev_raw.into()],
                                                "mixed_escape_matrix_step_detach_for_direct",
                                            )?;

                                            cg.env.push_scope();
                                            for slot in &step_escape_binder_slots {
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
                                            let arm_v = cg.codegen_expr_in_expected_context(
                                                &escape_arm.body,
                                                Some(out_ty),
                                            )?;
                                            let _arm_v = if out_ty == CgTy::Unit {
                                                CgValue::unit()
                                            } else {
                                                cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
                                            };
                                            cg.env.pop_scope();

                                            let k_loaded = cg
                                                .builder
                                                .build_load(
                                                    llvm_ref_ty,
                                                    cont_ptr,
                                                    "mixed_escape_matrix_step_k_unpin_load",
                                                )?
                                                .into_pointer_value();
                                            let unpin = cg.declare_runtime_gc_unpin();
                                            let _ = cg.builder.build_call(
                                                unpin,
                                                &[k_loaded.into()],
                                                "mixed_escape_matrix_step_k_unpin",
                                            )?;
                                            cg.builder.build_return(None)?;
                                            Ok(())
                                        },
                                    )?;
                                }
                            } else if !direct_site.resume_path.is_empty() {
                                let if_next_pc = if_next_site_pc_by_pc.get(&site_pc).copied();
                                let block_next_pc = block_next_site_pc_by_pc.get(&site_pc).copied();
                                if let Some(next_pc) = if_next_pc.or(block_next_pc) {
                                    let next_site = &escape_sites[next_pc];
                                    let mut emit_next_direct =
                                        |cg: &mut Self,
                                         next_pc: usize,
                                         direct_site: &MixedEscapeDirectSite<'hir>| {
                                            for (field_idx, cap) in
                                                outer_visible_supported.iter().enumerate()
                                            {
                                                let field_ptr = cg.builder.build_struct_gep(
                                                    state_ty,
                                                    state_ptr,
                                                    outer_field_base.saturating_add(field_idx as u32),
                                                    "mixed_escape_matrix_step_capture_outer_gep",
                                                )?;
                                                let local = cg.env.get(cap.id).ok_or(
                                                    LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local not found",
                                                        at: direct_site.decl.span.into(),
                                                    },
                                                )?;
                                                if local.ty != cap.ty {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local type mismatch",
                                                        at: direct_site.decl.span.into(),
                                                    });
                                                }
                                                cg.write_escape_capture_local_to_state(
                                                    span, field_ptr, local.ptr, cap.ty,
                                                )?;
                                            }

                                            for (field_idx, cap) in
                                                body_visible_supported.iter().enumerate()
                                            {
                                                let field_ptr = cg.builder.build_struct_gep(
                                                    state_ty,
                                                    state_ptr,
                                                    body_field_base.saturating_add(field_idx as u32),
                                                    "mixed_escape_matrix_step_capture_body_gep",
                                                )?;
                                                let Some(local) = cg.env.get(cap.id) else {
                                                    continue;
                                                };
                                                if local.ty != cap.ty {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local type mismatch",
                                                        at: direct_site.decl.span.into(),
                                                    });
                                                }
                                                cg.write_escape_capture_local_to_state(
                                                    span, field_ptr, local.ptr, cap.ty,
                                                )?;
                                            }

                                            let pc_ptr = cg.builder.build_struct_gep(
                                                state_ty,
                                                state_ptr,
                                                2,
                                                "mixed_escape_matrix_step_pc_store_gep",
                                            )?;
                                            let _ = cg.builder.build_store(
                                                pc_ptr,
                                                i32_ty.const_int(next_pc as u64, false),
                                            )?;

                                            for (slot, arg) in
                                                step_escape_binder_slots.iter().zip(direct_site.args.iter())
                                            {
                                                let hir::CallArg::Positional(expr) = arg else {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "handle mixed-arm escape named perform arg",
                                                        at: direct_site.decl.span.into(),
                                                    });
                                                };
                                                let v = cg.codegen_expr_in_expected_context(
                                                    expr,
                                                    Some(slot.ty),
                                                )?;
                                                let _stored = cg.store_local_value(
                                                    expr.span,
                                                    slot.ptr,
                                                    slot.ty,
                                                    v,
                                                )?;
                                            }

                                            let rt_cont_alloc =
                                                cg.declare_runtime_continuation_alloc();
                                            let step_ptr =
                                                step_fn.as_global_value().as_pointer_value();
                                            let cont_call = cg.builder.build_call(
                                                rt_cont_alloc,
                                                &[state_raw.into(), step_ptr.into()],
                                                "mixed_escape_matrix_step_cont_alloc",
                                            )?;
                                            let cont_raw = cont_call
                                                .try_as_basic_value()
                                                .basic()
                                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                                    kind: "mixed escape continuation alloc return value",
                                                    at: direct_site.decl.span.into(),
                                                })?;
                                            let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                                return Err(LlvmEmitError::UnsupportedMainBody {
                                                    kind: "mixed escape continuation alloc return type",
                                                    at: direct_site.decl.span.into(),
                                                });
                                            };
                                            let pin = cg.declare_runtime_gc_pin();
                                            let _ = cg.builder.build_call(
                                                pin,
                                                &[k_raw.into()],
                                                "mixed_escape_matrix_step_k_pin",
                                            )?;
                                            let _stored = cg.store_local_value(
                                                span,
                                                cont_ptr,
                                                CgTy::Ref,
                                                CgValue {
                                                    ty: CgTy::Ref,
                                                    value: Some(k_raw.into()),
                                                },
                                            )?;

                                            let rt_swap =
                                                cg.declare_runtime_effect_handler_stack_swap_top();
                                            let _ = cg.builder.build_call(
                                                rt_swap,
                                                &[prev_raw.into()],
                                                "mixed_escape_matrix_step_detach_for_direct",
                                            )?;

                                            cg.env.push_scope();
                                            for slot in &step_escape_binder_slots {
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
                                            let arm_v = cg.codegen_expr_in_expected_context(
                                                &escape_arm.body,
                                                Some(out_ty),
                                            )?;
                                            let _arm_v = if out_ty == CgTy::Unit {
                                                CgValue::unit()
                                            } else {
                                                cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
                                            };
                                            cg.env.pop_scope();

                                            let k_loaded = cg
                                                .builder
                                                .build_load(
                                                    llvm_ref_ty,
                                                    cont_ptr,
                                                    "mixed_escape_matrix_step_k_unpin_load",
                                                )?
                                                .into_pointer_value();
                                            let unpin = cg.declare_runtime_gc_unpin();
                                            let _ = cg.builder.build_call(
                                                unpin,
                                                &[k_loaded.into()],
                                                "mixed_escape_matrix_step_k_unpin",
                                            )?;
                                            cg.builder.build_return(None)?;
                                            Ok(())
                                        };
                                    let mut emit_next_indirect =
                                        |cg: &mut Self,
                                         next_pc: usize,
                                         indirect_site: &MixedEscapeIndirectSite<'hir>| {
                                            for (field_idx, cap) in
                                                outer_visible_supported.iter().enumerate()
                                            {
                                                let field_ptr = cg.builder.build_struct_gep(
                                                    state_ty,
                                                    state_ptr,
                                                    outer_field_base.saturating_add(field_idx as u32),
                                                    "mixed_escape_matrix_step_capture_outer_gep",
                                                )?;
                                                let local = cg.env.get(cap.id).ok_or(
                                                    LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local not found",
                                                        at: indirect_site.decl.span.into(),
                                                    },
                                                )?;
                                                if local.ty != cap.ty {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local type mismatch",
                                                        at: indirect_site.decl.span.into(),
                                                    });
                                                }
                                                cg.write_escape_capture_local_to_state(
                                                    span, field_ptr, local.ptr, cap.ty,
                                                )?;
                                            }

                                            for (field_idx, cap) in
                                                body_visible_supported.iter().enumerate()
                                            {
                                                let field_ptr = cg.builder.build_struct_gep(
                                                    state_ty,
                                                    state_ptr,
                                                    body_field_base.saturating_add(field_idx as u32),
                                                    "mixed_escape_matrix_step_capture_body_gep",
                                                )?;
                                                let Some(local) = cg.env.get(cap.id) else {
                                                    continue;
                                                };
                                                if local.ty != cap.ty {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local type mismatch",
                                                        at: indirect_site.decl.span.into(),
                                                    });
                                                }
                                                cg.write_escape_capture_local_to_state(
                                                    span, field_ptr, local.ptr, cap.ty,
                                                )?;
                                            }

                                            let pc_ptr = cg.builder.build_struct_gep(
                                                state_ty,
                                                state_ptr,
                                                2,
                                                "mixed_escape_matrix_step_pc_store_gep",
                                            )?;
                                            let _ = cg.builder.build_store(
                                                pc_ptr,
                                                i32_ty.const_int(next_pc as u64, false),
                                            )?;

                                            cg.push_raise_target(escape_dispatch_bb);
                                            cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                                indirect_site,
                                                &body_lift_ids,
                                            )?;
                                            cg.pop_raise_target();
                                            restore_step_raise_target(cg);
                                            Ok(())
                                        };
                                    if if_next_pc.is_some() {
                                        cg.codegen_mixed_escape_matrix_continue_to_next_if_site_after_direct(
                                            direct_site,
                                            next_pc,
                                            next_site,
                                            &body_lift_ids,
                                            &mut emit_next_direct,
                                            &mut emit_next_indirect,
                                        )?;
                                    } else {
                                        cg.codegen_mixed_escape_matrix_continue_to_next_block_site_after_direct(
                                            direct_site,
                                            next_pc,
                                            next_site,
                                            &body_lift_ids,
                                            &mut emit_next_direct,
                                            &mut emit_next_indirect,
                                        )?;
                                    }
                                    if let MatrixEscapeSiteKind::Indirect {
                                        site: next_indirect_site,
                                    } = &next_site.kind
                                        && let Some(bb) = cg.builder.get_insert_block()
                                        && bb.get_terminator().is_none()
                                    {
                                        if if_next_pc.is_some() {
                                            cg.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                                next_indirect_site,
                                                &body_lift_ids,
                                            )?;
                                        } else {
                                            cg.codegen_mixed_escape_matrix_nested_block_continue_after_indirect_site(
                                                next_indirect_site,
                                                &body_lift_ids,
                                            )?;
                                        }
                                    }
                                } else {
                                    cg.codegen_mixed_escape_matrix_nested_block_tail_after_site(
                                        direct_site,
                                        &body_lift_ids,
                                    )?;
                                }
                            }
                        }
                    }
                    MatrixEscapeSiteKind::Indirect {
                        site: indirect_site,
                    } => {
                        let if_prev_pc = if_prev_site_pc_by_pc.get(&site_pc).copied();
                        let block_prev_pc = block_prev_site_pc_by_pc.get(&site_pc).copied();
                        let while_prev_pc = while_prev_site_pc_by_pc.get(&site_pc).copied();
                        if let Some(prev_pc) = if_prev_pc.or(block_prev_pc).or(while_prev_pc) {
                            let MatrixEscapeSiteKind::Direct {
                                site: prev_direct_site,
                            } = &escape_sites[prev_pc].kind
                            else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (expected previous direct site)",
                                    at: site.decl.span.into(),
                                });
                            };
                            let mut unexpected_direct = |_cg: &mut Self,
                                                         _next_pc: usize,
                                                         _direct_site: &MixedEscapeDirectSite<
                                'hir,
                            >| {
                                Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape continuation (unexpected direct site while reconstructing indirect prefix)",
                                    at: site.decl.span.into(),
                                })
                            };
                            let mut emit_expected_indirect =
                                |_cg: &mut Self,
                                 _next_pc: usize,
                                 _indirect_site: &MixedEscapeIndirectSite<'hir>| Ok(());
                            if if_prev_pc.is_some() {
                                cg.codegen_mixed_escape_matrix_continue_to_next_if_site_after_direct(
                                    prev_direct_site,
                                    site_pc,
                                    site,
                                    &body_lift_ids,
                                    &mut unexpected_direct,
                                    &mut emit_expected_indirect,
                                )?;
                            } else if while_prev_pc.is_some() {
                                cg.codegen_mixed_escape_matrix_continue_to_next_while_site_after_direct(
                                    prev_direct_site,
                                    site_pc,
                                    site,
                                    &body_lift_ids,
                                    &mut unexpected_direct,
                                    &mut emit_expected_indirect,
                                )?;
                            } else {
                                cg.codegen_mixed_escape_matrix_continue_to_next_block_site_after_direct(
                                    prev_direct_site,
                                    site_pc,
                                    site,
                                    &body_lift_ids,
                                    &mut unexpected_direct,
                                    &mut emit_expected_indirect,
                                )?;
                            }
                        } else {
                            cg.codegen_mixed_escape_matrix_prefix_to_indirect_site(
                                indirect_site,
                                &handle.body.stmts[site.stmt_idx],
                                &body_lift_ids,
                            )?;
                        }
                        let rt_get_callee = cg.declare_runtime_callee_suspend_state_get();
                        let get_call = cg.builder.build_call(
                            rt_get_callee,
                            &[],
                            "mixed_escape_matrix_step_callee_state_get",
                        )?;
                        let callee_state_raw = get_call
                            .try_as_basic_value()
                            .basic()
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape matrix step callee_state_get return",
                                at: span.into(),
                            })?
                            .into_pointer_value();
                        let callee_prefix_ty = cg.llvm_callee_suspend_state_prefix_type();
                        let callee_state_ptr_ty = cg.llvm_ptr_type(AddressSpace::default());
                        let callee_state_ptr = cg.builder.build_pointer_cast(
                            callee_state_raw,
                            callee_state_ptr_ty,
                            "mixed_escape_matrix_step_callee_state_typed",
                        )?;
                        let callee_rw_ptr = cg.builder.build_struct_gep(
                            callee_prefix_ty,
                            callee_state_ptr,
                            1,
                            "mixed_escape_matrix_step_resume_word_gep",
                        )?;
                        let _ = cg.builder.build_store(callee_rw_ptr, resume_word)?;
                        let callee_rg_ptr = cg.builder.build_struct_gep(
                            callee_prefix_ty,
                            callee_state_ptr,
                            2,
                            "mixed_escape_matrix_step_resume_gc_ref_gep",
                        )?;
                        let wb = cg.declare_runtime_gc_write_barrier();
                        let slot_addr = cg.builder.build_pointer_cast(
                            callee_rg_ptr,
                            i8_ptr_ty,
                            "mixed_escape_matrix_step_resume_gc_slot",
                        )?;
                        let _ = cg.builder.build_call(
                            wb,
                            &[slot_addr.into(), resume_gc_ref.into()],
                            "mixed_escape_matrix_step_resume_gc_store",
                        )?;

                        let call_result = cg.codegen_expr_in_expected_context(
                            indirect_site.init,
                            Some(escape_resume_value_ty),
                        )?;
                        let target_ptr = if let Some(local) = cg.env.get(site.id) {
                            if local.ty != escape_resume_value_ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape perform value type mismatch",
                                    at: site.decl.span.into(),
                                });
                            }
                            local.ptr
                        } else {
                            let name = site
                                .decl
                                .name
                                .as_deref()
                                .unwrap_or("mixed_escape_matrix_indirect_result");
                            let ptr = cg.create_entry_alloca(
                                site.decl.span,
                                name,
                                escape_resume_value_ty,
                            )?;
                            cg.env.insert(
                                site.id,
                                CgLocal {
                                    hir_ty: Some(site.decl.ty),
                                    ty: escape_resume_value_ty,
                                    ptr,
                                    mutable: site.decl.mutable,
                                },
                            );
                            ptr
                        };
                        let _stored = cg.store_local_value(
                            site.decl.span,
                            target_ptr,
                            escape_resume_value_ty,
                            call_result,
                        )?;
                        if matches!(
                            indirect_site.resume_path.first(),
                            Some(MixedEscapeDirectFrame::WhileBody { .. })
                        ) {
                            if let Some(next_pc) = while_next_site_pc_by_pc.get(&site_pc).copied() {
                                let next_site = &escape_sites[next_pc];
                                let mut emit_next_direct = |cg: &mut Self,
                                                            next_pc: usize,
                                                            direct_site: &MixedEscapeDirectSite<
                                    'hir,
                                >| {
                                    for (field_idx, cap) in
                                        outer_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_ptr,
                                            outer_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_step_capture_outer_gep",
                                        )?;
                                        let local = cg.env.get(cap.id).ok_or(
                                            LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local not found",
                                                at: direct_site.decl.span.into(),
                                            },
                                        )?;
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: direct_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    for (field_idx, cap) in
                                        body_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_ptr,
                                            body_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_step_capture_body_gep",
                                        )?;
                                        let Some(local) = cg.env.get(cap.id) else {
                                            continue;
                                        };
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: direct_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    let pc_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        2,
                                        "mixed_escape_matrix_step_pc_store_gep",
                                    )?;
                                    let _ = cg.builder.build_store(
                                        pc_ptr,
                                        i32_ty.const_int(next_pc as u64, false),
                                    )?;

                                    for (slot, arg) in
                                        step_escape_binder_slots.iter().zip(direct_site.args.iter())
                                    {
                                        let hir::CallArg::Positional(expr) = arg else {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "handle mixed-arm escape named perform arg",
                                                at: direct_site.decl.span.into(),
                                            });
                                        };
                                        let v = cg.codegen_expr_in_expected_context(
                                            expr,
                                            Some(slot.ty),
                                        )?;
                                        let _stored =
                                            cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                    }

                                    let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                                    let step_ptr = step_fn.as_global_value().as_pointer_value();
                                    let cont_call = cg.builder.build_call(
                                        rt_cont_alloc,
                                        &[state_raw.into(), step_ptr.into()],
                                        "mixed_escape_matrix_step_cont_alloc",
                                    )?;
                                    let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                                        LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape continuation alloc return value",
                                            at: direct_site.decl.span.into(),
                                        },
                                    )?;
                                    let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape continuation alloc return type",
                                            at: direct_site.decl.span.into(),
                                        });
                                    };
                                    let pin = cg.declare_runtime_gc_pin();
                                    let _ = cg.builder.build_call(
                                        pin,
                                        &[k_raw.into()],
                                        "mixed_escape_matrix_step_k_pin",
                                    )?;
                                    let _stored = cg.store_local_value(
                                        span,
                                        cont_ptr,
                                        CgTy::Ref,
                                        CgValue {
                                            ty: CgTy::Ref,
                                            value: Some(k_raw.into()),
                                        },
                                    )?;

                                    let rt_swap =
                                        cg.declare_runtime_effect_handler_stack_swap_top();
                                    let _ = cg.builder.build_call(
                                        rt_swap,
                                        &[prev_raw.into()],
                                        "mixed_escape_matrix_step_detach_for_direct",
                                    )?;

                                    cg.env.push_scope();
                                    for slot in &step_escape_binder_slots {
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
                                    let arm_v = cg.codegen_expr_in_expected_context(
                                        &escape_arm.body,
                                        Some(out_ty),
                                    )?;
                                    let _arm_v = if out_ty == CgTy::Unit {
                                        CgValue::unit()
                                    } else {
                                        cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
                                    };
                                    cg.env.pop_scope();

                                    let k_loaded = cg
                                        .builder
                                        .build_load(
                                            llvm_ref_ty,
                                            cont_ptr,
                                            "mixed_escape_matrix_step_k_unpin_load",
                                        )?
                                        .into_pointer_value();
                                    let unpin = cg.declare_runtime_gc_unpin();
                                    let _ = cg.builder.build_call(
                                        unpin,
                                        &[k_loaded.into()],
                                        "mixed_escape_matrix_step_k_unpin",
                                    )?;
                                    cg.builder.build_return(None)?;
                                    Ok(())
                                };
                                let mut unexpected_indirect = |_cg: &mut Self,
                                                               _next_pc: usize,
                                                               _next_indirect: &MixedEscapeIndirectSite<
                                    'hir,
                                >| {
                                    Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle mixed-arm escape continuation (unexpected indirect site while continuing mixed while)",
                                        at: indirect_site.decl.span.into(),
                                    })
                                };
                                cg.codegen_mixed_escape_matrix_continue_to_next_while_site_after_indirect(
                                    indirect_site,
                                    next_pc,
                                    next_site,
                                    &body_lift_ids,
                                    &mut emit_next_direct,
                                    &mut unexpected_indirect,
                                )?;
                                if let MatrixEscapeSiteKind::Direct {
                                    site: next_direct_site,
                                } = &next_site.kind
                                    && let Some(bb) = cg.builder.get_insert_block()
                                    && bb.get_terminator().is_none()
                                {
                                    cg.codegen_mixed_escape_matrix_while_tail_after_site(
                                        &handle.body.stmts[next_site.stmt_idx],
                                        next_pc,
                                        next_direct_site,
                                        &body_lift_ids,
                                        &mut emit_next_direct,
                                    )?;
                                }
                            } else {
                                if let Some(prev_pc) = while_prev_pc {
                                    let MatrixEscapeSiteKind::Direct {
                                        site: prev_direct_site,
                                    } = &escape_sites[prev_pc].kind
                                    else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle mixed-arm escape continuation (expected previous direct site)",
                                            at: indirect_site.decl.span.into(),
                                        });
                                    };
                                    cg.codegen_mixed_escape_matrix_while_tail_after_mixed_indirect_site(
                                        indirect_site,
                                        prev_pc,
                                        prev_direct_site,
                                        &body_lift_ids,
                                        |cg, reenter_pc, future_direct_site| {
                                            for (field_idx, cap) in
                                                outer_visible_supported.iter().enumerate()
                                            {
                                                let field_ptr = cg.builder.build_struct_gep(
                                                    state_ty,
                                                    state_ptr,
                                                    outer_field_base.saturating_add(field_idx as u32),
                                                    "mixed_escape_matrix_step_capture_outer_gep",
                                                )?;
                                                let local = cg.env.get(cap.id).ok_or(
                                                    LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local not found",
                                                        at: future_direct_site.decl.span.into(),
                                                    },
                                                )?;
                                                if local.ty != cap.ty {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local type mismatch",
                                                        at: future_direct_site.decl.span.into(),
                                                    });
                                                }
                                                cg.write_escape_capture_local_to_state(
                                                    span, field_ptr, local.ptr, cap.ty,
                                                )?;
                                            }

                                            for (field_idx, cap) in
                                                body_visible_supported.iter().enumerate()
                                            {
                                                let field_ptr = cg.builder.build_struct_gep(
                                                    state_ty,
                                                    state_ptr,
                                                    body_field_base.saturating_add(field_idx as u32),
                                                    "mixed_escape_matrix_step_capture_body_gep",
                                                )?;
                                                let Some(local) = cg.env.get(cap.id) else {
                                                    continue;
                                                };
                                                if local.ty != cap.ty {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local type mismatch",
                                                        at: future_direct_site.decl.span.into(),
                                                    });
                                                }
                                                cg.write_escape_capture_local_to_state(
                                                    span, field_ptr, local.ptr, cap.ty,
                                                )?;
                                            }

                                            let pc_ptr = cg.builder.build_struct_gep(
                                                state_ty,
                                                state_ptr,
                                                2,
                                                "mixed_escape_matrix_step_pc_store_gep",
                                            )?;
                                            let _ = cg.builder.build_store(
                                                pc_ptr,
                                                i32_ty.const_int(reenter_pc as u64, false),
                                            )?;

                                            for (slot, arg) in
                                                step_escape_binder_slots.iter().zip(future_direct_site.args.iter())
                                            {
                                                let hir::CallArg::Positional(expr) = arg else {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "handle mixed-arm escape named perform arg",
                                                        at: future_direct_site.decl.span.into(),
                                                    });
                                                };
                                                let v = cg.codegen_expr_in_expected_context(
                                                    expr,
                                                    Some(slot.ty),
                                                )?;
                                                let _stored =
                                                    cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                            }

                                            let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                                            let step_ptr = step_fn.as_global_value().as_pointer_value();
                                            let cont_call = cg.builder.build_call(
                                                rt_cont_alloc,
                                                &[state_raw.into(), step_ptr.into()],
                                                "mixed_escape_matrix_step_cont_alloc",
                                            )?;
                                            let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                                                LlvmEmitError::UnsupportedMainBody {
                                                    kind: "mixed escape continuation alloc return value",
                                                    at: future_direct_site.decl.span.into(),
                                                },
                                            )?;
                                            let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                                return Err(LlvmEmitError::UnsupportedMainBody {
                                                    kind: "mixed escape continuation alloc return type",
                                                    at: future_direct_site.decl.span.into(),
                                                });
                                            };
                                            let pin = cg.declare_runtime_gc_pin();
                                            let _ = cg.builder.build_call(
                                                pin,
                                                &[k_raw.into()],
                                                "mixed_escape_matrix_step_k_pin",
                                            )?;
                                            let _stored = cg.store_local_value(
                                                span,
                                                cont_ptr,
                                                CgTy::Ref,
                                                CgValue {
                                                    ty: CgTy::Ref,
                                                    value: Some(k_raw.into()),
                                                },
                                            )?;

                                            let rt_swap =
                                                cg.declare_runtime_effect_handler_stack_swap_top();
                                            let _ = cg.builder.build_call(
                                                rt_swap,
                                                &[prev_raw.into()],
                                                "mixed_escape_matrix_step_detach_for_direct",
                                            )?;

                                            cg.env.push_scope();
                                            for slot in &step_escape_binder_slots {
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
                                            let arm_v = cg.codegen_expr_in_expected_context(
                                                &escape_arm.body,
                                                Some(out_ty),
                                            )?;
                                            let _arm_v = if out_ty == CgTy::Unit {
                                                CgValue::unit()
                                            } else {
                                                cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
                                            };
                                            cg.env.pop_scope();

                                            let k_loaded = cg
                                                .builder
                                                .build_load(
                                                    llvm_ref_ty,
                                                    cont_ptr,
                                                    "mixed_escape_matrix_step_k_unpin_load",
                                                )?
                                                .into_pointer_value();
                                            let unpin = cg.declare_runtime_gc_unpin();
                                            let _ = cg.builder.build_call(
                                                unpin,
                                                &[k_loaded.into()],
                                                "mixed_escape_matrix_step_k_unpin",
                                            )?;
                                            cg.builder.build_return(None)?;
                                            Ok(())
                                        },
                                    )?;
                                } else {
                                    cg.codegen_mixed_escape_matrix_while_tail_after_indirect_site(
                                        site_pc,
                                        indirect_site,
                                        &body_lift_ids,
                                        |cg, next_pc, next_site| {
                                            for (field_idx, cap) in
                                                outer_visible_supported.iter().enumerate()
                                            {
                                                let field_ptr = cg.builder.build_struct_gep(
                                                    state_ty,
                                                    state_ptr,
                                                    outer_field_base.saturating_add(field_idx as u32),
                                                    "mixed_escape_matrix_step_capture_outer_gep",
                                                )?;
                                                let local = cg.env.get(cap.id).ok_or(
                                                    LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local not found",
                                                        at: next_site.decl.span.into(),
                                                    },
                                                )?;
                                                if local.ty != cap.ty {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local type mismatch",
                                                        at: next_site.decl.span.into(),
                                                    });
                                                }
                                                cg.write_escape_capture_local_to_state(
                                                    span, field_ptr, local.ptr, cap.ty,
                                                )?;
                                            }

                                            for (field_idx, cap) in
                                                body_visible_supported.iter().enumerate()
                                            {
                                                let field_ptr = cg.builder.build_struct_gep(
                                                    state_ty,
                                                    state_ptr,
                                                    body_field_base.saturating_add(field_idx as u32),
                                                    "mixed_escape_matrix_step_capture_body_gep",
                                                )?;
                                                let Some(local) = cg.env.get(cap.id) else {
                                                    continue;
                                                };
                                                if local.ty != cap.ty {
                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                        kind: "mixed escape capture local type mismatch",
                                                        at: next_site.decl.span.into(),
                                                    });
                                                }
                                                cg.write_escape_capture_local_to_state(
                                                    span, field_ptr, local.ptr, cap.ty,
                                                )?;
                                            }

                                            let pc_ptr = cg.builder.build_struct_gep(
                                                state_ty,
                                                state_ptr,
                                                2,
                                                "mixed_escape_matrix_step_pc_store_gep",
                                            )?;
                                            let _ = cg.builder.build_store(
                                                pc_ptr,
                                                i32_ty.const_int(next_pc as u64, false),
                                            )?;

                                            cg.push_raise_target(escape_dispatch_bb);
                                            cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                                next_site,
                                                &body_lift_ids,
                                            )?;
                                            cg.pop_raise_target();
                                            restore_step_raise_target(cg);
                                            Ok(())
                                        },
                                    )?;
                                }
                            }
                        } else {
                            let if_next_pc = if_next_site_pc_by_pc.get(&site_pc).copied();
                            let block_next_pc = block_next_site_pc_by_pc.get(&site_pc).copied();
                            if let Some(next_pc) = if_next_pc.or(block_next_pc) {
                                let next_site = &escape_sites[next_pc];
                                let mut emit_next_direct = |cg: &mut Self,
                                                            next_pc: usize,
                                                            direct_site: &MixedEscapeDirectSite<
                                    'hir,
                                >| {
                                    for (field_idx, cap) in
                                        outer_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_ptr,
                                            outer_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_step_capture_outer_gep",
                                        )?;
                                        let local = cg.env.get(cap.id).ok_or(
                                            LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local not found",
                                                at: direct_site.decl.span.into(),
                                            },
                                        )?;
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: direct_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    for (field_idx, cap) in
                                        body_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_ptr,
                                            body_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_step_capture_body_gep",
                                        )?;
                                        let Some(local) = cg.env.get(cap.id) else {
                                            continue;
                                        };
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: direct_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    let pc_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        2,
                                        "mixed_escape_matrix_step_pc_store_gep",
                                    )?;
                                    let _ = cg.builder.build_store(
                                        pc_ptr,
                                        i32_ty.const_int(next_pc as u64, false),
                                    )?;

                                    for (slot, arg) in
                                        step_escape_binder_slots.iter().zip(direct_site.args.iter())
                                    {
                                        let hir::CallArg::Positional(expr) = arg else {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "handle mixed-arm escape named perform arg",
                                                at: direct_site.decl.span.into(),
                                            });
                                        };
                                        let v = cg.codegen_expr_in_expected_context(
                                            expr,
                                            Some(slot.ty),
                                        )?;
                                        let _stored =
                                            cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                    }

                                    let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                                    let step_ptr = step_fn.as_global_value().as_pointer_value();
                                    let cont_call = cg.builder.build_call(
                                        rt_cont_alloc,
                                        &[state_raw.into(), step_ptr.into()],
                                        "mixed_escape_matrix_step_cont_alloc",
                                    )?;
                                    let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                                        LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape continuation alloc return value",
                                            at: direct_site.decl.span.into(),
                                        },
                                    )?;
                                    let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape continuation alloc return type",
                                            at: direct_site.decl.span.into(),
                                        });
                                    };
                                    let pin = cg.declare_runtime_gc_pin();
                                    let _ = cg.builder.build_call(
                                        pin,
                                        &[k_raw.into()],
                                        "mixed_escape_matrix_step_k_pin",
                                    )?;
                                    let _stored = cg.store_local_value(
                                        span,
                                        cont_ptr,
                                        CgTy::Ref,
                                        CgValue {
                                            ty: CgTy::Ref,
                                            value: Some(k_raw.into()),
                                        },
                                    )?;

                                    let rt_swap =
                                        cg.declare_runtime_effect_handler_stack_swap_top();
                                    let _ = cg.builder.build_call(
                                        rt_swap,
                                        &[prev_raw.into()],
                                        "mixed_escape_matrix_step_detach_for_direct",
                                    )?;

                                    cg.env.push_scope();
                                    for slot in &step_escape_binder_slots {
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
                                    let arm_v = cg.codegen_expr_in_expected_context(
                                        &escape_arm.body,
                                        Some(out_ty),
                                    )?;
                                    let _arm_v = if out_ty == CgTy::Unit {
                                        CgValue::unit()
                                    } else {
                                        cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
                                    };
                                    cg.env.pop_scope();

                                    let k_loaded = cg
                                        .builder
                                        .build_load(
                                            llvm_ref_ty,
                                            cont_ptr,
                                            "mixed_escape_matrix_step_k_unpin_load",
                                        )?
                                        .into_pointer_value();
                                    let unpin = cg.declare_runtime_gc_unpin();
                                    let _ = cg.builder.build_call(
                                        unpin,
                                        &[k_loaded.into()],
                                        "mixed_escape_matrix_step_k_unpin",
                                    )?;
                                    cg.builder.build_return(None)?;
                                    Ok(())
                                };
                                let mut emit_next_indirect =
                                    |cg: &mut Self,
                                     next_pc: usize,
                                     next_indirect_site: &MixedEscapeIndirectSite<'hir>| {
                                    for (field_idx, cap) in
                                        outer_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_ptr,
                                            outer_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_step_capture_outer_gep",
                                        )?;
                                        let local = cg.env.get(cap.id).ok_or(
                                            LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local not found",
                                                at: next_indirect_site.decl.span.into(),
                                            },
                                        )?;
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: next_indirect_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    for (field_idx, cap) in
                                        body_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_ptr,
                                            body_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_step_capture_body_gep",
                                        )?;
                                        let Some(local) = cg.env.get(cap.id) else {
                                            continue;
                                        };
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: next_indirect_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    let pc_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        2,
                                        "mixed_escape_matrix_step_pc_store_gep",
                                    )?;
                                    let _ = cg.builder.build_store(
                                        pc_ptr,
                                        i32_ty.const_int(next_pc as u64, false),
                                    )?;

                                    cg.push_raise_target(escape_dispatch_bb);
                                    cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                        next_indirect_site,
                                        &body_lift_ids,
                                    )?;
                                    cg.pop_raise_target();
                                    restore_step_raise_target(cg);
                                    Ok(())
                                };
                                if if_next_pc.is_some() {
                                    cg.codegen_mixed_escape_matrix_continue_to_next_if_site_after_indirect(
                                        indirect_site,
                                        next_pc,
                                        next_site,
                                        &body_lift_ids,
                                        &mut emit_next_direct,
                                        &mut emit_next_indirect,
                                    )?;
                                } else {
                                    cg.codegen_mixed_escape_matrix_continue_to_next_block_site_after_indirect(
                                        indirect_site,
                                        next_pc,
                                        next_site,
                                        &body_lift_ids,
                                        &mut emit_next_direct,
                                        &mut emit_next_indirect,
                                    )?;
                                }
                                match &next_site.kind {
                                    MatrixEscapeSiteKind::Direct { .. } => {
                                        current_site_escaped = true;
                                    }
                                    MatrixEscapeSiteKind::Indirect {
                                        site: next_indirect_site,
                                    } => {
                                        if let Some(bb) = cg.builder.get_insert_block()
                                            && bb.get_terminator().is_none()
                                        {
                                            if if_next_pc.is_some() {
                                                cg.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                                    next_indirect_site,
                                                    &body_lift_ids,
                                                )?;
                                            } else {
                                                cg.codegen_mixed_escape_matrix_nested_block_continue_after_indirect_site(
                                                    next_indirect_site,
                                                    &body_lift_ids,
                                                )?;
                                            }
                                        }
                                    }
                                }
                            } else {
                                cg.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                    indirect_site,
                                    &body_lift_ids,
                                )?;
                            }
                        }
                    }
                }

                let mut escaped = current_site_escaped
                    || cg
                        .builder
                        .get_insert_block()
                        .is_some_and(|bb| bb.get_terminator().is_some());
                for (idx, stmt) in handle.body.stmts.iter().enumerate().skip(site.stmt_idx + 1) {
                    if escaped
                        || cg
                            .builder
                            .get_insert_block()
                            .is_some_and(|bb| bb.get_terminator().is_some())
                    {
                        break;
                    }

                    if let Some(mixed_sites) = if_mixed_site_pcs_by_stmt_idx.get(&idx) {
                        cg.codegen_mixed_escape_matrix_if_stmt_mixed_sites(
                            stmt,
                            mixed_sites,
                            &escape_sites,
                            &if_next_site_pc_by_pc,
                            &body_lift_ids,
                            |cg, next_pc, direct_site| {
                                for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        outer_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_outer_gep",
                                    )?;
                                    let local = cg.env.get(cap.id).ok_or(
                                        LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local not found",
                                            at: direct_site.decl.span.into(),
                                        },
                                    )?;
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: direct_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        body_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_body_gep",
                                    )?;
                                    let Some(local) = cg.env.get(cap.id) else {
                                        continue;
                                    };
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: direct_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                let pc_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_ptr,
                                    2,
                                    "mixed_escape_matrix_step_pc_store_gep",
                                )?;
                                let _ = cg
                                    .builder
                                    .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                                for (slot, arg) in
                                    step_escape_binder_slots.iter().zip(direct_site.args.iter())
                                {
                                    let hir::CallArg::Positional(expr) = arg else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle mixed-arm escape named perform arg",
                                            at: direct_site.decl.span.into(),
                                        });
                                    };
                                    let v =
                                        cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                    let _stored =
                                        cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                }

                                let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                                let step_ptr = step_fn.as_global_value().as_pointer_value();
                                let cont_call = cg.builder.build_call(
                                    rt_cont_alloc,
                                    &[state_raw.into(), step_ptr.into()],
                                    "mixed_escape_matrix_step_cont_alloc",
                                )?;
                                let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape continuation alloc return value",
                                        at: direct_site.decl.span.into(),
                                    },
                                )?;
                                let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape continuation alloc return type",
                                        at: direct_site.decl.span.into(),
                                    });
                                };
                                let pin = cg.declare_runtime_gc_pin();
                                let _ = cg.builder.build_call(
                                    pin,
                                    &[k_raw.into()],
                                    "mixed_escape_matrix_step_k_pin",
                                )?;
                                let _stored = cg.store_local_value(
                                    span,
                                    cont_ptr,
                                    CgTy::Ref,
                                    CgValue {
                                        ty: CgTy::Ref,
                                        value: Some(k_raw.into()),
                                    },
                                )?;

                                let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                                let _ = cg.builder.build_call(
                                    rt_swap,
                                    &[prev_raw.into()],
                                    "mixed_escape_matrix_step_detach_for_direct",
                                )?;

                                cg.env.push_scope();
                                for slot in &step_escape_binder_slots {
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
                                let arm_v = cg.codegen_expr_in_expected_context(
                                    &escape_arm.body,
                                    Some(out_ty),
                                )?;
                                let _arm_v = if out_ty == CgTy::Unit {
                                    CgValue::unit()
                                } else {
                                    cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
                                };
                                cg.env.pop_scope();

                                let k_loaded = cg
                                    .builder
                                    .build_load(
                                        llvm_ref_ty,
                                        cont_ptr,
                                        "mixed_escape_matrix_step_k_unpin_load",
                                    )?
                                    .into_pointer_value();
                                let unpin = cg.declare_runtime_gc_unpin();
                                let _ = cg.builder.build_call(
                                    unpin,
                                    &[k_loaded.into()],
                                    "mixed_escape_matrix_step_k_unpin",
                                )?;
                                cg.builder.build_return(None)?;
                                Ok(())
                            },
                            |cg, next_pc, indirect_site| {
                                for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        outer_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_outer_gep",
                                    )?;
                                    let local = cg.env.get(cap.id).ok_or(
                                        LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local not found",
                                            at: indirect_site.decl.span.into(),
                                        },
                                    )?;
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: indirect_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        body_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_body_gep",
                                    )?;
                                    let Some(local) = cg.env.get(cap.id) else {
                                        continue;
                                    };
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: indirect_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                let pc_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_ptr,
                                    2,
                                    "mixed_escape_matrix_step_pc_store_gep",
                                )?;
                                let _ = cg
                                    .builder
                                    .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                                cg.push_raise_target(escape_dispatch_bb);
                                cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                    indirect_site,
                                    &body_lift_ids,
                                )?;
                                cg.pop_raise_target();
                                restore_step_raise_target(cg);
                                Ok(())
                            },
                        )?;
                        continue;
                    }

                    if let Some(mixed_sites) = while_mixed_site_pcs_by_stmt_idx.get(&idx) {
                        cg.codegen_mixed_escape_matrix_while_stmt_mixed_sites(
                            stmt,
                            mixed_sites,
                            &escape_sites,
                            &body_lift_ids,
                            |cg, next_pc, direct_site| {
                                for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        outer_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_outer_gep",
                                    )?;
                                    let local = cg.env.get(cap.id).ok_or(
                                        LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local not found",
                                            at: direct_site.decl.span.into(),
                                        },
                                    )?;
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: direct_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        body_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_body_gep",
                                    )?;
                                    let Some(local) = cg.env.get(cap.id) else {
                                        continue;
                                    };
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: direct_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                let pc_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_ptr,
                                    2,
                                    "mixed_escape_matrix_step_pc_store_gep",
                                )?;
                                let _ = cg
                                    .builder
                                    .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                                for (slot, arg) in
                                    step_escape_binder_slots.iter().zip(direct_site.args.iter())
                                {
                                    let hir::CallArg::Positional(expr) = arg else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle mixed-arm escape named perform arg",
                                            at: direct_site.decl.span.into(),
                                        });
                                    };
                                    let v =
                                        cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                    let _stored =
                                        cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                }

                                let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                                let step_ptr = step_fn.as_global_value().as_pointer_value();
                                let cont_call = cg.builder.build_call(
                                    rt_cont_alloc,
                                    &[state_raw.into(), step_ptr.into()],
                                    "mixed_escape_matrix_step_cont_alloc",
                                )?;
                                let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape continuation alloc return value",
                                        at: direct_site.decl.span.into(),
                                    },
                                )?;
                                let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape continuation alloc return type",
                                        at: direct_site.decl.span.into(),
                                    });
                                };
                                let pin = cg.declare_runtime_gc_pin();
                                let _ = cg.builder.build_call(
                                    pin,
                                    &[k_raw.into()],
                                    "mixed_escape_matrix_step_k_pin",
                                )?;
                                let _stored = cg.store_local_value(
                                    span,
                                    cont_ptr,
                                    CgTy::Ref,
                                    CgValue {
                                        ty: CgTy::Ref,
                                        value: Some(k_raw.into()),
                                    },
                                )?;

                                let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                                let _ = cg.builder.build_call(
                                    rt_swap,
                                    &[prev_raw.into()],
                                    "mixed_escape_matrix_step_detach_for_direct",
                                )?;

                                cg.env.push_scope();
                                for slot in &step_escape_binder_slots {
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
                                let arm_v = cg.codegen_expr_in_expected_context(
                                    &escape_arm.body,
                                    Some(out_ty),
                                )?;
                                let _arm_v = if out_ty == CgTy::Unit {
                                    CgValue::unit()
                                } else {
                                    cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
                                };
                                cg.env.pop_scope();

                                let k_loaded = cg
                                    .builder
                                    .build_load(
                                        llvm_ref_ty,
                                        cont_ptr,
                                        "mixed_escape_matrix_step_k_unpin_load",
                                    )?
                                    .into_pointer_value();
                                let unpin = cg.declare_runtime_gc_unpin();
                                let _ = cg.builder.build_call(
                                    unpin,
                                    &[k_loaded.into()],
                                    "mixed_escape_matrix_step_k_unpin",
                                )?;
                                cg.builder.build_return(None)?;
                                Ok(())
                            },
                            |cg, next_pc, indirect_site| {
                                for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        outer_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_outer_gep",
                                    )?;
                                    let local = cg.env.get(cap.id).ok_or(
                                        LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local not found",
                                            at: indirect_site.decl.span.into(),
                                        },
                                    )?;
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: indirect_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        body_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_body_gep",
                                    )?;
                                    let Some(local) = cg.env.get(cap.id) else {
                                        continue;
                                    };
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: indirect_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                let pc_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_ptr,
                                    2,
                                    "mixed_escape_matrix_step_pc_store_gep",
                                )?;
                                let _ = cg
                                    .builder
                                    .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                                cg.push_raise_target(escape_dispatch_bb);
                                cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                    indirect_site,
                                    &body_lift_ids,
                                )?;
                                cg.pop_raise_target();
                                restore_step_raise_target(cg);
                                Ok(())
                            },
                        )?;
                        continue;
                    }

                    if let Some(direct_sites) = if_direct_site_pcs_by_stmt_idx.get(&idx) {
                        cg.codegen_mixed_escape_matrix_if_stmt_direct_sites(
                            stmt,
                            direct_sites,
                            &escape_sites,
                            &body_lift_ids,
                            |cg, next_pc, direct_site| {
                                for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        outer_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_outer_gep",
                                    )?;
                                    let local = cg.env.get(cap.id).ok_or(
                                        LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local not found",
                                            at: direct_site.decl.span.into(),
                                        },
                                    )?;
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: direct_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        body_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_body_gep",
                                    )?;
                                    let Some(local) = cg.env.get(cap.id) else {
                                        continue;
                                    };
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: direct_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                let pc_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_ptr,
                                    2,
                                    "mixed_escape_matrix_step_pc_store_gep",
                                )?;
                                let _ = cg
                                    .builder
                                    .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                                for (slot, arg) in
                                    step_escape_binder_slots.iter().zip(direct_site.args.iter())
                                {
                                    let hir::CallArg::Positional(expr) = arg else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle mixed-arm escape named perform arg",
                                            at: direct_site.decl.span.into(),
                                        });
                                    };
                                    let v =
                                        cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                    let _stored =
                                        cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                }

                                let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                                let step_ptr = step_fn.as_global_value().as_pointer_value();
                                let cont_call = cg.builder.build_call(
                                    rt_cont_alloc,
                                    &[state_raw.into(), step_ptr.into()],
                                    "mixed_escape_matrix_step_cont_alloc",
                                )?;
                                let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape continuation alloc return value",
                                        at: direct_site.decl.span.into(),
                                    },
                                )?;
                                let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape continuation alloc return type",
                                        at: direct_site.decl.span.into(),
                                    });
                                };
                                let pin = cg.declare_runtime_gc_pin();
                                let _ = cg.builder.build_call(
                                    pin,
                                    &[k_raw.into()],
                                    "mixed_escape_matrix_step_k_pin",
                                )?;
                                let _stored = cg.store_local_value(
                                    span,
                                    cont_ptr,
                                    CgTy::Ref,
                                    CgValue {
                                        ty: CgTy::Ref,
                                        value: Some(k_raw.into()),
                                    },
                                )?;

                                let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                                let _ = cg.builder.build_call(
                                    rt_swap,
                                    &[prev_raw.into()],
                                    "mixed_escape_matrix_step_detach_for_direct",
                                )?;

                                cg.env.push_scope();
                                for slot in &step_escape_binder_slots {
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
                                let arm_v = cg.codegen_expr_in_expected_context(
                                    &escape_arm.body,
                                    Some(out_ty),
                                )?;
                                let _arm_v = if out_ty == CgTy::Unit {
                                    CgValue::unit()
                                } else {
                                    cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
                                };
                                cg.env.pop_scope();

                                let k_loaded = cg
                                    .builder
                                    .build_load(
                                        llvm_ref_ty,
                                        cont_ptr,
                                        "mixed_escape_matrix_step_k_unpin_load",
                                    )?
                                    .into_pointer_value();
                                let unpin = cg.declare_runtime_gc_unpin();
                                let _ = cg.builder.build_call(
                                    unpin,
                                    &[k_loaded.into()],
                                    "mixed_escape_matrix_step_k_unpin",
                                )?;
                                cg.builder.build_return(None)?;
                                Ok(())
                            },
                        )?;
                        continue;
                    }

                    if let Some(indirect_sites) = if_indirect_site_pcs_by_stmt_idx.get(&idx) {
                        cg.codegen_mixed_escape_matrix_if_stmt_indirect_sites(
                            stmt,
                            indirect_sites,
                            &escape_sites,
                            &body_lift_ids,
                            |cg, next_pc, indirect_site| {
                                for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        outer_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_outer_gep",
                                    )?;
                                    let local = cg.env.get(cap.id).ok_or(
                                        LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local not found",
                                            at: indirect_site.decl.span.into(),
                                        },
                                    )?;
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: indirect_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        body_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_body_gep",
                                    )?;
                                    let Some(local) = cg.env.get(cap.id) else {
                                        continue;
                                    };
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: indirect_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                let pc_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_ptr,
                                    2,
                                    "mixed_escape_matrix_step_pc_store_gep",
                                )?;
                                let _ = cg
                                    .builder
                                    .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                                cg.push_raise_target(escape_dispatch_bb);
                                cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                    indirect_site,
                                    &body_lift_ids,
                                )?;
                                cg.pop_raise_target();
                                restore_step_raise_target(cg);
                                Ok(())
                            },
                        )?;
                        continue;
                    }

                    if let Some(&next_pc) = while_direct_site_pc_by_stmt_idx.get(&idx) {
                        let MatrixEscapeSiteKind::Direct { site: direct_site } =
                            &escape_sites[next_pc].kind
                        else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (expected direct site)",
                                at: stmt.span.into(),
                            });
                        };
                        cg.codegen_mixed_escape_matrix_while_stmt_direct_site(
                            stmt,
                            next_pc,
                            direct_site,
                            &body_lift_ids,
                            |cg, next_pc, direct_site| {
                                for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        outer_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_outer_gep",
                                    )?;
                                    let local = cg.env.get(cap.id).ok_or(
                                        LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local not found",
                                            at: direct_site.decl.span.into(),
                                        },
                                    )?;
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: direct_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        body_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_body_gep",
                                    )?;
                                    let Some(local) = cg.env.get(cap.id) else {
                                        continue;
                                    };
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: direct_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                let pc_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_ptr,
                                    2,
                                    "mixed_escape_matrix_step_pc_store_gep",
                                )?;
                                let _ = cg
                                    .builder
                                    .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                                for (slot, arg) in
                                    step_escape_binder_slots.iter().zip(direct_site.args.iter())
                                {
                                    let hir::CallArg::Positional(expr) = arg else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle mixed-arm escape named perform arg",
                                            at: direct_site.decl.span.into(),
                                        });
                                    };
                                    let v =
                                        cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                    let _stored =
                                        cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                }

                                let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                                let step_ptr = step_fn.as_global_value().as_pointer_value();
                                let cont_call = cg.builder.build_call(
                                    rt_cont_alloc,
                                    &[state_raw.into(), step_ptr.into()],
                                    "mixed_escape_matrix_step_cont_alloc",
                                )?;
                                let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape continuation alloc return value",
                                        at: direct_site.decl.span.into(),
                                    },
                                )?;
                                let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape continuation alloc return type",
                                        at: direct_site.decl.span.into(),
                                    });
                                };
                                let pin = cg.declare_runtime_gc_pin();
                                let _ = cg.builder.build_call(
                                    pin,
                                    &[k_raw.into()],
                                    "mixed_escape_matrix_step_k_pin",
                                )?;
                                let _stored = cg.store_local_value(
                                    span,
                                    cont_ptr,
                                    CgTy::Ref,
                                    CgValue {
                                        ty: CgTy::Ref,
                                        value: Some(k_raw.into()),
                                    },
                                )?;

                                let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                                let _ = cg.builder.build_call(
                                    rt_swap,
                                    &[prev_raw.into()],
                                    "mixed_escape_matrix_step_detach_for_direct",
                                )?;

                                cg.env.push_scope();
                                for slot in &step_escape_binder_slots {
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
                                let arm_v = cg.codegen_expr_in_expected_context(
                                    &escape_arm.body,
                                    Some(out_ty),
                                )?;
                                let _arm_v = if out_ty == CgTy::Unit {
                                    CgValue::unit()
                                } else {
                                    cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
                                };
                                cg.env.pop_scope();

                                let k_loaded = cg
                                    .builder
                                    .build_load(
                                        llvm_ref_ty,
                                        cont_ptr,
                                        "mixed_escape_matrix_step_k_unpin_load",
                                    )?
                                    .into_pointer_value();
                                let unpin = cg.declare_runtime_gc_unpin();
                                let _ = cg.builder.build_call(
                                    unpin,
                                    &[k_loaded.into()],
                                    "mixed_escape_matrix_step_k_unpin",
                                )?;
                                cg.builder.build_return(None)?;
                                Ok(())
                            },
                        )?;
                        continue;
                    }

                    if let Some(&next_pc) = while_indirect_site_pc_by_stmt_idx.get(&idx) {
                        let MatrixEscapeSiteKind::Indirect {
                            site: indirect_site,
                        } = &escape_sites[next_pc].kind
                        else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (expected indirect site)",
                                at: stmt.span.into(),
                            });
                        };
                        cg.codegen_mixed_escape_matrix_while_stmt_indirect_site(
                            stmt,
                            next_pc,
                            indirect_site,
                            &body_lift_ids,
                            |cg, next_pc, indirect_site| {
                                for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        outer_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_outer_gep",
                                    )?;
                                    let local = cg.env.get(cap.id).ok_or(
                                        LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local not found",
                                            at: indirect_site.decl.span.into(),
                                        },
                                    )?;
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: indirect_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                    let field_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_ptr,
                                        body_field_base.saturating_add(field_idx as u32),
                                        "mixed_escape_matrix_step_capture_body_gep",
                                    )?;
                                    let Some(local) = cg.env.get(cap.id) else {
                                        continue;
                                    };
                                    if local.ty != cap.ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape capture local type mismatch",
                                            at: indirect_site.decl.span.into(),
                                        });
                                    }
                                    cg.write_escape_capture_local_to_state(
                                        span, field_ptr, local.ptr, cap.ty,
                                    )?;
                                }

                                let pc_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_ptr,
                                    2,
                                    "mixed_escape_matrix_step_pc_store_gep",
                                )?;
                                let _ = cg
                                    .builder
                                    .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                                cg.push_raise_target(escape_dispatch_bb);
                                cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                    indirect_site,
                                    &body_lift_ids,
                                )?;
                                cg.pop_raise_target();
                                restore_step_raise_target(cg);
                                Ok(())
                            },
                        )?;
                        continue;
                    }

                    if let Some(&next_pc) = simple_escape_site_pc_by_stmt_idx.get(&idx) {
                        let next_site = &escape_sites[next_pc];
                        if let MatrixEscapeSiteKind::Direct { site: direct_site } = &next_site.kind
                            && !direct_site.resume_path.is_empty()
                        {
                            cg.codegen_mixed_escape_matrix_nested_block_prefix_to_site(
                                direct_site,
                                stmt,
                                &body_lift_ids,
                            )?;
                        } else if let MatrixEscapeSiteKind::Indirect {
                            site: indirect_site,
                        } = &next_site.kind
                            && !indirect_site.resume_path.is_empty()
                        {
                            cg.codegen_mixed_escape_matrix_prefix_to_indirect_site(
                                indirect_site,
                                stmt,
                                &body_lift_ids,
                            )?;
                        }

                        for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_ptr,
                                outer_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_step_capture_outer_gep",
                            )?;
                            let local =
                                cg.env
                                    .get(cap.id)
                                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: next_site.decl.span.into(),
                                    })?;
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: next_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_ptr,
                                body_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_step_capture_body_gep",
                            )?;
                            let Some(local) = cg.env.get(cap.id) else {
                                continue;
                            };
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: next_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        let pc_ptr = cg.builder.build_struct_gep(
                            state_ty,
                            state_ptr,
                            2,
                            "mixed_escape_matrix_step_pc_store_gep",
                        )?;
                        let _ = cg
                            .builder
                            .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                        match &next_site.kind {
                            MatrixEscapeSiteKind::Direct { site: direct_site } => {
                                for (slot, arg) in
                                    step_escape_binder_slots.iter().zip(direct_site.args.iter())
                                {
                                    let hir::CallArg::Positional(expr) = arg else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle mixed-arm escape named perform arg",
                                            at: direct_site.decl.span.into(),
                                        });
                                    };
                                    let v =
                                        cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                    let _stored =
                                        cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                }

                                let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                                let step_ptr = step_fn.as_global_value().as_pointer_value();
                                let cont_call = cg.builder.build_call(
                                    rt_cont_alloc,
                                    &[state_raw.into(), step_ptr.into()],
                                    "mixed_escape_matrix_step_cont_alloc",
                                )?;
                                let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape continuation alloc return value",
                                        at: direct_site.decl.span.into(),
                                    },
                                )?;
                                let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape continuation alloc return type",
                                        at: direct_site.decl.span.into(),
                                    });
                                };
                                let pin = cg.declare_runtime_gc_pin();
                                let _ = cg.builder.build_call(
                                    pin,
                                    &[k_raw.into()],
                                    "mixed_escape_matrix_step_k_pin",
                                )?;
                                let _stored = cg.store_local_value(
                                    span,
                                    cont_ptr,
                                    CgTy::Ref,
                                    CgValue {
                                        ty: CgTy::Ref,
                                        value: Some(k_raw.into()),
                                    },
                                )?;

                                let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                                let _ = cg.builder.build_call(
                                    rt_swap,
                                    &[prev_raw.into()],
                                    "mixed_escape_matrix_step_detach_for_direct",
                                )?;

                                cg.env.push_scope();
                                for slot in &step_escape_binder_slots {
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
                                let arm_v = cg.codegen_expr_in_expected_context(
                                    &escape_arm.body,
                                    Some(out_ty),
                                )?;
                                let _arm_v = if out_ty == CgTy::Unit {
                                    CgValue::unit()
                                } else {
                                    cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
                                };
                                cg.env.pop_scope();

                                let k_loaded = cg
                                    .builder
                                    .build_load(
                                        llvm_ref_ty,
                                        cont_ptr,
                                        "mixed_escape_matrix_step_k_unpin_load",
                                    )?
                                    .into_pointer_value();
                                let unpin = cg.declare_runtime_gc_unpin();
                                let _ = cg.builder.build_call(
                                    unpin,
                                    &[k_loaded.into()],
                                    "mixed_escape_matrix_step_k_unpin",
                                )?;
                                cg.builder.build_return(None)?;
                                escaped = true;
                                break;
                            }
                            MatrixEscapeSiteKind::Indirect {
                                site: indirect_site,
                            } => {
                                cg.push_raise_target(escape_dispatch_bb);
                                cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                    indirect_site,
                                    &body_lift_ids,
                                )?;
                                cg.pop_raise_target();
                                restore_step_raise_target(&mut cg);
                                cg.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                    indirect_site,
                                    &body_lift_ids,
                                )?;
                                continue;
                            }
                        }
                    }

                    if idx == perform_idx {
                        let hir::StmtKind::Val(decl) = &stmt.kind else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm immediate-resume body (expected perform binding)",
                                at: stmt.span.into(),
                            });
                        };
                        let immediate_arm_bb = self.context.append_basic_block(
                            step_fn,
                            &format!("mixed_escape_matrix_step_immediate_arm_{site_pc}"),
                        );
                        let immediate_resume_ok_bb = self.context.append_basic_block(
                            step_fn,
                            &format!("mixed_escape_matrix_step_immediate_ok_{site_pc}"),
                        );
                        let immediate_resume_missing_bb = self.context.append_basic_block(
                            step_fn,
                            &format!("mixed_escape_matrix_step_immediate_missing_{site_pc}"),
                        );
                        let target_ptr = cg.codegen_immediate_resume_site_binding(
                            &perform_site,
                            decl,
                            ImmediateResumeArmDispatch {
                                binder_slots: &step_immediate_binder_slots,
                                resume_used_ptr: step_immediate_resume_used_ptr,
                                arm_bb: immediate_arm_bb,
                            },
                            Some(step_immediate_target_ptr),
                        )?;

                        cg.builder.position_at_end(immediate_arm_bb);
                        let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                        let _ = cg.builder.build_call(
                            rt_swap,
                            &[prev_raw.into()],
                            "mixed_escape_matrix_step_detach_for_immediate_arm",
                        )?;
                        cg.env.push_scope();
                        for slot in &step_immediate_binder_slots {
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
                        let resume_ctx = ImmediateResumeCtx {
                            resume_symbol,
                            resume_value_ty,
                            resume_value_ptr: step_immediate_resume_value_ptr,
                            resume_used_ptr: step_immediate_resume_used_ptr,
                            state_ptr: step_immediate_state_ptr,
                            next_state: 1,
                        };
                        cg.push_immediate_resume_ctx(resume_ctx);
                        let _ = cg.codegen_expr_in_expected_context(
                            &immediate_arm.body,
                            Some(CgTy::Unit),
                        )?;
                        cg.pop_immediate_resume_ctx();

                        let used = cg
                            .builder
                            .build_load(
                                cg.context.bool_type(),
                                step_immediate_resume_used_ptr,
                                "mixed_escape_matrix_step_immediate_resume_used",
                            )?
                            .into_int_value();
                        cg.builder.build_conditional_branch(
                            used,
                            immediate_resume_ok_bb,
                            immediate_resume_missing_bb,
                        )?;

                        cg.builder.position_at_end(immediate_resume_missing_bb);
                        cg.emit_exit_with_code(span, 3)?;

                        cg.builder.position_at_end(immediate_resume_ok_bb);
                        let _ = cg.builder.build_call(
                            rt_swap,
                            &[frame_i8.into()],
                            "mixed_escape_matrix_step_restore_after_immediate_arm",
                        )?;
                        if let Some(ptr) = step_immediate_resume_value_ptr {
                            let llvm_ty = cg.llvm_basic_type_of(span, resume_value_ty)?;
                            let loaded = cg.builder.build_load(
                                llvm_ty,
                                ptr,
                                "mixed_escape_matrix_step_immediate_resume_value",
                            )?;
                            let _ = cg.store_local_value(
                                span,
                                target_ptr,
                                resume_value_ty,
                                CgValue {
                                    ty: resume_value_ty,
                                    value: Some(loaded),
                                },
                            )?;
                        }
                        cg.env.pop_scope();
                        continue;
                    }

                    match &stmt.kind {
                        hir::StmtKind::Empty => {}
                        hir::StmtKind::Val(decl) => {
                            if let Some(id) = decl.id
                                && body_lift_ids.contains(&id)
                            {
                                let Some(init) = decl.init.as_ref() else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "lifted local without init",
                                        at: decl.span.into(),
                                    });
                                };
                                let decl_ty = cg.cg_ty_of(decl.ty).ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "lifted local type",
                                        at: decl.span.into(),
                                    },
                                )?;
                                let target_ptr = if let Some(local) = cg.env.get(id) {
                                    if local.ty != decl_ty {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "lifted local type",
                                            at: decl.span.into(),
                                        });
                                    }
                                    local.ptr
                                } else {
                                    let name = decl.name.as_deref().unwrap_or("v");
                                    let ptr = cg.create_entry_alloca(decl.span, name, decl_ty)?;
                                    cg.env.insert(
                                        id,
                                        CgLocal {
                                            hir_ty: Some(decl.ty),
                                            ty: decl_ty,
                                            ptr,
                                            mutable: decl.mutable,
                                        },
                                    );
                                    ptr
                                };
                                let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                let _stored =
                                    cg.store_local_value(decl.span, target_ptr, decl_ty, v)?;
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
                        hir::StmtKind::While { cond, body } => {
                            cg.codegen_while_stmt(stmt.span, cond, body)?;
                        }
                        hir::StmtKind::Return { .. } => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "`return` inside mixed-arm escape continuation step",
                                at: stmt.span.into(),
                            });
                        }
                        hir::StmtKind::Break { .. }
                        | hir::StmtKind::Continue { .. }
                        | hir::StmtKind::Todo(_) => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "statement inside mixed-arm escape continuation step",
                                at: stmt.span.into(),
                            });
                        }
                    }
                }

                if !escaped
                    && let Some(bb) = cg.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    let unpin = cg.declare_runtime_gc_unpin();
                    let _ = cg.builder.build_call(
                        unpin,
                        &[state_raw.into()],
                        "mixed_escape_matrix_step_state_unpin",
                    )?;
                    cg.builder.build_return(None)?;
                }
            }

            cg.builder.position_at_end(escape_dispatch_bb);
            let rt_read_tag = cg.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call =
                cg.builder
                    .build_call(rt_read_tag, &[], "mixed_escape_matrix_step_read_op_tag")?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape matrix step read_op_tag return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape matrix step read_op_tag return type",
                    at: span.into(),
                });
            };
            let escape_tag = self.effect_op_tag(&escape_arm.op.op.fqn);
            let escape_tag_i32 = i32_ty.const_int(escape_tag as u64, false);
            let tag_matches = cg.builder.build_int_compare(
                IntPredicate::EQ,
                slot_tag,
                escape_tag_i32,
                "mixed_escape_matrix_step_tag_eq",
            )?;
            let escape_dispatch_fallback_bb =
                step_effect_dispatch_bb.unwrap_or(escape_dispatch_nomatch_bb);
            cg.builder.build_conditional_branch(
                tag_matches,
                escape_arm_bb,
                escape_dispatch_fallback_bb,
            )?;

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                let step_effect_dispatch_nomatch_bb = step_effect_dispatch_nomatch_bb
                    .expect("mixed escape matrix step dispatch_nomatch bb should exist");
                cg.builder.position_at_end(step_effect_dispatch_bb);
                let rt_read_tag = cg.declare_runtime_effect_perform_slot_read_op_tag();
                let tag_call = cg.builder.build_call(
                    rt_read_tag,
                    &[],
                    "mixed_escape_matrix_step_effect_read_op_tag",
                )?;
                let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "mixed escape matrix step effect read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "mixed escape matrix step effect read_op_tag return type",
                        at: span.into(),
                    });
                };

                let mut dispatch_cases: Vec<(
                    IntValue<'ctx>,
                    inkwell::basic_block::BasicBlock<'ctx>,
                )> = Vec::new();
                if let Some(step_raise_catch_bb) = step_raise_catch_bb {
                    let raise_tag = cg.effect_op_tag("scoop.core.Raise.raise");
                    dispatch_cases.push((
                        i32_ty.const_int(raise_tag as u64, false),
                        step_raise_catch_bb,
                    ));
                }
                for (idx, custom) in custom_siblings.iter().enumerate() {
                    dispatch_cases.push((
                        i32_ty.const_int(custom.op_tag as u64, false),
                        step_custom_catch_bbs[idx],
                    ));
                }
                cg.builder.build_switch(
                    slot_tag,
                    step_effect_dispatch_nomatch_bb,
                    &dispatch_cases,
                )?;

                cg.builder.position_at_end(step_effect_dispatch_nomatch_bb);
                let unpin = cg.declare_runtime_gc_unpin();
                let _ = cg.builder.build_call(
                    unpin,
                    &[state_raw.into()],
                    "mixed_escape_matrix_step_state_unpin_effect_nomatch",
                )?;
                cg.builder.build_return(None)?;

                if let (Some(raise_arm), Some(step_raise_catch_bb)) =
                    (raise_sibling, step_raise_catch_bb)
                {
                    let binder = &raise_arm.op.binders[0];
                    cg.builder.position_at_end(step_raise_catch_bb);
                    let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                    let _ = cg.builder.build_call(
                        rt_swap,
                        &[prev_raw.into()],
                        "mixed_escape_matrix_step_raise_detach",
                    )?;

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "mixed_escape_matrix_step_raise_read_slot_len_words",
                    )?;
                    let raw = call.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_len_words return value",
                            at: span.into(),
                        },
                    )?;
                    let BasicValueEnum::IntValue(len_words_i32) = raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_len_words return type",
                            at: span.into(),
                        });
                    };

                    let expected_len = cg.context.i32_type().const_int(2, false);
                    let len_ok = cg.builder.build_int_compare(
                        IntPredicate::EQ,
                        len_words_i32,
                        expected_len,
                        "mixed_escape_matrix_step_raise_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "mixed_escape_matrix_step_raise_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "mixed_escape_matrix_step_raise_slot_len_bad_bb",
                    );
                    cg.builder
                        .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                    cg.builder.position_at_end(len_bad_bb);
                    cg.emit_exit_with_code(span, 3)?;

                    cg.builder.position_at_end(len_ok_bb);

                    let rt_read_at = cg.declare_runtime_effect_perform_slot_read_u64_at();
                    let idx0 = cg.context.i32_type().const_int(0, false);
                    let idx1 = cg.context.i32_type().const_int(1, false);
                    let kind_call = cg.builder.build_call(
                        rt_read_at,
                        &[idx0.into()],
                        "mixed_escape_matrix_step_raise_read_slot_word0",
                    )?;
                    let kind_raw = kind_call.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_word0 return value",
                            at: span.into(),
                        },
                    )?;
                    let BasicValueEnum::IntValue(kind_u64) = kind_raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_word0 return type",
                            at: span.into(),
                        });
                    };
                    let value_call = cg.builder.build_call(
                        rt_read_at,
                        &[idx1.into()],
                        "mixed_escape_matrix_step_raise_read_slot_word1",
                    )?;
                    let value_raw = value_call.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_word1 return value",
                            at: span.into(),
                        },
                    )?;
                    let BasicValueEnum::IntValue(value_u64) = value_raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_word1 return type",
                            at: span.into(),
                        });
                    };

                    let rt_clear = cg.declare_runtime_effect_clear();
                    let _ = cg.builder.build_call(
                        rt_clear,
                        &[],
                        "mixed_escape_matrix_step_raise_clear",
                    )?;

                    cg.env.push_scope();
                    let binder_cg_ty =
                        cg.cg_ty_of(binder.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle binder type",
                                at: binder.span.into(),
                            })?;
                    let binder_value = match binder_cg_ty {
                        CgTy::Int(int_ty) => {
                            let expected = cg.context.i64_type().const_int(1, false);
                            let ok = cg.builder.build_int_compare(
                                IntPredicate::EQ,
                                kind_u64,
                                expected,
                                "mixed_escape_matrix_step_raise_kind_is_int",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "mixed_escape_matrix_step_raise_kind_int_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "mixed_escape_matrix_step_raise_kind_int_bad",
                            );
                            cg.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                            cg.builder.position_at_end(bad_bb);
                            cg.emit_exit_with_code(span, 3)?;

                            cg.builder.position_at_end(ok_bb);
                            let from_u64 = IntTy {
                                bits: 64,
                                signed: false,
                            };
                            let decoded = cg.cast_int(value_u64, from_u64, int_ty)?;
                            CgValue::int(decoded, int_ty)
                        }
                        CgTy::Enum(enum_ty) if cg.is_sysroot_runtime_error_enum(enum_ty) => {
                            let expected = cg.context.i64_type().const_int(2, false);
                            let ok = cg.builder.build_int_compare(
                                IntPredicate::EQ,
                                kind_u64,
                                expected,
                                "mixed_escape_matrix_step_raise_kind_is_runtime_error",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "mixed_escape_matrix_step_raise_kind_runtime_error_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "mixed_escape_matrix_step_raise_kind_runtime_error_bad",
                            );
                            cg.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                            cg.builder.position_at_end(bad_bb);
                            cg.emit_exit_with_code(span, 3)?;

                            cg.builder.position_at_end(ok_bb);

                            let repr = cg.cg_enum_layout(span, enum_ty)?.repr;
                            if !matches!(repr, CgEnumRepr::TaggedUnion) {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "Raise<RuntimeError> niche repr (not supported)",
                                    at: span.into(),
                                });
                            }

                            let tag_i32 = cg.builder.build_int_truncate(
                                value_u64,
                                cg.context.i32_type(),
                                "mixed_escape_matrix_step_runtime_error_tag_i32",
                            )?;
                            let payload_word_zero =
                                cg.int_type(cg.enum_payload_ty()).const_int(0, false);
                            let payload_ptr_zero = cg.llvm_gc_i8_ptr_type().const_null();
                            let llvm_enum_ty = cg.llvm_enum_value_type(span, enum_ty)?;
                            let llvm_enum_ty = llvm_enum_ty.into_struct_type();
                            let mut agg: AggregateValueEnum<'ctx> = llvm_enum_ty.get_undef().into();
                            agg = cg.builder.build_insert_value(
                                agg,
                                tag_i32,
                                0,
                                "mixed_escape_matrix_step_runtime_error_tag",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_word_zero,
                                1,
                                "mixed_escape_matrix_step_runtime_error_payload_word",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_ptr_zero,
                                2,
                                "mixed_escape_matrix_step_runtime_error_payload_ptr",
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
                    let binder_ptr =
                        cg.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
                    let _ =
                        cg.store_local_value(binder.span, binder_ptr, binder_cg_ty, binder_value)?;
                    cg.env.insert(
                        binder.id,
                        CgLocal {
                            hir_ty: Some(binder.ty),
                            ty: binder_cg_ty,
                            ptr: binder_ptr,
                            mutable: false,
                        },
                    );

                    for custom in &custom_siblings {
                        cg.push_effect_unwind_target(
                            &custom.arm.op.op.fqn,
                            step_effect_dispatch_nomatch_bb,
                        );
                    }
                    cg.push_raise_target(step_effect_dispatch_nomatch_bb);
                    let arm_v =
                        cg.codegen_expr_in_expected_context(&raise_arm.body, Some(out_ty))?;
                    cg.pop_raise_target();
                    for _ in custom_siblings.iter().rev() {
                        cg.pop_effect_unwind_target();
                    }
                    let _arm_v = if out_ty == CgTy::Unit {
                        CgValue::unit()
                    } else {
                        cg.coerce_value(raise_arm.body.span, arm_v, out_ty)?
                    };
                    cg.env.pop_scope();

                    if let Some(bb) = cg.builder.get_insert_block()
                        && bb.get_terminator().is_none()
                    {
                        let unpin = cg.declare_runtime_gc_unpin();
                        let _ = cg.builder.build_call(
                            unpin,
                            &[state_raw.into()],
                            "mixed_escape_matrix_step_state_unpin_raise",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }

                for (idx, custom) in custom_siblings.iter().enumerate() {
                    let arm = custom.arm;
                    let binder = &arm.op.binders[0];
                    cg.builder.position_at_end(step_custom_catch_bbs[idx]);
                    let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                    let _ = cg.builder.build_call(
                        rt_swap,
                        &[prev_raw.into()],
                        "mixed_escape_matrix_step_custom_detach",
                    )?;

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "mixed_escape_matrix_step_custom_read_slot_len_words",
                    )?;
                    let raw = call.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_len_words return value",
                            at: span.into(),
                        },
                    )?;
                    let BasicValueEnum::IntValue(len_words_i32) = raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_len_words return type",
                            at: span.into(),
                        });
                    };

                    let expected_len = cg.context.i32_type().const_int(1, false);
                    let len_ok = cg.builder.build_int_compare(
                        IntPredicate::EQ,
                        len_words_i32,
                        expected_len,
                        "mixed_escape_matrix_step_custom_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "mixed_escape_matrix_step_custom_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "mixed_escape_matrix_step_custom_slot_len_bad_bb",
                    );
                    cg.builder
                        .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                    cg.builder.position_at_end(len_bad_bb);
                    cg.emit_exit_with_code(span, 3)?;

                    cg.builder.position_at_end(len_ok_bb);

                    let rt_read = cg.declare_runtime_effect_perform_slot_read_u64();
                    let value_call = cg.builder.build_call(
                        rt_read,
                        &[],
                        "mixed_escape_matrix_step_custom_read_slot_word0",
                    )?;
                    let value_raw = value_call.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_word0 return value",
                            at: span.into(),
                        },
                    )?;
                    let BasicValueEnum::IntValue(value_u64) = value_raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_word0 return type",
                            at: span.into(),
                        });
                    };
                    let rt_read_gc = cg.declare_runtime_effect_perform_slot_read_gc_ref();
                    let gc_call = cg.builder.build_call(
                        rt_read_gc,
                        &[],
                        "mixed_escape_matrix_step_custom_read_slot_gc_ref",
                    )?;
                    let gc_raw = gc_call.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_gc_ref return value",
                            at: span.into(),
                        },
                    )?;
                    let BasicValueEnum::PointerValue(gc_ref_raw) = gc_raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_gc_ref return type",
                            at: span.into(),
                        });
                    };

                    cg.env.push_scope();
                    let binder_cg_ty =
                        cg.cg_ty_of(binder.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle binder type (custom non-resuming)",
                                at: binder.span.into(),
                            })?;
                    let binder_value = cg.decode_abi_payload_transport(
                        binder.span,
                        value_u64,
                        gc_ref_raw,
                        binder_cg_ty,
                    )?;
                    let binder_ptr =
                        cg.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
                    let _ =
                        cg.store_local_value(binder.span, binder_ptr, binder_cg_ty, binder_value)?;
                    cg.env.insert(
                        binder.id,
                        CgLocal {
                            hir_ty: Some(binder.ty),
                            ty: binder_cg_ty,
                            ptr: binder_ptr,
                            mutable: false,
                        },
                    );

                    let rt_clear = cg.declare_runtime_effect_clear();
                    let _ = cg.builder.build_call(
                        rt_clear,
                        &[],
                        "mixed_escape_matrix_step_custom_clear",
                    )?;

                    for custom in &custom_siblings {
                        cg.push_effect_unwind_target(
                            &custom.arm.op.op.fqn,
                            step_effect_dispatch_nomatch_bb,
                        );
                    }
                    cg.push_raise_target(step_effect_dispatch_nomatch_bb);
                    let arm_v = cg.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
                    cg.pop_raise_target();
                    for _ in custom_siblings.iter().rev() {
                        cg.pop_effect_unwind_target();
                    }
                    let _arm_v = if out_ty == CgTy::Unit {
                        CgValue::unit()
                    } else {
                        cg.coerce_value(arm.body.span, arm_v, out_ty)?
                    };
                    cg.env.pop_scope();

                    if let Some(bb) = cg.builder.get_insert_block()
                        && bb.get_terminator().is_none()
                    {
                        let unpin = cg.declare_runtime_gc_unpin();
                        let _ = cg.builder.build_call(
                            unpin,
                            &[state_raw.into()],
                            "mixed_escape_matrix_step_state_unpin_custom",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }
            }

            cg.builder.position_at_end(escape_dispatch_nomatch_bb);
            let unpin = cg.declare_runtime_gc_unpin();
            let _ = cg.builder.build_call(
                unpin,
                &[state_raw.into()],
                "mixed_escape_matrix_step_state_unpin_nomatch",
            )?;
            cg.builder.build_return(None)?;

            cg.builder.position_at_end(escape_arm_bb);
            if step_escape_binder_slots.len() > 1 {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape binder count (indirect, only 1 supported)",
                    at: escape_arm.op.span.into(),
                });
            }
            if let Some(slot) = step_escape_binder_slots.first() {
                let rt_read = cg.declare_runtime_effect_perform_slot_read_u64();
                let word_call = cg.builder.build_call(
                    rt_read,
                    &[],
                    "mixed_escape_matrix_step_read_binder_word",
                )?;
                let word_raw = word_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "mixed escape matrix step read binder return",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(word_u64) = word_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "mixed escape matrix step read binder type",
                        at: span.into(),
                    });
                };
                let rt_read_gc = cg.declare_runtime_effect_perform_slot_read_gc_ref();
                let gc_call = cg.builder.build_call(
                    rt_read_gc,
                    &[],
                    "mixed_escape_matrix_step_read_binder_gc",
                )?;
                let gc_raw = gc_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "mixed escape matrix step read binder gc value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(gc_ref_raw) = gc_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "mixed escape matrix step read binder gc type",
                        at: span.into(),
                    });
                };
                let binder_value =
                    cg.decode_abi_payload_transport(span, word_u64, gc_ref_raw, slot.ty)?;
                let _ = cg.store_local_value(span, slot.ptr, slot.ty, binder_value)?;
            }

            let rt_clear = cg.declare_runtime_effect_clear();
            let _ =
                cg.builder
                    .build_call(rt_clear, &[], "mixed_escape_matrix_step_effect_clear")?;

            let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
            let step_ptr = step_fn.as_global_value().as_pointer_value();
            let cont_call = cg.builder.build_call(
                rt_cont_alloc,
                &[state_raw.into(), step_ptr.into()],
                "mixed_escape_matrix_step_escape_cont_alloc",
            )?;
            let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape continuation alloc return value",
                    at: escape_arm.span.into(),
                },
            )?;
            let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape continuation alloc return type",
                    at: escape_arm.span.into(),
                });
            };
            let pin = cg.declare_runtime_gc_pin();
            let _ = cg.builder.build_call(
                pin,
                &[k_raw.into()],
                "mixed_escape_matrix_step_escape_k_pin",
            )?;
            let _stored = cg.store_local_value(
                span,
                cont_ptr,
                CgTy::Ref,
                CgValue {
                    ty: CgTy::Ref,
                    value: Some(k_raw.into()),
                },
            )?;

            let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
            let _ = cg.builder.build_call(
                rt_swap,
                &[prev_raw.into()],
                "mixed_escape_matrix_step_detach_for_indirect",
            )?;

            cg.env.push_scope();
            for slot in &step_escape_binder_slots {
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
            let arm_v = cg.codegen_expr_in_expected_context(&escape_arm.body, Some(out_ty))?;
            let _arm_v = if out_ty == CgTy::Unit {
                CgValue::unit()
            } else {
                cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
            };
            cg.env.pop_scope();

            let k_loaded = cg
                .builder
                .build_load(
                    llvm_ref_ty,
                    cont_ptr,
                    "mixed_escape_matrix_step_escape_k_unpin_load",
                )?
                .into_pointer_value();
            let unpin = cg.declare_runtime_gc_unpin();
            let _ = cg.builder.build_call(
                unpin,
                &[k_loaded.into()],
                "mixed_escape_matrix_step_escape_k_unpin",
            )?;
            cg.builder.build_return(None)?;

            cg.env.pop_scope();
        }
        self.builder.position_at_end(saved_block);

        let dispatch_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_resume_dispatch");
        let state0_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_resume_state0");
        let state1_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_resume_state1");
        let arm_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_resume_arm");
        let escape_dispatch_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_dispatch");
        let escape_arm_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_arm");
        let done_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_done");
        let bad_state_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_bad_state");
        let finally_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_finally");
        let finally_unwind_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_finally_unwind");
        let effect_dispatch_bb = if has_sibling_nonresuming {
            Some(
                self.context
                    .append_basic_block(func, "handle_mixed_escape_matrix_effect_dispatch"),
            )
        } else {
            None
        };
        let effect_dispatch_nomatch_bb = if has_sibling_nonresuming {
            Some(
                self.context
                    .append_basic_block(func, "handle_mixed_escape_matrix_effect_dispatch_nomatch"),
            )
        } else {
            None
        };
        let raise_catch_bb = if raise_sibling.is_some() {
            Some(
                self.context
                    .append_basic_block(func, "handle_mixed_escape_matrix_raise_catch"),
            )
        } else {
            None
        };
        let mut custom_catch_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        for (idx, _) in custom_siblings.iter().enumerate() {
            custom_catch_bbs.push(self.context.append_basic_block(
                func,
                &format!("handle_mixed_escape_matrix_custom_catch_{idx}"),
            ));
        }

        let state_ptr =
            self.create_entry_alloca_raw(span, "handle_mixed_escape_matrix_state", i32_ty.into())?;
        let resume_used_ptr = self.create_entry_alloca_raw(
            span,
            "handle_mixed_escape_matrix_resume_used",
            self.context.bool_type().into(),
        )?;
        let resume_value_ptr = if resume_value_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(
                span,
                "handle_mixed_escape_matrix_resume_value",
                resume_value_ty,
            )?)
        };
        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_mixed_escape_matrix_result", out_ty)?)
        };
        let immediate_target_ptr = self.create_entry_alloca(
            perform_decl.span,
            perform_decl
                .name
                .as_deref()
                .unwrap_or("handle_mixed_escape_matrix_immediate_value"),
            resume_value_ty,
        )?;

        let mut immediate_binder_slots: Vec<ImmediateResumeBinderSlot<'ctx>> = Vec::new();
        for binder in &immediate_arm.op.binders {
            let binder_ty = self
                .cg_ty_of(binder.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume binder type",
                    at: binder.span.into(),
                })?;
            let ptr = self.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
            immediate_binder_slots.push(ImmediateResumeBinderSlot {
                id: binder.id,
                hir_ty: binder.ty,
                ty: binder_ty,
                ptr,
            });
        }

        let mut escape_binder_slots: Vec<ImmediateResumeBinderSlot<'ctx>> = Vec::new();
        for binder in &escape_arm.op.binders {
            let binder_ty = if has_indirect_escape_site {
                match self.cg_ty_of(binder.ty) {
                    Some(CgTy::Int(int_ty)) => CgTy::Int(int_ty),
                    Some(_) | None => CgTy::Int(IntTy {
                        bits: self.host.word_bit_width(),
                        signed: true,
                    }),
                }
            } else {
                self.cg_ty_of(binder.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape binder type",
                        at: binder.span.into(),
                    })?
            };
            let ptr = self.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
            escape_binder_slots.push(ImmediateResumeBinderSlot {
                id: binder.id,
                hir_ty: binder.ty,
                ty: binder_ty,
                ptr,
            });
        }
        let cont_ptr = self.create_entry_alloca(
            span,
            &format!("handle_mixed_escape_matrix_k_{seq}"),
            CgTy::Ref,
        )?;
        let escape_binder_from_slot_ptr = self.create_entry_alloca_raw(
            span,
            "handle_mixed_escape_matrix_binder_from_slot",
            self.context.bool_type().into(),
        )?;
        let _ = self
            .builder
            .build_store(cont_ptr, self.llvm_gc_i8_ptr_type().const_null())?;
        let _ = self.builder.build_store(
            escape_binder_from_slot_ptr,
            self.context.bool_type().const_zero(),
        )?;

        let total_size = self.target_data.get_store_size(&state_ty);
        let size_bytes = self.target_data.get_store_size(&state_ty);
        let trace_start_offset_bytes =
            if outer_visible_supported.is_empty() && body_visible_supported.is_empty() {
                size_bytes
            } else {
                self.target_data
                    .offset_of_element(&state_ty, outer_field_base)
                    .unwrap_or(size_bytes)
            };
        let state_desc_global_name =
            format!("__scoop_type_desc_mixed_escape_matrix_state__{func_name}_{seq}");
        let state_desc = self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at: span,
            global_name: &state_desc_global_name,
            canonical_name: &state_ty_name,
            obj_ty: state_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })?;

        let rt_alloc = self.declare_runtime_alloc_typed();
        let size_v = i64_ty.const_int(total_size, false);
        let state_desc_i8 = self.builder.build_pointer_cast(
            state_desc.as_pointer_value(),
            i8_ptr_ty,
            "mixed_escape_matrix_state_desc_i8",
        )?;
        let call = self.builder.build_call(
            rt_alloc,
            &[state_desc_i8.into(), size_v.into()],
            "rt_alloc_mixed_escape_matrix_state",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "mixed escape matrix alloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(state_raw) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "mixed escape matrix alloc return type",
                at: span.into(),
            });
        };

        let pin = self.declare_runtime_gc_pin();
        let _ =
            self.builder
                .build_call(pin, &[state_raw.into()], "mixed_escape_matrix_state_pin")?;

        let state_gc_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let state_gc_ptr = self.builder.build_pointer_cast(
            state_raw,
            state_gc_ptr_ty,
            "mixed_escape_matrix_state_ptr",
        )?;

        for (idx, cap) in outer_visible_supported.iter().enumerate() {
            let field_idx = outer_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "mixed_escape_matrix_state_outer_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }
        for (idx, cap) in body_visible_supported.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "mixed_escape_matrix_state_body_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }

        let frame_ptr = self.builder.build_struct_gep(
            state_ty,
            state_gc_ptr,
            1,
            "mixed_escape_matrix_state_frame_gep",
        )?;
        let frame_i8 = self.builder.build_address_space_cast(
            frame_ptr,
            i8_ptr_ty,
            "mixed_escape_matrix_frame_i8",
        )?;
        let escape_tag = self.effect_op_tag(&escape_arm.op.op.fqn);
        let escape_tag_i32 = i32_ty.const_int(escape_tag as u64, false);
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), escape_tag_i32.into()],
            "mixed_escape_matrix_push",
        )?;
        let prev_ptr = self.builder.build_struct_gep(
            handler_frame_ty,
            frame_ptr,
            0,
            "mixed_escape_matrix_prev_gep",
        )?;
        let escape_outer_top = self
            .builder
            .build_load(i8_ptr_ty, prev_ptr, "mixed_escape_matrix_outer_top")?
            .into_pointer_value();

        let _ = self.builder.build_store(state_ptr, i32_ty.const_zero())?;
        let _ = self
            .builder
            .build_store(resume_used_ptr, self.context.bool_type().const_zero())?;
        let main_raise_target = effect_dispatch_bb.unwrap_or(finally_unwind_bb);

        self.builder.build_unconditional_branch(dispatch_bb)?;

        self.builder.position_at_end(dispatch_bb);
        let state = self
            .builder
            .build_load(i32_ty, state_ptr, "mixed_escape_matrix_state")?
            .into_int_value();
        let cases = [
            (i32_ty.const_int(0, false), state0_bb),
            (i32_ty.const_int(1, false), state1_bb),
        ];
        self.builder.build_switch(state, bad_state_bb, &cases)?;

        self.builder.position_at_end(bad_state_bb);
        self.emit_exit_with_code(span, 3)?;

        self.env.push_scope();
        self.env.insert(
            perform_site.id,
            CgLocal {
                hir_ty: Some(perform_decl.ty),
                ty: resume_value_ty,
                ptr: immediate_target_ptr,
                mutable: perform_decl.mutable,
            },
        );

        self.builder.position_at_end(state0_bb);
        for (idx, custom) in custom_siblings.iter().enumerate() {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom_catch_bbs[idx]);
        }
        self.push_raise_target(main_raise_target);
        for (idx, stmt) in handle.body.stmts.iter().enumerate().take(perform_idx + 1) {
            if idx < perform_idx {
                if let Some(mixed_sites) = if_mixed_site_pcs_by_stmt_idx.get(&idx) {
                    self.codegen_mixed_escape_matrix_if_stmt_mixed_sites(
                        stmt,
                        mixed_sites,
                        &escape_sites,
                        &if_next_site_pc_by_pc,
                        &body_lift_ids,
                        |cg, site_pc, direct_site| {
                            let pc_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                2,
                                "mixed_escape_matrix_state0_pc_gep",
                            )?;
                            let _ = cg
                                .builder
                                .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                            for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    outer_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_outer_gep",
                                )?;
                                let local = cg.env.get(cap.id).ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: direct_site.decl.span.into(),
                                    },
                                )?;
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: direct_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    body_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_body_gep",
                                )?;
                                let Some(local) = cg.env.get(cap.id) else {
                                    continue;
                                };
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: direct_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            for (slot, arg) in
                                escape_binder_slots.iter().zip(direct_site.args.iter())
                            {
                                let hir::CallArg::Positional(expr) = arg else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle mixed-arm escape named perform arg",
                                        at: direct_site.decl.span.into(),
                                    });
                                };
                                let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                let _ = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                            }

                            let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                            let step_ptr = step_fn.as_global_value().as_pointer_value();
                            let cont_call = cg.builder.build_call(
                                rt_cont_alloc,
                                &[state_raw.into(), step_ptr.into()],
                                "mixed_escape_matrix_state0_cont_alloc",
                            )?;
                            let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                                LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape continuation alloc return value",
                                    at: direct_site.decl.span.into(),
                                },
                            )?;
                            let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape continuation alloc return type",
                                    at: direct_site.decl.span.into(),
                                });
                            };
                            let _ = cg.builder.build_call(
                                pin,
                                &[k_raw.into()],
                                "mixed_escape_matrix_state0_k_pin",
                            )?;
                            let _ = cg.store_local_value(
                                span,
                                cont_ptr,
                                CgTy::Ref,
                                CgValue {
                                    ty: CgTy::Ref,
                                    value: Some(k_raw.into()),
                                },
                            )?;

                            let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                            let _ = cg.builder.build_call(
                                rt_swap,
                                &[escape_outer_top.into()],
                                "mixed_escape_matrix_state0_detach_for_direct",
                            )?;
                            cg.builder.build_unconditional_branch(escape_arm_bb)?;
                            Ok(())
                        },
                        |cg, site_pc, indirect_site| {
                            let pc_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                2,
                                "mixed_escape_matrix_state0_pc_gep",
                            )?;
                            let _ = cg
                                .builder
                                .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                            for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    outer_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_outer_gep",
                                )?;
                                let local = cg.env.get(cap.id).ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: indirect_site.decl.span.into(),
                                    },
                                )?;
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: indirect_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    body_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_body_gep",
                                )?;
                                let Some(local) = cg.env.get(cap.id) else {
                                    continue;
                                };
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: indirect_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            cg.pop_raise_target();
                            cg.push_raise_target(escape_dispatch_bb);
                            cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                indirect_site,
                                &body_lift_ids,
                            )?;
                            cg.pop_raise_target();
                            cg.push_raise_target(main_raise_target);
                            Ok(())
                        },
                    )?;
                    continue;
                }

                if let Some(mixed_sites) = while_mixed_site_pcs_by_stmt_idx.get(&idx) {
                    self.codegen_mixed_escape_matrix_while_stmt_mixed_sites(
                        stmt,
                        mixed_sites,
                        &escape_sites,
                        &body_lift_ids,
                        |cg, site_pc, direct_site| {
                            let pc_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                2,
                                "mixed_escape_matrix_state0_pc_gep",
                            )?;
                            let _ = cg
                                .builder
                                .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                            for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    outer_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_outer_gep",
                                )?;
                                let local = cg.env.get(cap.id).ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: direct_site.decl.span.into(),
                                    },
                                )?;
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: direct_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    body_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_body_gep",
                                )?;
                                let Some(local) = cg.env.get(cap.id) else {
                                    continue;
                                };
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: direct_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            for (slot, arg) in
                                escape_binder_slots.iter().zip(direct_site.args.iter())
                            {
                                let hir::CallArg::Positional(expr) = arg else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle mixed-arm escape named perform arg",
                                        at: direct_site.decl.span.into(),
                                    });
                                };
                                let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                let _ = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                            }

                            let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                            let step_ptr = step_fn.as_global_value().as_pointer_value();
                            let cont_call = cg.builder.build_call(
                                rt_cont_alloc,
                                &[state_raw.into(), step_ptr.into()],
                                "mixed_escape_matrix_state0_cont_alloc",
                            )?;
                            let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                                LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape continuation alloc return value",
                                    at: direct_site.decl.span.into(),
                                },
                            )?;
                            let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape continuation alloc return type",
                                    at: direct_site.decl.span.into(),
                                });
                            };
                            let _ = cg.builder.build_call(
                                pin,
                                &[k_raw.into()],
                                "mixed_escape_matrix_state0_k_pin",
                            )?;
                            let _ = cg.store_local_value(
                                span,
                                cont_ptr,
                                CgTy::Ref,
                                CgValue {
                                    ty: CgTy::Ref,
                                    value: Some(k_raw.into()),
                                },
                            )?;

                            let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                            let _ = cg.builder.build_call(
                                rt_swap,
                                &[escape_outer_top.into()],
                                "mixed_escape_matrix_state0_detach_for_direct",
                            )?;
                            cg.builder.build_unconditional_branch(escape_arm_bb)?;
                            Ok(())
                        },
                        |cg, site_pc, indirect_site| {
                            let pc_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                2,
                                "mixed_escape_matrix_state0_pc_gep",
                            )?;
                            let _ = cg
                                .builder
                                .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                            for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    outer_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_outer_gep",
                                )?;
                                let local = cg.env.get(cap.id).ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: indirect_site.decl.span.into(),
                                    },
                                )?;
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: indirect_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    body_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_body_gep",
                                )?;
                                let Some(local) = cg.env.get(cap.id) else {
                                    continue;
                                };
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: indirect_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            cg.pop_raise_target();
                            cg.push_raise_target(escape_dispatch_bb);
                            cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                indirect_site,
                                &body_lift_ids,
                            )?;
                            cg.pop_raise_target();
                            cg.push_raise_target(main_raise_target);
                            Ok(())
                        },
                    )?;
                    continue;
                }

                if let Some(direct_sites) = if_direct_site_pcs_by_stmt_idx.get(&idx) {
                    self.codegen_mixed_escape_matrix_if_stmt_direct_sites(
                        stmt,
                        direct_sites,
                        &escape_sites,
                        &body_lift_ids,
                        |cg, site_pc, direct_site| {
                            let pc_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                2,
                                "mixed_escape_matrix_state0_pc_gep",
                            )?;
                            let _ = cg
                                .builder
                                .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                            for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    outer_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_outer_gep",
                                )?;
                                let local = cg.env.get(cap.id).ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: direct_site.decl.span.into(),
                                    },
                                )?;
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: direct_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    body_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_body_gep",
                                )?;
                                let Some(local) = cg.env.get(cap.id) else {
                                    continue;
                                };
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: direct_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            for (slot, arg) in
                                escape_binder_slots.iter().zip(direct_site.args.iter())
                            {
                                let hir::CallArg::Positional(expr) = arg else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle mixed-arm escape named perform arg",
                                        at: direct_site.decl.span.into(),
                                    });
                                };
                                let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                let _ = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                            }

                            let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                            let step_ptr = step_fn.as_global_value().as_pointer_value();
                            let cont_call = cg.builder.build_call(
                                rt_cont_alloc,
                                &[state_raw.into(), step_ptr.into()],
                                "mixed_escape_matrix_state0_cont_alloc",
                            )?;
                            let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                                LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape continuation alloc return value",
                                    at: direct_site.decl.span.into(),
                                },
                            )?;
                            let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape continuation alloc return type",
                                    at: direct_site.decl.span.into(),
                                });
                            };
                            let _ = cg.builder.build_call(
                                pin,
                                &[k_raw.into()],
                                "mixed_escape_matrix_state0_k_pin",
                            )?;
                            let _ = cg.store_local_value(
                                span,
                                cont_ptr,
                                CgTy::Ref,
                                CgValue {
                                    ty: CgTy::Ref,
                                    value: Some(k_raw.into()),
                                },
                            )?;

                            let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                            let _ = cg.builder.build_call(
                                rt_swap,
                                &[escape_outer_top.into()],
                                "mixed_escape_matrix_state0_detach_for_direct",
                            )?;
                            cg.builder.build_unconditional_branch(escape_arm_bb)?;
                            Ok(())
                        },
                    )?;
                    continue;
                }

                if let Some(indirect_sites) = if_indirect_site_pcs_by_stmt_idx.get(&idx) {
                    self.codegen_mixed_escape_matrix_if_stmt_indirect_sites(
                        stmt,
                        indirect_sites,
                        &escape_sites,
                        &body_lift_ids,
                        |cg, site_pc, indirect_site| {
                            let pc_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                2,
                                "mixed_escape_matrix_state0_pc_gep",
                            )?;
                            let _ = cg
                                .builder
                                .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                            for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    outer_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_outer_gep",
                                )?;
                                let local = cg.env.get(cap.id).ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: indirect_site.decl.span.into(),
                                    },
                                )?;
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: indirect_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    body_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_body_gep",
                                )?;
                                let Some(local) = cg.env.get(cap.id) else {
                                    continue;
                                };
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: indirect_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            cg.pop_raise_target();
                            cg.push_raise_target(escape_dispatch_bb);
                            cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                indirect_site,
                                &body_lift_ids,
                            )?;
                            cg.pop_raise_target();
                            cg.push_raise_target(main_raise_target);
                            Ok(())
                        },
                    )?;
                    continue;
                }

                if let Some(&site_pc) = while_indirect_site_pc_by_stmt_idx.get(&idx) {
                    let MatrixEscapeSiteKind::Indirect {
                        site: indirect_site,
                    } = &escape_sites[site_pc].kind
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (expected indirect site)",
                            at: stmt.span.into(),
                        });
                    };
                    self.codegen_mixed_escape_matrix_while_stmt_indirect_site(
                        stmt,
                        site_pc,
                        indirect_site,
                        &body_lift_ids,
                        |cg, site_pc, indirect_site| {
                            let pc_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                2,
                                "mixed_escape_matrix_state0_pc_gep",
                            )?;
                            let _ = cg
                                .builder
                                .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                            for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    outer_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_outer_gep",
                                )?;
                                let local = cg.env.get(cap.id).ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: indirect_site.decl.span.into(),
                                    },
                                )?;
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: indirect_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    body_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_body_gep",
                                )?;
                                let Some(local) = cg.env.get(cap.id) else {
                                    continue;
                                };
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: indirect_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            cg.pop_raise_target();
                            cg.push_raise_target(escape_dispatch_bb);
                            cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                indirect_site,
                                &body_lift_ids,
                            )?;
                            cg.pop_raise_target();
                            cg.push_raise_target(main_raise_target);
                            Ok(())
                        },
                    )?;
                    continue;
                }

                if let Some(&site_pc) = while_direct_site_pc_by_stmt_idx.get(&idx) {
                    let MatrixEscapeSiteKind::Direct { site: direct_site } =
                        &escape_sites[site_pc].kind
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (expected direct site)",
                            at: stmt.span.into(),
                        });
                    };
                    self.codegen_mixed_escape_matrix_while_stmt_direct_site(
                        stmt,
                        site_pc,
                        direct_site,
                        &body_lift_ids,
                        |cg, site_pc, direct_site| {
                            let pc_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                2,
                                "mixed_escape_matrix_state0_pc_gep",
                            )?;
                            let _ = cg
                                .builder
                                .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                            for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    outer_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_outer_gep",
                                )?;
                                let local = cg.env.get(cap.id).ok_or(
                                    LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: direct_site.decl.span.into(),
                                    },
                                )?;
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: direct_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                                let field_ptr = cg.builder.build_struct_gep(
                                    state_ty,
                                    state_gc_ptr,
                                    body_field_base.saturating_add(field_idx as u32),
                                    "mixed_escape_matrix_state0_capture_body_gep",
                                )?;
                                let Some(local) = cg.env.get(cap.id) else {
                                    continue;
                                };
                                if local.ty != cap.ty {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local type mismatch",
                                        at: direct_site.decl.span.into(),
                                    });
                                }
                                cg.write_escape_capture_local_to_state(
                                    span, field_ptr, local.ptr, cap.ty,
                                )?;
                            }

                            for (slot, arg) in
                                escape_binder_slots.iter().zip(direct_site.args.iter())
                            {
                                let hir::CallArg::Positional(expr) = arg else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle mixed-arm escape named perform arg",
                                        at: direct_site.decl.span.into(),
                                    });
                                };
                                let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                let _ = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                            }

                            let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                            let step_ptr = step_fn.as_global_value().as_pointer_value();
                            let cont_call = cg.builder.build_call(
                                rt_cont_alloc,
                                &[state_raw.into(), step_ptr.into()],
                                "mixed_escape_matrix_state0_cont_alloc",
                            )?;
                            let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                                LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape continuation alloc return value",
                                    at: direct_site.decl.span.into(),
                                },
                            )?;
                            let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape continuation alloc return type",
                                    at: direct_site.decl.span.into(),
                                });
                            };
                            let _ = cg.builder.build_call(
                                pin,
                                &[k_raw.into()],
                                "mixed_escape_matrix_state0_k_pin",
                            )?;
                            let _ = cg.store_local_value(
                                span,
                                cont_ptr,
                                CgTy::Ref,
                                CgValue {
                                    ty: CgTy::Ref,
                                    value: Some(k_raw.into()),
                                },
                            )?;

                            let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                            let _ = cg.builder.build_call(
                                rt_swap,
                                &[escape_outer_top.into()],
                                "mixed_escape_matrix_state0_detach_for_direct",
                            )?;
                            cg.builder.build_unconditional_branch(escape_arm_bb)?;
                            Ok(())
                        },
                    )?;
                    continue;
                }

                if let Some(&site_pc) = simple_escape_site_pc_by_stmt_idx.get(&idx) {
                    let site = &escape_sites[site_pc];
                    if let MatrixEscapeSiteKind::Direct { site: direct_site } = &site.kind
                        && !direct_site.resume_path.is_empty()
                    {
                        self.codegen_mixed_escape_matrix_nested_block_prefix_to_site(
                            direct_site,
                            stmt,
                            &body_lift_ids,
                        )?;
                    } else if let MatrixEscapeSiteKind::Indirect {
                        site: indirect_site,
                    } = &site.kind
                        && !indirect_site.resume_path.is_empty()
                    {
                        self.codegen_mixed_escape_matrix_prefix_to_indirect_site(
                            indirect_site,
                            stmt,
                            &body_lift_ids,
                        )?;
                    }
                    let pc_ptr = self.builder.build_struct_gep(
                        state_ty,
                        state_gc_ptr,
                        2,
                        "mixed_escape_matrix_state0_pc_gep",
                    )?;
                    let _ = self
                        .builder
                        .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                    for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                        let field_ptr = self.builder.build_struct_gep(
                            state_ty,
                            state_gc_ptr,
                            outer_field_base.saturating_add(field_idx as u32),
                            "mixed_escape_matrix_state0_capture_outer_gep",
                        )?;
                        let local =
                            self.env
                                .get(cap.id)
                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local not found",
                                    at: site.decl.span.into(),
                                })?;
                        if local.ty != cap.ty {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape capture local type mismatch",
                                at: site.decl.span.into(),
                            });
                        }
                        self.write_escape_capture_local_to_state(
                            span, field_ptr, local.ptr, cap.ty,
                        )?;
                    }

                    for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                        let field_ptr = self.builder.build_struct_gep(
                            state_ty,
                            state_gc_ptr,
                            body_field_base.saturating_add(field_idx as u32),
                            "mixed_escape_matrix_state0_capture_body_gep",
                        )?;
                        let Some(local) = self.env.get(cap.id) else {
                            continue;
                        };
                        if local.ty != cap.ty {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape capture local type mismatch",
                                at: site.decl.span.into(),
                            });
                        }
                        self.write_escape_capture_local_to_state(
                            span, field_ptr, local.ptr, cap.ty,
                        )?;
                    }

                    match &site.kind {
                        MatrixEscapeSiteKind::Direct { site: direct_site } => {
                            for (slot, arg) in
                                escape_binder_slots.iter().zip(direct_site.args.iter())
                            {
                                let hir::CallArg::Positional(expr) = arg else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle mixed-arm escape named perform arg",
                                        at: direct_site.decl.span.into(),
                                    });
                                };
                                let v =
                                    self.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                let _ = self.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                            }

                            let rt_cont_alloc = self.declare_runtime_continuation_alloc();
                            let step_ptr = step_fn.as_global_value().as_pointer_value();
                            let cont_call = self.builder.build_call(
                                rt_cont_alloc,
                                &[state_raw.into(), step_ptr.into()],
                                "mixed_escape_matrix_state0_cont_alloc",
                            )?;
                            let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                                LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape continuation alloc return value",
                                    at: direct_site.decl.span.into(),
                                },
                            )?;
                            let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape continuation alloc return type",
                                    at: direct_site.decl.span.into(),
                                });
                            };
                            let _ = self.builder.build_call(
                                pin,
                                &[k_raw.into()],
                                "mixed_escape_matrix_state0_k_pin",
                            )?;
                            let _ = self.store_local_value(
                                span,
                                cont_ptr,
                                CgTy::Ref,
                                CgValue {
                                    ty: CgTy::Ref,
                                    value: Some(k_raw.into()),
                                },
                            )?;

                            let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
                            let _ = self.builder.build_call(
                                rt_swap,
                                &[escape_outer_top.into()],
                                "mixed_escape_matrix_state0_detach_for_direct",
                            )?;
                            self.builder.build_unconditional_branch(escape_arm_bb)?;
                            break;
                        }
                        MatrixEscapeSiteKind::Indirect {
                            site: indirect_site,
                        } => {
                            self.pop_raise_target();
                            self.push_raise_target(escape_dispatch_bb);
                            self.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                indirect_site,
                                &body_lift_ids,
                            )?;
                            self.pop_raise_target();
                            self.push_raise_target(main_raise_target);
                            if let Some(&next_pc) = block_next_site_pc_by_pc.get(&site_pc) {
                                let next_site = &escape_sites[next_pc];
                                self.codegen_mixed_escape_matrix_continue_to_next_block_site_after_indirect(
                                indirect_site,
                                next_pc,
                                next_site,
                                &body_lift_ids,
                                &mut |cg, next_pc, direct_site| {
                                    let pc_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_gc_ptr,
                                        2,
                                        "mixed_escape_matrix_state0_pc_gep",
                                    )?;
                                    let _ = cg
                                        .builder
                                        .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                                    for (field_idx, cap) in
                                        outer_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_gc_ptr,
                                            outer_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_state0_capture_outer_gep",
                                        )?;
                                        let local = cg.env.get(cap.id).ok_or(
                                            LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local not found",
                                                at: direct_site.decl.span.into(),
                                            },
                                        )?;
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: direct_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    for (field_idx, cap) in
                                        body_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_gc_ptr,
                                            body_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_state0_capture_body_gep",
                                        )?;
                                        let Some(local) = cg.env.get(cap.id) else {
                                            continue;
                                        };
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: direct_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    for (slot, arg) in
                                        escape_binder_slots.iter().zip(direct_site.args.iter())
                                    {
                                        let hir::CallArg::Positional(expr) = arg else {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "handle mixed-arm escape named perform arg",
                                                at: direct_site.decl.span.into(),
                                            });
                                        };
                                        let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                        let _stored = cg.store_local_value(
                                            expr.span,
                                            slot.ptr,
                                            slot.ty,
                                            v,
                                        )?;
                                    }

                                    let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                                    let step_ptr = step_fn.as_global_value().as_pointer_value();
                                    let cont_call = cg.builder.build_call(
                                        rt_cont_alloc,
                                        &[state_raw.into(), step_ptr.into()],
                                        "mixed_escape_matrix_state0_cont_alloc",
                                    )?;
                                    let cont_raw =
                                        cont_call.try_as_basic_value().basic().ok_or(
                                            LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape continuation alloc return value",
                                                at: direct_site.decl.span.into(),
                                            },
                                        )?;
                                    let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape continuation alloc return type",
                                            at: direct_site.decl.span.into(),
                                        });
                                    };
                                    let _ = cg.builder.build_call(
                                        pin,
                                        &[k_raw.into()],
                                        "mixed_escape_matrix_state0_k_pin",
                                    )?;
                                    let _ = cg.store_local_value(
                                        span,
                                        cont_ptr,
                                        CgTy::Ref,
                                        CgValue {
                                            ty: CgTy::Ref,
                                            value: Some(k_raw.into()),
                                        },
                                    )?;

                                    let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                                    let _ = cg.builder.build_call(
                                        rt_swap,
                                        &[escape_outer_top.into()],
                                        "mixed_escape_matrix_state0_detach_for_direct",
                                    )?;
                                    cg.builder.build_unconditional_branch(escape_arm_bb)?;
                                    Ok(())
                                },
                                &mut |cg, next_pc, next_indirect_site| {
                                    let pc_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_gc_ptr,
                                        2,
                                        "mixed_escape_matrix_state0_pc_gep",
                                    )?;
                                    let _ = cg
                                        .builder
                                        .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                                    for (field_idx, cap) in
                                        outer_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_gc_ptr,
                                            outer_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_state0_capture_outer_gep",
                                        )?;
                                        let local = cg.env.get(cap.id).ok_or(
                                            LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local not found",
                                                at: next_indirect_site.decl.span.into(),
                                            },
                                        )?;
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: next_indirect_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    for (field_idx, cap) in
                                        body_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_gc_ptr,
                                            body_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_state0_capture_body_gep",
                                        )?;
                                        let Some(local) = cg.env.get(cap.id) else {
                                            continue;
                                        };
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: next_indirect_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    cg.pop_raise_target();
                                    cg.push_raise_target(escape_dispatch_bb);
                                    cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                        next_indirect_site,
                                        &body_lift_ids,
                                    )?;
                                    cg.pop_raise_target();
                                    cg.push_raise_target(main_raise_target);
                                    Ok(())
                                },
                            )?;
                                if let MatrixEscapeSiteKind::Indirect {
                                    site: next_indirect_site,
                                } = &next_site.kind
                                    && let Some(bb) = self.builder.get_insert_block()
                                    && bb.get_terminator().is_none()
                                {
                                    self.codegen_mixed_escape_matrix_nested_block_continue_after_indirect_site(
                                    next_indirect_site,
                                    &body_lift_ids,
                                )?;
                                }
                                if let Some(bb) = self.builder.get_insert_block()
                                    && bb.get_terminator().is_some()
                                {
                                    break;
                                }
                            } else {
                                self.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                    indirect_site,
                                    &body_lift_ids,
                                )?;
                            }
                            continue;
                        }
                    }
                }

                self.codegen_immediate_resume_stmt_unit(stmt)?;
                continue;
            }

            let hir::StmtKind::Val(immediate_decl) = &stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm immediate-resume body (expected perform binding)",
                    at: stmt.span.into(),
                });
            };
            let _ = self.codegen_immediate_resume_site_binding(
                &perform_site,
                immediate_decl,
                ImmediateResumeArmDispatch {
                    binder_slots: &immediate_binder_slots,
                    resume_used_ptr,
                    arm_bb,
                },
                Some(immediate_target_ptr),
            )?;
            break;
        }
        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }

        self.builder.position_at_end(arm_bb);
        let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
        let _ = self.builder.build_call(
            rt_swap,
            &[escape_outer_top.into()],
            "mixed_escape_matrix_detach_for_immediate_arm",
        )?;

        self.env.push_scope();
        for slot in &immediate_binder_slots {
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
        for custom in &custom_siblings {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, finally_unwind_bb);
        }
        self.push_raise_target(finally_unwind_bb);
        let _ = self.codegen_expr_in_expected_context(&immediate_arm.body, Some(CgTy::Unit))?;
        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }
        self.pop_immediate_resume_ctx();

        let arm_insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let arm_func = arm_insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;
        let resume_ok_bb = self
            .context
            .append_basic_block(arm_func, "handle_mixed_escape_matrix_resume_arm_ok");
        let resume_missing_bb = self
            .context
            .append_basic_block(arm_func, "handle_mixed_escape_matrix_resume_arm_missing");

        let used = self
            .builder
            .build_load(self.context.bool_type(), resume_used_ptr, "resume_used")?
            .into_int_value();
        self.builder
            .build_conditional_branch(used, resume_ok_bb, resume_missing_bb)?;

        self.builder.position_at_end(resume_missing_bb);
        self.emit_exit_with_code(span, 3)?;

        self.builder.position_at_end(resume_ok_bb);
        let _ = self.builder.build_call(
            rt_swap,
            &[frame_i8.into()],
            "mixed_escape_matrix_restore_after_immediate_arm",
        )?;
        self.builder.build_unconditional_branch(dispatch_bb)?;
        self.env.pop_scope();

        self.builder.position_at_end(state1_bb);
        for (idx, custom) in custom_siblings.iter().enumerate() {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom_catch_bbs[idx]);
        }
        self.push_raise_target(main_raise_target);
        if let Some(ptr) = resume_value_ptr {
            let llvm_ty = self.llvm_basic_type_of(span, resume_value_ty)?;
            let loaded = self.builder.build_load(llvm_ty, ptr, "resume_value")?;
            let v = CgValue {
                ty: resume_value_ty,
                value: Some(loaded),
            };
            let _stored = self.store_local_value(span, immediate_target_ptr, resume_value_ty, v)?;
        }

        let mut escaped = false;
        let mut tail_value: CgValue<'ctx> = CgValue::unit();
        for (idx, stmt) in handle.body.stmts.iter().enumerate().skip(perform_idx + 1) {
            if let Some(mixed_sites) = if_mixed_site_pcs_by_stmt_idx.get(&idx) {
                self.codegen_mixed_escape_matrix_if_stmt_mixed_sites(
                    stmt,
                    mixed_sites,
                    &escape_sites,
                    &if_next_site_pc_by_pc,
                    &body_lift_ids,
                    |cg, site_pc, direct_site| {
                        let pc_ptr = cg.builder.build_struct_gep(
                            state_ty,
                            state_gc_ptr,
                            2,
                            "mixed_escape_matrix_pc_gep",
                        )?;
                        let _ = cg
                            .builder
                            .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                        for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                outer_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_outer_gep",
                            )?;
                            let local =
                                cg.env
                                    .get(cap.id)
                                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: direct_site.decl.span.into(),
                                    })?;
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: direct_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                body_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_body_gep",
                            )?;
                            let Some(local) = cg.env.get(cap.id) else {
                                continue;
                            };
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: direct_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        for (slot, arg) in escape_binder_slots.iter().zip(direct_site.args.iter()) {
                            let hir::CallArg::Positional(expr) = arg else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape named perform arg",
                                    at: direct_site.decl.span.into(),
                                });
                            };
                            let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                            let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                        }

                        let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                        let step_ptr = step_fn.as_global_value().as_pointer_value();
                        let cont_call = cg.builder.build_call(
                            rt_cont_alloc,
                            &[state_raw.into(), step_ptr.into()],
                            "mixed_escape_matrix_cont_alloc",
                        )?;
                        let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape continuation alloc return value",
                                at: direct_site.decl.span.into(),
                            },
                        )?;
                        let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape continuation alloc return type",
                                at: direct_site.decl.span.into(),
                            });
                        };
                        let _ = cg.builder.build_call(
                            pin,
                            &[k_raw.into()],
                            "mixed_escape_matrix_k_pin",
                        )?;
                        let _stored = cg.store_local_value(
                            span,
                            cont_ptr,
                            CgTy::Ref,
                            CgValue {
                                ty: CgTy::Ref,
                                value: Some(k_raw.into()),
                            },
                        )?;

                        let _ = cg.builder.build_call(
                            rt_swap,
                            &[escape_outer_top.into()],
                            "mixed_escape_matrix_detach_for_direct",
                        )?;
                        cg.builder.build_unconditional_branch(escape_arm_bb)?;
                        Ok(())
                    },
                    |cg, site_pc, indirect_site| {
                        let pc_ptr = cg.builder.build_struct_gep(
                            state_ty,
                            state_gc_ptr,
                            2,
                            "mixed_escape_matrix_pc_gep",
                        )?;
                        let _ = cg
                            .builder
                            .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                        for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                outer_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_outer_gep",
                            )?;
                            let local =
                                cg.env
                                    .get(cap.id)
                                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: indirect_site.decl.span.into(),
                                    })?;
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: indirect_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                body_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_body_gep",
                            )?;
                            let Some(local) = cg.env.get(cap.id) else {
                                continue;
                            };
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: indirect_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        cg.pop_raise_target();
                        cg.push_raise_target(escape_dispatch_bb);
                        cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                            indirect_site,
                            &body_lift_ids,
                        )?;
                        cg.pop_raise_target();
                        cg.push_raise_target(main_raise_target);
                        Ok(())
                    },
                )?;
                tail_value = CgValue::unit();
                continue;
            }

            if let Some(mixed_sites) = while_mixed_site_pcs_by_stmt_idx.get(&idx) {
                self.codegen_mixed_escape_matrix_while_stmt_mixed_sites(
                    stmt,
                    mixed_sites,
                    &escape_sites,
                    &body_lift_ids,
                    |cg, site_pc, direct_site| {
                        let pc_ptr = cg.builder.build_struct_gep(
                            state_ty,
                            state_gc_ptr,
                            2,
                            "mixed_escape_matrix_pc_gep",
                        )?;
                        let _ = cg
                            .builder
                            .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                        for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                outer_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_outer_gep",
                            )?;
                            let local =
                                cg.env
                                    .get(cap.id)
                                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: direct_site.decl.span.into(),
                                    })?;
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: direct_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                body_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_body_gep",
                            )?;
                            let Some(local) = cg.env.get(cap.id) else {
                                continue;
                            };
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: direct_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        for (slot, arg) in escape_binder_slots.iter().zip(direct_site.args.iter()) {
                            let hir::CallArg::Positional(expr) = arg else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape named perform arg",
                                    at: direct_site.decl.span.into(),
                                });
                            };
                            let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                            let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                        }

                        let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                        let step_ptr = step_fn.as_global_value().as_pointer_value();
                        let cont_call = cg.builder.build_call(
                            rt_cont_alloc,
                            &[state_raw.into(), step_ptr.into()],
                            "mixed_escape_matrix_cont_alloc",
                        )?;
                        let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape continuation alloc return value",
                                at: direct_site.decl.span.into(),
                            },
                        )?;
                        let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape continuation alloc return type",
                                at: direct_site.decl.span.into(),
                            });
                        };
                        let _ = cg.builder.build_call(
                            pin,
                            &[k_raw.into()],
                            "mixed_escape_matrix_k_pin",
                        )?;
                        let _stored = cg.store_local_value(
                            span,
                            cont_ptr,
                            CgTy::Ref,
                            CgValue {
                                ty: CgTy::Ref,
                                value: Some(k_raw.into()),
                            },
                        )?;

                        let _ = cg.builder.build_call(
                            rt_swap,
                            &[escape_outer_top.into()],
                            "mixed_escape_matrix_detach_for_direct",
                        )?;
                        cg.builder.build_unconditional_branch(escape_arm_bb)?;
                        Ok(())
                    },
                    |cg, site_pc, indirect_site| {
                        let pc_ptr = cg.builder.build_struct_gep(
                            state_ty,
                            state_gc_ptr,
                            2,
                            "mixed_escape_matrix_pc_gep",
                        )?;
                        let _ = cg
                            .builder
                            .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                        for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                outer_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_outer_gep",
                            )?;
                            let local =
                                cg.env
                                    .get(cap.id)
                                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: indirect_site.decl.span.into(),
                                    })?;
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: indirect_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                body_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_body_gep",
                            )?;
                            let Some(local) = cg.env.get(cap.id) else {
                                continue;
                            };
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: indirect_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        cg.pop_raise_target();
                        cg.push_raise_target(escape_dispatch_bb);
                        cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                            indirect_site,
                            &body_lift_ids,
                        )?;
                        cg.pop_raise_target();
                        cg.push_raise_target(main_raise_target);
                        Ok(())
                    },
                )?;
                tail_value = CgValue::unit();
                continue;
            }

            if let Some(direct_sites) = if_direct_site_pcs_by_stmt_idx.get(&idx) {
                self.codegen_mixed_escape_matrix_if_stmt_direct_sites(
                    stmt,
                    direct_sites,
                    &escape_sites,
                    &body_lift_ids,
                    |cg, site_pc, direct_site| {
                        let pc_ptr = cg.builder.build_struct_gep(
                            state_ty,
                            state_gc_ptr,
                            2,
                            "mixed_escape_matrix_pc_gep",
                        )?;
                        let _ = cg
                            .builder
                            .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                        for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                outer_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_outer_gep",
                            )?;
                            let local =
                                cg.env
                                    .get(cap.id)
                                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: direct_site.decl.span.into(),
                                    })?;
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: direct_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                body_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_body_gep",
                            )?;
                            let Some(local) = cg.env.get(cap.id) else {
                                continue;
                            };
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: direct_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        for (slot, arg) in escape_binder_slots.iter().zip(direct_site.args.iter()) {
                            let hir::CallArg::Positional(expr) = arg else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape named perform arg",
                                    at: direct_site.decl.span.into(),
                                });
                            };
                            let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                            let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                        }

                        let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                        let step_ptr = step_fn.as_global_value().as_pointer_value();
                        let cont_call = cg.builder.build_call(
                            rt_cont_alloc,
                            &[state_raw.into(), step_ptr.into()],
                            "mixed_escape_matrix_cont_alloc",
                        )?;
                        let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape continuation alloc return value",
                                at: direct_site.decl.span.into(),
                            },
                        )?;
                        let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape continuation alloc return type",
                                at: direct_site.decl.span.into(),
                            });
                        };
                        let _ = cg.builder.build_call(
                            pin,
                            &[k_raw.into()],
                            "mixed_escape_matrix_k_pin",
                        )?;
                        let _stored = cg.store_local_value(
                            span,
                            cont_ptr,
                            CgTy::Ref,
                            CgValue {
                                ty: CgTy::Ref,
                                value: Some(k_raw.into()),
                            },
                        )?;

                        let _ = cg.builder.build_call(
                            rt_swap,
                            &[escape_outer_top.into()],
                            "mixed_escape_matrix_detach_for_direct",
                        )?;
                        cg.builder.build_unconditional_branch(escape_arm_bb)?;
                        Ok(())
                    },
                )?;
                tail_value = CgValue::unit();
                continue;
            }

            if let Some(indirect_sites) = if_indirect_site_pcs_by_stmt_idx.get(&idx) {
                self.codegen_mixed_escape_matrix_if_stmt_indirect_sites(
                    stmt,
                    indirect_sites,
                    &escape_sites,
                    &body_lift_ids,
                    |cg, site_pc, indirect_site| {
                        let pc_ptr = cg.builder.build_struct_gep(
                            state_ty,
                            state_gc_ptr,
                            2,
                            "mixed_escape_matrix_pc_gep",
                        )?;
                        let _ = cg
                            .builder
                            .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                        for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                outer_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_outer_gep",
                            )?;
                            let local =
                                cg.env
                                    .get(cap.id)
                                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: indirect_site.decl.span.into(),
                                    })?;
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: indirect_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                body_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_body_gep",
                            )?;
                            let Some(local) = cg.env.get(cap.id) else {
                                continue;
                            };
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: indirect_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        cg.pop_raise_target();
                        cg.push_raise_target(escape_dispatch_bb);
                        cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                            indirect_site,
                            &body_lift_ids,
                        )?;
                        cg.pop_raise_target();
                        cg.push_raise_target(main_raise_target);
                        Ok(())
                    },
                )?;
                tail_value = CgValue::unit();
                continue;
            }

            if let Some(&site_pc) = while_indirect_site_pc_by_stmt_idx.get(&idx) {
                let MatrixEscapeSiteKind::Indirect {
                    site: indirect_site,
                } = &escape_sites[site_pc].kind
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected indirect site)",
                        at: stmt.span.into(),
                    });
                };
                self.codegen_mixed_escape_matrix_while_stmt_indirect_site(
                    stmt,
                    site_pc,
                    indirect_site,
                    &body_lift_ids,
                    |cg, site_pc, indirect_site| {
                        let pc_ptr = cg.builder.build_struct_gep(
                            state_ty,
                            state_gc_ptr,
                            2,
                            "mixed_escape_matrix_pc_gep",
                        )?;
                        let _ = cg
                            .builder
                            .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                        for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                outer_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_outer_gep",
                            )?;
                            let local =
                                cg.env
                                    .get(cap.id)
                                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: indirect_site.decl.span.into(),
                                    })?;
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: indirect_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                body_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_body_gep",
                            )?;
                            let Some(local) = cg.env.get(cap.id) else {
                                continue;
                            };
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: indirect_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        cg.pop_raise_target();
                        cg.push_raise_target(escape_dispatch_bb);
                        cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                            indirect_site,
                            &body_lift_ids,
                        )?;
                        cg.pop_raise_target();
                        cg.push_raise_target(main_raise_target);
                        Ok(())
                    },
                )?;
                tail_value = CgValue::unit();
                continue;
            }

            if let Some(&site_pc) = while_direct_site_pc_by_stmt_idx.get(&idx) {
                let MatrixEscapeSiteKind::Direct { site: direct_site } =
                    &escape_sites[site_pc].kind
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected direct site)",
                        at: stmt.span.into(),
                    });
                };
                self.codegen_mixed_escape_matrix_while_stmt_direct_site(
                    stmt,
                    site_pc,
                    direct_site,
                    &body_lift_ids,
                    |cg, site_pc, direct_site| {
                        let pc_ptr = cg.builder.build_struct_gep(
                            state_ty,
                            state_gc_ptr,
                            2,
                            "mixed_escape_matrix_pc_gep",
                        )?;
                        let _ = cg
                            .builder
                            .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                        for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                outer_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_outer_gep",
                            )?;
                            let local =
                                cg.env
                                    .get(cap.id)
                                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: direct_site.decl.span.into(),
                                    })?;
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: direct_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_gc_ptr,
                                body_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_matrix_capture_body_gep",
                            )?;
                            let Some(local) = cg.env.get(cap.id) else {
                                continue;
                            };
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: direct_site.decl.span.into(),
                                });
                            }
                            cg.write_escape_capture_local_to_state(
                                span, field_ptr, local.ptr, cap.ty,
                            )?;
                        }

                        for (slot, arg) in escape_binder_slots.iter().zip(direct_site.args.iter()) {
                            let hir::CallArg::Positional(expr) = arg else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape named perform arg",
                                    at: direct_site.decl.span.into(),
                                });
                            };
                            let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                            let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                        }

                        let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                        let step_ptr = step_fn.as_global_value().as_pointer_value();
                        let cont_call = cg.builder.build_call(
                            rt_cont_alloc,
                            &[state_raw.into(), step_ptr.into()],
                            "mixed_escape_matrix_cont_alloc",
                        )?;
                        let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape continuation alloc return value",
                                at: direct_site.decl.span.into(),
                            },
                        )?;
                        let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape continuation alloc return type",
                                at: direct_site.decl.span.into(),
                            });
                        };
                        let _ = cg.builder.build_call(
                            pin,
                            &[k_raw.into()],
                            "mixed_escape_matrix_k_pin",
                        )?;
                        let _stored = cg.store_local_value(
                            span,
                            cont_ptr,
                            CgTy::Ref,
                            CgValue {
                                ty: CgTy::Ref,
                                value: Some(k_raw.into()),
                            },
                        )?;

                        let _ = cg.builder.build_call(
                            rt_swap,
                            &[escape_outer_top.into()],
                            "mixed_escape_matrix_detach_for_direct",
                        )?;
                        cg.builder.build_unconditional_branch(escape_arm_bb)?;
                        Ok(())
                    },
                )?;
                tail_value = CgValue::unit();
                continue;
            }

            if let Some(&site_pc) = simple_escape_site_pc_by_stmt_idx.get(&idx) {
                let site = &escape_sites[site_pc];
                if let MatrixEscapeSiteKind::Direct { site: direct_site } = &site.kind
                    && !direct_site.resume_path.is_empty()
                {
                    self.codegen_mixed_escape_matrix_nested_block_prefix_to_site(
                        direct_site,
                        stmt,
                        &body_lift_ids,
                    )?;
                } else if let MatrixEscapeSiteKind::Indirect {
                    site: indirect_site,
                } = &site.kind
                    && !indirect_site.resume_path.is_empty()
                {
                    self.codegen_mixed_escape_matrix_prefix_to_indirect_site(
                        indirect_site,
                        stmt,
                        &body_lift_ids,
                    )?;
                }
                let pc_ptr = self.builder.build_struct_gep(
                    state_ty,
                    state_gc_ptr,
                    2,
                    "mixed_escape_matrix_pc_gep",
                )?;
                let _ = self
                    .builder
                    .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                    let field_ptr = self.builder.build_struct_gep(
                        state_ty,
                        state_gc_ptr,
                        outer_field_base.saturating_add(field_idx as u32),
                        "mixed_escape_matrix_capture_outer_gep",
                    )?;
                    let local = self
                        .env
                        .get(cap.id)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "mixed escape capture local not found",
                            at: site.decl.span.into(),
                        })?;
                    if local.ty != cap.ty {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "mixed escape capture local type mismatch",
                            at: site.decl.span.into(),
                        });
                    }
                    self.write_escape_capture_local_to_state(span, field_ptr, local.ptr, cap.ty)?;
                }

                for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                    let field_ptr = self.builder.build_struct_gep(
                        state_ty,
                        state_gc_ptr,
                        body_field_base.saturating_add(field_idx as u32),
                        "mixed_escape_matrix_capture_body_gep",
                    )?;
                    let Some(local) = self.env.get(cap.id) else {
                        continue;
                    };
                    if local.ty != cap.ty {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "mixed escape capture local type mismatch",
                            at: site.decl.span.into(),
                        });
                    }
                    self.write_escape_capture_local_to_state(span, field_ptr, local.ptr, cap.ty)?;
                }

                match &site.kind {
                    MatrixEscapeSiteKind::Direct { site: direct_site } => {
                        for (slot, arg) in escape_binder_slots.iter().zip(direct_site.args.iter()) {
                            let hir::CallArg::Positional(expr) = arg else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape named perform arg",
                                    at: direct_site.decl.span.into(),
                                });
                            };
                            let v = self.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                            let _stored =
                                self.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                        }

                        let rt_cont_alloc = self.declare_runtime_continuation_alloc();
                        let step_ptr = step_fn.as_global_value().as_pointer_value();
                        let cont_call = self.builder.build_call(
                            rt_cont_alloc,
                            &[state_raw.into(), step_ptr.into()],
                            "mixed_escape_matrix_cont_alloc",
                        )?;
                        let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape continuation alloc return value",
                                at: direct_site.decl.span.into(),
                            },
                        )?;
                        let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape continuation alloc return type",
                                at: direct_site.decl.span.into(),
                            });
                        };
                        let _ = self.builder.build_call(
                            pin,
                            &[k_raw.into()],
                            "mixed_escape_matrix_k_pin",
                        )?;
                        let _stored = self.store_local_value(
                            span,
                            cont_ptr,
                            CgTy::Ref,
                            CgValue {
                                ty: CgTy::Ref,
                                value: Some(k_raw.into()),
                            },
                        )?;

                        let _ = self.builder.build_call(
                            rt_swap,
                            &[escape_outer_top.into()],
                            "mixed_escape_matrix_detach_for_direct",
                        )?;

                        self.pop_raise_target();
                        self.env.pop_scope();
                        self.builder.build_unconditional_branch(escape_arm_bb)?;
                        escaped = true;
                        break;
                    }
                    MatrixEscapeSiteKind::Indirect {
                        site: indirect_site,
                    } => {
                        self.pop_raise_target();
                        self.push_raise_target(escape_dispatch_bb);
                        self.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                            indirect_site,
                            &body_lift_ids,
                        )?;
                        self.pop_raise_target();
                        self.push_raise_target(main_raise_target);
                        if let Some(&next_pc) = block_next_site_pc_by_pc.get(&site_pc) {
                            let next_site = &escape_sites[next_pc];
                            self.codegen_mixed_escape_matrix_continue_to_next_block_site_after_indirect(
                                indirect_site,
                                next_pc,
                                next_site,
                                &body_lift_ids,
                                &mut |cg, next_pc, direct_site| {
                                    let pc_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_gc_ptr,
                                        2,
                                        "mixed_escape_matrix_pc_gep",
                                    )?;
                                    let _ = cg
                                        .builder
                                        .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                                    for (field_idx, cap) in
                                        outer_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_gc_ptr,
                                            outer_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_capture_outer_gep",
                                        )?;
                                        let local = cg.env.get(cap.id).ok_or(
                                            LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local not found",
                                                at: direct_site.decl.span.into(),
                                            },
                                        )?;
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: direct_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    for (field_idx, cap) in
                                        body_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_gc_ptr,
                                            body_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_capture_body_gep",
                                        )?;
                                        let Some(local) = cg.env.get(cap.id) else {
                                            continue;
                                        };
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: direct_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    for (slot, arg) in
                                        escape_binder_slots.iter().zip(direct_site.args.iter())
                                    {
                                        let hir::CallArg::Positional(expr) = arg else {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "handle mixed-arm escape named perform arg",
                                                at: direct_site.decl.span.into(),
                                            });
                                        };
                                        let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                        let _stored = cg.store_local_value(
                                            expr.span,
                                            slot.ptr,
                                            slot.ty,
                                            v,
                                        )?;
                                    }

                                    let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                                    let step_ptr = step_fn.as_global_value().as_pointer_value();
                                    let cont_call = cg.builder.build_call(
                                        rt_cont_alloc,
                                        &[state_raw.into(), step_ptr.into()],
                                        "mixed_escape_matrix_cont_alloc",
                                    )?;
                                    let cont_raw =
                                        cont_call.try_as_basic_value().basic().ok_or(
                                            LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape continuation alloc return value",
                                                at: direct_site.decl.span.into(),
                                            },
                                        )?;
                                    let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "mixed escape continuation alloc return type",
                                            at: direct_site.decl.span.into(),
                                        });
                                    };
                                    let _ = cg.builder.build_call(
                                        pin,
                                        &[k_raw.into()],
                                        "mixed_escape_matrix_k_pin",
                                    )?;
                                    let _stored = cg.store_local_value(
                                        span,
                                        cont_ptr,
                                        CgTy::Ref,
                                        CgValue {
                                            ty: CgTy::Ref,
                                            value: Some(k_raw.into()),
                                        },
                                    )?;

                                    let _ = cg.builder.build_call(
                                        rt_swap,
                                        &[escape_outer_top.into()],
                                        "mixed_escape_matrix_detach_for_direct",
                                    )?;
                                    cg.builder.build_unconditional_branch(escape_arm_bb)?;
                                    Ok(())
                                },
                                &mut |cg, next_pc, next_indirect_site| {
                                    let pc_ptr = cg.builder.build_struct_gep(
                                        state_ty,
                                        state_gc_ptr,
                                        2,
                                        "mixed_escape_matrix_pc_gep",
                                    )?;
                                    let _ = cg
                                        .builder
                                        .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                                    for (field_idx, cap) in
                                        outer_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_gc_ptr,
                                            outer_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_capture_outer_gep",
                                        )?;
                                        let local = cg.env.get(cap.id).ok_or(
                                            LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local not found",
                                                at: next_indirect_site.decl.span.into(),
                                            },
                                        )?;
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: next_indirect_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    for (field_idx, cap) in
                                        body_visible_supported.iter().enumerate()
                                    {
                                        let field_ptr = cg.builder.build_struct_gep(
                                            state_ty,
                                            state_gc_ptr,
                                            body_field_base.saturating_add(field_idx as u32),
                                            "mixed_escape_matrix_capture_body_gep",
                                        )?;
                                        let Some(local) = cg.env.get(cap.id) else {
                                            continue;
                                        };
                                        if local.ty != cap.ty {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "mixed escape capture local type mismatch",
                                                at: next_indirect_site.decl.span.into(),
                                            });
                                        }
                                        cg.write_escape_capture_local_to_state(
                                            span, field_ptr, local.ptr, cap.ty,
                                        )?;
                                    }

                                    cg.pop_raise_target();
                                    cg.push_raise_target(escape_dispatch_bb);
                                    cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                        next_indirect_site,
                                        &body_lift_ids,
                                    )?;
                                    cg.pop_raise_target();
                                    cg.push_raise_target(main_raise_target);
                                    Ok(())
                                },
                            )?;
                            if let MatrixEscapeSiteKind::Indirect {
                                site: next_indirect_site,
                            } = &next_site.kind
                                && let Some(bb) = self.builder.get_insert_block()
                                && bb.get_terminator().is_none()
                            {
                                self.codegen_mixed_escape_matrix_nested_block_continue_after_indirect_site(
                                    next_indirect_site,
                                    &body_lift_ids,
                                )?;
                            }
                            if let Some(bb) = self.builder.get_insert_block()
                                && bb.get_terminator().is_some()
                            {
                                escaped = true;
                                break;
                            }
                        } else {
                            self.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                indirect_site,
                                &body_lift_ids,
                            )?;
                        }
                        tail_value = CgValue::unit();
                        continue;
                    }
                }
            }

            let is_last = idx + 1 == handle.body.stmts.len();
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    self.codegen_val_decl(decl)?;
                    tail_value = CgValue::unit();
                }
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                    tail_value = CgValue::unit();
                }
                hir::StmtKind::Expr(expr) => {
                    let v = self.codegen_expr(expr)?;
                    tail_value = if is_last { v } else { CgValue::unit() };
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

        if !escaped {
            self.codegen_immediate_resume_finalize_body(
                handle.body.span,
                out_ty,
                tail_value,
                result_ptr,
                ImmediateResumeHandlerExit::None,
                finally_bb,
            )?;
        }
        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }
        if !escaped {
            self.env.pop_scope();
        }

        self.builder.position_at_end(escape_dispatch_bb);
        let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
        let tag_call = self.builder.build_call(
            rt_read_tag,
            &[],
            "mixed_escape_matrix_dispatch_read_op_tag",
        )?;
        let tag_raw =
            tag_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape matrix dispatch read_op_tag return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "mixed escape matrix dispatch read_op_tag return type",
                at: span.into(),
            });
        };
        let tag_matches = self.builder.build_int_compare(
            IntPredicate::EQ,
            slot_tag,
            escape_tag_i32,
            "mixed_escape_matrix_dispatch_tag_eq",
        )?;
        let escape_arm_slot_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_arm_from_slot");
        let escape_dispatch_fallback_bb = effect_dispatch_bb.unwrap_or(finally_unwind_bb);
        self.builder.build_conditional_branch(
            tag_matches,
            escape_arm_slot_bb,
            escape_dispatch_fallback_bb,
        )?;

        if let Some(effect_dispatch_bb) = effect_dispatch_bb {
            let effect_dispatch_nomatch_bb = effect_dispatch_nomatch_bb
                .expect("escape + sibling non-resuming matrix dispatch_nomatch bb should exist");
            let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
            self.builder.position_at_end(effect_dispatch_bb);
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = self.builder.build_call(
                rt_read_tag,
                &[],
                "mixed_escape_matrix_effect_dispatch_read_op_tag",
            )?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape matrix effect dispatch read_op_tag return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape matrix effect dispatch read_op_tag return type",
                    at: span.into(),
                });
            };
            let mut dispatch_cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
                Vec::new();
            if let Some(raise_catch_bb) = raise_catch_bb {
                let raise_tag = self.effect_op_tag("scoop.core.Raise.raise");
                dispatch_cases.push((i32_ty.const_int(raise_tag as u64, false), raise_catch_bb));
            }
            for (idx, custom) in custom_siblings.iter().enumerate() {
                dispatch_cases.push((
                    i32_ty.const_int(custom.op_tag as u64, false),
                    custom_catch_bbs[idx],
                ));
            }
            self.builder
                .build_switch(slot_tag, effect_dispatch_nomatch_bb, &dispatch_cases)?;

            self.builder.position_at_end(effect_dispatch_nomatch_bb);
            self.builder.build_unconditional_branch(finally_unwind_bb)?;

            if let (Some(raise_arm), Some(raise_catch_bb)) = (raise_sibling, raise_catch_bb) {
                let binder = &raise_arm.op.binders[0];
                self.builder.position_at_end(raise_catch_bb);
                let _ = self.builder.build_call(
                    rt_swap,
                    &[escape_outer_top.into()],
                    "mixed_escape_matrix_raise_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "mixed_escape_matrix_raise_read_slot_len_words",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return value",
                        at: span.into(),
                    },
                )?;
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
                    "mixed_escape_matrix_raise_slot_len_ok",
                )?;
                let len_ok_bb = self
                    .context
                    .append_basic_block(func, "mixed_escape_matrix_raise_slot_len_ok_bb");
                let len_bad_bb = self
                    .context
                    .append_basic_block(func, "mixed_escape_matrix_raise_slot_len_bad_bb");
                self.builder
                    .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                self.builder.position_at_end(len_bad_bb);
                self.emit_exit_with_code(span, 3)?;

                self.builder.position_at_end(len_ok_bb);

                let rt_read_at = self.declare_runtime_effect_perform_slot_read_u64_at();
                let idx0 = self.context.i32_type().const_int(0, false);
                let idx1 = self.context.i32_type().const_int(1, false);

                let kind_call = self.builder.build_call(
                    rt_read_at,
                    &[idx0.into()],
                    "mixed_escape_matrix_raise_read_slot_word0",
                )?;
                let kind_raw = kind_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word0 return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(kind_u64) = kind_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word0 return type",
                        at: span.into(),
                    });
                };

                let value_call = self.builder.build_call(
                    rt_read_at,
                    &[idx1.into()],
                    "mixed_escape_matrix_raise_read_slot_word1",
                )?;
                let value_raw = value_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word1 return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(value_u64) = value_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word1 return type",
                        at: span.into(),
                    });
                };

                let rt_clear = self.declare_runtime_effect_clear();
                let _ =
                    self.builder
                        .build_call(rt_clear, &[], "mixed_escape_matrix_raise_clear")?;

                self.env.push_scope();

                let binder_cg_ty =
                    self.cg_ty_of(binder.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle binder type",
                            at: binder.span.into(),
                        })?;
                let binder_value = match binder_cg_ty {
                    CgTy::Int(int_ty) => {
                        let expected = self.context.i64_type().const_int(1, false);
                        let ok = self.builder.build_int_compare(
                            IntPredicate::EQ,
                            kind_u64,
                            expected,
                            "mixed_escape_matrix_raise_kind_is_int",
                        )?;
                        let ok_bb = self
                            .context
                            .append_basic_block(func, "mixed_escape_matrix_raise_kind_int_ok");
                        let bad_bb = self
                            .context
                            .append_basic_block(func, "mixed_escape_matrix_raise_kind_int_bad");
                        self.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                        self.builder.position_at_end(bad_bb);
                        self.emit_exit_with_code(span, 3)?;

                        self.builder.position_at_end(ok_bb);
                        let from_u64 = IntTy {
                            bits: 64,
                            signed: false,
                        };
                        let decoded = self.cast_int(value_u64, from_u64, int_ty)?;
                        CgValue::int(decoded, int_ty)
                    }
                    CgTy::Enum(enum_ty) if self.is_sysroot_runtime_error_enum(enum_ty) => {
                        let expected = self.context.i64_type().const_int(2, false);
                        let ok = self.builder.build_int_compare(
                            IntPredicate::EQ,
                            kind_u64,
                            expected,
                            "mixed_escape_matrix_raise_kind_is_runtime_error",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "mixed_escape_matrix_raise_kind_runtime_error_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "mixed_escape_matrix_raise_kind_runtime_error_bad",
                        );
                        self.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                        self.builder.position_at_end(bad_bb);
                        self.emit_exit_with_code(span, 3)?;

                        self.builder.position_at_end(ok_bb);

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
                            "mixed_escape_matrix_runtime_error_tag_i32",
                        )?;
                        let payload_word_zero =
                            self.int_type(self.enum_payload_ty()).const_int(0, false);
                        let payload_ptr_zero = self.llvm_gc_i8_ptr_type().const_null();

                        let llvm_enum_ty = self.llvm_enum_value_type(span, enum_ty)?;
                        let llvm_enum_ty = llvm_enum_ty.into_struct_type();
                        let mut agg: AggregateValueEnum<'ctx> = llvm_enum_ty.get_undef().into();
                        agg = self.builder.build_insert_value(
                            agg,
                            tag_i32,
                            0,
                            "mixed_escape_matrix_runtime_error_tag",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_word_zero,
                            1,
                            "mixed_escape_matrix_runtime_error_payload_word",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_ptr_zero,
                            2,
                            "mixed_escape_matrix_runtime_error_payload_ptr",
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
                let binder_ptr =
                    self.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
                let _ =
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

                for custom in &custom_siblings {
                    self.push_effect_unwind_target(&custom.arm.op.op.fqn, finally_unwind_bb);
                }
                self.push_raise_target(finally_unwind_bb);
                let arm_v = self.codegen_expr_in_expected_context(&raise_arm.body, Some(out_ty))?;
                self.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    self.pop_effect_unwind_target();
                }
                let arm_v = if out_ty == CgTy::Unit {
                    CgValue::unit()
                } else {
                    self.coerce_value(raise_arm.body.span, arm_v, out_ty)?
                };

                let catch_end =
                    self.builder
                        .get_insert_block()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "builder has no insert block",
                            at: span.into(),
                        })?;
                if catch_end.get_terminator().is_none() {
                    if let Some(ptr) = result_ptr {
                        let _ = self.store_local_value(raise_arm.body.span, ptr, out_ty, arm_v)?;
                    }
                    self.builder.build_unconditional_branch(finally_bb)?;
                }
                self.env.pop_scope();
            }

            for (idx, custom) in custom_siblings.iter().enumerate() {
                let arm = custom.arm;
                let binder = &arm.op.binders[0];
                self.builder.position_at_end(custom_catch_bbs[idx]);
                let _ = self.builder.build_call(
                    rt_swap,
                    &[escape_outer_top.into()],
                    "mixed_escape_matrix_custom_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "mixed_escape_matrix_custom_read_slot_len_words",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return value",
                        at: span.into(),
                    },
                )?;
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
                    "mixed_escape_matrix_custom_slot_len_ok",
                )?;
                let len_ok_bb = self
                    .context
                    .append_basic_block(func, "mixed_escape_matrix_custom_slot_len_ok_bb");
                let len_bad_bb = self
                    .context
                    .append_basic_block(func, "mixed_escape_matrix_custom_slot_len_bad_bb");
                self.builder
                    .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                self.builder.position_at_end(len_bad_bb);
                self.emit_exit_with_code(span, 3)?;

                self.builder.position_at_end(len_ok_bb);

                let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
                let value_call = self.builder.build_call(
                    rt_read,
                    &[],
                    "mixed_escape_matrix_custom_read_slot_word0",
                )?;
                let value_raw = value_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word0 return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(value_u64) = value_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word0 return type",
                        at: span.into(),
                    });
                };
                let rt_read_gc = self.declare_runtime_effect_perform_slot_read_gc_ref();
                let gc_call = self.builder.build_call(
                    rt_read_gc,
                    &[],
                    "mixed_escape_matrix_custom_read_slot_gc_ref",
                )?;
                let gc_raw = gc_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_gc_ref return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(gc_ref_raw) = gc_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_gc_ref return type",
                        at: span.into(),
                    });
                };

                self.env.push_scope();

                let binder_cg_ty =
                    self.cg_ty_of(binder.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle binder type (custom non-resuming)",
                            at: binder.span.into(),
                        })?;
                let binder_value = self.decode_abi_payload_transport(
                    binder.span,
                    value_u64,
                    gc_ref_raw,
                    binder_cg_ty,
                )?;
                let binder_ptr =
                    self.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
                let _ =
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

                let rt_clear = self.declare_runtime_effect_clear();
                let _ =
                    self.builder
                        .build_call(rt_clear, &[], "mixed_escape_matrix_custom_clear")?;

                for custom in &custom_siblings {
                    self.push_effect_unwind_target(&custom.arm.op.op.fqn, finally_unwind_bb);
                }
                self.push_raise_target(finally_unwind_bb);
                let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
                self.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    self.pop_effect_unwind_target();
                }
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
                if catch_end.get_terminator().is_none() {
                    if let Some(ptr) = result_ptr {
                        let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
                    }
                    self.builder.build_unconditional_branch(finally_bb)?;
                }
                self.env.pop_scope();
            }
        }

        self.builder.position_at_end(escape_arm_slot_bb);
        let _ = self.builder.build_store(
            escape_binder_from_slot_ptr,
            self.context.bool_type().const_all_ones(),
        )?;
        self.builder.build_unconditional_branch(escape_arm_bb)?;

        self.builder.position_at_end(escape_arm_bb);
        let binder_from_slot = self
            .builder
            .build_load(
                self.context.bool_type(),
                escape_binder_from_slot_ptr,
                "mixed_escape_matrix_binder_from_slot",
            )?
            .into_int_value();
        let binder_read_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_arm_read_binder");
        let binder_skip_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_arm_skip_binder");
        let binder_merge_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_arm_after_binder");
        self.builder
            .build_conditional_branch(binder_from_slot, binder_read_bb, binder_skip_bb)?;

        self.builder.position_at_end(binder_read_bb);
        if escape_binder_slots.len() > 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape binder count (indirect, only 1 supported)",
                at: escape_arm.op.span.into(),
            });
        }
        if let Some(slot) = escape_binder_slots.first() {
            let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
            let word_call = self.builder.build_call(
                rt_read,
                &[],
                "mixed_escape_matrix_arm_read_binder_word",
            )?;
            let word_raw = word_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape matrix arm read binder return",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(word_u64) = word_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape matrix arm read binder type",
                    at: span.into(),
                });
            };
            let rt_read_gc = self.declare_runtime_effect_perform_slot_read_gc_ref();
            let gc_call = self.builder.build_call(
                rt_read_gc,
                &[],
                "mixed_escape_matrix_arm_read_binder_gc",
            )?;
            let gc_raw =
                gc_call
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "mixed escape matrix arm read binder gc value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::PointerValue(gc_ref_raw) = gc_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape matrix arm read binder gc type",
                    at: span.into(),
                });
            };
            let binder_value =
                self.decode_abi_payload_transport(span, word_u64, gc_ref_raw, slot.ty)?;
            let _ = self.store_local_value(span, slot.ptr, slot.ty, binder_value)?;
        }
        self.builder.build_unconditional_branch(binder_merge_bb)?;
        self.builder.position_at_end(binder_skip_bb);
        self.builder.build_unconditional_branch(binder_merge_bb)?;
        self.builder.position_at_end(binder_merge_bb);

        let rt_clear = self.declare_runtime_effect_clear();
        let _ = self
            .builder
            .build_call(rt_clear, &[], "mixed_escape_matrix_arm_effect_clear")?;

        let rt_cont_alloc = self.declare_runtime_continuation_alloc();
        let step_ptr = step_fn.as_global_value().as_pointer_value();
        let cont_call = self.builder.build_call(
            rt_cont_alloc,
            &[state_raw.into(), step_ptr.into()],
            "mixed_escape_matrix_arm_cont_alloc",
        )?;
        let cont_raw =
            cont_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape continuation alloc return value",
                    at: escape_arm.span.into(),
                })?;
        let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "mixed escape continuation alloc return type",
                at: escape_arm.span.into(),
            });
        };
        let _ = self
            .builder
            .build_call(pin, &[k_raw.into()], "mixed_escape_matrix_arm_k_pin")?;
        let _stored = self.store_local_value(
            span,
            cont_ptr,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(k_raw.into()),
            },
        )?;

        let _ = self.builder.build_call(
            rt_swap,
            &[escape_outer_top.into()],
            "mixed_escape_matrix_detach_for_indirect",
        )?;

        self.env.push_scope();
        for slot in &escape_binder_slots {
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

        for custom in &custom_siblings {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, finally_unwind_bb);
        }
        self.push_raise_target(finally_unwind_bb);
        let arm_v = self.codegen_expr_in_expected_context(&escape_arm.body, Some(out_ty))?;
        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }
        let arm_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(escape_arm.body.span, arm_v, out_ty)?
        };

        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(escape_arm.body.span, ptr, out_ty, arm_v)?;
            }
            self.builder.build_unconditional_branch(finally_bb)?;
        }
        self.env.pop_scope();

        self.builder.position_at_end(finally_unwind_bb);
        let _ = self.builder.build_call(
            rt_swap,
            &[escape_outer_top.into()],
            "mixed_escape_matrix_finally_unwind_detach",
        )?;
        let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
        let k_loaded = self
            .builder
            .build_load(
                llvm_ref_ty,
                cont_ptr,
                "mixed_escape_matrix_k_maybe_unpin_load",
            )?
            .into_pointer_value();
        let k_is_null = self
            .builder
            .build_is_null(k_loaded, "mixed_escape_matrix_k_is_null")?;
        let finally_unwind_state_unpin_bb = self.context.append_basic_block(
            func,
            "handle_mixed_escape_matrix_finally_unwind_state_unpin",
        );
        let finally_unwind_state_keep_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_finally_unwind_state_keep");
        let finally_unwind_state_merge_bb = self.context.append_basic_block(
            func,
            "handle_mixed_escape_matrix_finally_unwind_state_merge",
        );
        self.builder.build_conditional_branch(
            k_is_null,
            finally_unwind_state_unpin_bb,
            finally_unwind_state_keep_bb,
        )?;
        self.builder.position_at_end(finally_unwind_state_unpin_bb);
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self.builder.build_call(
            unpin,
            &[state_raw.into()],
            "mixed_escape_matrix_state_unpin_finally_unwind",
        )?;
        self.builder
            .build_unconditional_branch(finally_unwind_state_merge_bb)?;
        self.builder.position_at_end(finally_unwind_state_keep_bb);
        self.builder
            .build_unconditional_branch(finally_unwind_state_merge_bb)?;
        self.builder.position_at_end(finally_unwind_state_merge_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            if let Some(target) = outer_raise_target {
                self.builder.build_unconditional_branch(target)?;
            } else {
                let ret_ty = self.current_fun_return_ty.ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape finally unwind needs function return type",
                        at: span.into(),
                    },
                )?;
                let v = self.default_value(span, ret_ty)?;
                self.emit_return(span, ret_ty, v)?;
            }
        }

        self.builder.position_at_end(finally_bb);
        let _ = self.builder.build_call(
            rt_swap,
            &[escape_outer_top.into()],
            "mixed_escape_matrix_finally_detach",
        )?;
        let k_loaded = self
            .builder
            .build_load(
                llvm_ref_ty,
                cont_ptr,
                "mixed_escape_matrix_k_maybe_unpin_done_load",
            )?
            .into_pointer_value();
        let k_is_null = self
            .builder
            .build_is_null(k_loaded, "mixed_escape_matrix_k_done_is_null")?;
        let finally_state_unpin_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_finally_state_unpin");
        let finally_state_keep_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_finally_state_keep");
        let finally_state_merge_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_finally_state_merge");
        self.builder.build_conditional_branch(
            k_is_null,
            finally_state_unpin_bb,
            finally_state_keep_bb,
        )?;
        self.builder.position_at_end(finally_state_unpin_bb);
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self.builder.build_call(
            unpin,
            &[state_raw.into()],
            "mixed_escape_matrix_state_unpin_finally",
        )?;
        self.builder
            .build_unconditional_branch(finally_state_merge_bb)?;
        self.builder.position_at_end(finally_state_keep_bb);
        self.builder
            .build_unconditional_branch(finally_state_merge_bb)?;
        self.builder.position_at_end(finally_state_merge_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.builder.build_unconditional_branch(done_bb)?;
        }

        self.builder.position_at_end(done_bb);
        let k_loaded = self
            .builder
            .build_load(llvm_ref_ty, cont_ptr, "mixed_escape_matrix_k_unpin_load")?
            .into_pointer_value();
        let k_is_null = self
            .builder
            .build_is_null(k_loaded, "mixed_escape_matrix_k_unpin_is_null")?;
        let k_unpin_skip_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_k_unpin_skip");
        let k_unpin_do_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_k_unpin_do");
        let k_unpin_merge_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_matrix_k_unpin_merge");
        self.builder
            .build_conditional_branch(k_is_null, k_unpin_skip_bb, k_unpin_do_bb)?;
        self.builder.position_at_end(k_unpin_do_bb);
        let unpin = self.declare_runtime_gc_unpin();
        let _ =
            self.builder
                .build_call(unpin, &[k_loaded.into()], "mixed_escape_matrix_k_unpin")?;
        self.builder.build_unconditional_branch(k_unpin_merge_bb)?;
        self.builder.position_at_end(k_unpin_skip_bb);
        self.builder.build_unconditional_branch(k_unpin_merge_bb)?;
        self.builder.position_at_end(k_unpin_merge_bb);

        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
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
                        kind: "handle result slot",
                        at: span.into(),
                    });
                };
                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded =
                    self.builder
                        .build_load(llvm_ty, ptr, "handle_mixed_escape_matrix_result")?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }

}
