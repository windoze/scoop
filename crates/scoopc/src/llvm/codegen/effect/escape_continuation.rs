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
            ResumeFrame::WhenArm {
                when_expr: a_e,
                arm_index: a_i,
                ..
            },
            ResumeFrame::WhenArm {
                when_expr: b_e,
                arm_index: b_i,
                ..
            },
        ) => std::ptr::eq(*a_e, *b_e) && a_i == b_i,
        (
            ResumeFrame::WhileBody { while_body: a_b, .. },
            ResumeFrame::WhileBody { while_body: b_b, .. },
        ) => std::ptr::eq(*a_b, *b_b),
        (ResumeFrame::Block { block: a_b, .. }, ResumeFrame::Block { block: b_b, .. }) => {
            std::ptr::eq(*a_b, *b_b)
        }
        _ => false,
    }
}

/// Declaration info for lift analysis: tracks all val decls in handle body (at any nesting depth).
#[derive(Debug)]
struct DeclInfo<'hir> {
    decl: &'hir hir::ValDecl,
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

#[derive(Debug, Clone, Copy)]
pub(super) struct EscapeContinuationHandleLowering<'hir> {
    handle: &'hir hir::HandleExpr,
    state_machine_plan: &'hir HandleStateMachinePlan,
    arm_id: ArmPlanId,
    arm: &'hir hir::HandleArm,
    continuation_symbol: hir::SymbolId,
    seq: u32,
    out_ty: CgTy,
}

#[derive(Debug)]
struct ResolvedEscapeDirectSites<'hir> {
    perform_sites: Vec<NestedPerformSite<'hir>>,
    decl_map: HashMap<hir::SymbolId, DeclInfo<'hir>>,
    capture_ids: HashSet<hir::SymbolId>,
}

#[derive(Debug)]
struct ResolvedEscapeIndirectSites {
    indirect_sites: Vec<IndirectPerformCallSite>,
    capture_ids: HashSet<hir::SymbolId>,
}

#[derive(Debug)]
struct ResolvedPlanMixedEscapeDirectSite<'hir> {
    arm_id: ArmPlanId,
    site: MixedEscapeDirectSite<'hir>,
}

#[derive(Debug)]
struct ResolvedPlanMixedEscapeDirectSites<'hir> {
    direct_sites: Vec<ResolvedPlanMixedEscapeDirectSite<'hir>>,
    capture_ids: HashSet<hir::SymbolId>,
}

#[derive(Debug)]
struct ResolvedPlanMixedEscapeIndirectSites<'hir> {
    indirect_sites: Vec<MixedEscapeIndirectSite<'hir>>,
    capture_ids: HashSet<hir::SymbolId>,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn record_escape_decl_info_in_stmts<'hir>(
        stmts: &'hir [hir::Stmt],
        decl_map: &mut HashMap<hir::SymbolId, DeclInfo<'hir>>,
    ) {
        for stmt in stmts {
            match &stmt.kind {
                hir::StmtKind::Val(decl) => {
                    if let Some(id) = decl.id {
                        decl_map.entry(id).or_insert(DeclInfo { decl });
                    }
                }
                hir::StmtKind::Expr(expr) => Self::record_escape_decl_info_in_expr(expr, decl_map),
                hir::StmtKind::While { body, .. } => {
                    Self::record_escape_decl_info_in_stmts(&body.stmts, decl_map);
                }
                hir::StmtKind::Assign { .. }
                | hir::StmtKind::Return { .. }
                | hir::StmtKind::Empty
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {}
            }
        }
    }

    fn record_escape_decl_info_in_expr<'hir>(
        expr: &'hir hir::Expr,
        decl_map: &mut HashMap<hir::SymbolId, DeclInfo<'hir>>,
    ) {
        match &expr.kind {
            hir::ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                if let hir::ExprKind::Block(block) = &then_branch.kind {
                    Self::record_escape_decl_info_in_stmts(&block.stmts, decl_map);
                }
                if let Some(else_expr) = else_branch.as_deref()
                    && let hir::ExprKind::Block(block) = &else_expr.kind
                {
                    Self::record_escape_decl_info_in_stmts(&block.stmts, decl_map);
                }
            }
            hir::ExprKind::When { arms, .. } => {
                for when_arm in arms {
                    if let hir::ExprKind::Block(block) = &when_arm.body.kind {
                        Self::record_escape_decl_info_in_stmts(&block.stmts, decl_map);
                    }
                }
            }
            hir::ExprKind::Block(block) => {
                Self::record_escape_decl_info_in_stmts(&block.stmts, decl_map);
            }
            _ => {}
        }
    }

    fn collect_escape_decl_map<'hir>(
        handle: &'hir hir::HandleExpr,
    ) -> HashMap<hir::SymbolId, DeclInfo<'hir>> {
        let mut decl_map = HashMap::new();
        Self::record_escape_decl_info_in_stmts(&handle.body.stmts, &mut decl_map);
        decl_map
    }

    fn resolve_escape_stmt_from_source_path<'hir>(
        handle: &'hir hir::HandleExpr,
        source_path: &SuspendSourcePath,
        site_span: crate::span::Span,
    ) -> Result<(&'hir hir::Stmt, Vec<ResumeFrame<'hir>>), LlvmEmitError> {
        let mut current_stmt = handle.body.stmts.get(source_path.top_level_stmt_idx).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "handle escape body (invalid top-level suspend path)",
                at: site_span.into(),
            },
        )?;
        let mut resume_path = Vec::with_capacity(source_path.frames.len());

        for frame in &source_path.frames {
            match frame {
                SuspendSourceFramePath::Block { block_span, stmt_idx } => {
                    let hir::StmtKind::Expr(expr) = &current_stmt.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (expected block expression statement)",
                            at: current_stmt.span.into(),
                        });
                    };
                    let hir::ExprKind::Block(block) = &expr.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (expected block expression)",
                            at: expr.span.into(),
                        });
                    };
                    if block.span != *block_span {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (block path mismatch)",
                            at: block.span.into(),
                        });
                    }
                    current_stmt = block.stmts.get(*stmt_idx).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (invalid block stmt path)",
                            at: block.span.into(),
                        },
                    )?;
                    resume_path.push(ResumeFrame::Block {
                        block,
                        resume_after_stmt: *stmt_idx,
                    });
                }
                SuspendSourceFramePath::WhenArm {
                    when_span,
                    arm_index,
                    arm_span,
                    stmt_idx,
                } => {
                    let hir::StmtKind::Expr(expr) = &current_stmt.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (expected when expression statement)",
                            at: current_stmt.span.into(),
                        });
                    };
                    if expr.span != *when_span {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (when path mismatch)",
                            at: expr.span.into(),
                        });
                    }
                    let hir::ExprKind::When { arms, .. } = &expr.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (expected when expression)",
                            at: expr.span.into(),
                        });
                    };
                    let when_arm = arms.get(*arm_index).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (invalid when arm path)",
                            at: expr.span.into(),
                        },
                    )?;
                    let hir::ExprKind::Block(block) = &when_arm.body.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (expected when arm block)",
                            at: when_arm.body.span.into(),
                        });
                    };
                    if block.span != *arm_span {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (when arm path mismatch)",
                            at: block.span.into(),
                        });
                    }
                    current_stmt = block.stmts.get(*stmt_idx).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (invalid when arm stmt path)",
                            at: block.span.into(),
                        },
                    )?;
                    resume_path.push(ResumeFrame::WhenArm {
                        when_expr: expr,
                        arm_index: *arm_index,
                        arm_block_stmts: &block.stmts,
                        resume_after_stmt: *stmt_idx,
                    });
                }
                SuspendSourceFramePath::IfThen {
                    if_span,
                    then_span,
                    stmt_idx,
                } => {
                    let hir::StmtKind::Expr(expr) = &current_stmt.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (expected if expression statement)",
                            at: current_stmt.span.into(),
                        });
                    };
                    if expr.span != *if_span {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (if path mismatch)",
                            at: expr.span.into(),
                        });
                    }
                    let hir::ExprKind::If { then_branch, .. } = &expr.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (expected if expression)",
                            at: expr.span.into(),
                        });
                    };
                    let hir::ExprKind::Block(block) = &then_branch.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (expected if-then block)",
                            at: then_branch.span.into(),
                        });
                    };
                    if block.span != *then_span {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (if-then path mismatch)",
                            at: block.span.into(),
                        });
                    }
                    current_stmt = block.stmts.get(*stmt_idx).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (invalid if-then stmt path)",
                            at: block.span.into(),
                        },
                    )?;
                    resume_path.push(ResumeFrame::IfThen {
                        if_expr: expr,
                        then_block_stmts: &block.stmts,
                        resume_after_stmt: *stmt_idx,
                    });
                }
                SuspendSourceFramePath::IfElse {
                    if_span,
                    else_span,
                    stmt_idx,
                } => {
                    let hir::StmtKind::Expr(expr) = &current_stmt.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (expected if expression statement)",
                            at: current_stmt.span.into(),
                        });
                    };
                    if expr.span != *if_span {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (if path mismatch)",
                            at: expr.span.into(),
                        });
                    }
                    let hir::ExprKind::If { else_branch, .. } = &expr.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (expected if expression)",
                            at: expr.span.into(),
                        });
                    };
                    let Some(else_expr) = else_branch.as_deref() else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (missing if-else branch)",
                            at: expr.span.into(),
                        });
                    };
                    let hir::ExprKind::Block(block) = &else_expr.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (expected if-else block)",
                            at: else_expr.span.into(),
                        });
                    };
                    if block.span != *else_span {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (if-else path mismatch)",
                            at: block.span.into(),
                        });
                    }
                    current_stmt = block.stmts.get(*stmt_idx).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (invalid if-else stmt path)",
                            at: block.span.into(),
                        },
                    )?;
                    resume_path.push(ResumeFrame::IfElse {
                        if_expr: expr,
                        else_block_stmts: &block.stmts,
                        resume_after_stmt: *stmt_idx,
                    });
                }
                SuspendSourceFramePath::WhileBody {
                    while_cond_span,
                    while_body_span,
                    stmt_idx,
                } => {
                    let hir::StmtKind::While { cond, body } = &current_stmt.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (expected while statement)",
                            at: current_stmt.span.into(),
                        });
                    };
                    if cond.span != *while_cond_span || body.span != *while_body_span {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (while path mismatch)",
                            at: current_stmt.span.into(),
                        });
                    }
                    current_stmt = body.stmts.get(*stmt_idx).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (invalid while stmt path)",
                            at: body.span.into(),
                        },
                    )?;
                    resume_path.push(ResumeFrame::WhileBody {
                        while_cond: cond,
                        while_body: body,
                        resume_after_stmt: *stmt_idx,
                    });
                }
            }
        }

        Ok((current_stmt, resume_path))
    }

    fn mixed_escape_resume_path_from_frames<'hir>(
        frames: &[ResumeFrame<'hir>],
        site_span: crate::span::Span,
    ) -> Result<Vec<MixedEscapeDirectFrame<'hir>>, LlvmEmitError> {
        let mut resume_path = Vec::with_capacity(frames.len());
        for frame in frames {
            let mixed = match frame {
                ResumeFrame::Block {
                    block,
                    resume_after_stmt,
                } => MixedEscapeDirectFrame::Block {
                    block,
                    stmt_idx: *resume_after_stmt,
                },
                ResumeFrame::IfThen {
                    if_expr,
                    resume_after_stmt,
                    ..
                } => {
                    let hir::ExprKind::If { then_branch, .. } = &if_expr.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (invalid unified source path)",
                            at: site_span.into(),
                        });
                    };
                    let hir::ExprKind::Block(then_block) = &then_branch.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (invalid unified source path)",
                            at: site_span.into(),
                        });
                    };
                    MixedEscapeDirectFrame::IfThen {
                        if_expr,
                        then_block,
                        stmt_idx: *resume_after_stmt,
                    }
                }
                ResumeFrame::IfElse {
                    if_expr,
                    resume_after_stmt,
                    ..
                } => {
                    let hir::ExprKind::If { else_branch, .. } = &if_expr.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (invalid unified source path)",
                            at: site_span.into(),
                        });
                    };
                    let Some(else_expr) = else_branch.as_deref() else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (invalid unified source path)",
                            at: site_span.into(),
                        });
                    };
                    let hir::ExprKind::Block(else_block) = &else_expr.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (invalid unified source path)",
                            at: site_span.into(),
                        });
                    };
                    MixedEscapeDirectFrame::IfElse {
                        if_expr,
                        else_block,
                        stmt_idx: *resume_after_stmt,
                    }
                }
                ResumeFrame::WhileBody {
                    while_cond,
                    while_body,
                    resume_after_stmt,
                } => MixedEscapeDirectFrame::WhileBody {
                    while_cond,
                    while_body,
                    stmt_idx: *resume_after_stmt,
                },
                ResumeFrame::WhenArm { when_expr, .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (when arm sites not yet supported)",
                        at: when_expr.span.into(),
                    });
                }
            };
            resume_path.push(mixed);
        }
        Ok(resume_path)
    }

    fn resolve_mixed_escape_direct_sites_from_plan<'hir>(
        handle: &'hir hir::HandleExpr,
        state_machine_plan: &HandleStateMachinePlan,
        escape_arms: &[(&'hir hir::HandleArm, ArmPlanId)],
    ) -> Result<ResolvedPlanMixedEscapeDirectSites<'hir>, LlvmEmitError> {
        let mut capture_ids = HashSet::new();
        let mut matching_sites = state_machine_plan
            .suspend_sites
            .iter()
            .filter_map(|site| {
                let SuspendSiteKind::DirectPerform { op_fqn } = &site.kind else {
                    return None;
                };
                escape_arms
                    .iter()
                    .find(|(arm, arm_id)| {
                        arm.op.op.fqn == *op_fqn && site.matching_arms.contains(arm_id)
                    })
                    .map(|(_, arm_id)| (site, *arm_id))
            })
            .collect::<Vec<_>>();
        matching_sites.sort_by_key(|(site, _)| site.id);

        let mut direct_sites = Vec::with_capacity(matching_sites.len());
        for (site, arm_id) in matching_sites {
            capture_ids.extend(site.capture_locals.iter().copied());

            let Some(source_path) = &site.source_path else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (missing unified source path)",
                    at: site.span.into(),
                });
            };
            let (stmt, resume_frames) =
                Self::resolve_escape_stmt_from_source_path(handle, source_path, site.span)?;
            let resume_path =
                Self::mixed_escape_resume_path_from_frames(resume_frames.as_slice(), site.span)?;
            let hir::StmtKind::Val(decl) = &stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (perform must be bound to val)",
                    at: stmt.span.into(),
                });
            };
            let Some(init) = decl.init.as_ref() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (missing perform init)",
                    at: decl.span.into(),
                });
            };
            let hir::ExprKind::Perform { args, .. } = &init.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (expected direct perform)",
                    at: init.span.into(),
                });
            };
            if init.span != site.span {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (unified plan/source mismatch)",
                    at: init.span.into(),
                });
            }
            let Some(id) = decl.id else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation perform binding id",
                    at: decl.span.into(),
                });
            };

            direct_sites.push(ResolvedPlanMixedEscapeDirectSite {
                arm_id,
                site: MixedEscapeDirectSite {
                    top_level_stmt_idx: source_path.top_level_stmt_idx,
                    decl,
                    args: args.as_slice(),
                    id,
                    resume_path,
                },
            });
        }

        Ok(ResolvedPlanMixedEscapeDirectSites {
            direct_sites,
            capture_ids,
        })
    }

    fn resolve_mixed_escape_indirect_sites_from_plan<'hir>(
        handle: &'hir hir::HandleExpr,
        state_machine_plan: &HandleStateMachinePlan,
    ) -> Result<ResolvedPlanMixedEscapeIndirectSites<'hir>, LlvmEmitError> {
        let mut capture_ids = HashSet::new();
        let mut matching_sites = state_machine_plan
            .suspend_sites
            .iter()
            .filter(|site| {
                matches!(
                    site.kind,
                    SuspendSiteKind::IndirectCallMaySuspend { .. }
                        | SuspendSiteKind::CallStateMachineCallee { .. }
                )
            })
            .collect::<Vec<_>>();
        matching_sites.sort_by_key(|site| site.id);

        let mut indirect_sites = Vec::with_capacity(matching_sites.len());
        for site in matching_sites {
            capture_ids.extend(site.capture_locals.iter().copied());

            let Some(source_path) = &site.source_path else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (missing unified source path)",
                    at: site.span.into(),
                });
            };
            let (stmt, resume_frames) =
                Self::resolve_escape_stmt_from_source_path(handle, source_path, site.span)?;
            let resume_path =
                Self::mixed_escape_resume_path_from_frames(resume_frames.as_slice(), site.span)?;
            let hir::StmtKind::Val(decl) = &stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (indirect site must be val-bound)",
                    at: stmt.span.into(),
                });
            };
            let Some(init) = decl.init.as_ref() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (missing call init)",
                    at: decl.span.into(),
                });
            };
            if !matches!(init.kind, hir::ExprKind::Call { .. }) || init.span != site.span {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (unified plan/source mismatch)",
                    at: init.span.into(),
                });
            }
            let Some(id) = decl.id else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation perform binding id",
                    at: decl.span.into(),
                });
            };

            indirect_sites.push(MixedEscapeIndirectSite {
                top_level_stmt_idx: source_path.top_level_stmt_idx,
                decl,
                init,
                id,
                resume_path,
            });
        }

        Ok(ResolvedPlanMixedEscapeIndirectSites {
            indirect_sites,
            capture_ids,
        })
    }

    fn resolve_escape_direct_sites_from_plan<'hir>(
        handle: &'hir hir::HandleExpr,
        state_machine_plan: &HandleStateMachinePlan,
        arm_id: ArmPlanId,
        arm_op_fqn: &str,
    ) -> Result<ResolvedEscapeDirectSites<'hir>, LlvmEmitError> {
        let decl_map = Self::collect_escape_decl_map(handle);
        let mut capture_ids = HashSet::new();
        let mut matching_sites = state_machine_plan
            .suspend_sites
            .iter()
            .filter(|site| {
                matches!(
                    &site.kind,
                    SuspendSiteKind::DirectPerform { op_fqn } if op_fqn == arm_op_fqn
                ) && site.matching_arms.contains(&arm_id)
            })
            .collect::<Vec<_>>();
        matching_sites.sort_by_key(|site| site.id);

        let mut perform_sites = Vec::with_capacity(matching_sites.len());
        for (pc, site) in matching_sites.into_iter().enumerate() {
            capture_ids.extend(site.capture_locals.iter().copied());

            let Some(source_path) = &site.source_path else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape body (missing unified source path)",
                    at: site.span.into(),
                });
            };
            let (stmt, resume_path) =
                Self::resolve_escape_stmt_from_source_path(handle, source_path, site.span)?;
            let hir::StmtKind::Val(decl) = &stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape body (perform must be bound to val)",
                    at: stmt.span.into(),
                });
            };
            let Some(init) = decl.init.as_ref() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape body (missing perform init)",
                    at: decl.span.into(),
                });
            };
            let hir::ExprKind::Perform { op, args } = &init.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape body (expected direct perform)",
                    at: init.span.into(),
                });
            };
            if init.span != site.span || op.fqn != arm_op_fqn {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape body (unified plan/source mismatch)",
                    at: init.span.into(),
                });
            }
            let Some(id) = decl.id else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape perform binding id",
                    at: decl.span.into(),
                });
            };

            perform_sites.push(NestedPerformSite {
                pc,
                decl,
                op,
                args: args.as_slice(),
                id,
                resume_path,
                top_level_stmt_idx: source_path.top_level_stmt_idx,
            });
        }

        Ok(ResolvedEscapeDirectSites {
            perform_sites,
            decl_map,
            capture_ids,
        })
    }

    fn resolve_escape_indirect_sites_from_plan(
        handle: &hir::HandleExpr,
        state_machine_plan: &HandleStateMachinePlan,
    ) -> Result<ResolvedEscapeIndirectSites, LlvmEmitError> {
        let mut capture_ids = HashSet::new();
        let mut matching_sites = state_machine_plan
            .suspend_sites
            .iter()
            .filter(|site| {
                matches!(
                    site.kind,
                    SuspendSiteKind::IndirectCallMaySuspend { .. }
                        | SuspendSiteKind::CallStateMachineCallee { .. }
                )
            })
            .collect::<Vec<_>>();
        matching_sites.sort_by_key(|site| site.id);

        let mut indirect_sites = Vec::with_capacity(matching_sites.len());
        for site in matching_sites {
            capture_ids.extend(site.capture_locals.iter().copied());

            let Some(source_path) = &site.source_path else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape continuation indirect site (missing unified source path)",
                    at: site.span.into(),
                });
            };
            if !source_path.frames.is_empty() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape continuation indirect site (only top-level call sites supported)",
                    at: site.span.into(),
                });
            }
            let (stmt, _) =
                Self::resolve_escape_stmt_from_source_path(handle, source_path, site.span)?;
            let hir::StmtKind::Val(decl) = &stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape continuation indirect site (call must be bound to val)",
                    at: stmt.span.into(),
                });
            };
            let Some(init) = decl.init.as_ref() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape continuation indirect site (missing call init)",
                    at: decl.span.into(),
                });
            };
            if !matches!(init.kind, hir::ExprKind::Call { .. }) || init.span != site.span {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape continuation indirect site (unified plan/source mismatch)",
                    at: init.span.into(),
                });
            }
            let Some(id) = decl.id else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape continuation indirect site (call result id)",
                    at: decl.span.into(),
                });
            };

            indirect_sites.push(IndirectPerformCallSite {
                stmt_idx: source_path.top_level_stmt_idx,
                _result_id: id,
                result_ty: decl.ty,
            });
        }

        Ok(ResolvedEscapeIndirectSites {
            indirect_sites,
            capture_ids,
        })
    }

    pub(super) fn codegen_handle_expr_escape_continuation(
        &mut self,
        span: crate::span::Span,
        lowering: EscapeContinuationHandleLowering<'_>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let EscapeContinuationHandleLowering {
            handle,
            state_machine_plan,
            arm_id,
            arm,
            continuation_symbol,
            seq,
            out_ty,
        } = lowering;
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

        // 1) 从 unified plan 恢复 direct / indirect suspend site。
        let arm_capture_ids = state_machine_plan.arm_capture_locals(arm_id);
        let ResolvedEscapeDirectSites {
            perform_sites,
            decl_map,
            mut capture_ids,
        } = Self::resolve_escape_direct_sites_from_plan(
            handle,
            state_machine_plan,
            arm_id,
            &arm.op.op.fqn,
        )?;
        capture_ids.extend(arm_capture_ids.iter().copied());

        let Some(first_site) = perform_sites.first() else {
            // T1606f-2: No direct performs found. Check for indirect performs through function calls.
            let ResolvedEscapeIndirectSites {
                indirect_sites,
                mut capture_ids,
            } = Self::resolve_escape_indirect_sites_from_plan(handle, state_machine_plan)?;
            capture_ids.extend(arm_capture_ids.iter().copied());
            if !indirect_sites.is_empty() {
                if indirect_sites.len() > 1 {
                    return self
                        .codegen_handle_expr_escape_with_nonresuming_siblings_indirect_multi(
                            span,
                            handle,
                            state_machine_plan,
                            (arm, arm_id, continuation_symbol),
                            &[],
                            out_ty,
                        );
                }
                return self.codegen_handle_expr_escape_continuation_indirect(
                    span,
                    handle,
                    arm,
                    IndirectEscapeContinuationPlan {
                        continuation_symbol,
                        seq,
                        out_ty,
                        indirect_sites,
                        capture_ids,
                    },
                );
            }
            // T1606a：没有匹配 op 的 perform 点，arm 不可达；退化为顺序执行 `body -> finally` 并返回 body 值。
            return self.codegen_handle_expr_no_perform(span, handle, out_ty);
        };
        let perform_idx = first_site.top_level_stmt_idx;
        let perform_decl = first_site.decl;
        let perform_op = first_site.op;
        let perform_args = first_site.args;
        if perform_op.fqn != arm.op.op.fqn {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle escape op mismatch",
                at: perform_op.span.into(),
            });
        }
        let _perform_id = first_site.id;

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

        // T1606e：top-level 拦截表 — 映射 top_level_stmt_idx -> [(pc, resume_path)]。
        // 对于 flat performs，resume_path 为空；对于嵌套 performs，resume_path 描述控制流嵌套。
        // 同一 top_level_stmt_idx 下可能有多个 perform（例如 if/else 两侧各有一个 perform）。
        let mut top_level_intercepts: HashMap<usize, Vec<(usize, &[ResumeFrame<'_>])>> =
            HashMap::new();
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

        // 1) unified plan 已为匹配的 suspend sites 计算 capture 集合。这里仅把这些
        //    capture ids 映射回“外层 env local”与“handle body local”两类存储来源。
        let mut capture_ids = capture_ids.into_iter().collect::<Vec<_>>();
        capture_ids.sort_by_key(|id| id.as_u32());

        let mut outer_captures: Vec<CapturedLocal> = Vec::new();
        let mut body_lift_ids: Vec<hir::SymbolId> = Vec::new();
        for id in capture_ids {
            if let Some(local) = self.env.get(id) {
                if self.escape_capture_storage_kind(span, local.ty)?.is_some() {
                    outer_captures.push(CapturedLocal {
                        id,
                        hir_ty: local.hir_ty,
                        ty: local.ty,
                        mutable: local.mutable,
                    });
                }
            } else {
                body_lift_ids.push(id);
            }
        }

        // 2) body lifted locals：跨 suspension 使用的 handle body locals（Ref/String/Bool/Int）。
        let mut body_lifts: Vec<CapturedLocal> = Vec::new();
        for &id in &body_lift_ids {
            let Some(info) = decl_map.get(&id) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape capture local decl",
                    at: span.into(),
                });
            };
            let decl = info.decl;

            let decl_ty = self
                .cg_ty_of(decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape capture local type",
                    at: decl.span.into(),
                })?;

            if self
                .escape_capture_storage_kind(decl.span, decl_ty)?
                .is_none()
            {
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
                fields.push(match self.escape_capture_storage_kind(span, cap.ty)? {
                    Some(EscapeCaptureStorageKind::Word) => i64_ty.into(),
                    Some(EscapeCaptureStorageKind::GcRef) => gc_i8_ptr_ty.into(),
                    None => unreachable!("captures filtered by type"),
                });
            }
            for cap in &body_lifts {
                fields.push(match self.escape_capture_storage_kind(span, cap.ty)? {
                    Some(EscapeCaptureStorageKind::Word) => i64_ty.into(),
                    Some(EscapeCaptureStorageKind::GcRef) => gc_i8_ptr_ty.into(),
                    None => unreachable!("captures filtered by type"),
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
            let state_ptr_ty = cg.llvm_ptr_type(cg.gc_address_space());
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
                // 注意：这里不能把 "state 字段地址（addrspace(1)）" 直接当作 local slot。
                //
                // 原因：
                // - `field_ptr` 是一个 derived pointer，位于 GC address space；
                // - LLVM statepoint/stackmap 可能把它当作 GC root，但 runtime 的 roots 更新
                //   只支持对象头指针，不支持 derived pointer。
                //
                // 因此统一把 capture 恢复到本函数栈 slot（alloca）中，再通过 env 参与后续 codegen。
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

            for (idx, cap) in body_lifts.iter().enumerate() {
                let field_idx = body_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "cont_step_lift_gep",
                )?;
                let name = format!("lift_{}", cap.id.as_u32());
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
            let intercept_bb = self
                .context
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

                let resume_value = cg.decode_abi_payload_transport(
                    site.decl.span,
                    resume_word,
                    resume_gc_ref,
                    resume_value_ty,
                )?;

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
                            if let Some(&(next_pc, _)) =
                                intercepts.iter().find(|(_, rp)| rp.is_empty())
                            {
                                // Direct flat intercept.
                                let next_site = &perform_sites[next_pc];
                                for (slot, arg) in binder_slots.iter().zip(next_site.args.iter()) {
                                    let hir::CallArg::Positional(expr) = arg else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle escape named perform arg",
                                            at: span.into(),
                                        });
                                    };
                                    let v =
                                        cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                    let _stored =
                                        cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                }
                                let _ = cg.builder.build_store(
                                    intercept_next_pc_ptr,
                                    i32_ty.const_int(next_pc as u64, false),
                                )?;
                                cg.builder.build_unconditional_branch(intercept_bb)?;
                                terminated = true;
                                break;
                            }
                            // T1606e: nested intercepts — generate control flow with interception.
                            let first = &intercepts[0];
                            let (intercept_pc, inner_path) = *first;
                            if !inner_path.is_empty() {
                                match &inner_path[0] {
                                    ResumeFrame::IfThen {
                                        if_expr,
                                        then_block_stmts,
                                        resume_after_stmt: perform_stmt_idx,
                                        ..
                                    } => {
                                        if let hir::ExprKind::If {
                                            cond: if_cond,
                                            then_branch: _,
                                            else_branch,
                                        } = &if_expr.kind
                                        {
                                            let cond_v = cg.codegen_expr_in_expected_context(
                                                if_cond,
                                                Some(CgTy::Bool),
                                            )?;
                                            let cond_b = cond_v.as_bool().ok_or(
                                                LlvmEmitError::UnsupportedMainBody {
                                                    kind: "if cond (tail nested intercept)",
                                                    at: if_cond.span.into(),
                                                },
                                            )?;
                                            let then_bb_i = self.context.append_basic_block(
                                                step_fn,
                                                &format!("step_pc{pc}_tail_if_then"),
                                            );
                                            let has_else = else_branch.is_some();
                                            let else_or_after = self.context.append_basic_block(
                                                step_fn,
                                                &format!(
                                                    "step_pc{pc}_tail_if_{}",
                                                    if has_else { "else" } else { "after" }
                                                ),
                                            );
                                            let after_if_bb = if has_else {
                                                self.context.append_basic_block(
                                                    step_fn,
                                                    &format!("step_pc{pc}_tail_if_after"),
                                                )
                                            } else {
                                                else_or_after
                                            };
                                            cg.builder.build_conditional_branch(
                                                cond_b,
                                                then_bb_i,
                                                else_or_after,
                                            )?;

                                            // Then-branch: stmts before perform, then intercept
                                            cg.builder.position_at_end(then_bb_i);
                                            for (ti, tstmt) in then_block_stmts.iter().enumerate() {
                                                if ti == *perform_stmt_idx {
                                                    let is = &perform_sites[intercept_pc];
                                                    for (slot, arg) in
                                                        binder_slots.iter().zip(is.args.iter())
                                                    {
                                                        let hir::CallArg::Positional(expr) = arg
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
                                                        let _stored = cg.store_local_value(
                                                            expr.span, slot.ptr, slot.ty, v,
                                                        )?;
                                                    }
                                                    let _ = cg.builder.build_store(
                                                        intercept_next_pc_ptr,
                                                        i32_ty
                                                            .const_int(intercept_pc as u64, false),
                                                    )?;
                                                    cg.builder
                                                        .build_unconditional_branch(intercept_bb)?;
                                                    break;
                                                }
                                                match &tstmt.kind {
                                                    hir::StmtKind::Empty => {}
                                                    hir::StmtKind::Val(decl) => {
                                                        if let Some(id) = decl.id {
                                                            if body_lift_ids.contains(&id) {
                                                                let Some(init) = decl.init.as_ref()
                                                                else {
                                                                    return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                };
                                                                let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                let _stored = cg
                                                                    .store_local_value(
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
                                                    hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                                                        cg.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                                                    }
                                                    hir::StmtKind::Expr(expr) => {
                                                        let _ = cg.codegen_expr(expr)?;
                                                    }
                                                    _ => {}
                                                }
                                            }

                                            // Else-branch: check if also intercepted (both branches have performs).
                                            let else_intercept =
                                                intercepts.iter().find(|(_, rp)| {
                                                    matches!(
                                                        rp.first(),
                                                        Some(ResumeFrame::IfElse { .. })
                                                    )
                                                });
                                            if let Some(&(else_ipc, else_rp)) = else_intercept {
                                                if let ResumeFrame::IfElse {
                                                    else_block_stmts: ebs,
                                                    resume_after_stmt: epi,
                                                    ..
                                                } = &else_rp[0]
                                                {
                                                    cg.builder.position_at_end(else_or_after);
                                                    for (ei, estmt) in ebs.iter().enumerate() {
                                                        if ei == *epi {
                                                            let es = &perform_sites[else_ipc];
                                                            for (slot, arg) in binder_slots
                                                                .iter()
                                                                .zip(es.args.iter())
                                                            {
                                                                let hir::CallArg::Positional(expr) =
                                                                    arg
                                                                else {
                                                                    return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                                };
                                                                let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                let _stored = cg
                                                                    .store_local_value(
                                                                        expr.span, slot.ptr,
                                                                        slot.ty, v,
                                                                    )?;
                                                            }
                                                            let _ = cg.builder.build_store(
                                                                intercept_next_pc_ptr,
                                                                i32_ty.const_int(
                                                                    else_ipc as u64,
                                                                    false,
                                                                ),
                                                            )?;
                                                            cg.builder.build_unconditional_branch(
                                                                intercept_bb,
                                                            )?;
                                                            break;
                                                        }
                                                        match &estmt.kind {
                                                            hir::StmtKind::Empty => {}
                                                            hir::StmtKind::Val(decl) => {
                                                                if let Some(id) = decl.id {
                                                                    if body_lift_ids.contains(&id) {
                                                                        let Some(init) =
                                                                            decl.init.as_ref()
                                                                        else {
                                                                            return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                        };
                                                                        let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                        let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                        let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                        let _stored = cg
                                                                            .store_local_value(
                                                                                decl.span,
                                                                                local.ptr, decl_ty,
                                                                                v,
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
                                                            _ => {}
                                                        }
                                                    }
                                                }
                                            } else if let Some(else_expr) = else_branch.as_deref() {
                                                cg.builder.position_at_end(else_or_after);
                                                let _ = cg.codegen_expr(else_expr)?;
                                                cg.builder
                                                    .build_unconditional_branch(after_if_bb)?;
                                            }
                                            // After-if: continue with remaining tail stmts
                                            cg.builder.position_at_end(after_if_bb);
                                            continue;
                                        }
                                    }
                                    ResumeFrame::IfElse {
                                        if_expr,
                                        else_block_stmts,
                                        resume_after_stmt: perform_stmt_idx,
                                        ..
                                    } => {
                                        if let hir::ExprKind::If {
                                            cond: if_cond,
                                            then_branch,
                                            else_branch: _,
                                        } = &if_expr.kind
                                        {
                                            let cond_v = cg.codegen_expr_in_expected_context(
                                                if_cond,
                                                Some(CgTy::Bool),
                                            )?;
                                            let cond_b = cond_v.as_bool().ok_or(
                                                LlvmEmitError::UnsupportedMainBody {
                                                    kind: "if cond (tail nested intercept)",
                                                    at: if_cond.span.into(),
                                                },
                                            )?;
                                            let then_bb_i = self.context.append_basic_block(
                                                step_fn,
                                                &format!("step_pc{pc}_tail_if_then"),
                                            );
                                            let else_bb_i = self.context.append_basic_block(
                                                step_fn,
                                                &format!("step_pc{pc}_tail_if_else"),
                                            );
                                            let after_if_bb = self.context.append_basic_block(
                                                step_fn,
                                                &format!("step_pc{pc}_tail_if_after"),
                                            );
                                            cg.builder.build_conditional_branch(
                                                cond_b, then_bb_i, else_bb_i,
                                            )?;

                                            // Then-branch: check if also intercepted (both branches have performs).
                                            let then_intercept =
                                                intercepts.iter().find(|(_, rp)| {
                                                    matches!(
                                                        rp.first(),
                                                        Some(ResumeFrame::IfThen { .. })
                                                    )
                                                });
                                            if let Some(&(then_ipc, then_rp)) = then_intercept {
                                                if let ResumeFrame::IfThen {
                                                    then_block_stmts: tbs,
                                                    resume_after_stmt: tpi,
                                                    ..
                                                } = &then_rp[0]
                                                {
                                                    cg.builder.position_at_end(then_bb_i);
                                                    for (ti, tstmt) in tbs.iter().enumerate() {
                                                        if ti == *tpi {
                                                            let ts = &perform_sites[then_ipc];
                                                            for (slot, arg) in binder_slots
                                                                .iter()
                                                                .zip(ts.args.iter())
                                                            {
                                                                let hir::CallArg::Positional(expr) =
                                                                    arg
                                                                else {
                                                                    return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                                };
                                                                let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                let _stored = cg
                                                                    .store_local_value(
                                                                        expr.span, slot.ptr,
                                                                        slot.ty, v,
                                                                    )?;
                                                            }
                                                            let _ = cg.builder.build_store(
                                                                intercept_next_pc_ptr,
                                                                i32_ty.const_int(
                                                                    then_ipc as u64,
                                                                    false,
                                                                ),
                                                            )?;
                                                            cg.builder.build_unconditional_branch(
                                                                intercept_bb,
                                                            )?;
                                                            break;
                                                        }
                                                        match &tstmt.kind {
                                                            hir::StmtKind::Empty => {}
                                                            hir::StmtKind::Val(decl) => {
                                                                if let Some(id) = decl.id {
                                                                    if body_lift_ids.contains(&id) {
                                                                        let Some(init) =
                                                                            decl.init.as_ref()
                                                                        else {
                                                                            return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                        };
                                                                        let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                        let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                        let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                        let _stored = cg
                                                                            .store_local_value(
                                                                                decl.span,
                                                                                local.ptr, decl_ty,
                                                                                v,
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
                                                            _ => {}
                                                        }
                                                    }
                                                }
                                            } else {
                                                cg.builder.position_at_end(then_bb_i);
                                                let _ = cg.codegen_expr(then_branch)?;
                                                cg.builder
                                                    .build_unconditional_branch(after_if_bb)?;
                                            }

                                            // Else-branch: stmts before perform, then intercept
                                            cg.builder.position_at_end(else_bb_i);
                                            for (ei, estmt) in else_block_stmts.iter().enumerate() {
                                                if ei == *perform_stmt_idx {
                                                    let is = &perform_sites[intercept_pc];
                                                    for (slot, arg) in
                                                        binder_slots.iter().zip(is.args.iter())
                                                    {
                                                        let hir::CallArg::Positional(expr) = arg
                                                        else {
                                                            return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                        };
                                                        let v = cg
                                                            .codegen_expr_in_expected_context(
                                                                expr,
                                                                Some(slot.ty),
                                                            )?;
                                                        let _stored = cg.store_local_value(
                                                            expr.span, slot.ptr, slot.ty, v,
                                                        )?;
                                                    }
                                                    let _ = cg.builder.build_store(
                                                        intercept_next_pc_ptr,
                                                        i32_ty
                                                            .const_int(intercept_pc as u64, false),
                                                    )?;
                                                    cg.builder
                                                        .build_unconditional_branch(intercept_bb)?;
                                                    break;
                                                }
                                                match &estmt.kind {
                                                    hir::StmtKind::Empty => {}
                                                    hir::StmtKind::Val(decl) => {
                                                        if let Some(id) = decl.id {
                                                            if body_lift_ids.contains(&id) {
                                                                let Some(init) = decl.init.as_ref()
                                                                else {
                                                                    return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                };
                                                                let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                let _stored = cg
                                                                    .store_local_value(
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
                                                    hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                                                        cg.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                                                    }
                                                    hir::StmtKind::Expr(expr) => {
                                                        let _ = cg.codegen_expr(expr)?;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            // After-if: continue
                                            cg.builder.position_at_end(after_if_bb);
                                            continue;
                                        }
                                    }
                                    ResumeFrame::WhileBody {
                                        while_cond,
                                        while_body,
                                        resume_after_stmt: perform_body_idx,
                                        ..
                                    } => {
                                        // Generate while loop with interception at the perform point.
                                        let wc_bb = self.context.append_basic_block(
                                            step_fn,
                                            &format!("step_pc{pc}_tail_while_cond"),
                                        );
                                        let wb_bb = self.context.append_basic_block(
                                            step_fn,
                                            &format!("step_pc{pc}_tail_while_body"),
                                        );
                                        let wa_bb = self.context.append_basic_block(
                                            step_fn,
                                            &format!("step_pc{pc}_tail_while_after"),
                                        );
                                        cg.builder.build_unconditional_branch(wc_bb)?;
                                        cg.builder.position_at_end(wc_bb);
                                        let cv = cg.codegen_expr_in_expected_context(
                                            while_cond,
                                            Some(CgTy::Bool),
                                        )?;
                                        let cb = cv.as_bool().ok_or(
                                            LlvmEmitError::UnsupportedMainBody {
                                                kind: "while cond (tail nested intercept)",
                                                at: while_cond.span.into(),
                                            },
                                        )?;
                                        cg.builder.build_conditional_branch(cb, wb_bb, wa_bb)?;

                                        cg.builder.position_at_end(wb_bb);
                                        cg.env.push_scope();
                                        let mut while_term = false;
                                        for (bi, bstmt) in while_body.stmts.iter().enumerate() {
                                            if while_term {
                                                break;
                                            }
                                            if bi == *perform_body_idx {
                                                // T1606e: check for deeper nesting (if/else within while body).
                                                if inner_path.len() > 1 {
                                                    match &inner_path[1] {
                                                        ResumeFrame::IfThen {
                                                            if_expr: nested_if,
                                                            then_block_stmts: nested_tbs,
                                                            resume_after_stmt: nested_tpi,
                                                            ..
                                                        } => {
                                                            if let hir::ExprKind::If {
                                                                cond: if_cond,
                                                                then_branch: _,
                                                                else_branch,
                                                            } = &nested_if.kind
                                                            {
                                                                let cond_v = cg.codegen_expr_in_expected_context(if_cond, Some(CgTy::Bool))?;
                                                                let cond_b = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                    kind: "if cond in while body (tail intercept)",
                                                                    at: if_cond.span.into(),
                                                                })?;
                                                                let if_then_bb = self
                                                                    .context
                                                                    .append_basic_block(
                                                                        step_fn,
                                                                        &format!(
                                                                            "step_pc{pc}_wif_then"
                                                                        ),
                                                                    );
                                                                let has_else =
                                                                    else_branch.is_some();
                                                                let if_else_or_after = self
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
                                                                let if_after_bb = if has_else {
                                                                    self.context.append_basic_block(
                                                                        step_fn,
                                                                        &format!(
                                                                            "step_pc{pc}_wif_after"
                                                                        ),
                                                                    )
                                                                } else {
                                                                    if_else_or_after
                                                                };
                                                                cg.builder
                                                                    .build_conditional_branch(
                                                                        cond_b,
                                                                        if_then_bb,
                                                                        if_else_or_after,
                                                                    )?;

                                                                // Then-branch: stmts before perform, then intercept
                                                                cg.builder
                                                                    .position_at_end(if_then_bb);
                                                                for (ti, tstmt) in
                                                                    nested_tbs.iter().enumerate()
                                                                {
                                                                    if ti == *nested_tpi {
                                                                        let is = &perform_sites
                                                                            [intercept_pc];
                                                                        for (slot, arg) in
                                                                            binder_slots
                                                                                .iter()
                                                                                .zip(is.args.iter())
                                                                        {
                                                                            let hir::CallArg::Positional(expr) = arg else {
                                                                                return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                                            };
                                                                            let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                            let _stored = cg
                                                                                .store_local_value(
                                                                                    expr.span,
                                                                                    slot.ptr,
                                                                                    slot.ty, v,
                                                                                )?;
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
                                                                    match &tstmt.kind {
                                                                        hir::StmtKind::Empty => {}
                                                                        hir::StmtKind::Val(
                                                                            decl,
                                                                        ) => {
                                                                            if let Some(id) =
                                                                                decl.id
                                                                            {
                                                                                if body_lift_ids
                                                                                    .contains(&id)
                                                                                {
                                                                                    let Some(init) =
                                                                                        decl.init
                                                                                            .as_ref(
                                                                                            )
                                                                                    else {
                                                                                        return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                                    };
                                                                                    let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                                    let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                                    let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                                    let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
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
                                                                        hir::StmtKind::Expr(
                                                                            expr,
                                                                        ) => {
                                                                            let _ = cg
                                                                                .codegen_expr(
                                                                                    expr,
                                                                                )?;
                                                                        }
                                                                        _ => {}
                                                                    }
                                                                }

                                                                // Else-branch: check if also intercepted, else codegen normally.
                                                                let else_in_while = intercepts.iter().find(|(_, rp)| {
                                                                    rp.len() > 1 && matches!(rp[0], ResumeFrame::WhileBody { .. }) && matches!(rp[1], ResumeFrame::IfElse { .. })
                                                                });
                                                                if let Some(&(else_wpc, else_wrp)) =
                                                                    else_in_while
                                                                {
                                                                    if let ResumeFrame::IfElse {
                                                                        else_block_stmts: ebs,
                                                                        resume_after_stmt: epi,
                                                                        ..
                                                                    } = &else_wrp[1]
                                                                    {
                                                                        cg.builder.position_at_end(
                                                                            if_else_or_after,
                                                                        );
                                                                        for (ei, estmt) in
                                                                            ebs.iter().enumerate()
                                                                        {
                                                                            if ei == *epi {
                                                                                let es =
                                                                                    &perform_sites
                                                                                        [else_wpc];
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
                                                                } else if let Some(else_expr) =
                                                                    else_branch.as_deref()
                                                                {
                                                                    cg.builder.position_at_end(
                                                                        if_else_or_after,
                                                                    );
                                                                    let _ =
                                                                        cg.codegen_expr(else_expr)?;
                                                                    cg.builder
                                                                        .build_unconditional_branch(
                                                                            if_after_bb,
                                                                        )?;
                                                                }

                                                                // After-if: remaining while body stmts, loop back
                                                                cg.builder
                                                                    .position_at_end(if_after_bb);
                                                                for remaining in while_body.stmts
                                                                    [bi + 1..]
                                                                    .iter()
                                                                {
                                                                    match &remaining.kind {
                                                                        hir::StmtKind::Empty => {}
                                                                        hir::StmtKind::Val(
                                                                            decl,
                                                                        ) => {
                                                                            if let Some(id) =
                                                                                decl.id
                                                                            {
                                                                                if body_lift_ids
                                                                                    .contains(&id)
                                                                                {
                                                                                    let Some(init) =
                                                                                        decl.init
                                                                                            .as_ref(
                                                                                            )
                                                                                    else {
                                                                                        return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                                    };
                                                                                    let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                                    let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                                    let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                                    let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
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
                                                                        hir::StmtKind::Expr(
                                                                            expr,
                                                                        ) => {
                                                                            let _ = cg
                                                                                .codegen_expr(
                                                                                    expr,
                                                                                )?;
                                                                        }
                                                                        hir::StmtKind::While {
                                                                            cond,
                                                                            body,
                                                                        } => {
                                                                            cg.codegen_while_stmt(
                                                                                remaining.span,
                                                                                cond,
                                                                                body,
                                                                            )?;
                                                                        }
                                                                        _ => {}
                                                                    }
                                                                }
                                                                cg.builder
                                                                    .build_unconditional_branch(
                                                                        wc_bb,
                                                                    )?;
                                                                while_term = true;
                                                            }
                                                        }
                                                        ResumeFrame::IfElse {
                                                            if_expr: nested_if,
                                                            else_block_stmts: nested_ebs,
                                                            resume_after_stmt: nested_epi,
                                                            ..
                                                        } => {
                                                            if let hir::ExprKind::If {
                                                                cond: if_cond,
                                                                then_branch,
                                                                else_branch: _,
                                                            } = &nested_if.kind
                                                            {
                                                                let cond_v = cg.codegen_expr_in_expected_context(if_cond, Some(CgTy::Bool))?;
                                                                let cond_b = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                    kind: "if cond in while body (tail intercept)",
                                                                    at: if_cond.span.into(),
                                                                })?;
                                                                let if_then_bb = self
                                                                    .context
                                                                    .append_basic_block(
                                                                        step_fn,
                                                                        &format!(
                                                                            "step_pc{pc}_wif_then"
                                                                        ),
                                                                    );
                                                                let if_else_bb = self
                                                                    .context
                                                                    .append_basic_block(
                                                                        step_fn,
                                                                        &format!(
                                                                            "step_pc{pc}_wif_else"
                                                                        ),
                                                                    );
                                                                let if_after_bb = self
                                                                    .context
                                                                    .append_basic_block(
                                                                        step_fn,
                                                                        &format!(
                                                                            "step_pc{pc}_wif_after"
                                                                        ),
                                                                    );
                                                                cg.builder
                                                                    .build_conditional_branch(
                                                                        cond_b, if_then_bb,
                                                                        if_else_bb,
                                                                    )?;

                                                                // Then-branch: check if also intercepted, else codegen normally.
                                                                let then_in_while = intercepts.iter().find(|(_, rp)| {
                                                                    rp.len() > 1 && matches!(rp[0], ResumeFrame::WhileBody { .. }) && matches!(rp[1], ResumeFrame::IfThen { .. })
                                                                });
                                                                if let Some(&(then_wpc, then_wrp)) =
                                                                    then_in_while
                                                                {
                                                                    if let ResumeFrame::IfThen {
                                                                        then_block_stmts: tbs,
                                                                        resume_after_stmt: tpi,
                                                                        ..
                                                                    } = &then_wrp[1]
                                                                    {
                                                                        cg.builder.position_at_end(
                                                                            if_then_bb,
                                                                        );
                                                                        for (ti, tstmt) in
                                                                            tbs.iter().enumerate()
                                                                        {
                                                                            if ti == *tpi {
                                                                                let ts =
                                                                                    &perform_sites
                                                                                        [then_wpc];
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
                                                                    cg.builder.position_at_end(
                                                                        if_then_bb,
                                                                    );
                                                                    let _ = cg.codegen_expr(
                                                                        then_branch,
                                                                    )?;
                                                                    cg.builder
                                                                        .build_unconditional_branch(
                                                                            if_after_bb,
                                                                        )?;
                                                                }

                                                                // Else-branch: stmts before perform, then intercept
                                                                cg.builder
                                                                    .position_at_end(if_else_bb);
                                                                for (ei, estmt) in
                                                                    nested_ebs.iter().enumerate()
                                                                {
                                                                    if ei == *nested_epi {
                                                                        let is = &perform_sites
                                                                            [intercept_pc];
                                                                        for (slot, arg) in
                                                                            binder_slots
                                                                                .iter()
                                                                                .zip(is.args.iter())
                                                                        {
                                                                            let hir::CallArg::Positional(expr) = arg else { return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() }); };
                                                                            let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                            let _stored = cg
                                                                                .store_local_value(
                                                                                    expr.span,
                                                                                    slot.ptr,
                                                                                    slot.ty, v,
                                                                                )?;
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
                                                                        hir::StmtKind::Val(
                                                                            decl,
                                                                        ) => {
                                                                            if let Some(id) =
                                                                                decl.id
                                                                            {
                                                                                if body_lift_ids
                                                                                    .contains(&id)
                                                                                {
                                                                                    let Some(init) =
                                                                                        decl.init
                                                                                            .as_ref(
                                                                                            )
                                                                                    else {
                                                                                        return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                                    };
                                                                                    let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                                    let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                                    let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                                    let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
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
                                                                        hir::StmtKind::Expr(
                                                                            expr,
                                                                        ) => {
                                                                            let _ = cg
                                                                                .codegen_expr(
                                                                                    expr,
                                                                                )?;
                                                                        }
                                                                        _ => {}
                                                                    }
                                                                }

                                                                // After-if: remaining while body stmts, loop back
                                                                cg.builder
                                                                    .position_at_end(if_after_bb);
                                                                for remaining in while_body.stmts
                                                                    [bi + 1..]
                                                                    .iter()
                                                                {
                                                                    match &remaining.kind {
                                                                        hir::StmtKind::Empty => {}
                                                                        hir::StmtKind::Val(
                                                                            decl,
                                                                        ) => {
                                                                            if let Some(id) =
                                                                                decl.id
                                                                            {
                                                                                if body_lift_ids
                                                                                    .contains(&id)
                                                                                {
                                                                                    let Some(init) =
                                                                                        decl.init
                                                                                            .as_ref(
                                                                                            )
                                                                                    else {
                                                                                        return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                                    };
                                                                                    let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                                    let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                                    let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                                    let _stored = cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
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
                                                                        hir::StmtKind::Expr(
                                                                            expr,
                                                                        ) => {
                                                                            let _ = cg
                                                                                .codegen_expr(
                                                                                    expr,
                                                                                )?;
                                                                        }
                                                                        hir::StmtKind::While {
                                                                            cond,
                                                                            body,
                                                                        } => {
                                                                            cg.codegen_while_stmt(
                                                                                remaining.span,
                                                                                cond,
                                                                                body,
                                                                            )?;
                                                                        }
                                                                        _ => {}
                                                                    }
                                                                }
                                                                cg.builder
                                                                    .build_unconditional_branch(
                                                                        wc_bb,
                                                                    )?;
                                                                while_term = true;
                                                            }
                                                        }
                                                        _ => {
                                                            // Direct flat intercept in while body (inner_path[1] is unsupported nested type)
                                                            let is = &perform_sites[intercept_pc];
                                                            for (slot, arg) in binder_slots
                                                                .iter()
                                                                .zip(is.args.iter())
                                                            {
                                                                let hir::CallArg::Positional(expr) =
                                                                    arg
                                                                else {
                                                                    return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                                };
                                                                let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                let _stored = cg
                                                                    .store_local_value(
                                                                        expr.span, slot.ptr,
                                                                        slot.ty, v,
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
                                                            while_term = true;
                                                        }
                                                    }
                                                } else {
                                                    // Direct flat intercept in while body (perform is directly at this stmt)
                                                    let is = &perform_sites[intercept_pc];
                                                    for (slot, arg) in
                                                        binder_slots.iter().zip(is.args.iter())
                                                    {
                                                        let hir::CallArg::Positional(expr) = arg
                                                        else {
                                                            return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                        };
                                                        let v = cg
                                                            .codegen_expr_in_expected_context(
                                                                expr,
                                                                Some(slot.ty),
                                                            )?;
                                                        let _stored = cg.store_local_value(
                                                            expr.span, slot.ptr, slot.ty, v,
                                                        )?;
                                                    }
                                                    let _ = cg.builder.build_store(
                                                        intercept_next_pc_ptr,
                                                        i32_ty
                                                            .const_int(intercept_pc as u64, false),
                                                    )?;
                                                    cg.builder
                                                        .build_unconditional_branch(intercept_bb)?;
                                                    while_term = true;
                                                }
                                                break;
                                            }
                                            match &bstmt.kind {
                                                hir::StmtKind::Empty => {}
                                                hir::StmtKind::Val(decl) => {
                                                    if let Some(id) = decl.id {
                                                        if body_lift_ids.contains(&id) {
                                                            let Some(init) = decl.init.as_ref()
                                                            else {
                                                                return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                            };
                                                            let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                            let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                            let v = cg
                                                                .codegen_expr_in_expected_context(
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
                                                hir::StmtKind::While { cond, body } => {
                                                    cg.codegen_while_stmt(bstmt.span, cond, body)?;
                                                }
                                                _ => {
                                                    return Err(
                                                        LlvmEmitError::UnsupportedMainBody {
                                                            kind: "stmt in while body (tail nested intercept)",
                                                            at: bstmt.span.into(),
                                                        },
                                                    );
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
                                        let local = cg.env.get(id).ok_or(
                                            LlvmEmitError::UnsupportedMainBody {
                                                kind: "lifted local slot missing",
                                                at: decl.span.into(),
                                            },
                                        )?;
                                        let v = cg.codegen_expr_in_expected_context(
                                            init,
                                            Some(decl_ty),
                                        )?;
                                        let _stored =
                                            cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
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
                        let (tail_stmts, tail_base_idx): (&[hir::Stmt], usize) = match frame {
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
                        for (other_pc, other_site) in perform_sites.iter().enumerate() {
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
                            if !resume_frame_same_structure(frame, other_frame) {
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
                                let inner_path = &other_site.resume_path[level + 1..];
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
                            if let Some(intercepts) = tail_intercept_map.get(&actual_idx) {
                                let (intercept_pc, inner_path) = intercepts
                                    .iter()
                                    .find(|(_, ip)| ip.is_empty())
                                    .copied()
                                    .unwrap_or(intercepts[0]);
                                if inner_path.is_empty() {
                                    // Direct perform at this level
                                    let intercept_site = &perform_sites[intercept_pc];
                                    for (slot, arg) in
                                        binder_slots.iter().zip(intercept_site.args.iter())
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
                                        let _stored =
                                            cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                    }
                                    let _ = cg.builder.build_store(
                                        intercept_next_pc_ptr,
                                        i32_ty.const_int(intercept_pc as u64, false),
                                    )?;
                                    cg.builder.build_unconditional_branch(intercept_bb)?;
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
                                            let local = cg.env.get(id).ok_or(
                                                LlvmEmitError::UnsupportedMainBody {
                                                    kind: "lifted local slot missing",
                                                    at: decl.span.into(),
                                                },
                                            )?;
                                            let v = cg.codegen_expr_in_expected_context(
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
                                hir::StmtKind::While { cond, body } => {
                                    cg.codegen_while_stmt(tail_stmt.span, cond, body)?;
                                }
                                _ => {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "statement inside continuation step (nested tail)",
                                        at: tail_stmt.span.into(),
                                    });
                                }
                            }
                        }

                        // For WhileBody: after the tail, re-enter the while
                        // loop with perform interception for all sites in
                        // this body.
                        if !terminated
                            && let ResumeFrame::WhileBody {
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
                            for (other_pc, other_site) in perform_sites.iter().enumerate() {
                                for (fi, fcheck) in other_site.resume_path.iter().enumerate() {
                                    if let ResumeFrame::WhileBody {
                                        while_body: wb,
                                        resume_after_stmt: ras,
                                        ..
                                    } = fcheck
                                        && std::ptr::eq(*wb, *while_body)
                                    {
                                        while_intercept_map
                                            .entry(*ras)
                                            .or_default()
                                            .push((other_pc, &other_site.resume_path[fi + 1..]));
                                        break;
                                    }
                                }
                            }

                            // Generate while loop.
                            let while_cond_bb = self
                                .context
                                .append_basic_block(step_fn, &format!("step_pc{pc}_while_cond"));
                            let while_body_bb = self
                                .context
                                .append_basic_block(step_fn, &format!("step_pc{pc}_while_body"));
                            let while_after_bb = self
                                .context
                                .append_basic_block(step_fn, &format!("step_pc{pc}_while_after"));

                            cg.builder.build_unconditional_branch(while_cond_bb)?;
                            cg.builder.position_at_end(while_cond_bb);
                            let cv =
                                cg.codegen_expr_in_expected_context(while_cond, Some(CgTy::Bool))?;
                            let cb = cv.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "while cond value (step while re-exec)",
                                at: while_cond.span.into(),
                            })?;
                            cg.builder.build_conditional_branch(
                                cb,
                                while_body_bb,
                                while_after_bb,
                            )?;

                            cg.builder.position_at_end(while_body_bb);
                            cg.env.push_scope();
                            let mut while_terminated = false;
                            for (body_idx, body_stmt) in while_body.stmts.iter().enumerate() {
                                if while_terminated {
                                    break;
                                }
                                if let Some(intercepts) = while_intercept_map.get(&body_idx) {
                                    let (intercept_pc, inner_path) = intercepts
                                        .iter()
                                        .find(|(_, ip)| ip.is_empty())
                                        .copied()
                                        .unwrap_or(intercepts[0]);
                                    if inner_path.is_empty() {
                                        // Direct perform in while body
                                        let intercept_site = &perform_sites[intercept_pc];
                                        for (slot, arg) in
                                            binder_slots.iter().zip(intercept_site.args.iter())
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
                                            i32_ty.const_int(intercept_pc as u64, false),
                                        )?;
                                        cg.builder.build_unconditional_branch(intercept_bb)?;
                                        while_terminated = true;
                                        break;
                                    } else {
                                        // Nested perform in if/etc inside while body.
                                        // Generate control flow with interception.
                                        match &inner_path[0] {
                                            ResumeFrame::IfThen {
                                                if_expr,
                                                then_block_stmts,
                                                resume_after_stmt: perform_stmt_idx,
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
                                                            Some(CgTy::Bool),
                                                        )?;
                                                    let cond_b = cond_v
                                                            .as_bool()
                                                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                kind: "if cond value (while body intercept)",
                                                                at: if_cond.span.into(),
                                                            })?;
                                                    let then_bb_i =
                                                        self.context.append_basic_block(
                                                            step_fn,
                                                            &format!("step_pc{pc}_wif_then"),
                                                        );
                                                    let has_else = else_branch.is_some();
                                                    let else_or_after =
                                                        self.context.append_basic_block(
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
                                                    let after_if_bb = if has_else {
                                                        self.context.append_basic_block(
                                                            step_fn,
                                                            &format!("step_pc{pc}_wif_after"),
                                                        )
                                                    } else {
                                                        else_or_after
                                                    };
                                                    cg.builder.build_conditional_branch(
                                                        cond_b,
                                                        then_bb_i,
                                                        else_or_after,
                                                    )?;

                                                    // Then-branch: codegen stmts before perform, then intercept
                                                    cg.builder.position_at_end(then_bb_i);
                                                    for (ti, tstmt) in
                                                        then_block_stmts.iter().enumerate()
                                                    {
                                                        if ti == *perform_stmt_idx {
                                                            let is = &perform_sites[intercept_pc];
                                                            for (slot, arg) in binder_slots
                                                                .iter()
                                                                .zip(is.args.iter())
                                                            {
                                                                let hir::CallArg::Positional(expr) =
                                                                    arg
                                                                else {
                                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                                            kind: "handle escape named perform arg",
                                                                            at: span.into(),
                                                                        });
                                                                };
                                                                let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                let _stored = cg
                                                                    .store_local_value(
                                                                        expr.span, slot.ptr,
                                                                        slot.ty, v,
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
                                                            break;
                                                        }
                                                        // Normal stmt before the perform
                                                        match &tstmt.kind {
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
                                                                        let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                                kind: "lifted local type",
                                                                                at: decl.span.into(),
                                                                            })?;
                                                                        let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                                kind: "lifted local slot missing",
                                                                                at: decl.span.into(),
                                                                            })?;
                                                                        let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                        let _stored = cg
                                                                            .store_local_value(
                                                                                decl.span,
                                                                                local.ptr, decl_ty,
                                                                                v,
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
                                                            _ => {}
                                                        }
                                                    }

                                                    // Else-branch: codegen normally
                                                    if let Some(else_expr) = else_branch.as_deref()
                                                    {
                                                        cg.builder.position_at_end(else_or_after);
                                                        let _ = cg.codegen_expr(else_expr)?;
                                                        cg.builder.build_unconditional_branch(
                                                            after_if_bb,
                                                        )?;
                                                    }
                                                    // After-if: continue while body
                                                    cg.builder.position_at_end(after_if_bb);
                                                }
                                            }
                                            ResumeFrame::IfElse {
                                                if_expr,
                                                else_block_stmts,
                                                resume_after_stmt: perform_stmt_idx,
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
                                                            Some(CgTy::Bool),
                                                        )?;
                                                    let cond_b = cond_v
                                                            .as_bool()
                                                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                kind: "if cond value (while body intercept)",
                                                                at: if_cond.span.into(),
                                                            })?;
                                                    let then_bb_i =
                                                        self.context.append_basic_block(
                                                            step_fn,
                                                            &format!("step_pc{pc}_wif_then"),
                                                        );
                                                    let else_bb_i =
                                                        self.context.append_basic_block(
                                                            step_fn,
                                                            &format!("step_pc{pc}_wif_else"),
                                                        );
                                                    let after_if_bb =
                                                        self.context.append_basic_block(
                                                            step_fn,
                                                            &format!("step_pc{pc}_wif_after"),
                                                        );
                                                    cg.builder.build_conditional_branch(
                                                        cond_b, then_bb_i, else_bb_i,
                                                    )?;

                                                    // Then-branch: codegen normally
                                                    cg.builder.position_at_end(then_bb_i);
                                                    let _ = cg.codegen_expr(then_branch)?;
                                                    cg.builder
                                                        .build_unconditional_branch(after_if_bb)?;

                                                    // Else-branch: stmts before perform, then intercept
                                                    cg.builder.position_at_end(else_bb_i);
                                                    for (ei, estmt) in
                                                        else_block_stmts.iter().enumerate()
                                                    {
                                                        if ei == *perform_stmt_idx {
                                                            let is = &perform_sites[intercept_pc];
                                                            for (slot, arg) in binder_slots
                                                                .iter()
                                                                .zip(is.args.iter())
                                                            {
                                                                let hir::CallArg::Positional(expr) =
                                                                    arg
                                                                else {
                                                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                                                            kind: "handle escape named perform arg",
                                                                            at: span.into(),
                                                                        });
                                                                };
                                                                let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                let _stored = cg
                                                                    .store_local_value(
                                                                        expr.span, slot.ptr,
                                                                        slot.ty, v,
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
                                                            break;
                                                        }
                                                        match &estmt.kind {
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
                                                                        let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                                kind: "lifted local type",
                                                                                at: decl.span.into(),
                                                                            })?;
                                                                        let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody {
                                                                                kind: "lifted local slot missing",
                                                                                at: decl.span.into(),
                                                                            })?;
                                                                        let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                        let _stored = cg
                                                                            .store_local_value(
                                                                                decl.span,
                                                                                local.ptr, decl_ty,
                                                                                v,
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
                                                            _ => {}
                                                        }
                                                    }

                                                    // After-if
                                                    cg.builder.position_at_end(after_if_bb);
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
                                                if body_lift_ids.contains(&id) {
                                                    let Some(init) = decl.init.as_ref() else {
                                                        return Err(
                                                            LlvmEmitError::UnsupportedMainBody {
                                                                kind: "lifted local without init",
                                                                at: decl.span.into(),
                                                            },
                                                        );
                                                    };
                                                    let decl_ty = cg.cg_ty_of(decl.ty).ok_or(
                                                        LlvmEmitError::UnsupportedMainBody {
                                                            kind: "lifted local type",
                                                            at: decl.span.into(),
                                                        },
                                                    )?;
                                                    let local = cg.env.get(id).ok_or(
                                                        LlvmEmitError::UnsupportedMainBody {
                                                            kind: "lifted local slot missing",
                                                            at: decl.span.into(),
                                                        },
                                                    )?;
                                                    let v = cg.codegen_expr_in_expected_context(
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
                                        hir::StmtKind::While { cond, body } => {
                                            cg.codegen_while_stmt(body_stmt.span, cond, body)?;
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
                                cg.builder.build_unconditional_branch(while_cond_bb)?;
                            }
                            cg.builder.position_at_end(while_after_bb);
                        }
                    }

                    // Top-level tail stmts (after top_level_stmt_idx)
                    if !terminated {
                        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
                            if terminated {
                                break;
                            }
                            if idx <= site.top_level_stmt_idx {
                                continue;
                            }
                            if let Some(intercepts) = top_level_intercepts.get(&idx) {
                                // Check for flat intercept first.
                                if let Some(&(next_pc, _)) =
                                    intercepts.iter().find(|(_, rp)| rp.is_empty())
                                {
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
                                        let _stored =
                                            cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                    }
                                    let _ = cg.builder.build_store(
                                        intercept_next_pc_ptr,
                                        i32_ty.const_int(next_pc as u64, false),
                                    )?;
                                    cg.builder.build_unconditional_branch(intercept_bb)?;
                                    terminated = true;
                                    break;
                                }
                                // T1606e: nested intercepts — same logic as flat path.
                                let first = &intercepts[0];
                                let (intercept_pc, inner_path) = *first;
                                if !inner_path.is_empty() {
                                    match &inner_path[0] {
                                        ResumeFrame::IfThen {
                                            if_expr,
                                            then_block_stmts,
                                            resume_after_stmt: perform_stmt_idx,
                                            ..
                                        } => {
                                            if let hir::ExprKind::If {
                                                cond: if_cond,
                                                then_branch: _,
                                                else_branch,
                                            } = &if_expr.kind
                                            {
                                                let cond_v = cg.codegen_expr_in_expected_context(
                                                    if_cond,
                                                    Some(CgTy::Bool),
                                                )?;
                                                let cond_b = cond_v.as_bool().ok_or(
                                                    LlvmEmitError::UnsupportedMainBody {
                                                        kind: "if cond (nested tail intercept)",
                                                        at: if_cond.span.into(),
                                                    },
                                                )?;
                                                let then_bb_i = self.context.append_basic_block(
                                                    step_fn,
                                                    &format!("step_pc{pc}_ntail_if_then"),
                                                );
                                                let has_else = else_branch.is_some();
                                                let else_or_after =
                                                    self.context.append_basic_block(
                                                        step_fn,
                                                        &format!(
                                                            "step_pc{pc}_ntail_if_{}",
                                                            if has_else { "else" } else { "after" }
                                                        ),
                                                    );
                                                let after_if_bb = if has_else {
                                                    self.context.append_basic_block(
                                                        step_fn,
                                                        &format!("step_pc{pc}_ntail_if_after"),
                                                    )
                                                } else {
                                                    else_or_after
                                                };
                                                cg.builder.build_conditional_branch(
                                                    cond_b,
                                                    then_bb_i,
                                                    else_or_after,
                                                )?;
                                                cg.builder.position_at_end(then_bb_i);
                                                for (ti, tstmt) in
                                                    then_block_stmts.iter().enumerate()
                                                {
                                                    if ti == *perform_stmt_idx {
                                                        let is = &perform_sites[intercept_pc];
                                                        for (slot, arg) in
                                                            binder_slots.iter().zip(is.args.iter())
                                                        {
                                                            let hir::CallArg::Positional(expr) =
                                                                arg
                                                            else {
                                                                return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                            };
                                                            let v = cg
                                                                .codegen_expr_in_expected_context(
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
                                                        break;
                                                    }
                                                    match &tstmt.kind {
                                                        hir::StmtKind::Empty => {}
                                                        hir::StmtKind::Val(decl) => {
                                                            if let Some(id) = decl.id {
                                                                if body_lift_ids.contains(&id) {
                                                                    let Some(init) =
                                                                        decl.init.as_ref()
                                                                    else {
                                                                        return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                    };
                                                                    let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                    let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                    let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                    let _stored = cg
                                                                        .store_local_value(
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
                                                        _ => {}
                                                    }
                                                }
                                                // Else-branch: check if also intercepted (both branches).
                                                let else_intercept =
                                                    intercepts.iter().find(|(_, rp)| {
                                                        matches!(
                                                            rp.first(),
                                                            Some(ResumeFrame::IfElse { .. })
                                                        )
                                                    });
                                                if let Some(&(else_ipc, else_rp)) = else_intercept {
                                                    if let ResumeFrame::IfElse {
                                                        else_block_stmts: ebs,
                                                        resume_after_stmt: epi,
                                                        ..
                                                    } = &else_rp[0]
                                                    {
                                                        cg.builder.position_at_end(else_or_after);
                                                        for (ei, estmt) in ebs.iter().enumerate() {
                                                            if ei == *epi {
                                                                let es = &perform_sites[else_ipc];
                                                                for (slot, arg) in binder_slots
                                                                    .iter()
                                                                    .zip(es.args.iter())
                                                                {
                                                                    let hir::CallArg::Positional(
                                                                        expr,
                                                                    ) = arg
                                                                    else {
                                                                        return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                                    };
                                                                    let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                                                    let _stored = cg
                                                                        .store_local_value(
                                                                            expr.span, slot.ptr,
                                                                            slot.ty, v,
                                                                        )?;
                                                                }
                                                                let _ = cg.builder.build_store(
                                                                    intercept_next_pc_ptr,
                                                                    i32_ty.const_int(
                                                                        else_ipc as u64,
                                                                        false,
                                                                    ),
                                                                )?;
                                                                cg.builder
                                                                    .build_unconditional_branch(
                                                                        intercept_bb,
                                                                    )?;
                                                                break;
                                                            }
                                                            match &estmt.kind {
                                                                hir::StmtKind::Empty => {}
                                                                hir::StmtKind::Val(decl) => {
                                                                    if let Some(id) = decl.id {
                                                                        if body_lift_ids
                                                                            .contains(&id)
                                                                        {
                                                                            let Some(init) =
                                                                                decl.init.as_ref()
                                                                            else {
                                                                                return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                            };
                                                                            let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                            let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                            let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                            let _stored = cg
                                                                                .store_local_value(
                                                                                    decl.span,
                                                                                    local.ptr,
                                                                                    decl_ty, v,
                                                                                )?;
                                                                        } else {
                                                                            cg.codegen_val_decl(
                                                                                decl,
                                                                            )?;
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
                                                                    let _ =
                                                                        cg.codegen_expr(expr)?;
                                                                }
                                                                _ => {}
                                                            }
                                                        }
                                                    }
                                                } else if let Some(else_expr) =
                                                    else_branch.as_deref()
                                                {
                                                    cg.builder.position_at_end(else_or_after);
                                                    let _ = cg.codegen_expr(else_expr)?;
                                                    cg.builder
                                                        .build_unconditional_branch(after_if_bb)?;
                                                }
                                                cg.builder.position_at_end(after_if_bb);
                                                continue;
                                            }
                                        }
                                        ResumeFrame::WhileBody {
                                            while_cond,
                                            while_body,
                                            resume_after_stmt: perform_body_idx,
                                            ..
                                        } => {
                                            let wc_bb = self.context.append_basic_block(
                                                step_fn,
                                                &format!("step_pc{pc}_ntail_wc"),
                                            );
                                            let wb_bb = self.context.append_basic_block(
                                                step_fn,
                                                &format!("step_pc{pc}_ntail_wb"),
                                            );
                                            let wa_bb = self.context.append_basic_block(
                                                step_fn,
                                                &format!("step_pc{pc}_ntail_wa"),
                                            );
                                            cg.builder.build_unconditional_branch(wc_bb)?;
                                            cg.builder.position_at_end(wc_bb);
                                            let cv = cg.codegen_expr_in_expected_context(
                                                while_cond,
                                                Some(CgTy::Bool),
                                            )?;
                                            let cb = cv.as_bool().ok_or(
                                                LlvmEmitError::UnsupportedMainBody {
                                                    kind: "while cond (nested tail intercept)",
                                                    at: while_cond.span.into(),
                                                },
                                            )?;
                                            cg.builder
                                                .build_conditional_branch(cb, wb_bb, wa_bb)?;
                                            cg.builder.position_at_end(wb_bb);
                                            cg.env.push_scope();
                                            let mut wt = false;
                                            for (bi, bstmt) in while_body.stmts.iter().enumerate() {
                                                if wt {
                                                    break;
                                                }
                                                if bi == *perform_body_idx {
                                                    let is = &perform_sites[intercept_pc];
                                                    for (slot, arg) in
                                                        binder_slots.iter().zip(is.args.iter())
                                                    {
                                                        let hir::CallArg::Positional(expr) = arg
                                                        else {
                                                            return Err(LlvmEmitError::UnsupportedMainBody { kind: "handle escape named perform arg", at: span.into() });
                                                        };
                                                        let v = cg
                                                            .codegen_expr_in_expected_context(
                                                                expr,
                                                                Some(slot.ty),
                                                            )?;
                                                        let _stored = cg.store_local_value(
                                                            expr.span, slot.ptr, slot.ty, v,
                                                        )?;
                                                    }
                                                    let _ = cg.builder.build_store(
                                                        intercept_next_pc_ptr,
                                                        i32_ty
                                                            .const_int(intercept_pc as u64, false),
                                                    )?;
                                                    cg.builder
                                                        .build_unconditional_branch(intercept_bb)?;
                                                    wt = true;
                                                    break;
                                                }
                                                match &bstmt.kind {
                                                    hir::StmtKind::Empty => {}
                                                    hir::StmtKind::Val(decl) => {
                                                        if let Some(id) = decl.id {
                                                            if body_lift_ids.contains(&id) {
                                                                let Some(init) = decl.init.as_ref()
                                                                else {
                                                                    return Err(LlvmEmitError::UnsupportedMainBody { kind: "lifted local without init", at: decl.span.into() });
                                                                };
                                                                let decl_ty = cg.cg_ty_of(decl.ty).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local type", at: decl.span.into() })?;
                                                                let local = cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody { kind: "lifted local slot missing", at: decl.span.into() })?;
                                                                let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                                                let _stored = cg
                                                                    .store_local_value(
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
                                                    hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                                                        cg.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                                                    }
                                                    hir::StmtKind::Expr(expr) => {
                                                        let _ = cg.codegen_expr(expr)?;
                                                    }
                                                    hir::StmtKind::While { cond, body } => {
                                                        cg.codegen_while_stmt(
                                                            bstmt.span, cond, body,
                                                        )?;
                                                    }
                                                    _ => {
                                                        return Err(
                                                            LlvmEmitError::UnsupportedMainBody {
                                                                kind: "stmt in while (nested tail intercept)",
                                                                at: bstmt.span.into(),
                                                            },
                                                        );
                                                    }
                                                }
                                            }
                                            cg.env.pop_scope();
                                            if !wt {
                                                cg.builder.build_unconditional_branch(wc_bb)?;
                                            }
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
                                            let local = cg.env.get(id).ok_or(
                                                LlvmEmitError::UnsupportedMainBody {
                                                    kind: "lifted local slot missing",
                                                    at: decl.span.into(),
                                                },
                                            )?;
                                            let v = cg.codegen_expr_in_expected_context(
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
                                hir::StmtKind::While { cond, body } => {
                                    cg.codegen_while_stmt(stmt.span, cond, body)?;
                                }
                                _ => {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "statement inside continuation step",
                                        at: stmt.span.into(),
                                    });
                                }
                            }
                        }
                    }
                }

                // Completion: unpin state + return
                if !terminated {
                    let unpin = cg.declare_runtime_gc_unpin();
                    let _ =
                        cg.builder
                            .build_call(unpin, &[state_raw.into()], "cont_state_unpin")?;
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
                    let local = cg
                        .env
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
                    cg.write_escape_capture_local_to_state(span, field_ptr, local.ptr, cap.ty)?;
                }

                for (idx, cap) in body_lifts.iter().enumerate() {
                    let field_idx = body_field_base.saturating_add(idx as u32);
                    let field_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        field_idx,
                        "intercept_lift_gep",
                    )?;
                    let local = cg
                        .env
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
                    cg.write_escape_capture_local_to_state(span, field_ptr, local.ptr, cap.ty)?;
                }

                // Update pc in state from the alloca set by each interception point.
                let next_pc_val = cg.builder.build_load(
                    i32_ty,
                    intercept_next_pc_ptr,
                    "intercept_load_next_pc",
                )?;
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
                let _ = cg
                    .builder
                    .build_call(pin, &[k_raw.into()], "intercept_k_pin")?;

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
                let frame_ptr =
                    cg.builder
                        .build_struct_gep(state_ty, state_ptr, 1, "intercept_frame_gep")?;
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
                let _ = cg
                    .builder
                    .build_call(rt_swap, &[prev_raw.into()], "intercept_detach")?;

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
                let _ = cg
                    .builder
                    .build_call(unpin, &[k_loaded.into()], "intercept_k_unpin")?;

                cg.builder.build_return(None)?;
            } // if intercept_reachable

            cg.env.pop_scope();
        }

        // 恢复外层插入点。
        self.builder.position_at_end(saved_block);

        // 3) 生成 handle 的初始执行：push handler frame → 在 perform 点创建 continuation → 执行 arm → 返回。
        let has_finally = handle.finally.is_some();
        let handle_blocks = self.build_escape_handle_blocks(func, "handle_escape", false, has_finally);
        let body_bb = handle_blocks.body_bb;
        let arm_bb = handle_blocks.arm_bb;
        let done_bb = handle_blocks.done_bb;
        let finally_bb = handle_blocks.finally_bb;
        let finally_unwind_bb = handle_blocks.finally_unwind_bb;
        let outer_raise_target = self.current_raise_target();

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
        let _ = self
            .builder
            .build_call(pin, &[state_raw.into()], "cont_state_pin")?;

        let state_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
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
            let pc_ptr =
                self.builder
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
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }
        for (idx, cap) in body_lifts.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_ptr,
                field_idx,
                "cont_state_capture_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
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
        let op_tag_i32 = self.context.i32_type().const_int(escape_tag as u64, false);
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
            self.write_escape_capture_local_to_state(span, field_ptr, local.ptr, cap.ty)?;
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
            self.write_escape_capture_local_to_state(span, field_ptr, local.ptr, cap.ty)?;
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
            let pc_ptr =
                self.builder
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

        // T1609: arm body 若发生 Raise，先执行 finally 再向外传播。
        if let Some(fu_bb) = finally_unwind_bb {
            self.push_raise_target(fu_bb);
        }
        let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
        if finally_unwind_bb.is_some() {
            self.pop_raise_target();
        }
        let arm_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(arm.body.span, arm_v, out_ty)?
        };

        // arm 正常完成：保存结果，跳到 finally（若有）或 done。
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
            }
            let target = finally_bb.unwrap_or(done_bb);
            self.builder.build_unconditional_branch(target)?;
        }

        self.env.pop_scope();

        // --- finally_unwind (T1609) ---
        // arm body 内发生 Raise 时：先执行 finally，再向外层传播 raise（不清 flag/slot）。
        if let Some(fu_bb) = finally_unwind_bb {
            self.builder.position_at_end(fu_bb);
            if let Some(finally) = handle.finally.as_ref() {
                let _ = self.codegen_block_value(finally)?;
            }
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                if let Some(target) = outer_raise_target {
                    self.builder.build_unconditional_branch(target)?;
                } else {
                    let ret_ty =
                        self.current_fun_return_ty
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle escape finally unwind needs function return type",
                                at: span.into(),
                            })?;
                    let v = self.default_value(span, ret_ty)?;
                    self.emit_return(span, ret_ty, v)?;
                }
            }
        }

        // --- finally (T1609) ---
        // arm body 正常完成：执行 finally，然后进入 done。
        if let Some(f_bb) = finally_bb {
            self.builder.position_at_end(f_bb);
            if let Some(finally) = handle.finally.as_ref() {
                let _ = self.codegen_block_value(finally)?;
            }
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                self.builder.build_unconditional_branch(done_bb)?;
            }
        }

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
            CgTy::Never => CgValue::never(),
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


    /// T1606f-2: Escape continuation codegen when the handle body has no direct performs,
    /// but has function calls that may indirectly perform.
    ///
    /// Architecture:
    /// - Handle body executes stmts normally (including function calls).
    /// - A raise_target dispatches flag-based unwind after calls to a catch block.
    /// - The catch block checks op_tag: if matching, saves state, creates continuation, runs arm.
    /// - The step function resumes by writing resume_word to TLS callee state and re-calling.
    fn codegen_handle_expr_escape_continuation_indirect(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        arm: &hir::HandleArm,
        plan: IndirectEscapeContinuationPlan,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let IndirectEscapeContinuationPlan {
            continuation_symbol,
            seq,
            out_ty,
            indirect_sites,
            capture_ids,
        } = plan;
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

        // For indirect performs, we need the call result type (= resume value type for the callee).
        let first_site = &indirect_sites[0];
        let call_result_cg_ty =
            self.cg_ty_of(first_site.result_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "indirect perform call result type",
                    at: span.into(),
                })?;

        // Compute outer captures: locals from enclosing scope used inside handle body.
        struct CapturedLocal {
            id: hir::SymbolId,
            hir_ty: Option<TypeId>,
            ty: CgTy,
            mutable: bool,
        }
        let decl_map = Self::collect_escape_decl_map(handle);
        let mut capture_ids = capture_ids.into_iter().collect::<Vec<_>>();
        capture_ids.sort_by_key(|id| id.as_u32());

        let mut outer_captures: Vec<CapturedLocal> = Vec::new();
        let mut body_lift_ids: Vec<hir::SymbolId> = Vec::new();
        for id in capture_ids {
            if let Some(local) = self.env.get(id)
                && matches!(
                    local.ty,
                    CgTy::Ref
                        | CgTy::String
                        | CgTy::Bool
                        | CgTy::Float64
                        | CgTy::Float32
                        | CgTy::Int(_)
                )
            {
                outer_captures.push(CapturedLocal {
                    id,
                    hir_ty: local.hir_ty,
                    ty: local.ty,
                    mutable: local.mutable,
                });
            } else if self.env.get(id).is_none() {
                body_lift_ids.push(id);
            }
        }
        outer_captures.sort_by_key(|c| c.id.as_u32());

        let mut body_lifts: Vec<CapturedLocal> = Vec::new();
        for id in body_lift_ids {
            let Some(info) = decl_map.get(&id) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "indirect perform body lift decl",
                    at: span.into(),
                });
            };
            let decl = info.decl;
            let decl_ty =
                self.cg_ty_of(decl.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "indirect perform body lift type",
                        at: decl.span.into(),
                    })?;
            body_lifts.push(CapturedLocal {
                id,
                hir_ty: Some(decl.ty),
                ty: decl_ty,
                mutable: decl.mutable,
            });
        }
        body_lifts.sort_by_key(|c| c.id.as_u32());

        // ContState struct: { header, handler_frame, pc, outer_captures..., body_lifts... }
        let state_ty_name = format!("scoop.runtime.ContState__{func_name}_{seq}");
        let state_ty = if let Some(existing) = self.context.get_struct_type(&state_ty_name) {
            existing
        } else {
            let ty = self.context.opaque_struct_type(&state_ty_name);
            let header_ty = self.llvm_gc_object_header_type();
            let frame_ty = self.llvm_effect_handler_frame_type();
            let mut fields: Vec<BasicTypeEnum<'ctx>> = Vec::new();
            fields.push(header_ty.into()); // 0: header
            fields.push(frame_ty.into()); // 1: handler_frame
            fields.push(i32_ty.into()); // 2: pc
            for cap in &outer_captures {
                fields.push(match cap.ty {
                    CgTy::Ref | CgTy::String => gc_i8_ptr_ty.into(),
                    CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => i64_ty.into(),
                    _ => unreachable!("captures filtered by type"),
                });
            }
            for cap in &body_lifts {
                fields.push(match cap.ty {
                    CgTy::Ref | CgTy::String => gc_i8_ptr_ty.into(),
                    CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => i64_ty.into(),
                    _ => unreachable!("lifts filtered by type"),
                });
            }
            ty.set_body(&fields, false);
            ty
        };

        // Step function: void step(void* state, u64 resume_word, void* resume_gc_ref)
        let step_name = format!("__scoop_cont_step__{func_name}_{seq}");
        let step_fn_ty = self.context.void_type().fn_type(
            &[gc_i8_ptr_ty.into(), i64_ty.into(), gc_i8_ptr_ty.into()],
            false,
        );
        let step_fn = self.module.add_function(&step_name, step_fn_ty, None);
        step_fn.set_linkage(Linkage::Internal);
        step_fn.set_gc(super::super::LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE);

        let saved_block = insert_block;

        // ── Generate step function body ──
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

            // Parse step function params.
            let state_raw = step_fn
                .get_nth_param(0)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "step state param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_ptr_ty = cg.llvm_ptr_type(cg.gc_address_space());
            let state_ptr =
                cg.builder
                    .build_pointer_cast(state_raw, state_ptr_ty, "step_state_ptr")?;
            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "step resume_word param",
                    at: span.into(),
                })?
                .into_int_value();
            let resume_gc_ref = step_fn
                .get_nth_param(2)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "step resume_gc_ref param",
                    at: span.into(),
                })?
                .into_pointer_value();

            // Restore outer captures from ContState.
            let outer_field_base = 3u32;
            let body_field_base = outer_field_base.saturating_add(outer_captures.len() as u32);
            for (idx, cap) in outer_captures.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "step_capture_gep",
                )?;
                let name = format!("cap_{}", cap.id.as_u32());
                match cap.ty {
                    CgTy::Ref => {
                        let loaded = cg
                            .builder
                            .build_load(gc_i8_ptr_ty, field_ptr, "step_cap_ref")?
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
                            .build_load(gc_i8_ptr_ty, field_ptr, "step_cap_str")?
                            .into_pointer_value();
                        let str_ptr_ty = cg.llvm_scoop_string_ptr_type();
                        let casted = cg.builder.build_pointer_cast(
                            loaded,
                            str_ptr_ty,
                            "step_cap_str_cast",
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
                    CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => {
                        let loaded = cg
                            .builder
                            .build_load(i64_ty, field_ptr, "step_cap_scalar")?
                            .into_int_value();
                        let restored = cg.decode_u64_word_to_cg_value(span, loaded, cap.ty)?;
                        let ptr = cg.create_entry_alloca(span, &name, cap.ty)?;
                        let _ = cg.store_local_value(span, ptr, cap.ty, restored)?;
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
                    _ => unreachable!("captures filtered"),
                }
            }

            // Restore body lifts.
            for (idx, cap) in body_lifts.iter().enumerate() {
                let field_idx = body_field_base.saturating_add(idx as u32);
                let field_ptr =
                    cg.builder
                        .build_struct_gep(state_ty, state_ptr, field_idx, "step_lift_gep")?;
                let name = format!("lift_{}", cap.id.as_u32());
                match cap.ty {
                    CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => {
                        let loaded = cg
                            .builder
                            .build_load(i64_ty, field_ptr, "step_lift_scalar")?
                            .into_int_value();
                        let restored = cg.decode_u64_word_to_cg_value(span, loaded, cap.ty)?;
                        let ptr = cg.create_entry_alloca(span, &name, cap.ty)?;
                        let _ = cg.store_local_value(span, ptr, cap.ty, restored)?;
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
                    CgTy::Ref => {
                        let loaded = cg
                            .builder
                            .build_load(gc_i8_ptr_ty, field_ptr, "step_lift_ref")?
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
                            .build_load(gc_i8_ptr_ty, field_ptr, "step_lift_str")?
                            .into_pointer_value();
                        let str_ptr_ty = cg.llvm_scoop_string_ptr_type();
                        let casted = cg.builder.build_pointer_cast(
                            loaded,
                            str_ptr_ty,
                            "step_lift_str_cast",
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
                    _ => unreachable!("body lifts filtered by type"),
                }
            }

            // Step function body for pc=0 (indirect perform call-site):
            // 1. Read callee_suspend_state from TLS
            // 2. Write (resume_word, resume_gc_ref) into CalleeSuspendState
            // 3. Re-call the function (with default args since callee resume path ignores them)
            // 4. Continue with post-call stmts
            // 5. Unpin state + return

            // Read callee_suspend_state from TLS.
            let rt_get_callee = cg.declare_runtime_callee_suspend_state_get();
            let get_call = cg
                .builder
                .build_call(rt_get_callee, &[], "step_callee_state_get")?;
            let callee_state_raw = get_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "step callee_state_get return",
                    at: span.into(),
                })?
                .into_pointer_value();

            let callee_prefix_ty = cg.llvm_callee_suspend_state_prefix_type();
            let callee_state_ptr_ty = cg.llvm_ptr_type(AddressSpace::default());
            let callee_state_ptr = cg.builder.build_pointer_cast(
                callee_state_raw,
                callee_state_ptr_ty,
                "callee_state_typed",
            )?;
            let callee_rw_ptr = cg.builder.build_struct_gep(
                callee_prefix_ty,
                callee_state_ptr,
                1,
                "callee_resume_word_gep",
            )?;
            let _ = cg.builder.build_store(callee_rw_ptr, resume_word)?;

            let callee_rg_ptr = cg.builder.build_struct_gep(
                callee_prefix_ty,
                callee_state_ptr,
                2,
                "callee_resume_gc_ref_gep",
            )?;
            let wb = cg.declare_runtime_gc_write_barrier();
            let slot_addr =
                cg.builder
                    .build_pointer_cast(callee_rg_ptr, i8_ptr_ty, "callee_resume_gc_slot")?;
            let _ = cg.builder.build_call(
                wb,
                &[slot_addr.into(), resume_gc_ref.into()],
                "callee_resume_gc_store",
            )?;

            // Re-call the callee function.
            // The callee will detect its saved state in TLS, decode the transport payload, and return the actual result.
            let site = &indirect_sites[0];
            let call_stmt = &handle.body.stmts[site.stmt_idx];
            let hir::StmtKind::Val(call_decl) = &call_stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "indirect perform: expected val decl",
                    at: call_stmt.span.into(),
                });
            };
            let Some(call_init) = &call_decl.init else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "indirect perform: expected init",
                    at: call_decl.span.into(),
                });
            };

            // Codegen the call expression (the callee will detect resume via TLS).
            let call_result =
                cg.codegen_expr_in_expected_context(call_init, Some(call_result_cg_ty))?;

            // Bind the call result to the val decl's local.
            if let Some(id) = call_decl.id {
                let ptr = cg.create_entry_alloca(
                    call_decl.span,
                    call_decl.name.as_deref().unwrap_or("v"),
                    call_result_cg_ty,
                )?;
                let _ =
                    cg.store_local_value(call_decl.span, ptr, call_result_cg_ty, call_result)?;
                cg.env.insert(
                    id,
                    CgLocal {
                        hir_ty: Some(call_decl.ty),
                        ty: call_result_cg_ty,
                        ptr,
                        mutable: call_decl.mutable,
                    },
                );
            }

            // Execute remaining stmts after the call.
            for stmt in handle.body.stmts.iter().skip(site.stmt_idx + 1) {
                match &stmt.kind {
                    hir::StmtKind::Empty => {}
                    hir::StmtKind::Val(decl) => {
                        cg.codegen_val_decl(decl)?;
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
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "stmt in step function (indirect perform)",
                            at: stmt.span.into(),
                        });
                    }
                }
            }

            // Unpin cont_state + return.
            let unpin = cg.declare_runtime_gc_unpin();
            let _ = cg
                .builder
                .build_call(unpin, &[state_raw.into()], "step_unpin")?;
            cg.builder.build_return(None)?;

            cg.env.pop_scope();
        }

        // Restore outer insertion point.
        self.builder.position_at_end(saved_block);

        // ── Handle body: initial execution ──
        let has_finally = handle.finally.is_some();
        let handle_blocks =
            self.build_escape_handle_blocks(func, "handle_indirect", true, has_finally);
        let body_bb = handle_blocks.body_bb;
        let dispatch_bb = handle_blocks
            .dispatch_bb
            .expect("handle indirect scaffold should allocate dispatch");
        let dispatch_no_match_bb = handle_blocks
            .dispatch_nomatch_bb
            .expect("handle indirect scaffold should allocate nomatch dispatch");
        let arm_bb = handle_blocks.arm_bb;
        let done_bb = handle_blocks.done_bb;
        let finally_bb = handle_blocks.finally_bb;
        let finally_unwind_bb = handle_blocks.finally_unwind_bb;

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_indirect_result", out_ty)?)
        };

        // Continuation binder local.
        let cont_ptr =
            self.create_entry_alloca(span, &format!("handle_indirect_k_{seq}"), CgTy::Ref)?;

        // Binder slots for the arm pattern (e.g., op args).
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
                    kind: "handle indirect binder type",
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

        self.builder.build_unconditional_branch(body_bb)?;

        // ── Body ──
        self.builder.position_at_end(body_bb);
        self.env.push_scope();

        // Allocate and pin ContState.
        let total_size = self.target_data.get_store_size(&state_ty);
        let state_desc_global_name = format!("__scoop_type_desc_cont_state__{func_name}_{seq}");
        let first_capture_field_index = 3u32;
        let trace_start_offset_bytes = if outer_captures.is_empty() && body_lifts.is_empty() {
            total_size
        } else {
            self.target_data
                .offset_of_element(&state_ty, first_capture_field_index)
                .unwrap_or(total_size)
        };
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

        let pin = self.declare_runtime_gc_pin();
        let _ = self
            .builder
            .build_call(pin, &[state_raw.into()], "cont_state_pin")?;

        let state_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let state_ptr =
            self.builder
                .build_pointer_cast(state_raw, state_ptr_ty, "cont_state_ptr")?;

        // Zero-init pc and capture fields.
        {
            let pc_ptr =
                self.builder
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
                "cont_state_cap_init",
            )?;
            match cap.ty {
                CgTy::Ref | CgTy::String => {
                    let _ = self
                        .builder
                        .build_store(field_ptr, gc_i8_ptr_ty.const_null())?;
                }
                CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => {
                    let _ = self.builder.build_store(field_ptr, i64_ty.const_zero())?;
                }
                _ => unreachable!(),
            }
        }
        for (idx, cap) in body_lifts.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_ptr,
                field_idx,
                "cont_state_lift_init",
            )?;
            match cap.ty {
                CgTy::Ref | CgTy::String => {
                    let _ = self
                        .builder
                        .build_store(field_ptr, gc_i8_ptr_ty.const_null())?;
                }
                CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => {
                    let _ = self.builder.build_store(field_ptr, i64_ty.const_zero())?;
                }
                _ => unreachable!(),
            }
        }

        // Push handler frame.
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let frame_ptr =
            self.builder
                .build_struct_gep(state_ty, state_ptr, 1, "cont_state_frame_gep")?;
        let frame_i8 = self.builder.build_address_space_cast(
            frame_ptr,
            i8_ptr_ty,
            "handle_indirect_frame_i8",
        )?;
        let escape_tag = self.effect_op_tag(&arm.op.op.fqn);
        let op_tag_i32 = i32_ty.const_int(escape_tag as u64, false);
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), op_tag_i32.into()],
            "handle_indirect_push",
        )?;

        // Push raise_target → dispatch_bb so emit_effect_unwind_if_active routes there.
        let outer_raise_target = self.current_raise_target();
        self.raise_target_stack.push(dispatch_bb);

        // Execute body stmts.
        let mut body_tail: Option<CgValue<'ctx>> = None;
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            // T1606f-3: Save body lifts into ContState right before the
            // indirect perform call site. At this point, all body lift locals
            // are in scope and have their correct values. If the call triggers
            // a perform and dispatch routes to arm_bb, the body lifts will
            // already be saved in ContState for the step function to restore.
            if idx == first_site.stmt_idx && !body_lifts.is_empty() {
                for (li, cap) in body_lifts.iter().enumerate() {
                    let field_idx = body_field_base.saturating_add(li as u32);
                    let field_ptr = self.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        field_idx,
                        "body_save_lift",
                    )?;
                    let local = self
                        .env
                        .get(cap.id)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "body save lift not found",
                            at: span.into(),
                        })?;
                    match cap.ty {
                        CgTy::Int(int_ty) => {
                            let llvm_ty = self.llvm_basic_type_of(span, cap.ty)?;
                            let loaded = self
                                .builder
                                .build_load(llvm_ty, local.ptr, "body_lift_int")?
                                .into_int_value();
                            let ext = if int_ty.bits == 64 {
                                loaded
                            } else if int_ty.signed {
                                self.builder
                                    .build_int_s_extend(loaded, i64_ty, "body_lift_sext")?
                            } else {
                                self.builder
                                    .build_int_z_extend(loaded, i64_ty, "body_lift_zext")?
                            };
                            let _ = self.builder.build_store(field_ptr, ext)?;
                        }
                        CgTy::Bool => {
                            let llvm_ty = self.llvm_basic_type_of(span, CgTy::Bool)?;
                            let loaded = self
                                .builder
                                .build_load(llvm_ty, local.ptr, "body_lift_bool")?
                                .into_int_value();
                            let ext = self.builder.build_int_z_extend(
                                loaded,
                                i64_ty,
                                "body_lift_bool_zext",
                            )?;
                            let _ = self.builder.build_store(field_ptr, ext)?;
                        }
                        CgTy::Ref => {
                            let llvm_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
                            let loaded = self
                                .builder
                                .build_load(llvm_ty, local.ptr, "body_lift_ref")?
                                .into_pointer_value();
                            let casted = self.builder.build_pointer_cast(
                                loaded,
                                gc_i8_ptr_ty,
                                "body_lift_ref_i8",
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
                            let loaded = self
                                .builder
                                .build_load(llvm_ty, local.ptr, "body_lift_str")?
                                .into_pointer_value();
                            let casted = self.builder.build_pointer_cast(
                                loaded,
                                gc_i8_ptr_ty,
                                "body_lift_str_i8",
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
                        _ => unreachable!("body lifts filtered by type"),
                    }
                }
            }

            let is_last = idx + 1 == handle.body.stmts.len();
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    self.codegen_val_decl(decl)?;
                    body_tail = None;
                }
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                    body_tail = None;
                }
                hir::StmtKind::Expr(expr) => {
                    let expected = if is_last {
                        Some(out_ty)
                    } else {
                        Some(CgTy::Unit)
                    };
                    let v = self.codegen_expr_in_expected_context(expr, expected)?;
                    body_tail = if is_last { Some(v) } else { None };
                }
                hir::StmtKind::While { cond, body } => {
                    self.codegen_while_stmt(stmt.span, cond, body)?;
                    body_tail = None;
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "stmt in handle body (indirect perform)",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        // Pop raise_target.
        self.raise_target_stack.pop();

        // Body completed normally (no perform happened).
        // Pop handler frame and store body result.
        let rt_pop = self.declare_runtime_effect_handler_stack_pop();
        let _ = self
            .builder
            .build_call(rt_pop, &[frame_i8.into()], "handle_indirect_pop")?;

        // Unpin cont_state (no continuation needed).
        let unpin_fn = self.declare_runtime_gc_unpin();
        let _ = self
            .builder
            .build_call(unpin_fn, &[state_raw.into()], "cont_state_unpin_body")?;

        if out_ty != CgTy::Unit
            && let Some(v) = body_tail
        {
            let v = self.coerce_value(span, v, out_ty)?;
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(span, ptr, out_ty, v)?;
            }
        }

        self.env.pop_scope();
        self.builder.build_unconditional_branch(done_bb)?;

        // ── Dispatch (flag-based unwind after a call in the body) ──
        self.builder.position_at_end(dispatch_bb);
        {
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = self
                .builder
                .build_call(rt_read_tag, &[], "dispatch_read_op_tag")?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "dispatch read_op_tag return value",
                    at: span.into(),
                },
            )?;
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
            self.builder
                .build_conditional_branch(tag_matches, arm_bb, dispatch_no_match_bb)?;
        }

        // ── Dispatch no match ──
        self.builder.position_at_end(dispatch_no_match_bb);
        {
            let rt_pop = self.declare_runtime_effect_handler_stack_pop();
            let _ = self
                .builder
                .build_call(rt_pop, &[frame_i8.into()], "dispatch_nomatch_pop")?;
            if let Some(outer) = outer_raise_target {
                self.builder.build_unconditional_branch(outer)?;
            } else {
                let ret_ty =
                    self.current_fun_return_ty
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "dispatch no-match needs function return type",
                            at: span.into(),
                        })?;
                let v = self.default_value(span, ret_ty)?;
                self.emit_return(span, ret_ty, v)?;
            }
        }

        // ── Arm (tag matched) ──
        self.builder.position_at_end(arm_bb);
        {
            // Save captures to ContState.
            for (idx, cap) in outer_captures.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = self.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "arm_save_cap",
                )?;
                let local = self
                    .env
                    .get(cap.id)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "arm save capture not found",
                        at: span.into(),
                    })?;
                match cap.ty {
                    CgTy::Int(int_ty) => {
                        let llvm_ty = self.llvm_basic_type_of(span, cap.ty)?;
                        let loaded = self
                            .builder
                            .build_load(llvm_ty, local.ptr, "arm_cap_int")?
                            .into_int_value();
                        let ext = if int_ty.bits == 64 {
                            loaded
                        } else if int_ty.signed {
                            self.builder
                                .build_int_s_extend(loaded, i64_ty, "arm_cap_sext")?
                        } else {
                            self.builder
                                .build_int_z_extend(loaded, i64_ty, "arm_cap_zext")?
                        };
                        let _ = self.builder.build_store(field_ptr, ext)?;
                    }
                    CgTy::Bool => {
                        let llvm_ty = self.llvm_basic_type_of(span, CgTy::Bool)?;
                        let loaded = self
                            .builder
                            .build_load(llvm_ty, local.ptr, "arm_cap_bool")?
                            .into_int_value();
                        let ext =
                            self.builder
                                .build_int_z_extend(loaded, i64_ty, "arm_cap_bool_zext")?;
                        let _ = self.builder.build_store(field_ptr, ext)?;
                    }
                    CgTy::Ref => {
                        let llvm_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
                        let loaded = self
                            .builder
                            .build_load(llvm_ty, local.ptr, "arm_cap_ref")?
                            .into_pointer_value();
                        let casted = self.builder.build_pointer_cast(
                            loaded,
                            gc_i8_ptr_ty,
                            "arm_cap_ref_i8",
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
                        let loaded = self
                            .builder
                            .build_load(llvm_ty, local.ptr, "arm_cap_str")?
                            .into_pointer_value();
                        let casted = self.builder.build_pointer_cast(
                            loaded,
                            gc_i8_ptr_ty,
                            "arm_cap_str_i8",
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
                    _ => unreachable!(),
                }
            }

            // Note: body lifts are saved before the call site in the body
            // execution loop (T1606f-3), so they are already in ContState here.

            // Set pc = 0.
            {
                let pc_ptr = self
                    .builder
                    .build_struct_gep(state_ty, state_ptr, 2, "arm_pc_gep")?;
                let _ = self.builder.build_store(pc_ptr, i32_ty.const_zero())?;
            }

            // Read binder values from perform slot (for arm pattern binders).
            for (slot_idx, slot) in binder_slots.iter().enumerate() {
                if slot_idx != 0 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle indirect binder count (only 1 supported)",
                        at: arm.op.span.into(),
                    });
                }
                let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
                let word_call = self
                    .builder
                    .build_call(rt_read, &[], "arm_read_binder_word")?;
                let word_raw = word_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "arm read binder return",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(word_u64) = word_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "arm read binder type",
                        at: span.into(),
                    });
                };
                let rt_read_gc = self.declare_runtime_effect_perform_slot_read_gc_ref();
                let gc_call = self
                    .builder
                    .build_call(rt_read_gc, &[], "arm_read_binder_gc")?;
                let gc_raw = gc_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "arm read binder gc value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(gc_ref_raw) = gc_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "arm read binder gc type",
                        at: span.into(),
                    });
                };
                let binder_value =
                    self.decode_abi_payload_transport(span, word_u64, gc_ref_raw, slot.ty)?;
                let _ = self.store_local_value(span, slot.ptr, slot.ty, binder_value)?;
            }

            // Clear effect active flag (the dispatch caught it).
            let rt_clear = self.declare_runtime_effect_clear();
            let _ = self.builder.build_call(rt_clear, &[], "arm_effect_clear")?;

            // Create continuation.
            let rt_cont_alloc = self.declare_runtime_continuation_alloc();
            let step_ptr = step_fn.as_global_value().as_pointer_value();
            let cont_call = self.builder.build_call(
                rt_cont_alloc,
                &[state_raw.into(), step_ptr.into()],
                "arm_cont_alloc",
            )?;
            let k_raw = cont_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "continuation alloc return",
                    at: span.into(),
                })?
                .into_pointer_value();

            // Pin continuation.
            let _ = self.builder.build_call(pin, &[k_raw.into()], "arm_k_pin")?;
            let _ = self.store_local_value(
                span,
                cont_ptr,
                CgTy::Ref,
                CgValue {
                    ty: CgTy::Ref,
                    value: Some(k_raw.into()),
                },
            )?;

            // Detach handler frame from TLS handler stack.
            let handler_frame_ty = self.llvm_effect_handler_frame_type();
            let frame_ptr_for_detach =
                self.builder
                    .build_struct_gep(state_ty, state_ptr, 1, "arm_frame_gep")?;
            let prev_ptr = self.builder.build_struct_gep(
                handler_frame_ty,
                frame_ptr_for_detach,
                0,
                "arm_prev_gep",
            )?;
            let prev_raw = self.builder.build_load(i8_ptr_ty, prev_ptr, "arm_prev")?;
            let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
            let _ = self
                .builder
                .build_call(rt_swap, &[prev_raw.into()], "arm_detach")?;

            // Execute arm body.
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

            // T1609: arm body 若发生 Raise，先执行 finally 再向外传播。
            if let Some(fu_bb) = finally_unwind_bb {
                self.push_raise_target(fu_bb);
            }
            let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
            if finally_unwind_bb.is_some() {
                self.pop_raise_target();
            }
            let arm_v = if out_ty == CgTy::Unit {
                CgValue::unit()
            } else {
                self.coerce_value(arm.body.span, arm_v, out_ty)?
            };

            // arm 正常完成：保存结果，跳到 finally（若有）或 done。
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                if let Some(ptr) = result_ptr {
                    let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
                }
                let target = finally_bb.unwrap_or(done_bb);
                self.builder.build_unconditional_branch(target)?;
            }

            self.env.pop_scope();
        }

        // --- finally_unwind (T1609) ---
        if let Some(fu_bb) = finally_unwind_bb {
            self.builder.position_at_end(fu_bb);
            if let Some(finally) = handle.finally.as_ref() {
                let _ = self.codegen_block_value(finally)?;
            }
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                if let Some(target) = outer_raise_target {
                    self.builder.build_unconditional_branch(target)?;
                } else {
                    let ret_ty =
                        self.current_fun_return_ty
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle indirect finally unwind needs function return type",
                                at: span.into(),
                            })?;
                    let v = self.default_value(span, ret_ty)?;
                    self.emit_return(span, ret_ty, v)?;
                }
            }
        }

        // --- finally (T1609) ---
        if let Some(f_bb) = finally_bb {
            self.builder.position_at_end(f_bb);
            if let Some(finally) = handle.finally.as_ref() {
                let _ = self.codegen_block_value(finally)?;
            }
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                self.builder.build_unconditional_branch(done_bb)?;
            }
        }

        // ── Done ──
        self.builder.position_at_end(done_bb);
        let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
        let k_loaded = self
            .builder
            .build_load(llvm_ref_ty, cont_ptr, "handle_indirect_k_unpin_load")?
            .into_pointer_value();
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self
            .builder
            .build_call(unpin, &[k_loaded.into()], "handle_indirect_k_unpin")?;

        Ok(match out_ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Never => CgValue::never(),
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
                        kind: "handle indirect result slot",
                        at: span.into(),
                    });
                };
                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, ptr, "handle_indirect_result")?;
                CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                }
            }
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
        let payload = self.encode_abi_payload_transport(value_expr.span, value)?;

        // T1607：获取 continuation 结构体类型，GEP 到 resume_word / resume_gc_ref 槽位。
        let cont_ty = self.llvm_continuation_struct_type();
        let cont_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let cont_ptr =
            self.builder
                .build_pointer_cast(k_ptr, cont_ptr_ty, "cont_resume_k_typed")?;
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let word_ptr =
            self.builder
                .build_struct_gep(cont_ty, cont_ptr, 6, "cont_resume_word_gep")?;
        let _ = self.builder.build_store(word_ptr, payload.word)?;

        let ref_ptr =
            self.builder
                .build_struct_gep(cont_ty, cont_ptr, 7, "cont_resume_gc_ref_gep")?;
        if let Some(gc_ref) = payload.gc_ref {
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
            let _ = self.builder.build_call(
                wb,
                &[slot_addr.into(), gc_ref.into()],
                "cont_resume_wb",
            )?;
        } else {
            let _ = self
                .builder
                .build_store(ref_ptr, gc_i8_ptr_ty.const_null())?;
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

}
