impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn codegen_handle_expr_escape_with_nonresuming_siblings<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        escape: (&'hir hir::HandleArm, hir::SymbolId),
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (escape_arm, _) = escape;
        let direct_sites = self.scan_mixed_escape_direct_sites(handle, &escape_arm.op.op.fqn)?;
        let indirect_sites = self.scan_mixed_escape_indirect_sites(handle)?;

        if !direct_sites.is_empty() && !indirect_sites.is_empty() {
            let direct_supported = direct_sites.iter().all(|site| {
                site.resume_path.is_empty()
                    || Self::mixed_escape_block_only_path_supported(&site.resume_path)
                    || Self::mixed_escape_if_branch_path_supported(&site.resume_path)
                    || Self::mixed_escape_while_nested_path_supported(&site.resume_path)
            });
            let indirect_supported = indirect_sites.iter().all(|site| {
                site.resume_path.is_empty()
                    || Self::mixed_escape_block_only_path_supported(&site.resume_path)
                    || Self::mixed_escape_if_branch_path_supported(&site.resume_path)
                    || Self::mixed_escape_while_nested_path_supported(&site.resume_path)
            });
            if direct_supported && indirect_supported {
                return self.codegen_handle_expr_escape_with_nonresuming_siblings_top_level_mixed(
                    span,
                    handle,
                    escape,
                    sibling_nonresuming_arms,
                    out_ty,
                );
            }
        }

        if !direct_sites.is_empty() && indirect_sites.is_empty() {
            return self.codegen_handle_expr_escape_with_nonresuming_siblings_direct(
                span,
                handle,
                escape,
                sibling_nonresuming_arms,
                out_ty,
            );
        }

        if direct_sites.is_empty() && !indirect_sites.is_empty() {
            if indirect_sites
                .iter()
                .all(|site| site.resume_path.is_empty())
            {
                if indirect_sites.len() == 1 {
                    return self.codegen_handle_expr_escape_with_nonresuming_siblings_indirect(
                        span,
                        handle,
                        escape,
                        sibling_nonresuming_arms,
                        out_ty,
                    );
                }
                return self.codegen_handle_expr_escape_with_nonresuming_siblings_indirect_multi(
                    span,
                    handle,
                    escape,
                    sibling_nonresuming_arms,
                    out_ty,
                );
            }

            if indirect_sites.iter().all(|site| {
                site.resume_path.is_empty()
                    || Self::mixed_escape_block_only_path_supported(&site.resume_path)
                    || Self::mixed_escape_if_branch_path_supported(&site.resume_path)
                    || Self::mixed_escape_while_nested_path_supported(&site.resume_path)
            }) {
                return self.codegen_handle_expr_escape_with_nonresuming_siblings_indirect_multi(
                    span,
                    handle,
                    escape,
                    sibling_nonresuming_arms,
                    out_ty,
                );
            }
        }

        let at = direct_sites
            .first()
            .map(|site| site.decl.span)
            .or_else(|| indirect_sites.first().map(|site| site.decl.span))
            .unwrap_or(span);
        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle multi-arm without immediate-resume (escape site matrix not yet supported)",
            at: at.into(),
        })
    }

    fn codegen_handle_expr_escape_with_nonresuming_siblings_top_level_mixed<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        escape: (&'hir hir::HandleArm, hir::SymbolId),
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (escape_arm, continuation_symbol) = escape;
        let sibling_plan = self.collect_sibling_nonresuming_plan(sibling_nonresuming_arms)?;
        let raise_sibling = sibling_plan.raise_arm;
        let custom_siblings = sibling_plan.custom_arms.clone();
        let has_sibling_nonresuming = sibling_plan.has_any();

        let mut direct_sites =
            self.scan_mixed_escape_direct_sites(handle, &escape_arm.op.op.fqn)?;
        let mut indirect_sites = self.scan_mixed_escape_indirect_sites(handle)?;
        if direct_sites.is_empty()
            || indirect_sites.is_empty()
            || direct_sites.iter().any(|site| {
                !site.resume_path.is_empty()
                    && !Self::mixed_escape_block_only_path_supported(&site.resume_path)
                    && !Self::mixed_escape_if_branch_path_supported(&site.resume_path)
                    && !Self::mixed_escape_while_nested_path_supported(&site.resume_path)
            })
            || indirect_sites.iter().any(|site| {
                !site.resume_path.is_empty()
                    && !Self::mixed_escape_block_only_path_supported(&site.resume_path)
                    && !Self::mixed_escape_if_branch_path_supported(&site.resume_path)
                    && !Self::mixed_escape_while_nested_path_supported(&site.resume_path)
            })
        {
            let at = direct_sites
                .first()
                .map(|site| site.decl.span)
                .or_else(|| indirect_sites.first().map(|site| site.decl.span))
                .unwrap_or(span);
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multi-arm without immediate-resume (only top-level direct+indirect mixed sites, statement-position nested block direct / indirect coexistence, if-branch direct / indirect coexistence, or while-body same-stmt direct / indirect coexistence supported)",
                at: at.into(),
            });
        }
        direct_sites.sort_by_key(|site| (site.top_level_stmt_idx, site.decl.span.start));
        indirect_sites.sort_by_key(|site| (site.top_level_stmt_idx, site.decl.span.start));

        if escape_arm.op.binders.len() > 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multi-arm without immediate-resume escape binder count (indirect, only 1 supported)",
                at: escape_arm.op.span.into(),
            });
        }
        for site in &direct_sites {
            if escape_arm.op.binders.len() != site.args.len() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape binder arity mismatch",
                    at: escape_arm.op.span.into(),
                });
            }
        }

        let mut escape_sites: Vec<MatrixEscapeSite<'hir>> = Vec::new();
        for site in &direct_sites {
            escape_sites.push(MatrixEscapeSite {
                stmt_idx: site.top_level_stmt_idx,
                decl: site.decl,
                id: site.id,
                kind: MatrixEscapeSiteKind::Direct { site: site.clone() },
            });
        }
        for site in &indirect_sites {
            escape_sites.push(MatrixEscapeSite {
                stmt_idx: site.top_level_stmt_idx,
                decl: site.decl,
                id: site.id,
                kind: MatrixEscapeSiteKind::Indirect { site: site.clone() },
            });
        }
        escape_sites.sort_by_key(|site| (site.stmt_idx, site.decl.span.start));

        let first_site = escape_sites
            .first()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multi-arm without immediate-resume (top-level direct+indirect mixed site missing)",
                at: span.into(),
            })?;

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
                    continue;
                }
                if let MatrixEscapeSiteKind::Indirect { site } = &escape_sites[pc].kind
                    && matches!(
                        site.resume_path.first(),
                        Some(
                            MixedEscapeDirectFrame::IfThen { .. }
                                | MixedEscapeDirectFrame::IfElse { .. }
                        )
                    )
                {
                    if_indirect_sites.push(pc);
                    continue;
                }
                if let MatrixEscapeSiteKind::Direct { site } = &escape_sites[pc].kind
                    && matches!(
                        site.resume_path.first(),
                        Some(MixedEscapeDirectFrame::WhileBody { .. })
                    )
                {
                    while_direct_sites.push(pc);
                    continue;
                }
                if let MatrixEscapeSiteKind::Indirect { site } = &escape_sites[pc].kind
                    && matches!(
                        site.resume_path.first(),
                        Some(MixedEscapeDirectFrame::WhileBody { .. })
                    )
                {
                    while_indirect_sites.push(pc);
                    continue;
                }

                let resume_path = Self::mixed_escape_matrix_site_resume_path(&escape_sites[pc]);
                if Self::mixed_escape_block_only_path_supported(resume_path) {
                    block_sites.push(pc);
                    continue;
                }

                if !resume_path.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-arm without immediate-resume (only top-level direct+indirect mixed sites, statement-position nested block direct / indirect coexistence, if-branch direct / indirect coexistence, or while-body same-stmt direct / indirect coexistence supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
            }

            if !if_direct_sites.is_empty() || !if_indirect_sites.is_empty() {
                if !while_direct_sites.is_empty()
                    || !while_indirect_sites.is_empty()
                    || !block_sites.is_empty()
                {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-arm without immediate-resume (multiple sites per top-level statement not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }

                if !if_direct_sites.is_empty() && !if_indirect_sites.is_empty() {
                    let mut then_direct_pc: Option<usize> = None;
                    let mut then_indirect_pc: Option<usize> = None;
                    let mut else_direct_pc: Option<usize> = None;
                    let mut else_indirect_pc: Option<usize> = None;

                    for &pc in &if_direct_sites {
                        let MatrixEscapeSiteKind::Direct { site } = &escape_sites[pc].kind else {
                            unreachable!("classified if-direct site");
                        };
                        match site.resume_path.first() {
                            Some(MixedEscapeDirectFrame::IfThen { .. }) => {
                                if then_direct_pc.replace(pc).is_some() {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle multi-arm without immediate-resume (multiple direct sites in the same if-then branch not yet supported)",
                                        at: site.decl.span.into(),
                                    });
                                }
                            }
                            Some(MixedEscapeDirectFrame::IfElse { .. }) => {
                                if else_direct_pc.replace(pc).is_some() {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle multi-arm without immediate-resume (multiple direct sites in the same if-else branch not yet supported)",
                                        at: site.decl.span.into(),
                                    });
                                }
                            }
                            _ => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle multi-arm without immediate-resume (expected if branch site)",
                                    at: site.decl.span.into(),
                                });
                            }
                        }
                    }

                    for &pc in &if_indirect_sites {
                        let MatrixEscapeSiteKind::Indirect { site } = &escape_sites[pc].kind else {
                            unreachable!("classified if-indirect site");
                        };
                        match site.resume_path.first() {
                            Some(MixedEscapeDirectFrame::IfThen { .. }) => {
                                if then_indirect_pc.replace(pc).is_some() {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle multi-arm without immediate-resume (multiple indirect sites in the same if-then branch not yet supported)",
                                        at: site.decl.span.into(),
                                    });
                                }
                            }
                            Some(MixedEscapeDirectFrame::IfElse { .. }) => {
                                if else_indirect_pc.replace(pc).is_some() {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle multi-arm without immediate-resume (multiple indirect sites in the same if-else branch not yet supported)",
                                        at: site.decl.span.into(),
                                    });
                                }
                            }
                            _ => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle multi-arm without immediate-resume (expected if branch site)",
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
                            unreachable!("stored if-direct site");
                        };
                        let MatrixEscapeSiteKind::Indirect { site: indirect_site } =
                            &escape_sites[indirect_pc].kind
                        else {
                            unreachable!("stored if-indirect site");
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
                                    kind: "handle multi-arm without immediate-resume (if mixed site order ambiguous)",
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

                if !if_direct_sites.is_empty() {
                    if_direct_site_pcs_by_stmt_idx.insert(*stmt_idx, if_direct_sites);
                    continue;
                }
                if !if_indirect_sites.is_empty() {
                    if_indirect_site_pcs_by_stmt_idx.insert(*stmt_idx, if_indirect_sites);
                    continue;
                }
            }

            if !block_sites.is_empty() {
                if block_sites.len() != site_pcs.len()
                    || !while_direct_sites.is_empty()
                    || !while_indirect_sites.is_empty()
                {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-arm without immediate-resume (multiple sites per top-level statement not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
                if block_sites.len() > 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-arm without immediate-resume (multiple nested block mixed sites per top-level statement not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
                let direct_count = block_sites
                    .iter()
                    .filter(|&&pc| matches!(escape_sites[pc].kind, MatrixEscapeSiteKind::Direct { .. }))
                    .count();
                let indirect_count = block_sites
                    .iter()
                    .filter(|&&pc| matches!(escape_sites[pc].kind, MatrixEscapeSiteKind::Indirect { .. }))
                    .count();
                if block_sites.len() == 2 && (direct_count != 1 || indirect_count != 1) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-arm without immediate-resume (multiple nested block mixed sites per top-level statement not yet supported)",
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

            if !while_direct_sites.is_empty() || !while_indirect_sites.is_empty() {
                if site_pcs.len() != 2
                    || while_direct_sites.len() != 1
                    || while_indirect_sites.len() != 1
                {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-arm without immediate-resume (only same-body-stmt direct / indirect coexistence in while body supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }

                let direct_pc = while_direct_sites[0];
                let indirect_pc = while_indirect_sites[0];
                let MatrixEscapeSiteKind::Direct { site: direct_site } =
                    &escape_sites[direct_pc].kind
                else {
                    unreachable!("classified while-direct site");
                };
                let MatrixEscapeSiteKind::Indirect { site: indirect_site } =
                    &escape_sites[indirect_pc].kind
                else {
                    unreachable!("classified while-indirect site");
                };
                if !Self::mixed_escape_while_same_stmt_mixed_path_supported(
                    &direct_site.resume_path,
                    &indirect_site.resume_path,
                ) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-arm without immediate-resume (only same-body-stmt direct / indirect coexistence in while body supported)",
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
                            kind: "handle multi-arm without immediate-resume (while mixed site order ambiguous)",
                            at: direct_site.decl.span.into(),
                        });
                    }
                }
                let mut mixed_sites = while_direct_sites.clone();
                mixed_sites.extend(while_indirect_sites.iter().copied());
                while_mixed_site_pcs_by_stmt_idx.insert(*stmt_idx, mixed_sites);
                continue;
            }

            if site_pcs.len() > 1 {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multi-arm without immediate-resume (multiple sites per top-level statement not yet supported)",
                    at: handle.body.stmts[*stmt_idx].span.into(),
                });
            }
            simple_escape_site_pc_by_stmt_idx.insert(*stmt_idx, site_pcs[0]);
        }

        let escape_resume_value_ty =
            self.cg_ty_of(first_site.decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape perform value type",
                    at: first_site.decl.span.into(),
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
                MatrixEscapeSiteKind::Indirect { site: indirect_site } => {
                    Self::collect_mixed_escape_used_after_indirect_site(
                        indirect_site,
                        &handle.body.stmts,
                        &mut used_after,
                    );
                    if let Some(&prev_pc) = if_prev_site_pc_by_pc.get(&site_pc) {
                        let MatrixEscapeSiteKind::Direct {
                            site: prev_direct_site,
                        } = &escape_sites[prev_pc].kind
                        else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-arm without immediate-resume (expected previous direct site)",
                                at: site.decl.span.into(),
                            });
                        };
                        Self::collect_mixed_escape_used_between_if_sites(
                            prev_direct_site,
                            indirect_site,
                            &mut used_after,
                        )?;
                    } else if let Some(&prev_pc) = block_prev_site_pc_by_pc.get(&site_pc) {
                        let MatrixEscapeSiteKind::Direct {
                            site: prev_direct_site,
                        } = &escape_sites[prev_pc].kind
                        else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-arm without immediate-resume (expected previous direct site)",
                                at: site.decl.span.into(),
                            });
                        };
                        Self::collect_mixed_escape_used_between_block_sites(
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
                                kind: "handle multi-arm without immediate-resume (expected previous direct site)",
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
                    at: first_site.decl.span.into(),
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

        let state_ty_name =
            format!("scoop.runtime.MultiEscapeNoImmediateTopLevelMixedState__{func_name}_{seq}");
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

        let step_name =
            format!("__scoop_multi_escape_no_immediate_top_level_mixed_step__{func_name}_{seq}");
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
                    kind: "multi escape no-immediate top-level mixed step state param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_ptr_ty = cg.llvm_ptr_type(cg.gc_address_space());
            let state_ptr = cg.builder.build_pointer_cast(
                state_raw,
                state_ptr_ty,
                "multi_escape_no_immediate_top_level_mixed_step_state_ptr",
            )?;

            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate top-level mixed step resume_word param",
                    at: span.into(),
                })?
                .into_int_value();
            let resume_gc_ref = step_fn
                .get_nth_param(2)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate top-level mixed step resume_gc_ref param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_pc_ptr = cg.builder.build_struct_gep(
                state_ty,
                state_ptr,
                2,
                "multi_escape_no_immediate_top_level_mixed_step_pc_gep",
            )?;

            for (idx, cap) in outer_visible_supported.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "multi_escape_no_immediate_top_level_mixed_step_outer_gep",
                )?;
                let name =
                    format!("multi_escape_no_immediate_top_level_mixed_outer_{}", cap.id.as_u32());
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
                    "multi_escape_no_immediate_top_level_mixed_step_body_gep",
                )?;
                let name =
                    format!("multi_escape_no_immediate_top_level_mixed_body_{}", cap.id.as_u32());
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
                let binder_ty =
                    cg.cg_ty_of(binder.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape binder type",
                            at: binder.span.into(),
                        })?;
                let ptr = cg.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
                step_escape_binder_slots.push(ImmediateResumeBinderSlot {
                    id: binder.id,
                    hir_ty: binder.ty,
                    ty: binder_ty,
                    ptr,
                });
            }
            let step_cont_ptr = cg.create_entry_alloca(
                span,
                &format!("handle_multi_escape_no_immediate_top_level_mixed_step_k_{seq}"),
                CgTy::Ref,
            )?;
            let step_escape_binder_from_slot_ptr = cg.create_entry_alloca_raw(
                span,
                "handle_multi_escape_no_immediate_top_level_mixed_step_binder_from_slot",
                cg.context.bool_type().into(),
            )?;
            let _ = cg.builder.build_store(
                step_escape_binder_from_slot_ptr,
                cg.context.bool_type().const_zero(),
            )?;

            let step_sibling_dispatch = cg.build_sibling_nonresuming_dispatch_blocks(
                step_fn,
                "multi_escape_no_immediate_top_level_mixed_step",
                &sibling_plan,
            );
            let step_effect_dispatch_bb = step_sibling_dispatch.effect_dispatch_bb;
            let step_effect_dispatch_nomatch_bb =
                step_sibling_dispatch.effect_dispatch_nomatch_bb;
            let step_raise_catch_bb = step_sibling_dispatch.raise_catch_bb;
            let step_custom_catch_bbs = step_sibling_dispatch.custom_catch_bbs;
            let step_escape_dispatch_bb = self.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_top_level_mixed_step_escape_dispatch",
            );
            let step_escape_fallback_bb = self.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_top_level_mixed_step_escape_fallback",
            );
            let step_escape_arm_bb = self.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_top_level_mixed_step_escape_arm",
            );
            let step_escape_arm_slot_bb = self.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_top_level_mixed_step_arm_from_slot",
            );
            let step_escape_arm_unwind_bb = if has_sibling_nonresuming {
                Some(self.context.append_basic_block(
                    step_fn,
                    "multi_escape_no_immediate_top_level_mixed_step_escape_arm_unwind",
                ))
            } else {
                None
            };
            let dispatch_bb = self.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_top_level_mixed_step_dispatch",
            );
            let bad_state_bb = self.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_top_level_mixed_step_bad_pc",
            );
            let mut state_bbs = Vec::new();
            for pc in 0..escape_sites.len() {
                state_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_escape_no_immediate_top_level_mixed_step_pc_{pc}"),
                ));
            }

            cg.builder.build_unconditional_branch(dispatch_bb)?;

            cg.builder.position_at_end(dispatch_bb);
            let pc = cg
                .builder
                .build_load(
                    i32_ty,
                    state_pc_ptr,
                    "multi_escape_no_immediate_top_level_mixed_step_pc",
                )?
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
                                    kind: "handle escape perform value type mismatch",
                                    at: site.decl.span.into(),
                                });
                            }
                            local.ptr
                        } else {
                            let local_name = site.decl.name.as_deref().unwrap_or("resume_value");
                            let ptr = cg.create_entry_alloca(
                                site.decl.span,
                                local_name,
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
                        let _ = cg.store_local_value(
                            site.decl.span,
                            target_ptr,
                            escape_resume_value_ty,
                            resume_value,
                        )?;
                    }
                    MatrixEscapeSiteKind::Indirect { site: indirect_site } => {
                        let if_prev_pc = if_prev_site_pc_by_pc.get(&site_pc).copied();
                        let block_prev_pc = block_prev_site_pc_by_pc.get(&site_pc).copied();
                        let while_prev_pc = while_prev_site_pc_by_pc.get(&site_pc).copied();
                        if let Some(prev_pc) = if_prev_pc.or(block_prev_pc).or(while_prev_pc) {
                            let MatrixEscapeSiteKind::Direct {
                                site: prev_direct_site,
                            } = &escape_sites[prev_pc].kind
                            else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle multi-arm without immediate-resume (expected previous direct site)",
                                    at: site.decl.span.into(),
                                });
                            };
                            let mut unexpected_direct = |_cg: &mut Self,
                                                         _next_pc: usize,
                                                         _direct_site: &MixedEscapeDirectSite<
                                'hir,
                            >| {
                                Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle multi-arm without immediate-resume (unexpected direct site while reconstructing indirect prefix)",
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
                            "multi_escape_no_immediate_top_level_mixed_step_callee_state_get",
                        )?;
                        let callee_state_raw = get_call
                            .try_as_basic_value()
                            .basic()
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "multi escape no-immediate top-level mixed step callee_state_get return",
                                at: span.into(),
                            })?
                            .into_pointer_value();
                        let callee_prefix_ty = cg.llvm_callee_suspend_state_prefix_type();
                        let callee_state_ptr_ty = cg.llvm_ptr_type(AddressSpace::default());
                        let callee_state_ptr = cg.builder.build_pointer_cast(
                            callee_state_raw,
                            callee_state_ptr_ty,
                            "multi_escape_no_immediate_top_level_mixed_step_callee_state_typed",
                        )?;
                        let callee_rw_ptr = cg.builder.build_struct_gep(
                            callee_prefix_ty,
                            callee_state_ptr,
                            1,
                            "multi_escape_no_immediate_top_level_mixed_step_resume_word_gep",
                        )?;
                        let _ = cg.builder.build_store(callee_rw_ptr, resume_word)?;
                        let callee_rg_ptr = cg.builder.build_struct_gep(
                            callee_prefix_ty,
                            callee_state_ptr,
                            2,
                            "multi_escape_no_immediate_top_level_mixed_step_resume_gc_ref_gep",
                        )?;
                        let wb = cg.declare_runtime_gc_write_barrier();
                        let slot_addr = cg.builder.build_pointer_cast(
                            callee_rg_ptr,
                            i8_ptr_ty,
                            "multi_escape_no_immediate_top_level_mixed_step_resume_gc_slot",
                        )?;
                        let _ = cg.builder.build_call(
                            wb,
                            &[slot_addr.into(), resume_gc_ref.into()],
                            "multi_escape_no_immediate_top_level_mixed_step_resume_gc_store",
                        )?;

                        let call_result = cg.codegen_expr_in_expected_context(
                            indirect_site.init,
                            Some(escape_resume_value_ty),
                        )?;
                        let target_ptr = if let Some(local) = cg.env.get(site.id) {
                            if local.ty != escape_resume_value_ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle escape perform value type mismatch",
                                    at: site.decl.span.into(),
                                });
                            }
                            local.ptr
                        } else {
                            let name =
                                site.decl.name.as_deref().unwrap_or("indirect_result");
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
                        let _ = cg.store_local_value(
                            site.decl.span,
                            target_ptr,
                            escape_resume_value_ty,
                            call_result,
                        )?;
                    }
                }

                let emit_step_direct_site =
                    |cg: &mut Self,
                     next_pc: usize,
                     next_direct_site: &MixedEscapeDirectSite<'hir>,
                     scopes_to_pop: usize| {
                        cg.capture_escape_state_with_pc(
                            next_direct_site.decl.span,
                            state_ty,
                            state_ptr,
                            &outer_visible_supported,
                            outer_field_base,
                            &body_visible_supported,
                            body_field_base,
                            2,
                            next_pc,
                        )?;
                        let _ = cg.builder.build_store(
                            step_escape_binder_from_slot_ptr,
                            cg.context.bool_type().const_zero(),
                        )?;
                        for (slot, arg) in
                            step_escape_binder_slots.iter().zip(next_direct_site.args.iter())
                        {
                            let hir::CallArg::Positional(expr) = arg else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape named perform arg",
                                    at: span.into(),
                                });
                            };
                            let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                            let _ = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                        }
                        for _ in 0..scopes_to_pop {
                            cg.env.pop_scope();
                        }
                        cg.builder.build_unconditional_branch(step_escape_arm_bb)?;
                        Ok(())
                    };
                let emit_step_indirect_site =
                    |cg: &mut Self,
                     next_pc: usize,
                     next_indirect_site: &MixedEscapeIndirectSite<'hir>| {
                        cg.capture_escape_state_with_pc(
                            next_indirect_site.decl.span,
                            state_ty,
                            state_ptr,
                            &outer_visible_supported,
                            outer_field_base,
                            &body_visible_supported,
                            body_field_base,
                            2,
                            next_pc,
                        )?;
                        if step_effect_dispatch_bb.is_some() {
                            cg.pop_raise_target();
                        }
                        cg.push_raise_target(step_escape_dispatch_bb);
                        cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                            next_indirect_site,
                            &body_lift_ids,
                        )?;
                        cg.pop_raise_target();
                        if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                            cg.push_raise_target(step_effect_dispatch_bb);
                        }
                        Ok(())
                    };

                let mut terminated = false;
                let if_next_pc = if_next_site_pc_by_pc.get(&site_pc).copied();
                let block_next_pc = block_next_site_pc_by_pc.get(&site_pc).copied();
                let while_next_pc = while_next_site_pc_by_pc.get(&site_pc).copied();
                let while_prev_pc = while_prev_site_pc_by_pc.get(&site_pc).copied();
                if let Some(next_pc) = if_next_pc.or(block_next_pc).or(while_next_pc) {
                    let next_site = &escape_sites[next_pc];
                    match &site.kind {
                        MatrixEscapeSiteKind::Direct { site: direct_site } => {
                            if if_next_pc.is_some() {
                                let mut unexpected_direct = |_cg: &mut Self,
                                                             _next_pc: usize,
                                                             _next_direct: &MixedEscapeDirectSite<'hir>| {
                                    Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle multi-arm without immediate-resume (unexpected direct site while continuing mixed if)",
                                        at: site.decl.span.into(),
                                    })
                                };
                                cg.codegen_mixed_escape_matrix_continue_to_next_if_site_after_direct(
                                    direct_site,
                                    next_pc,
                                    next_site,
                                    &body_lift_ids,
                                    &mut unexpected_direct,
                                    &mut |cg, next_pc, next_indirect_site| {
                                        emit_step_indirect_site(
                                            cg,
                                            next_pc,
                                            next_indirect_site,
                                        )
                                    },
                                )?;
                            } else if while_next_pc.is_some() {
                                let mut unexpected_direct = |_cg: &mut Self,
                                                             _next_pc: usize,
                                                             _next_direct: &MixedEscapeDirectSite<'hir>| {
                                    Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle multi-arm without immediate-resume (unexpected direct site while continuing mixed while)",
                                        at: site.decl.span.into(),
                                    })
                                };
                                cg.codegen_mixed_escape_matrix_continue_to_next_while_site_after_direct(
                                    direct_site,
                                    next_pc,
                                    next_site,
                                    &body_lift_ids,
                                    &mut unexpected_direct,
                                    &mut |cg, next_pc, next_indirect_site| {
                                        emit_step_indirect_site(
                                            cg,
                                            next_pc,
                                            next_indirect_site,
                                        )
                                    },
                                )?;
                            } else {
                                let mut unexpected_direct = |_cg: &mut Self,
                                                             _next_pc: usize,
                                                             _next_direct: &MixedEscapeDirectSite<'hir>| {
                                    Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle multi-arm without immediate-resume (unexpected direct site while continuing mixed block)",
                                        at: site.decl.span.into(),
                                    })
                                };
                                cg.codegen_mixed_escape_matrix_continue_to_next_block_site_after_direct(
                                    direct_site,
                                    next_pc,
                                    next_site,
                                    &body_lift_ids,
                                    &mut unexpected_direct,
                                    &mut |cg, next_pc, next_indirect_site| {
                                        emit_step_indirect_site(
                                            cg,
                                            next_pc,
                                            next_indirect_site,
                                        )
                                    },
                                )?;
                            }
                            if let MatrixEscapeSiteKind::Indirect {
                                site: next_indirect_site,
                            } = &next_site.kind
                                && let Some(bb) = cg.builder.get_insert_block()
                                && bb.get_terminator().is_none()
                            {
                                if while_next_pc.is_some() {
                                    cg.codegen_mixed_escape_matrix_while_tail_after_mixed_indirect_site(
                                        next_indirect_site,
                                        site_pc,
                                        direct_site,
                                        &body_lift_ids,
                                        |cg, reenter_pc, future_direct_site| {
                                            emit_step_direct_site(
                                                cg,
                                                reenter_pc,
                                                future_direct_site,
                                                0,
                                            )
                                        },
                                    )?;
                                } else {
                                    cg.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                        next_indirect_site,
                                        &body_lift_ids,
                                    )?;
                                }
                            }
                        }
                        MatrixEscapeSiteKind::Indirect { site: indirect_site } => {
                            if if_next_pc.is_some() {
                                let mut unexpected_indirect = |_cg: &mut Self,
                                                               _next_pc: usize,
                                                               _next_indirect: &MixedEscapeIndirectSite<
                                    'hir,
                                >| {
                                    Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle multi-arm without immediate-resume (unexpected indirect site while continuing mixed if)",
                                        at: site.decl.span.into(),
                                    })
                                };
                                cg.codegen_mixed_escape_matrix_continue_to_next_if_site_after_indirect(
                                    indirect_site,
                                    next_pc,
                                    next_site,
                                    &body_lift_ids,
                                    &mut |cg, next_pc, next_direct_site| {
                                        emit_step_direct_site(cg, next_pc, next_direct_site, 0)
                                    },
                                    &mut unexpected_indirect,
                                )?;
                            } else if while_next_pc.is_some() {
                                let mut unexpected_indirect = |_cg: &mut Self,
                                                               _next_pc: usize,
                                                               _next_indirect: &MixedEscapeIndirectSite<
                                    'hir,
                                >| {
                                    Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle multi-arm without immediate-resume (unexpected indirect site while continuing mixed while)",
                                        at: site.decl.span.into(),
                                    })
                                };
                                cg.codegen_mixed_escape_matrix_continue_to_next_while_site_after_indirect(
                                    indirect_site,
                                    next_pc,
                                    next_site,
                                    &body_lift_ids,
                                    &mut |cg, next_pc, next_direct_site| {
                                        emit_step_direct_site(cg, next_pc, next_direct_site, 0)
                                    },
                                    &mut unexpected_indirect,
                                )?;
                            } else {
                                let mut unexpected_indirect = |_cg: &mut Self,
                                                               _next_pc: usize,
                                                               _next_indirect: &MixedEscapeIndirectSite<
                                    'hir,
                                >| {
                                    Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle multi-arm without immediate-resume (unexpected indirect site while continuing mixed block)",
                                        at: site.decl.span.into(),
                                    })
                                };
                                cg.codegen_mixed_escape_matrix_continue_to_next_block_site_after_indirect(
                                    indirect_site,
                                    next_pc,
                                    next_site,
                                    &body_lift_ids,
                                    &mut |cg, next_pc, next_direct_site| {
                                        emit_step_direct_site(cg, next_pc, next_direct_site, 0)
                                    },
                                    &mut unexpected_indirect,
                                )?;
                            }
                            if let MatrixEscapeSiteKind::Direct {
                                site: next_direct_site,
                            } = &next_site.kind
                                && while_next_pc.is_some()
                                && let Some(bb) = cg.builder.get_insert_block()
                                && bb.get_terminator().is_none()
                            {
                                cg.codegen_mixed_escape_matrix_while_tail_after_site(
                                    &handle.body.stmts[next_site.stmt_idx],
                                    next_pc,
                                    next_direct_site,
                                    &body_lift_ids,
                                    |cg, reenter_pc, future_direct_site| {
                                        emit_step_direct_site(
                                            cg,
                                            reenter_pc,
                                            future_direct_site,
                                            0,
                                        )
                                    },
                                )?;
                            } else if let MatrixEscapeSiteKind::Indirect {
                                site: next_indirect_site,
                            } = &next_site.kind
                                && let Some(bb) = cg.builder.get_insert_block()
                                && bb.get_terminator().is_none()
                            {
                                cg.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                    next_indirect_site,
                                    &body_lift_ids,
                                )?;
                            }
                        }
                    }
                    if let Some(bb) = cg.builder.get_insert_block()
                        && bb.get_terminator().is_some()
                    {
                        terminated = true;
                    }
                } else {
                    match &site.kind {
                        MatrixEscapeSiteKind::Direct { site: direct_site }
                            if matches!(
                                direct_site.resume_path.first(),
                                Some(MixedEscapeDirectFrame::WhileBody { .. })
                            ) =>
                        {
                            cg.codegen_mixed_escape_matrix_while_tail_after_site(
                                &handle.body.stmts[site.stmt_idx],
                                site_pc,
                                direct_site,
                                &body_lift_ids,
                                |cg, reenter_pc, future_direct_site| {
                                    emit_step_direct_site(
                                        cg,
                                        reenter_pc,
                                        future_direct_site,
                                        0,
                                    )
                                },
                            )?;
                        }
                        MatrixEscapeSiteKind::Direct { site: direct_site }
                            if !direct_site.resume_path.is_empty() =>
                        {
                            cg.codegen_mixed_escape_matrix_nested_block_tail_after_site(
                                direct_site,
                                &body_lift_ids,
                            )?;
                        }
                        MatrixEscapeSiteKind::Indirect { site: indirect_site }
                            if while_prev_pc.is_some() =>
                        {
                            let prev_pc = while_prev_pc
                                .expect("while_prev_pc is checked by the match guard");
                            let MatrixEscapeSiteKind::Direct {
                                site: prev_direct_site,
                            } = &escape_sites[prev_pc].kind
                            else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle multi-arm without immediate-resume (expected previous direct site)",
                                    at: site.decl.span.into(),
                                });
                            };
                            cg.codegen_mixed_escape_matrix_while_tail_after_mixed_indirect_site(
                                indirect_site,
                                prev_pc,
                                prev_direct_site,
                                &body_lift_ids,
                                |cg, reenter_pc, future_direct_site| {
                                    emit_step_direct_site(
                                        cg,
                                        reenter_pc,
                                        future_direct_site,
                                        0,
                                    )
                                },
                            )?;
                        }
                        MatrixEscapeSiteKind::Indirect { site: indirect_site }
                            if !indirect_site.resume_path.is_empty() =>
                        {
                            for _ in &indirect_site.resume_path {
                                cg.env.push_scope();
                            }
                            cg.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                indirect_site,
                                &body_lift_ids,
                            )?;
                        }
                        _ => {}
                    }
                }
                for (idx, stmt) in handle
                    .body
                    .stmts
                    .iter()
                    .enumerate()
                    .skip(site.stmt_idx + 1)
                {
                    if terminated {
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
                                emit_step_direct_site(cg, next_pc, direct_site, 0)
                            },
                            |cg, next_pc, indirect_site| {
                                emit_step_indirect_site(cg, next_pc, indirect_site)
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
                                emit_step_direct_site(cg, next_pc, direct_site, 0)
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
                                emit_step_indirect_site(cg, next_pc, indirect_site)
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
                                emit_step_direct_site(cg, next_pc, direct_site, 0)
                            },
                            |cg, next_pc, indirect_site| {
                                emit_step_indirect_site(cg, next_pc, indirect_site)
                            },
                        )?;
                        continue;
                    }
                    if let Some(next_pc) = simple_escape_site_pc_by_stmt_idx.get(&idx).copied() {
                        let next_site = &escape_sites[next_pc];
                        match &next_site.kind {
                            MatrixEscapeSiteKind::Direct { site: direct_site } => {
                                if !direct_site.resume_path.is_empty() {
                                    cg.codegen_mixed_escape_matrix_nested_block_prefix_to_site(
                                        direct_site,
                                        stmt,
                                        &body_lift_ids,
                                    )?;
                                }
                                cg.capture_escape_state_with_pc(
                                    next_site.decl.span,
                                    state_ty,
                                    state_ptr,
                                    &outer_visible_supported,
                                    outer_field_base,
                                    &body_visible_supported,
                                    body_field_base,
                                    2,
                                    next_pc,
                                )?;
                                let _ = cg.builder.build_store(
                                    step_escape_binder_from_slot_ptr,
                                    cg.context.bool_type().const_zero(),
                                )?;
                                for (slot, arg) in
                                    step_escape_binder_slots.iter().zip(direct_site.args.iter())
                                {
                                    let hir::CallArg::Positional(expr) = arg else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle mixed-arm escape named perform arg",
                                            at: span.into(),
                                        });
                                    };
                                    let v =
                                        cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                    let _ =
                                        cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                }
                                for _ in 0..direct_site.resume_path.len() {
                                    cg.env.pop_scope();
                                }
                                cg.builder.build_unconditional_branch(step_escape_arm_bb)?;
                                terminated = true;
                                break;
                            }
                            MatrixEscapeSiteKind::Indirect { site: indirect_site } => {
                                if !indirect_site.resume_path.is_empty() {
                                    cg.codegen_mixed_escape_matrix_prefix_to_indirect_site(
                                        indirect_site,
                                        stmt,
                                        &body_lift_ids,
                                    )?;
                                }
                                cg.capture_escape_state_with_pc(
                                    next_site.decl.span,
                                    state_ty,
                                    state_ptr,
                                    &outer_visible_supported,
                                    outer_field_base,
                                    &body_visible_supported,
                                    body_field_base,
                                    2,
                                    next_pc,
                                )?;
                                if step_effect_dispatch_bb.is_some() {
                                    cg.pop_raise_target();
                                }
                                cg.push_raise_target(step_escape_dispatch_bb);
                                cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                    indirect_site,
                                    &body_lift_ids,
                                )?;
                                cg.pop_raise_target();
                                if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                                    cg.push_raise_target(step_effect_dispatch_bb);
                                }
                                if let Some(&block_next_pc) =
                                    block_next_site_pc_by_pc.get(&next_pc)
                                {
                                    let block_next_site = &escape_sites[block_next_pc];
                                    let mut emit_next_direct = |cg: &mut Self,
                                                               next_pc: usize,
                                                               next_direct_site: &MixedEscapeDirectSite<'hir>| {
                                        cg.capture_escape_state_with_pc(
                                            next_direct_site.decl.span,
                                            state_ty,
                                            state_ptr,
                                            &outer_visible_supported,
                                            outer_field_base,
                                            &body_visible_supported,
                                            body_field_base,
                                            2,
                                            next_pc,
                                        )?;
                                        let _ = cg.builder.build_store(
                                            step_escape_binder_from_slot_ptr,
                                            cg.context.bool_type().const_zero(),
                                        )?;
                                        for (slot, arg) in step_escape_binder_slots
                                            .iter()
                                            .zip(next_direct_site.args.iter())
                                        {
                                            let hir::CallArg::Positional(expr) = arg else {
                                                return Err(LlvmEmitError::UnsupportedMainBody {
                                                    kind: "handle mixed-arm escape named perform arg",
                                                    at: span.into(),
                                                });
                                            };
                                            let v = cg.codegen_expr_in_expected_context(
                                                expr,
                                                Some(slot.ty),
                                            )?;
                                            let _ = cg.store_local_value(
                                                expr.span,
                                                slot.ptr,
                                                slot.ty,
                                                v,
                                            )?;
                                        }
                                        cg.builder.build_unconditional_branch(step_escape_arm_bb)?;
                                        Ok(())
                                    };
                                    let mut unexpected_indirect = |_cg: &mut Self,
                                                                   _next_pc: usize,
                                                                   _next_indirect: &MixedEscapeIndirectSite<
                                        'hir,
                                    >| {
                                        Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle multi-arm without immediate-resume (unexpected indirect site while continuing mixed block)",
                                            at: next_site.decl.span.into(),
                                        })
                                    };
                                    cg.codegen_mixed_escape_matrix_continue_to_next_block_site_after_indirect(
                                        indirect_site,
                                        block_next_pc,
                                        block_next_site,
                                        &body_lift_ids,
                                        &mut emit_next_direct,
                                        &mut unexpected_indirect,
                                    )?;
                                    if let MatrixEscapeSiteKind::Indirect {
                                        site: next_indirect_site,
                                    } = &block_next_site.kind
                                        && let Some(bb) = cg.builder.get_insert_block()
                                        && bb.get_terminator().is_none()
                                    {
                                        cg.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                            next_indirect_site,
                                            &body_lift_ids,
                                        )?;
                                    }
                                } else if let Some(bb) = cg.builder.get_insert_block()
                                    && bb.get_terminator().is_none()
                                {
                                    cg.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                        indirect_site,
                                        &body_lift_ids,
                                    )?;
                                }
                                if let Some(bb) = cg.builder.get_insert_block()
                                    && bb.get_terminator().is_some()
                                {
                                    terminated = true;
                                    break;
                                }
                                continue;
                            }
                        }
                    }

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

                if !terminated
                    && let Some(bb) = cg.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    let unpin = cg.declare_runtime_gc_unpin();
                    let _ = cg.builder.build_call(
                        unpin,
                        &[state_raw.into()],
                        "multi_escape_no_immediate_top_level_mixed_step_state_unpin",
                    )?;
                    cg.builder.build_return(None)?;
                }
            }

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                let step_effect_dispatch_nomatch_bb = step_effect_dispatch_nomatch_bb
                    .expect("top-level mixed step dispatch_nomatch bb should exist");
                cg.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    cg.pop_effect_unwind_target();
                }

                cg.builder.position_at_end(step_effect_dispatch_bb);
                let rt_read_tag = cg.declare_runtime_effect_perform_slot_read_op_tag();
                let tag_call = cg.builder.build_call(
                    rt_read_tag,
                    &[],
                    "multi_escape_no_immediate_top_level_mixed_step_read_op_tag",
                )?;
                let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate top-level mixed step read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate top-level mixed step read_op_tag return type",
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
                    "multi_escape_no_immediate_top_level_mixed_step_state_unpin_nomatch",
                )?;
                cg.builder.build_return(None)?;

                if let (Some(raise_arm), Some(step_raise_catch_bb)) =
                    (raise_sibling, step_raise_catch_bb)
                {
                    let binder = &raise_arm.op.binders[0];
                    cg.builder.position_at_end(step_raise_catch_bb);
                    let frame_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        1,
                        "multi_escape_no_immediate_top_level_mixed_step_raise_frame_gep",
                    )?;
                    let prev_ptr = cg.builder.build_struct_gep(
                        handler_frame_ty,
                        frame_ptr,
                        0,
                        "multi_escape_no_immediate_top_level_mixed_step_raise_prev_gep",
                    )?;
                    let prev_raw = cg.builder.build_load(
                        i8_ptr_ty,
                        prev_ptr,
                        "multi_escape_no_immediate_top_level_mixed_step_raise_prev",
                    )?;
                    let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                    let _ = cg.builder.build_call(
                        rt_swap,
                        &[prev_raw.into()],
                        "multi_escape_no_immediate_top_level_mixed_step_raise_detach",
                    )?;

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "multi_escape_no_immediate_top_level_mixed_step_raise_read_slot_len_words",
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
                        "multi_escape_no_immediate_top_level_mixed_step_raise_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_top_level_mixed_step_raise_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_top_level_mixed_step_raise_slot_len_bad_bb",
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
                        "multi_escape_no_immediate_top_level_mixed_step_raise_read_slot_word0",
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
                        "multi_escape_no_immediate_top_level_mixed_step_raise_read_slot_word1",
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
                        "multi_escape_no_immediate_top_level_mixed_step_raise_clear",
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
                                "multi_escape_no_immediate_top_level_mixed_step_raise_kind_is_int",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_top_level_mixed_step_raise_kind_int_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_top_level_mixed_step_raise_kind_int_bad",
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
                                "multi_escape_no_immediate_top_level_mixed_step_raise_kind_is_runtime_error",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_top_level_mixed_step_raise_kind_runtime_error_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_top_level_mixed_step_raise_kind_runtime_error_bad",
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
                                "multi_escape_no_immediate_top_level_mixed_step_runtime_error_tag_i32",
                            )?;
                            let payload_word_zero =
                                cg.int_type(cg.enum_payload_ty()).const_int(0, false);
                            let payload_ptr_zero = cg.llvm_gc_i8_ptr_type().const_null();
                            let llvm_enum_ty = cg.llvm_enum_value_type(span, enum_ty)?;
                            let llvm_enum_ty = llvm_enum_ty.into_struct_type();
                            let mut agg: AggregateValueEnum<'ctx> =
                                llvm_enum_ty.get_undef().into();
                            agg = cg.builder.build_insert_value(
                                agg,
                                tag_i32,
                                0,
                                "multi_escape_no_immediate_top_level_mixed_step_runtime_error_tag",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_word_zero,
                                1,
                                "multi_escape_no_immediate_top_level_mixed_step_runtime_error_payload_word",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_ptr_zero,
                                2,
                                "multi_escape_no_immediate_top_level_mixed_step_runtime_error_payload_ptr",
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
                            "multi_escape_no_immediate_top_level_mixed_step_state_unpin_raise",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }

                for (idx, custom) in custom_siblings.iter().enumerate() {
                    let arm = custom.arm;
                    let binder = &arm.op.binders[0];
                    cg.builder.position_at_end(step_custom_catch_bbs[idx]);

                    let frame_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        1,
                        "multi_escape_no_immediate_top_level_mixed_step_custom_frame_gep",
                    )?;
                    let prev_ptr = cg.builder.build_struct_gep(
                        handler_frame_ty,
                        frame_ptr,
                        0,
                        "multi_escape_no_immediate_top_level_mixed_step_custom_prev_gep",
                    )?;
                    let prev_raw = cg.builder.build_load(
                        i8_ptr_ty,
                        prev_ptr,
                        "multi_escape_no_immediate_top_level_mixed_step_custom_prev",
                    )?;
                    let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                    let _ = cg.builder.build_call(
                        rt_swap,
                        &[prev_raw.into()],
                        "multi_escape_no_immediate_top_level_mixed_step_custom_detach",
                    )?;

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "multi_escape_no_immediate_top_level_mixed_step_custom_read_slot_len_words",
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
                        "multi_escape_no_immediate_top_level_mixed_step_custom_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_top_level_mixed_step_custom_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_top_level_mixed_step_custom_slot_len_bad_bb",
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
                        "multi_escape_no_immediate_top_level_mixed_step_custom_read_slot_word0",
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
                        "multi_escape_no_immediate_top_level_mixed_step_custom_read_slot_gc_ref",
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
                        "multi_escape_no_immediate_top_level_mixed_step_custom_clear",
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
                            "multi_escape_no_immediate_top_level_mixed_step_state_unpin_custom",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }
            }

            cg.builder.position_at_end(step_escape_dispatch_bb);
            let rt_read_tag = cg.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = cg.builder.build_call(
                rt_read_tag,
                &[],
                "multi_escape_no_immediate_top_level_mixed_step_escape_read_op_tag",
            )?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate top-level mixed step escape read_op_tag return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate top-level mixed step escape read_op_tag return type",
                    at: span.into(),
                });
            };
            let escape_tag = self.effect_op_tag(&escape_arm.op.op.fqn);
            let tag_matches = cg.builder.build_int_compare(
                IntPredicate::EQ,
                slot_tag,
                i32_ty.const_int(escape_tag as u64, false),
                "multi_escape_no_immediate_top_level_mixed_step_escape_tag_eq",
            )?;
            cg.builder.build_conditional_branch(
                tag_matches,
                step_escape_arm_slot_bb,
                step_escape_fallback_bb,
            )?;

            cg.builder.position_at_end(step_escape_fallback_bb);
            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                cg.builder
                    .build_unconditional_branch(step_effect_dispatch_bb)?;
            } else {
                let unpin = cg.declare_runtime_gc_unpin();
                let _ = cg.builder.build_call(
                    unpin,
                    &[state_raw.into()],
                    "multi_escape_no_immediate_top_level_mixed_step_state_unpin_escape_nomatch",
                )?;
                cg.builder.build_return(None)?;
            }

            cg.builder.position_at_end(step_escape_arm_slot_bb);
            let _ = cg.builder.build_store(
                step_escape_binder_from_slot_ptr,
                cg.context.bool_type().const_all_ones(),
            )?;
            cg.builder.build_unconditional_branch(step_escape_arm_bb)?;

            cg.builder.position_at_end(step_escape_arm_bb);
            let binder_from_slot = cg
                .builder
                .build_load(
                    cg.context.bool_type(),
                    step_escape_binder_from_slot_ptr,
                    "multi_escape_no_immediate_top_level_mixed_step_binder_from_slot",
                )?
                .into_int_value();
            let binder_read_bb = cg.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_top_level_mixed_step_arm_read_binder",
            );
            let binder_skip_bb = cg.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_top_level_mixed_step_arm_skip_binder",
            );
            let binder_merge_bb = cg.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_top_level_mixed_step_arm_after_binder",
            );
            cg.builder.build_conditional_branch(
                binder_from_slot,
                binder_read_bb,
                binder_skip_bb,
            )?;

            cg.builder.position_at_end(binder_read_bb);
            if let Some(slot) = step_escape_binder_slots.first() {
                let rt_read = cg.declare_runtime_effect_perform_slot_read_u64();
                let word_call = cg.builder.build_call(
                    rt_read,
                    &[],
                    "multi_escape_no_immediate_top_level_mixed_step_arm_read_binder_word",
                )?;
                let word_raw = word_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate top-level mixed step arm read binder return",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(word_u64) = word_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate top-level mixed step arm read binder type",
                        at: span.into(),
                    });
                };
                let rt_read_gc = cg.declare_runtime_effect_perform_slot_read_gc_ref();
                let gc_call = cg.builder.build_call(
                    rt_read_gc,
                    &[],
                    "multi_escape_no_immediate_top_level_mixed_step_arm_read_binder_gc",
                )?;
                let gc_raw = gc_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate top-level mixed step arm read binder gc value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(gc_ref_raw) = gc_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate top-level mixed step arm read binder gc type",
                        at: span.into(),
                    });
                };
                let binder_value =
                    cg.decode_abi_payload_transport(span, word_u64, gc_ref_raw, slot.ty)?;
                let _ = cg.store_local_value(span, slot.ptr, slot.ty, binder_value)?;
            }
            cg.builder.build_unconditional_branch(binder_merge_bb)?;
            cg.builder.position_at_end(binder_skip_bb);
            cg.builder.build_unconditional_branch(binder_merge_bb)?;
            cg.builder.position_at_end(binder_merge_bb);

            let rt_clear = cg.declare_runtime_effect_clear();
            let _ = cg.builder.build_call(
                rt_clear,
                &[],
                "multi_escape_no_immediate_top_level_mixed_step_arm_effect_clear",
            )?;

            let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
            let step_ptr = step_fn.as_global_value().as_pointer_value();
            let cont_call = cg.builder.build_call(
                rt_cont_alloc,
                &[state_raw.into(), step_ptr.into()],
                "multi_escape_no_immediate_top_level_mixed_step_cont_alloc",
            )?;
            let cont_raw = cont_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate top-level mixed step continuation alloc return value",
                    at: escape_arm.span.into(),
                })?;
            let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate top-level mixed step continuation alloc return type",
                    at: escape_arm.span.into(),
                });
            };
            let pin = cg.declare_runtime_gc_pin();
            let _ = cg.builder.build_call(
                pin,
                &[k_raw.into()],
                "multi_escape_no_immediate_top_level_mixed_step_k_pin",
            )?;
            let _ = cg.store_local_value(
                span,
                step_cont_ptr,
                CgTy::Ref,
                CgValue {
                    ty: CgTy::Ref,
                    value: Some(k_raw.into()),
                },
            )?;

            let frame_ptr = cg.builder.build_struct_gep(
                state_ty,
                state_ptr,
                1,
                "multi_escape_no_immediate_top_level_mixed_step_arm_frame_gep",
            )?;
            let prev_ptr = cg.builder.build_struct_gep(
                handler_frame_ty,
                frame_ptr,
                0,
                "multi_escape_no_immediate_top_level_mixed_step_arm_prev_gep",
            )?;
            let prev_raw = cg.builder.build_load(
                i8_ptr_ty,
                prev_ptr,
                "multi_escape_no_immediate_top_level_mixed_step_arm_prev",
            )?;
            let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
            let _ = cg.builder.build_call(
                rt_swap,
                &[prev_raw.into()],
                "multi_escape_no_immediate_top_level_mixed_step_arm_detach",
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
                    ptr: step_cont_ptr,
                    mutable: false,
                },
            );
            if let Some(step_escape_arm_unwind_bb) = step_escape_arm_unwind_bb {
                for custom in &custom_siblings {
                    cg.push_effect_unwind_target(&custom.arm.op.op.fqn, step_escape_arm_unwind_bb);
                }
                cg.push_raise_target(step_escape_arm_unwind_bb);
            }
            let arm_v = cg.codegen_expr_in_expected_context(&escape_arm.body, Some(out_ty))?;
            if step_escape_arm_unwind_bb.is_some() {
                cg.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    cg.pop_effect_unwind_target();
                }
            }
            let _arm_v = if out_ty == CgTy::Unit {
                CgValue::unit()
            } else {
                cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
            };
            cg.env.pop_scope();

            if let Some(bb) = cg.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                let llvm_ref_ty = cg.llvm_basic_type_of(span, CgTy::Ref)?;
                let k_loaded = cg
                    .builder
                    .build_load(
                        llvm_ref_ty,
                        step_cont_ptr,
                        "multi_escape_no_immediate_top_level_mixed_step_k_unpin_load",
                    )?
                    .into_pointer_value();
                let unpin = cg.declare_runtime_gc_unpin();
                let _ = cg.builder.build_call(
                    unpin,
                    &[k_loaded.into()],
                    "multi_escape_no_immediate_top_level_mixed_step_k_unpin",
                )?;
                cg.builder.build_return(None)?;
            }

            if let Some(step_escape_arm_unwind_bb) = step_escape_arm_unwind_bb {
                cg.builder.position_at_end(step_escape_arm_unwind_bb);
                cg.builder.build_return(None)?;
            }

            cg.env.pop_scope();
        }
        self.builder.position_at_end(saved_block);

        let body_bb =
            self.context
                .append_basic_block(func, "handle_multi_escape_no_immediate_top_level_mixed_body");
        let escape_dispatch_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_top_level_mixed_dispatch",
        );
        let escape_dispatch_nomatch_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_top_level_mixed_dispatch_nomatch",
        );
        let escape_arm_bb =
            self.context
                .append_basic_block(func, "handle_multi_escape_no_immediate_top_level_mixed_arm");
        let escape_arm_slot_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_top_level_mixed_arm_from_slot",
        );
        let done_bb =
            self.context
                .append_basic_block(func, "handle_multi_escape_no_immediate_top_level_mixed_done");
        let finally_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_top_level_mixed_finally",
        );
        let finally_unwind_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_top_level_mixed_finally_unwind",
        );
        let sibling_dispatch = self.build_sibling_nonresuming_dispatch_blocks(
            func,
            "handle_multi_escape_no_immediate_top_level_mixed",
            &sibling_plan,
        );
        let effect_dispatch_bb = sibling_dispatch.effect_dispatch_bb;
        let effect_dispatch_nomatch_bb = sibling_dispatch.effect_dispatch_nomatch_bb;
        let raise_catch_bb = sibling_dispatch.raise_catch_bb;
        let custom_catch_bbs = sibling_dispatch.custom_catch_bbs;

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(
                span,
                "handle_multi_escape_no_immediate_top_level_mixed_result",
                out_ty,
            )?)
        };
        let continuation_created_ptr = self.create_entry_alloca_raw(
            span,
            "handle_multi_escape_no_immediate_top_level_mixed_cont_created",
            self.context.bool_type().into(),
        )?;
        let _ = self.builder.build_store(
            continuation_created_ptr,
            self.context.bool_type().const_zero(),
        )?;

        let mut escape_binder_slots: Vec<ImmediateResumeBinderSlot<'ctx>> = Vec::new();
        for binder in &escape_arm.op.binders {
            let binder_ty = self
                .cg_ty_of(binder.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape binder type",
                    at: binder.span.into(),
                })?;
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
            &format!("handle_multi_escape_no_immediate_top_level_mixed_k_{seq}"),
            CgTy::Ref,
        )?;
        let escape_binder_from_slot_ptr = self.create_entry_alloca_raw(
            span,
            "handle_multi_escape_no_immediate_top_level_mixed_binder_from_slot",
            self.context.bool_type().into(),
        )?;
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
        let state_desc_global_name = format!(
            "__scoop_type_desc_multi_escape_no_immediate_top_level_mixed_state__{func_name}_{seq}"
        );
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
            "multi_escape_no_immediate_top_level_mixed_state_desc_i8",
        )?;
        let alloc_call = self.builder.build_call(
            rt_alloc,
            &[state_desc_i8.into(), size_v.into()],
            "rt_alloc_multi_escape_no_immediate_top_level_mixed_state",
        )?;
        let alloc_raw =
            alloc_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate top-level mixed alloc return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::PointerValue(state_raw) = alloc_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate top-level mixed alloc return type",
                at: span.into(),
            });
        };

        let pin = self.declare_runtime_gc_pin();
        let _ = self.builder.build_call(
            pin,
            &[state_raw.into()],
            "multi_escape_no_immediate_top_level_mixed_state_pin",
        )?;

        let state_gc_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let state_gc_ptr = self.builder.build_pointer_cast(
            state_raw,
            state_gc_ptr_ty,
            "multi_escape_no_immediate_top_level_mixed_state_ptr",
        )?;

        let state_pc_ptr = self.builder.build_struct_gep(
            state_ty,
            state_gc_ptr,
            2,
            "multi_escape_no_immediate_top_level_mixed_state_pc_gep",
        )?;
        let _ = self
            .builder
            .build_store(state_pc_ptr, i32_ty.const_zero())?;

        for (idx, cap) in outer_visible_supported.iter().enumerate() {
            let field_idx = outer_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_escape_no_immediate_top_level_mixed_state_outer_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }
        for (idx, cap) in body_visible_supported.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_escape_no_immediate_top_level_mixed_state_body_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }

        let frame_ptr = self.builder.build_struct_gep(
            state_ty,
            state_gc_ptr,
            1,
            "multi_escape_no_immediate_top_level_mixed_state_frame_gep",
        )?;
        let frame_i8 = self.builder.build_address_space_cast(
            frame_ptr,
            i8_ptr_ty,
            "multi_escape_no_immediate_top_level_mixed_frame_i8",
        )?;
        let escape_tag = self.effect_op_tag(&escape_arm.op.op.fqn);
        let escape_tag_i32 = i32_ty.const_int(escape_tag as u64, false);
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), escape_tag_i32.into()],
            "multi_escape_no_immediate_top_level_mixed_push",
        )?;
        let prev_ptr = self.builder.build_struct_gep(
            handler_frame_ty,
            frame_ptr,
            0,
            "multi_escape_no_immediate_top_level_mixed_prev_gep",
        )?;
        let escape_outer_top = self
            .builder
            .build_load(
                i8_ptr_ty,
                prev_ptr,
                "multi_escape_no_immediate_top_level_mixed_outer_top",
            )?
            .into_pointer_value();
        let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
        let main_raise_target = effect_dispatch_bb.unwrap_or(finally_unwind_bb);

        self.builder.build_unconditional_branch(body_bb)?;

        self.builder.position_at_end(body_bb);
        self.env.push_scope();
        for (idx, custom) in custom_siblings.iter().enumerate() {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom_catch_bbs[idx]);
        }
        self.push_raise_target(main_raise_target);

        let emit_initial_direct_site =
            |cg: &mut Self,
             site_pc: usize,
             direct_site: &MixedEscapeDirectSite<'hir>,
             scopes_to_pop: usize| {
                cg.capture_escape_state_with_pc(
                    direct_site.decl.span,
                    state_ty,
                    state_gc_ptr,
                    &outer_visible_supported,
                    outer_field_base,
                    &body_visible_supported,
                    body_field_base,
                    2,
                    site_pc,
                )?;
                let _ = cg.builder.build_store(
                    escape_binder_from_slot_ptr,
                    cg.context.bool_type().const_zero(),
                )?;
                for (slot, arg) in escape_binder_slots.iter().zip(direct_site.args.iter()) {
                    let hir::CallArg::Positional(expr) = arg else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape named perform arg",
                            at: span.into(),
                        });
                    };
                    let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                    let _ = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                }
                for _ in 0..scopes_to_pop {
                    cg.env.pop_scope();
                }
                cg.builder.build_unconditional_branch(escape_arm_bb)?;
                Ok(())
            };
        let emit_initial_indirect_site =
            |cg: &mut Self,
             site_pc: usize,
             indirect_site: &MixedEscapeIndirectSite<'hir>| {
                cg.capture_escape_state_with_pc(
                    indirect_site.decl.span,
                    state_ty,
                    state_gc_ptr,
                    &outer_visible_supported,
                    outer_field_base,
                    &body_visible_supported,
                    body_field_base,
                    2,
                    site_pc,
                )?;
                cg.pop_raise_target();
                cg.push_raise_target(escape_dispatch_bb);
                cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                    indirect_site,
                    &body_lift_ids,
                )?;
                cg.pop_raise_target();
                cg.push_raise_target(main_raise_target);
                Ok(())
            };

        let mut body_tail: Option<CgValue<'ctx>> = None;
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if self
                .builder
                .get_insert_block()
                .is_some_and(|bb| bb.get_terminator().is_some())
            {
                break;
            }

            if let Some(mixed_sites) = if_mixed_site_pcs_by_stmt_idx.get(&idx) {
                self.codegen_mixed_escape_matrix_if_stmt_mixed_sites(
                    stmt,
                    mixed_sites,
                    &escape_sites,
                    &if_next_site_pc_by_pc,
                    &body_lift_ids,
                    |cg, site_pc, direct_site| {
                        emit_initial_direct_site(cg, site_pc, direct_site, 0)
                    },
                    |cg, site_pc, indirect_site| {
                        emit_initial_indirect_site(cg, site_pc, indirect_site)
                    },
                )?;
                body_tail = None;
                continue;
            }

            if let Some(direct_sites) = if_direct_site_pcs_by_stmt_idx.get(&idx) {
                self.codegen_mixed_escape_matrix_if_stmt_direct_sites(
                    stmt,
                    direct_sites,
                    &escape_sites,
                    &body_lift_ids,
                    |cg, site_pc, direct_site| {
                        emit_initial_direct_site(cg, site_pc, direct_site, 0)
                    },
                )?;
                body_tail = None;
                continue;
            }

            if let Some(indirect_sites) = if_indirect_site_pcs_by_stmt_idx.get(&idx) {
                self.codegen_mixed_escape_matrix_if_stmt_indirect_sites(
                    stmt,
                    indirect_sites,
                    &escape_sites,
                    &body_lift_ids,
                    |cg, site_pc, indirect_site| {
                        emit_initial_indirect_site(cg, site_pc, indirect_site)
                    },
                )?;
                body_tail = None;
                continue;
            }

            if let Some(mixed_sites) = while_mixed_site_pcs_by_stmt_idx.get(&idx) {
                self.codegen_mixed_escape_matrix_while_stmt_mixed_sites(
                    stmt,
                    mixed_sites,
                    &escape_sites,
                    &body_lift_ids,
                    |cg, site_pc, direct_site| {
                        emit_initial_direct_site(cg, site_pc, direct_site, 0)
                    },
                    |cg, site_pc, indirect_site| {
                        emit_initial_indirect_site(cg, site_pc, indirect_site)
                    },
                )?;
                body_tail = None;
                continue;
            }

            if let Some(&site_pc) = simple_escape_site_pc_by_stmt_idx.get(&idx) {
                let site = &escape_sites[site_pc];
                match &site.kind {
                    MatrixEscapeSiteKind::Direct { site: direct_site } => {
                        if !direct_site.resume_path.is_empty() {
                            self.codegen_mixed_escape_matrix_nested_block_prefix_to_site(
                                direct_site,
                                stmt,
                                &body_lift_ids,
                            )?;
                        }
                        emit_initial_direct_site(
                            self,
                            site_pc,
                            direct_site,
                            direct_site.resume_path.len(),
                        )?;
                    }
                    MatrixEscapeSiteKind::Indirect { site: indirect_site } => {
                        if !indirect_site.resume_path.is_empty() {
                            self.codegen_mixed_escape_matrix_prefix_to_indirect_site(
                                indirect_site,
                                stmt,
                                &body_lift_ids,
                            )?;
                        }
                        emit_initial_indirect_site(self, site_pc, indirect_site)?;
                        if let Some(&next_pc) = block_next_site_pc_by_pc.get(&site_pc) {
                            let next_site = &escape_sites[next_pc];
                            self.codegen_mixed_escape_matrix_continue_to_next_block_site_after_indirect(
                                indirect_site,
                                next_pc,
                                next_site,
                                &body_lift_ids,
                                &mut |cg, next_pc, direct_site| {
                                    cg.capture_escape_state_with_pc(
                                        direct_site.decl.span,
                                        state_ty,
                                        state_gc_ptr,
                                        &outer_visible_supported,
                                        outer_field_base,
                                        &body_visible_supported,
                                        body_field_base,
                                        2,
                                        next_pc,
                                    )?;
                                    let _ = cg.builder.build_store(
                                        escape_binder_from_slot_ptr,
                                        cg.context.bool_type().const_zero(),
                                    )?;
                                    for (slot, arg) in
                                        escape_binder_slots.iter().zip(direct_site.args.iter())
                                    {
                                        let hir::CallArg::Positional(expr) = arg else {
                                            return Err(LlvmEmitError::UnsupportedMainBody {
                                                kind: "handle mixed-arm escape named perform arg",
                                                at: span.into(),
                                            });
                                        };
                                        let v = cg.codegen_expr_in_expected_context(
                                            expr,
                                            Some(slot.ty),
                                        )?;
                                        let _ = cg.store_local_value(
                                            expr.span,
                                            slot.ptr,
                                            slot.ty,
                                            v,
                                        )?;
                                    }
                                    cg.builder.build_unconditional_branch(escape_arm_bb)?;
                                    Ok(())
                                },
                                &mut |_cg, _next_pc, _next_indirect_site| {
                                    Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle multi-arm without immediate-resume (unexpected indirect site while continuing mixed block)",
                                        at: site.decl.span.into(),
                                    })
                                },
                            )?;
                            if let MatrixEscapeSiteKind::Indirect {
                                site: next_indirect_site,
                            } = &next_site.kind
                                && let Some(bb) = self.builder.get_insert_block()
                                && bb.get_terminator().is_none()
                            {
                                self.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                    next_indirect_site,
                                    &body_lift_ids,
                                )?;
                            }
                        } else if let Some(bb) = self.builder.get_insert_block()
                            && bb.get_terminator().is_none()
                        {
                            self.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                indirect_site,
                                &body_lift_ids,
                            )?;
                        }
                    }
                }
                body_tail = None;
                continue;
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
                hir::StmtKind::Return { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "stmt in handle body (mixed direct+indirect top-level)",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }
        self.env.pop_scope();

        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            if out_ty != CgTy::Unit
                && let Some(v) = body_tail
            {
                let v = self.coerce_value(handle.body.span, v, out_ty)?;
                if let Some(ptr) = result_ptr {
                    let _ = self.store_local_value(handle.body.span, ptr, out_ty, v)?;
                }
            }
            self.builder.build_unconditional_branch(finally_bb)?;
        }

        self.builder.position_at_end(escape_dispatch_bb);
        let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
        let tag_call = self.builder.build_call(
            rt_read_tag,
            &[],
            "multi_escape_no_immediate_top_level_mixed_escape_read_op_tag",
        )?;
        let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate top-level mixed escape read_op_tag return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate top-level mixed escape read_op_tag return type",
                at: span.into(),
            });
        };
        let tag_matches = self.builder.build_int_compare(
            IntPredicate::EQ,
            slot_tag,
            escape_tag_i32,
            "multi_escape_no_immediate_top_level_mixed_escape_tag_eq",
        )?;
        self.builder.build_conditional_branch(
            tag_matches,
            escape_arm_slot_bb,
            escape_dispatch_nomatch_bb,
        )?;

        self.builder.position_at_end(escape_dispatch_nomatch_bb);
        if let Some(effect_dispatch_bb) = effect_dispatch_bb {
            self.builder.build_unconditional_branch(effect_dispatch_bb)?;
        } else {
            self.builder.build_unconditional_branch(finally_unwind_bb)?;
        }

        if let Some(effect_dispatch_bb) = effect_dispatch_bb {
            let effect_dispatch_nomatch_bb = effect_dispatch_nomatch_bb
                .expect("top-level mixed effect dispatch_nomatch bb should exist");
            self.builder.position_at_end(effect_dispatch_bb);
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = self.builder.build_call(
                rt_read_tag,
                &[],
                "multi_escape_no_immediate_top_level_mixed_dispatch_read_op_tag",
            )?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate top-level mixed dispatch read_op_tag return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate top-level mixed dispatch read_op_tag return type",
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
                    "multi_escape_no_immediate_top_level_mixed_raise_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "multi_escape_no_immediate_top_level_mixed_raise_read_slot_len_words",
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
                    "multi_escape_no_immediate_top_level_mixed_raise_slot_len_ok",
                )?;
                let len_ok_bb = self.context.append_basic_block(
                    func,
                    "multi_escape_no_immediate_top_level_mixed_raise_slot_len_ok_bb",
                );
                let len_bad_bb = self.context.append_basic_block(
                    func,
                    "multi_escape_no_immediate_top_level_mixed_raise_slot_len_bad_bb",
                );
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
                    "multi_escape_no_immediate_top_level_mixed_raise_read_slot_word0",
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
                    "multi_escape_no_immediate_top_level_mixed_raise_read_slot_word1",
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
                let _ = self.builder.build_call(
                    rt_clear,
                    &[],
                    "multi_escape_no_immediate_top_level_mixed_raise_clear",
                )?;

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
                            "multi_escape_no_immediate_top_level_mixed_raise_kind_is_int",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_top_level_mixed_raise_kind_int_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_top_level_mixed_raise_kind_int_bad",
                        );
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
                            "multi_escape_no_immediate_top_level_mixed_raise_kind_is_runtime_error",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_top_level_mixed_raise_kind_runtime_error_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_top_level_mixed_raise_kind_runtime_error_bad",
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
                            "multi_escape_no_immediate_top_level_mixed_runtime_error_tag_i32",
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
                            "multi_escape_no_immediate_top_level_mixed_runtime_error_tag",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_word_zero,
                            1,
                            "multi_escape_no_immediate_top_level_mixed_runtime_error_payload_word",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_ptr_zero,
                            2,
                            "multi_escape_no_immediate_top_level_mixed_runtime_error_payload_ptr",
                        )?;
                        CgValue {
                            ty: CgTy::Enum(enum_ty),
                            value: Some(agg.as_basic_value_enum()),
                        }
                    }
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle binder type (Raise payload decode)",
                            at: span.into(),
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
                    "multi_escape_no_immediate_top_level_mixed_custom_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "multi_escape_no_immediate_top_level_mixed_custom_read_slot_len_words",
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
                    "multi_escape_no_immediate_top_level_mixed_custom_slot_len_ok",
                )?;
                let len_ok_bb = self.context.append_basic_block(
                    func,
                    "multi_escape_no_immediate_top_level_mixed_custom_slot_len_ok_bb",
                );
                let len_bad_bb = self.context.append_basic_block(
                    func,
                    "multi_escape_no_immediate_top_level_mixed_custom_slot_len_bad_bb",
                );
                self.builder
                    .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                self.builder.position_at_end(len_bad_bb);
                self.emit_exit_with_code(span, 3)?;

                self.builder.position_at_end(len_ok_bb);
                let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
                let value_call = self.builder.build_call(
                    rt_read,
                    &[],
                    "multi_escape_no_immediate_top_level_mixed_custom_read_slot_word0",
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
                    "multi_escape_no_immediate_top_level_mixed_custom_read_slot_gc_ref",
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
                let _ = self.builder.build_call(
                    rt_clear,
                    &[],
                    "multi_escape_no_immediate_top_level_mixed_custom_clear",
                )?;

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
                "multi_escape_no_immediate_top_level_mixed_binder_from_slot",
            )?
            .into_int_value();
        let binder_read_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_top_level_mixed_arm_read_binder",
        );
        let binder_skip_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_top_level_mixed_arm_skip_binder",
        );
        let binder_merge_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_top_level_mixed_arm_after_binder",
        );
        self.builder
            .build_conditional_branch(binder_from_slot, binder_read_bb, binder_skip_bb)?;

        self.builder.position_at_end(binder_read_bb);
        if let Some(slot) = escape_binder_slots.first() {
            let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
            let word_call = self.builder.build_call(
                rt_read,
                &[],
                "multi_escape_no_immediate_top_level_mixed_arm_read_binder_word",
            )?;
            let word_raw = word_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate top-level mixed arm read binder return",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(word_u64) = word_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate top-level mixed arm read binder type",
                    at: span.into(),
                });
            };
            let rt_read_gc = self.declare_runtime_effect_perform_slot_read_gc_ref();
            let gc_call = self.builder.build_call(
                rt_read_gc,
                &[],
                "multi_escape_no_immediate_top_level_mixed_arm_read_binder_gc",
            )?;
            let gc_raw =
                gc_call
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate top-level mixed arm read binder gc value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::PointerValue(gc_ref_raw) = gc_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate top-level mixed arm read binder gc type",
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
        let _ = self.builder.build_call(
            rt_clear,
            &[],
            "multi_escape_no_immediate_top_level_mixed_arm_effect_clear",
        )?;

        let step_ptr = step_fn.as_global_value().as_pointer_value();
        let cont_call = self.builder.build_call(
            self.declare_runtime_continuation_alloc(),
            &[state_raw.into(), step_ptr.into()],
            "multi_escape_no_immediate_top_level_mixed_cont_alloc",
        )?;
        let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate top-level mixed continuation alloc return value",
                at: escape_arm.span.into(),
            },
        )?;
        let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate top-level mixed continuation alloc return type",
                at: escape_arm.span.into(),
            });
        };

        let _ = self.builder.build_call(
            pin,
            &[k_raw.into()],
            "multi_escape_no_immediate_top_level_mixed_k_pin",
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
        let _ = self.builder.build_store(
            continuation_created_ptr,
            self.context.bool_type().const_all_ones(),
        )?;

        let _ = self.builder.build_call(
            rt_swap,
            &[escape_outer_top.into()],
            "multi_escape_no_immediate_top_level_mixed_detach",
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
            "multi_escape_no_immediate_top_level_mixed_finally_unwind_detach",
        )?;
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            let created = self
                .builder
                .build_load(
                    self.context.bool_type(),
                    continuation_created_ptr,
                    "multi_escape_no_immediate_top_level_mixed_unwind_cont_created",
                )?
                .into_int_value();
            let unwind_propagate_bb = self.context.append_basic_block(
                func,
                "multi_escape_no_immediate_top_level_mixed_finally_unwind_propagate",
            );
            let unwind_unpin_bb = self.context.append_basic_block(
                func,
                "multi_escape_no_immediate_top_level_mixed_finally_unwind_unpin",
            );
            self.builder
                .build_conditional_branch(created, unwind_propagate_bb, unwind_unpin_bb)?;

            self.builder.position_at_end(unwind_unpin_bb);
            let unpin = self.declare_runtime_gc_unpin();
            let _ = self.builder.build_call(
                unpin,
                &[state_raw.into()],
                "multi_escape_no_immediate_top_level_mixed_state_unpin_unwind",
            )?;
            self.builder
                .build_unconditional_branch(unwind_propagate_bb)?;

            self.builder.position_at_end(unwind_propagate_bb);
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
            "multi_escape_no_immediate_top_level_mixed_finally_detach",
        )?;
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.builder.build_unconditional_branch(done_bb)?;
        }

        self.builder.position_at_end(done_bb);
        let done_with_k_bb = self.context.append_basic_block(
            func,
            "multi_escape_no_immediate_top_level_mixed_done_with_k",
        );
        let done_without_k_bb = self.context.append_basic_block(
            func,
            "multi_escape_no_immediate_top_level_mixed_done_without_k",
        );
        let done_merge_bb = self.context.append_basic_block(
            func,
            "multi_escape_no_immediate_top_level_mixed_done_merge",
        );
        let created = self
            .builder
            .build_load(
                self.context.bool_type(),
                continuation_created_ptr,
                "multi_escape_no_immediate_top_level_mixed_done_cont_created",
            )?
            .into_int_value();
        self.builder
            .build_conditional_branch(created, done_with_k_bb, done_without_k_bb)?;

        self.builder.position_at_end(done_with_k_bb);
        let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
        let k_loaded = self
            .builder
            .build_load(
                llvm_ref_ty,
                cont_ptr,
                "multi_escape_no_immediate_top_level_mixed_k_unpin_load",
            )?
            .into_pointer_value();
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self.builder.build_call(
            unpin,
            &[k_loaded.into()],
            "multi_escape_no_immediate_top_level_mixed_k_unpin",
        )?;
        self.builder.build_unconditional_branch(done_merge_bb)?;

        self.builder.position_at_end(done_without_k_bb);
        let _ = self.builder.build_call(
            unpin,
            &[state_raw.into()],
            "multi_escape_no_immediate_top_level_mixed_state_unpin_done",
        )?;
        self.builder.build_unconditional_branch(done_merge_bb)?;

        self.builder.position_at_end(done_merge_bb);
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
                let loaded = self.builder.build_load(
                    llvm_ty,
                    ptr,
                    "handle_multi_escape_no_immediate_top_level_mixed_result",
                )?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }

    fn codegen_handle_expr_escape_with_nonresuming_siblings_indirect<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        escape: (&'hir hir::HandleArm, hir::SymbolId),
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        #[derive(Clone, Copy)]
        struct CaptureMeta {
            id: hir::SymbolId,
            hir_ty: Option<TypeId>,
            ty: CgTy,
            mutable: bool,
        }

        #[derive(Clone, Copy)]
        struct CustomSiblingArm<'hir> {
            arm: &'hir hir::HandleArm,
            op_tag: u32,
        }

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

        let direct_sites = self.scan_mixed_escape_direct_sites(handle, &escape_arm.op.op.fqn)?;
        let indirect_sites = self.scan_mixed_escape_indirect_sites(handle)?;
        if !direct_sites.is_empty()
            || indirect_sites.len() != 1
            || !indirect_sites[0].resume_path.is_empty()
        {
            let at = direct_sites
                .first()
                .map(|site| site.decl.span)
                .or_else(|| indirect_sites.first().map(|site| site.decl.span))
                .unwrap_or(span);
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multi-arm without immediate-resume (only top-level indirect single-site supported)",
                at: at.into(),
            });
        }
        let escape_site = &indirect_sites[0];
        let escape_stmt_idx = escape_site.top_level_stmt_idx;

        if escape_arm.op.binders.len() > 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multi-arm without immediate-resume escape binder count (indirect, only 1 supported)",
                at: escape_arm.op.span.into(),
            });
        }

        let escape_resume_value_ty =
            self.cg_ty_of(escape_site.decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape perform value type",
                    at: escape_site.decl.span.into(),
                })?;

        let mut outer_visible_supported: Vec<CaptureMeta> = Vec::new();
        let mut outer_visible_all: HashMap<hir::SymbolId, CaptureMeta> = HashMap::new();
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
            let meta = CaptureMeta {
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

        let mut body_decl_all: HashMap<hir::SymbolId, CaptureMeta> = HashMap::new();
        let mut body_decl_spans: HashMap<hir::SymbolId, crate::span::Span> = HashMap::new();
        for stmt in handle.body.stmts.iter().take(escape_stmt_idx) {
            if let hir::StmtKind::Val(decl) = &stmt.kind
                && let Some(id) = decl.id
            {
                let ty = self
                    .cg_ty_of(decl.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape capture local type",
                        at: decl.span.into(),
                    })?;
                let meta = CaptureMeta {
                    id,
                    hir_ty: Some(decl.ty),
                    ty,
                    mutable: decl.mutable,
                };
                body_decl_all.insert(id, meta);
                body_decl_spans.insert(id, decl.span);
            }
        }

        let mut used_after: HashSet<hir::SymbolId> = HashSet::new();
        Self::collect_used_locals_in_stmt_static(
            &handle.body.stmts[escape_stmt_idx],
            &mut used_after,
        );
        for stmt in handle.body.stmts.iter().skip(escape_stmt_idx + 1) {
            Self::collect_used_locals_in_stmt_static(stmt, &mut used_after);
        }
        used_after.remove(&escape_site.id);

        let mut body_visible_supported: Vec<CaptureMeta> = Vec::new();
        for id in used_after {
            if let Some(meta) = body_decl_all.get(&id) {
                let at = body_decl_spans
                    .get(&id)
                    .copied()
                    .unwrap_or(escape_site.decl.span);
                if self.escape_capture_storage_kind(at, meta.ty)?.is_none() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape capture local type",
                        at: at.into(),
                    });
                }
                body_visible_supported.push(*meta);
                continue;
            }
            if let Some(meta) = outer_visible_all.get(&id) {
                if self
                    .escape_capture_storage_kind(escape_site.decl.span, meta.ty)?
                    .is_none()
                {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape capture local type",
                        at: escape_site.decl.span.into(),
                    });
                }
                continue;
            }
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape capture local missing",
                at: escape_site.decl.span.into(),
            });
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

        let state_ty_name =
            format!("scoop.runtime.MultiEscapeNoImmediateIndirectState__{func_name}_{seq}");
        let state_ty = if let Some(existing) = self.context.get_struct_type(&state_ty_name) {
            existing
        } else {
            let ty = self.context.opaque_struct_type(&state_ty_name);
            let header_ty = self.llvm_gc_object_header_type();
            let mut fields: Vec<BasicTypeEnum<'ctx>> =
                vec![header_ty.into(), handler_frame_ty.into()];
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

        let step_name =
            format!("__scoop_multi_escape_no_immediate_indirect_step__{func_name}_{seq}");
        let step_fn_ty = self.context.void_type().fn_type(
            &[gc_i8_ptr_ty.into(), i64_ty.into(), gc_i8_ptr_ty.into()],
            false,
        );
        let step_fn = self.module.add_function(&step_name, step_fn_ty, None);
        step_fn.set_linkage(Linkage::Internal);
        step_fn.set_gc(super::super::LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE);

        let saved_block = insert_block;
        let outer_field_base = 2u32;
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
                    kind: "multi escape no-immediate indirect step state param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_ptr_ty = cg.llvm_ptr_type(cg.gc_address_space());
            let state_ptr = cg.builder.build_pointer_cast(
                state_raw,
                state_ptr_ty,
                "multi_escape_no_immediate_indirect_step_state_ptr",
            )?;

            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect step resume_word param",
                    at: span.into(),
                })?
                .into_int_value();
            let resume_gc_ref = step_fn
                .get_nth_param(2)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect step resume_gc_ref param",
                    at: span.into(),
                })?
                .into_pointer_value();

            for (idx, cap) in outer_visible_supported.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "multi_escape_no_immediate_indirect_step_outer_gep",
                )?;
                let name = format!(
                    "multi_escape_no_immediate_indirect_outer_{}",
                    cap.id.as_u32()
                );
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
                    "multi_escape_no_immediate_indirect_step_body_gep",
                )?;
                let name = format!(
                    "multi_escape_no_immediate_indirect_body_{}",
                    cap.id.as_u32()
                );
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

            let step_has_sibling_dispatch = has_sibling_nonresuming;
            let step_effect_dispatch_bb = if step_has_sibling_dispatch {
                Some(self.context.append_basic_block(
                    step_fn,
                    "multi_escape_no_immediate_indirect_step_effect_dispatch",
                ))
            } else {
                None
            };
            let step_effect_dispatch_nomatch_bb = if step_has_sibling_dispatch {
                Some(self.context.append_basic_block(
                    step_fn,
                    "multi_escape_no_immediate_indirect_step_effect_dispatch_nomatch",
                ))
            } else {
                None
            };
            let step_raise_catch_bb = if raise_sibling.is_some() {
                Some(self.context.append_basic_block(
                    step_fn,
                    "multi_escape_no_immediate_indirect_step_raise_catch",
                ))
            } else {
                None
            };
            let mut step_custom_catch_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            for (idx, _) in custom_siblings.iter().enumerate() {
                step_custom_catch_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_escape_no_immediate_indirect_step_custom_catch_{idx}"),
                ));
            }

            let rt_get_callee = cg.declare_runtime_callee_suspend_state_get();
            let get_call = cg.builder.build_call(
                rt_get_callee,
                &[],
                "multi_escape_no_immediate_indirect_step_callee_state_get",
            )?;
            let callee_state_raw = get_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect step callee_state_get return",
                    at: span.into(),
                })?
                .into_pointer_value();

            let callee_prefix_ty = cg.llvm_callee_suspend_state_prefix_type();
            let callee_state_ptr_ty = cg.llvm_ptr_type(AddressSpace::default());
            let callee_state_ptr = cg.builder.build_pointer_cast(
                callee_state_raw,
                callee_state_ptr_ty,
                "multi_escape_no_immediate_indirect_step_callee_state_typed",
            )?;
            let callee_rw_ptr = cg.builder.build_struct_gep(
                callee_prefix_ty,
                callee_state_ptr,
                1,
                "multi_escape_no_immediate_indirect_step_resume_word_gep",
            )?;
            let _ = cg.builder.build_store(callee_rw_ptr, resume_word)?;

            let callee_rg_ptr = cg.builder.build_struct_gep(
                callee_prefix_ty,
                callee_state_ptr,
                2,
                "multi_escape_no_immediate_indirect_step_resume_gc_ref_gep",
            )?;
            let wb = cg.declare_runtime_gc_write_barrier();
            let slot_addr = cg.builder.build_pointer_cast(
                callee_rg_ptr,
                i8_ptr_ty,
                "multi_escape_no_immediate_indirect_step_resume_gc_slot",
            )?;
            let _ = cg.builder.build_call(
                wb,
                &[slot_addr.into(), resume_gc_ref.into()],
                "multi_escape_no_immediate_indirect_step_resume_gc_store",
            )?;

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                for (idx, custom) in custom_siblings.iter().enumerate() {
                    cg.push_effect_unwind_target(&custom.arm.op.op.fqn, step_custom_catch_bbs[idx]);
                }
                cg.push_raise_target(step_effect_dispatch_bb);
            }

            let call_result = cg
                .codegen_expr_in_expected_context(escape_site.init, Some(escape_resume_value_ty))?;
            let call_result_ptr = cg.create_entry_alloca(
                escape_site.decl.span,
                escape_site
                    .decl
                    .name
                    .as_deref()
                    .unwrap_or("multi_escape_no_immediate_indirect_result"),
                escape_resume_value_ty,
            )?;
            let _stored = cg.store_local_value(
                escape_site.decl.span,
                call_result_ptr,
                escape_resume_value_ty,
                call_result,
            )?;
            cg.env.insert(
                escape_site.id,
                CgLocal {
                    hir_ty: Some(escape_site.decl.ty),
                    ty: escape_resume_value_ty,
                    ptr: call_result_ptr,
                    mutable: escape_site.decl.mutable,
                },
            );

            for stmt in handle.body.stmts.iter().skip(escape_stmt_idx + 1) {
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

            if step_effect_dispatch_bb.is_some() {
                cg.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    cg.pop_effect_unwind_target();
                }
            }

            if let Some(bb) = cg.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                let unpin = cg.declare_runtime_gc_unpin();
                let _ = cg.builder.build_call(
                    unpin,
                    &[state_raw.into()],
                    "multi_escape_no_immediate_indirect_step_state_unpin",
                )?;
                cg.builder.build_return(None)?;
            }

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                let step_effect_dispatch_nomatch_bb = step_effect_dispatch_nomatch_bb
                    .expect("no-immediate indirect step dispatch_nomatch bb should exist");
                cg.builder.position_at_end(step_effect_dispatch_bb);
                let rt_read_tag = cg.declare_runtime_effect_perform_slot_read_op_tag();
                let tag_call = cg.builder.build_call(
                    rt_read_tag,
                    &[],
                    "multi_escape_no_immediate_indirect_step_read_op_tag",
                )?;
                let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate indirect step read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate indirect step read_op_tag return type",
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
                    "multi_escape_no_immediate_indirect_step_state_unpin_nomatch",
                )?;
                cg.builder.build_return(None)?;

                if let (Some(raise_arm), Some(step_raise_catch_bb)) =
                    (raise_sibling, step_raise_catch_bb)
                {
                    let binder = &raise_arm.op.binders[0];
                    cg.builder.position_at_end(step_raise_catch_bb);
                    let frame_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        1,
                        "multi_escape_no_immediate_indirect_step_raise_frame_gep",
                    )?;
                    let prev_ptr = cg.builder.build_struct_gep(
                        handler_frame_ty,
                        frame_ptr,
                        0,
                        "multi_escape_no_immediate_indirect_step_raise_prev_gep",
                    )?;
                    let prev_raw = cg.builder.build_load(
                        i8_ptr_ty,
                        prev_ptr,
                        "multi_escape_no_immediate_indirect_step_raise_prev",
                    )?;
                    let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                    let _ = cg.builder.build_call(
                        rt_swap,
                        &[prev_raw.into()],
                        "multi_escape_no_immediate_indirect_step_raise_detach",
                    )?;

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "multi_escape_no_immediate_indirect_step_raise_read_slot_len_words",
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
                        "multi_escape_no_immediate_indirect_step_raise_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_indirect_step_raise_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_indirect_step_raise_slot_len_bad_bb",
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
                        "multi_escape_no_immediate_indirect_step_raise_read_slot_word0",
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
                        "multi_escape_no_immediate_indirect_step_raise_read_slot_word1",
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
                        "multi_escape_no_immediate_indirect_step_raise_clear",
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
                                "multi_escape_no_immediate_indirect_step_raise_kind_is_int",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_indirect_step_raise_kind_int_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_indirect_step_raise_kind_int_bad",
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
                                "multi_escape_no_immediate_indirect_step_raise_kind_is_runtime_error",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_indirect_step_raise_kind_runtime_error_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_indirect_step_raise_kind_runtime_error_bad",
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
                                "multi_escape_no_immediate_indirect_step_runtime_error_tag_i32",
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
                                "multi_escape_no_immediate_indirect_step_runtime_error_tag",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_word_zero,
                                1,
                                "multi_escape_no_immediate_indirect_step_runtime_error_payload_word",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_ptr_zero,
                                2,
                                "multi_escape_no_immediate_indirect_step_runtime_error_payload_ptr",
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
                            "multi_escape_no_immediate_indirect_step_state_unpin_raise",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }

                for (idx, custom) in custom_siblings.iter().enumerate() {
                    let arm = custom.arm;
                    let binder = &arm.op.binders[0];
                    cg.builder.position_at_end(step_custom_catch_bbs[idx]);

                    let frame_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        1,
                        "multi_escape_no_immediate_indirect_step_custom_frame_gep",
                    )?;
                    let prev_ptr = cg.builder.build_struct_gep(
                        handler_frame_ty,
                        frame_ptr,
                        0,
                        "multi_escape_no_immediate_indirect_step_custom_prev_gep",
                    )?;
                    let prev_raw = cg.builder.build_load(
                        i8_ptr_ty,
                        prev_ptr,
                        "multi_escape_no_immediate_indirect_step_custom_prev",
                    )?;
                    let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                    let _ = cg.builder.build_call(
                        rt_swap,
                        &[prev_raw.into()],
                        "multi_escape_no_immediate_indirect_step_custom_detach",
                    )?;

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "multi_escape_no_immediate_indirect_step_custom_read_slot_len_words",
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
                        "multi_escape_no_immediate_indirect_step_custom_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_indirect_step_custom_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_indirect_step_custom_slot_len_bad_bb",
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
                        "multi_escape_no_immediate_indirect_step_custom_read_slot_word0",
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
                        "multi_escape_no_immediate_indirect_step_custom_read_slot_gc_ref",
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
                        "multi_escape_no_immediate_indirect_step_custom_clear",
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
                            "multi_escape_no_immediate_indirect_step_state_unpin_custom",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }
            }

            cg.env.pop_scope();
        }
        self.builder.position_at_end(saved_block);

        let body_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_no_immediate_indirect_body");
        let escape_dispatch_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_no_immediate_indirect_dispatch");
        let escape_arm_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_no_immediate_indirect_arm");
        let done_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_no_immediate_indirect_done");
        let finally_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_no_immediate_indirect_finally");
        let finally_unwind_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_indirect_finally_unwind",
        );
        let effect_dispatch_bb = if has_sibling_nonresuming {
            Some(self.context.append_basic_block(
                func,
                "handle_multi_escape_no_immediate_indirect_effect_dispatch",
            ))
        } else {
            None
        };
        let effect_dispatch_nomatch_bb = if has_sibling_nonresuming {
            Some(self.context.append_basic_block(
                func,
                "handle_multi_escape_no_immediate_indirect_effect_dispatch_nomatch",
            ))
        } else {
            None
        };
        let raise_catch_bb = if raise_sibling.is_some() {
            Some(self.context.append_basic_block(
                func,
                "handle_multi_escape_no_immediate_indirect_raise_catch",
            ))
        } else {
            None
        };
        let mut custom_catch_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        for (idx, _) in custom_siblings.iter().enumerate() {
            custom_catch_bbs.push(self.context.append_basic_block(
                func,
                &format!("handle_multi_escape_no_immediate_indirect_custom_catch_{idx}"),
            ));
        }

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(
                span,
                "handle_multi_escape_no_immediate_indirect_result",
                out_ty,
            )?)
        };
        let continuation_created_ptr = self.create_entry_alloca_raw(
            span,
            "handle_multi_escape_no_immediate_indirect_cont_created",
            self.context.bool_type().into(),
        )?;
        let _ = self.builder.build_store(
            continuation_created_ptr,
            self.context.bool_type().const_zero(),
        )?;

        let mut escape_binder_slots: Vec<ImmediateResumeBinderSlot<'ctx>> = Vec::new();
        for binder in &escape_arm.op.binders {
            let binder_ty = self
                .cg_ty_of(binder.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape binder type",
                    at: binder.span.into(),
                })?;
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
            &format!("handle_multi_escape_no_immediate_indirect_k_{seq}"),
            CgTy::Ref,
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
        let state_desc_global_name = format!(
            "__scoop_type_desc_multi_escape_no_immediate_indirect_state__{func_name}_{seq}"
        );
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
            "multi_escape_no_immediate_indirect_state_desc_i8",
        )?;
        let alloc_call = self.builder.build_call(
            rt_alloc,
            &[state_desc_i8.into(), size_v.into()],
            "rt_alloc_multi_escape_no_immediate_indirect_state",
        )?;
        let alloc_raw =
            alloc_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect alloc return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::PointerValue(state_raw) = alloc_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate indirect alloc return type",
                at: span.into(),
            });
        };

        let pin = self.declare_runtime_gc_pin();
        let _ = self.builder.build_call(
            pin,
            &[state_raw.into()],
            "multi_escape_no_immediate_indirect_state_pin",
        )?;

        let state_gc_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let state_gc_ptr = self.builder.build_pointer_cast(
            state_raw,
            state_gc_ptr_ty,
            "multi_escape_no_immediate_indirect_state_ptr",
        )?;

        for (idx, cap) in outer_visible_supported.iter().enumerate() {
            let field_idx = outer_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_escape_no_immediate_indirect_state_outer_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }
        for (idx, cap) in body_visible_supported.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_escape_no_immediate_indirect_state_body_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }

        let frame_ptr = self.builder.build_struct_gep(
            state_ty,
            state_gc_ptr,
            1,
            "multi_escape_no_immediate_indirect_state_frame_gep",
        )?;
        let frame_i8 = self.builder.build_address_space_cast(
            frame_ptr,
            i8_ptr_ty,
            "multi_escape_no_immediate_indirect_frame_i8",
        )?;
        let escape_tag = self.effect_op_tag(&escape_arm.op.op.fqn);
        let escape_tag_i32 = i32_ty.const_int(escape_tag as u64, false);
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), escape_tag_i32.into()],
            "multi_escape_no_immediate_indirect_push",
        )?;
        let prev_ptr = self.builder.build_struct_gep(
            handler_frame_ty,
            frame_ptr,
            0,
            "multi_escape_no_immediate_indirect_prev_gep",
        )?;
        let escape_outer_top = self
            .builder
            .build_load(
                i8_ptr_ty,
                prev_ptr,
                "multi_escape_no_immediate_indirect_outer_top",
            )?
            .into_pointer_value();
        let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
        let main_raise_target = effect_dispatch_bb.unwrap_or(finally_unwind_bb);

        self.builder.build_unconditional_branch(body_bb)?;

        self.builder.position_at_end(body_bb);
        self.env.push_scope();
        for (idx, custom) in custom_siblings.iter().enumerate() {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom_catch_bbs[idx]);
        }
        self.push_raise_target(main_raise_target);

        let mut body_tail: Option<CgValue<'ctx>> = None;
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if idx == escape_stmt_idx {
                for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                    let field_ptr = self.builder.build_struct_gep(
                        state_ty,
                        state_gc_ptr,
                        outer_field_base.saturating_add(field_idx as u32),
                        "multi_escape_no_immediate_indirect_capture_outer_gep",
                    )?;
                    let local = self
                        .env
                        .get(cap.id)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "multi escape no-immediate indirect capture local not found",
                            at: escape_site.decl.span.into(),
                        })?;
                    if local.ty != cap.ty {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "multi escape no-immediate indirect capture local type mismatch",
                            at: escape_site.decl.span.into(),
                        });
                    }
                    self.write_escape_capture_local_to_state(span, field_ptr, local.ptr, cap.ty)?;
                }
                for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                    let field_ptr = self.builder.build_struct_gep(
                        state_ty,
                        state_gc_ptr,
                        body_field_base.saturating_add(field_idx as u32),
                        "multi_escape_no_immediate_indirect_capture_body_gep",
                    )?;
                    let Some(local) = self.env.get(cap.id) else {
                        continue;
                    };
                    if local.ty != cap.ty {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "multi escape no-immediate indirect capture local type mismatch",
                            at: escape_site.decl.span.into(),
                        });
                    }
                    self.write_escape_capture_local_to_state(span, field_ptr, local.ptr, cap.ty)?;
                }

                self.pop_raise_target();
                self.push_raise_target(escape_dispatch_bb);
                self.codegen_val_decl(escape_site.decl)?;
                self.pop_raise_target();
                self.push_raise_target(main_raise_target);
                body_tail = None;
                continue;
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
                hir::StmtKind::Return { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "stmt in handle body (indirect perform)",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }
        self.env.pop_scope();

        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            if out_ty != CgTy::Unit
                && let Some(v) = body_tail
            {
                let v = self.coerce_value(handle.body.span, v, out_ty)?;
                if let Some(ptr) = result_ptr {
                    let _ = self.store_local_value(handle.body.span, ptr, out_ty, v)?;
                }
            }
            self.builder.build_unconditional_branch(finally_bb)?;
        }

        if let Some(effect_dispatch_bb) = effect_dispatch_bb {
            let effect_dispatch_nomatch_bb = effect_dispatch_nomatch_bb
                .expect("no-immediate indirect effect dispatch_nomatch bb should exist");
            self.builder.position_at_end(effect_dispatch_bb);
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = self.builder.build_call(
                rt_read_tag,
                &[],
                "multi_escape_no_immediate_indirect_dispatch_read_op_tag",
            )?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect dispatch read_op_tag return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect dispatch read_op_tag return type",
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
                    "multi_escape_no_immediate_indirect_raise_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "multi_escape_no_immediate_indirect_raise_read_slot_len_words",
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
                    "multi_escape_no_immediate_indirect_raise_slot_len_ok",
                )?;
                let len_ok_bb = self.context.append_basic_block(
                    func,
                    "multi_escape_no_immediate_indirect_raise_slot_len_ok_bb",
                );
                let len_bad_bb = self.context.append_basic_block(
                    func,
                    "multi_escape_no_immediate_indirect_raise_slot_len_bad_bb",
                );
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
                    "multi_escape_no_immediate_indirect_raise_read_slot_word0",
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
                    "multi_escape_no_immediate_indirect_raise_read_slot_word1",
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
                let _ = self.builder.build_call(
                    rt_clear,
                    &[],
                    "multi_escape_no_immediate_indirect_raise_clear",
                )?;

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
                            "multi_escape_no_immediate_indirect_raise_kind_is_int",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_indirect_raise_kind_int_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_indirect_raise_kind_int_bad",
                        );
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
                            "multi_escape_no_immediate_indirect_raise_kind_is_runtime_error",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_indirect_raise_kind_runtime_error_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_indirect_raise_kind_runtime_error_bad",
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
                            "multi_escape_no_immediate_indirect_runtime_error_tag_i32",
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
                            "multi_escape_no_immediate_indirect_runtime_error_tag",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_word_zero,
                            1,
                            "multi_escape_no_immediate_indirect_runtime_error_payload_word",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_ptr_zero,
                            2,
                            "multi_escape_no_immediate_indirect_runtime_error_payload_ptr",
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
                    "multi_escape_no_immediate_indirect_custom_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "multi_escape_no_immediate_indirect_custom_read_slot_len_words",
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
                    "multi_escape_no_immediate_indirect_custom_slot_len_ok",
                )?;
                let len_ok_bb = self.context.append_basic_block(
                    func,
                    "multi_escape_no_immediate_indirect_custom_slot_len_ok_bb",
                );
                let len_bad_bb = self.context.append_basic_block(
                    func,
                    "multi_escape_no_immediate_indirect_custom_slot_len_bad_bb",
                );
                self.builder
                    .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                self.builder.position_at_end(len_bad_bb);
                self.emit_exit_with_code(span, 3)?;

                self.builder.position_at_end(len_ok_bb);
                let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
                let value_call = self.builder.build_call(
                    rt_read,
                    &[],
                    "multi_escape_no_immediate_indirect_custom_read_slot_word0",
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
                    "multi_escape_no_immediate_indirect_custom_read_slot_gc_ref",
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
                let _ = self.builder.build_call(
                    rt_clear,
                    &[],
                    "multi_escape_no_immediate_indirect_custom_clear",
                )?;

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

        self.builder.position_at_end(escape_dispatch_bb);
        let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
        let tag_call = self.builder.build_call(
            rt_read_tag,
            &[],
            "multi_escape_no_immediate_indirect_escape_read_op_tag",
        )?;
        let tag_raw =
            tag_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect escape read_op_tag return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate indirect escape read_op_tag return type",
                at: span.into(),
            });
        };
        let tag_matches = self.builder.build_int_compare(
            IntPredicate::EQ,
            slot_tag,
            escape_tag_i32,
            "multi_escape_no_immediate_indirect_escape_tag_eq",
        )?;
        let escape_dispatch_fallback_bb = effect_dispatch_bb.unwrap_or(finally_unwind_bb);
        self.builder.build_conditional_branch(
            tag_matches,
            escape_arm_bb,
            escape_dispatch_fallback_bb,
        )?;

        self.builder.position_at_end(escape_arm_bb);
        if let Some(slot) = escape_binder_slots.first() {
            let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
            let word_call = self.builder.build_call(
                rt_read,
                &[],
                "multi_escape_no_immediate_indirect_arm_read_binder_word",
            )?;
            let word_raw = word_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect arm read binder return",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(word_u64) = word_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect arm read binder type",
                    at: span.into(),
                });
            };
            let rt_read_gc = self.declare_runtime_effect_perform_slot_read_gc_ref();
            let gc_call = self.builder.build_call(
                rt_read_gc,
                &[],
                "multi_escape_no_immediate_indirect_arm_read_binder_gc",
            )?;
            let gc_raw =
                gc_call
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate indirect arm read binder gc value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::PointerValue(gc_ref_raw) = gc_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect arm read binder gc type",
                    at: span.into(),
                });
            };
            let binder_value =
                self.decode_abi_payload_transport(span, word_u64, gc_ref_raw, slot.ty)?;
            let _ = self.store_local_value(span, slot.ptr, slot.ty, binder_value)?;
        }

        let rt_clear = self.declare_runtime_effect_clear();
        let _ = self.builder.build_call(
            rt_clear,
            &[],
            "multi_escape_no_immediate_indirect_arm_effect_clear",
        )?;

        let step_ptr = step_fn.as_global_value().as_pointer_value();
        let cont_call = self.builder.build_call(
            self.declare_runtime_continuation_alloc(),
            &[state_raw.into(), step_ptr.into()],
            "multi_escape_no_immediate_indirect_cont_alloc",
        )?;
        let cont_raw =
            cont_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect continuation alloc return value",
                    at: escape_arm.span.into(),
                })?;
        let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate indirect continuation alloc return type",
                at: escape_arm.span.into(),
            });
        };

        let _ = self.builder.build_call(
            pin,
            &[k_raw.into()],
            "multi_escape_no_immediate_indirect_k_pin",
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
        let _ = self.builder.build_store(
            continuation_created_ptr,
            self.context.bool_type().const_all_ones(),
        )?;

        let _ = self.builder.build_call(
            rt_swap,
            &[escape_outer_top.into()],
            "multi_escape_no_immediate_indirect_detach",
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
            "multi_escape_no_immediate_indirect_finally_unwind_detach",
        )?;
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            let created = self
                .builder
                .build_load(
                    self.context.bool_type(),
                    continuation_created_ptr,
                    "multi_escape_no_immediate_indirect_unwind_cont_created",
                )?
                .into_int_value();
            let unwind_propagate_bb = self.context.append_basic_block(
                func,
                "multi_escape_no_immediate_indirect_finally_unwind_propagate",
            );
            let unwind_unpin_bb = self.context.append_basic_block(
                func,
                "multi_escape_no_immediate_indirect_finally_unwind_unpin",
            );
            self.builder
                .build_conditional_branch(created, unwind_propagate_bb, unwind_unpin_bb)?;

            self.builder.position_at_end(unwind_unpin_bb);
            let unpin = self.declare_runtime_gc_unpin();
            let _ = self.builder.build_call(
                unpin,
                &[state_raw.into()],
                "multi_escape_no_immediate_indirect_state_unpin_unwind",
            )?;
            self.builder
                .build_unconditional_branch(unwind_propagate_bb)?;

            self.builder.position_at_end(unwind_propagate_bb);
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
            "multi_escape_no_immediate_indirect_finally_detach",
        )?;
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.builder.build_unconditional_branch(done_bb)?;
        }

        self.builder.position_at_end(done_bb);
        let done_with_k_bb = self
            .context
            .append_basic_block(func, "multi_escape_no_immediate_indirect_done_with_k");
        let done_without_k_bb = self
            .context
            .append_basic_block(func, "multi_escape_no_immediate_indirect_done_without_k");
        let done_merge_bb = self
            .context
            .append_basic_block(func, "multi_escape_no_immediate_indirect_done_merge");
        let created = self
            .builder
            .build_load(
                self.context.bool_type(),
                continuation_created_ptr,
                "multi_escape_no_immediate_indirect_done_cont_created",
            )?
            .into_int_value();
        self.builder
            .build_conditional_branch(created, done_with_k_bb, done_without_k_bb)?;

        self.builder.position_at_end(done_with_k_bb);
        let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
        let k_loaded = self
            .builder
            .build_load(
                llvm_ref_ty,
                cont_ptr,
                "multi_escape_no_immediate_indirect_k_unpin_load",
            )?
            .into_pointer_value();
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self.builder.build_call(
            unpin,
            &[k_loaded.into()],
            "multi_escape_no_immediate_indirect_k_unpin",
        )?;
        self.builder.build_unconditional_branch(done_merge_bb)?;

        self.builder.position_at_end(done_without_k_bb);
        let _ = self.builder.build_call(
            unpin,
            &[state_raw.into()],
            "multi_escape_no_immediate_indirect_state_unpin_done",
        )?;
        self.builder.build_unconditional_branch(done_merge_bb)?;

        self.builder.position_at_end(done_merge_bb);
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
                let loaded = self.builder.build_load(
                    llvm_ty,
                    ptr,
                    "handle_multi_escape_no_immediate_indirect_result",
                )?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }

    fn codegen_handle_expr_escape_with_nonresuming_siblings_indirect_multi<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        escape: (&'hir hir::HandleArm, hir::SymbolId),
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        #[derive(Clone, Copy)]
        struct CustomSiblingArm<'hir> {
            arm: &'hir hir::HandleArm,
            op_tag: u32,
        }

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

        let direct_sites = self.scan_mixed_escape_direct_sites(handle, &escape_arm.op.op.fqn)?;
        let mut indirect_sites = self.scan_mixed_escape_indirect_sites(handle)?;
        if !direct_sites.is_empty() || indirect_sites.is_empty() {
            let at = direct_sites
                .first()
                .map(|site| site.decl.span)
                .or_else(|| indirect_sites.first().map(|site| site.decl.span))
                .unwrap_or(span);
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multi-arm without immediate-resume (only top-level indirect site-matrix supported)",
                at: at.into(),
            });
        }
        indirect_sites.sort_by_key(|site| (site.top_level_stmt_idx, site.decl.span.start));

        let mut escape_site_pcs_by_stmt_idx: HashMap<usize, Vec<usize>> = HashMap::new();
        for (pc, site) in indirect_sites.iter().enumerate() {
            if !site.resume_path.is_empty()
                && !Self::mixed_escape_block_only_path_supported(&site.resume_path)
                && !Self::mixed_escape_if_branch_path_supported(&site.resume_path)
                && !Self::mixed_escape_while_nested_path_supported(&site.resume_path)
            {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multi-arm without immediate-resume (only top-level, statement-position nested block, if-branch, or while-body indirect sites supported)",
                    at: site.decl.span.into(),
                });
            }
            escape_site_pcs_by_stmt_idx
                .entry(site.top_level_stmt_idx)
                .or_default()
                .push(pc);
        }

        let mut if_indirect_site_pcs_by_stmt_idx: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut while_indirect_site_pc_by_stmt_idx: HashMap<usize, usize> = HashMap::new();
        let mut simple_escape_site_pc_by_stmt_idx: HashMap<usize, usize> = HashMap::new();
        for (stmt_idx, site_pcs) in &escape_site_pcs_by_stmt_idx {
            let mut then_site_pc: Option<usize> = None;
            let mut else_site_pc: Option<usize> = None;
            let mut while_site_pc: Option<usize> = None;
            let mut simple_site_pc: Option<usize> = None;
            for &pc in site_pcs {
                let site = &indirect_sites[pc];
                match site.resume_path.first() {
                    Some(MixedEscapeDirectFrame::IfThen { .. }) => {
                        if then_site_pc.replace(pc).is_some() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-arm without immediate-resume (multiple indirect sites in the same if-then branch not yet supported)",
                                at: site.decl.span.into(),
                            });
                        }
                    }
                    Some(MixedEscapeDirectFrame::IfElse { .. }) => {
                        if else_site_pc.replace(pc).is_some() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-arm without immediate-resume (multiple indirect sites in the same if-else branch not yet supported)",
                                at: site.decl.span.into(),
                            });
                        }
                    }
                    Some(MixedEscapeDirectFrame::Block { .. }) | None => {
                        if simple_site_pc.replace(pc).is_some() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-arm without immediate-resume (multiple sites per top-level statement not yet supported)",
                                at: handle.body.stmts[*stmt_idx].span.into(),
                            });
                        }
                    }
                    Some(MixedEscapeDirectFrame::WhileBody { .. }) => {
                        if while_site_pc.replace(pc).is_some() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-arm without immediate-resume (multiple sites in the same while body not yet supported)",
                                at: handle.body.stmts[*stmt_idx].span.into(),
                            });
                        }
                    }
                }
            }

            if then_site_pc.is_some() || else_site_pc.is_some() {
                if simple_site_pc.is_some() || while_site_pc.is_some() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-arm without immediate-resume (multiple sites per top-level statement not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
                let mut branch_site_pcs = Vec::new();
                if let Some(pc) = then_site_pc {
                    branch_site_pcs.push(pc);
                }
                if let Some(pc) = else_site_pc {
                    branch_site_pcs.push(pc);
                }
                if_indirect_site_pcs_by_stmt_idx.insert(*stmt_idx, branch_site_pcs);
                continue;
            }

            if let Some(site_pc) = while_site_pc {
                if simple_site_pc.is_some() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-arm without immediate-resume (multiple sites per top-level statement not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
                while_indirect_site_pc_by_stmt_idx.insert(*stmt_idx, site_pc);
                continue;
            }

            if let Some(site_pc) = simple_site_pc {
                simple_escape_site_pc_by_stmt_idx.insert(*stmt_idx, site_pc);
            }
        }

        if escape_arm.op.binders.len() > 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multi-arm without immediate-resume escape binder count (indirect, only 1 supported)",
                at: escape_arm.op.span.into(),
            });
        }

        let first_site = indirect_sites
            .first()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multi-arm without immediate-resume (indirect site missing)",
                at: span.into(),
            })?;
        let escape_resume_value_ty =
            self.cg_ty_of(first_site.decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape perform value type",
                    at: first_site.decl.span.into(),
                })?;
        for site in indirect_sites.iter().skip(1) {
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

        let mut body_lift_ids: HashSet<hir::SymbolId> = HashSet::new();
        for site in &indirect_sites {
            let Some(&site_order) = body_decl_order.get(&site.id) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation perform binding id",
                    at: site.decl.span.into(),
                });
            };

            let mut used_after: HashSet<hir::SymbolId> = HashSet::new();
            Self::collect_mixed_escape_used_after_indirect_site(
                site,
                &handle.body.stmts,
                &mut used_after,
            );

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
                    at: first_site.decl.span.into(),
                });
            };
            body_visible_supported.push(meta);
        }
        body_visible_supported.sort_by_key(|meta| meta.id.as_u32());
        let matrix_escape_sites: Vec<MatrixEscapeSite<'hir>> = indirect_sites
            .iter()
            .map(|site| MatrixEscapeSite {
                stmt_idx: site.top_level_stmt_idx,
                decl: site.decl,
                id: site.id,
                kind: MatrixEscapeSiteKind::Indirect { site: site.clone() },
            })
            .collect();

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

        let state_ty_name =
            format!("scoop.runtime.MultiEscapeNoImmediateIndirectMatrixState__{func_name}_{seq}");
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

        let step_name =
            format!("__scoop_multi_escape_no_immediate_indirect_matrix_step__{func_name}_{seq}");
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
                    kind: "multi escape no-immediate indirect-matrix step state param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_ptr_ty = cg.llvm_ptr_type(cg.gc_address_space());
            let state_ptr = cg.builder.build_pointer_cast(
                state_raw,
                state_ptr_ty,
                "multi_escape_no_immediate_indirect_matrix_step_state_ptr",
            )?;

            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect-matrix step resume_word param",
                    at: span.into(),
                })?
                .into_int_value();
            let resume_gc_ref = step_fn
                .get_nth_param(2)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect-matrix step resume_gc_ref param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_pc_ptr = cg.builder.build_struct_gep(
                state_ty,
                state_ptr,
                2,
                "multi_escape_no_immediate_indirect_matrix_step_pc_gep",
            )?;

            for (idx, cap) in outer_visible_supported.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "multi_escape_no_immediate_indirect_matrix_step_outer_gep",
                )?;
                let name = format!(
                    "multi_escape_no_immediate_indirect_matrix_outer_{}",
                    cap.id.as_u32()
                );
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
                    "multi_escape_no_immediate_indirect_matrix_step_body_gep",
                )?;
                let name = format!(
                    "multi_escape_no_immediate_indirect_matrix_body_{}",
                    cap.id.as_u32()
                );
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
                let binder_ty =
                    cg.cg_ty_of(binder.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape binder type",
                            at: binder.span.into(),
                        })?;
                let ptr = cg.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
                step_escape_binder_slots.push(ImmediateResumeBinderSlot {
                    id: binder.id,
                    hir_ty: binder.ty,
                    ty: binder_ty,
                    ptr,
                });
            }
            let step_cont_ptr = cg.create_entry_alloca(
                span,
                &format!("handle_multi_escape_no_immediate_indirect_matrix_step_k_{seq}"),
                CgTy::Ref,
            )?;

            let step_has_sibling_dispatch = has_sibling_nonresuming;
            let step_effect_dispatch_bb = if step_has_sibling_dispatch {
                Some(self.context.append_basic_block(
                    step_fn,
                    "multi_escape_no_immediate_indirect_matrix_step_effect_dispatch",
                ))
            } else {
                None
            };
            let step_effect_dispatch_nomatch_bb = if step_has_sibling_dispatch {
                Some(self.context.append_basic_block(
                    step_fn,
                    "multi_escape_no_immediate_indirect_matrix_step_effect_dispatch_nomatch",
                ))
            } else {
                None
            };
            let step_raise_catch_bb = if raise_sibling.is_some() {
                Some(self.context.append_basic_block(
                    step_fn,
                    "multi_escape_no_immediate_indirect_matrix_step_raise_catch",
                ))
            } else {
                None
            };
            let mut step_custom_catch_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            for (idx, _) in custom_siblings.iter().enumerate() {
                step_custom_catch_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_escape_no_immediate_indirect_matrix_step_custom_catch_{idx}"),
                ));
            }
            let step_escape_dispatch_bb = self.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_indirect_matrix_step_escape_dispatch",
            );
            let step_escape_fallback_bb = self.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_indirect_matrix_step_escape_fallback",
            );
            let step_escape_arm_bb = self.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_indirect_matrix_step_escape_arm",
            );
            let step_escape_arm_unwind_bb = if has_sibling_nonresuming {
                Some(self.context.append_basic_block(
                    step_fn,
                    "multi_escape_no_immediate_indirect_matrix_step_escape_arm_unwind",
                ))
            } else {
                None
            };
            let dispatch_bb = self.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_indirect_matrix_step_dispatch",
            );
            let bad_state_bb = self.context.append_basic_block(
                step_fn,
                "multi_escape_no_immediate_indirect_matrix_step_bad_pc",
            );
            let mut state_bbs = Vec::new();
            for pc in 0..indirect_sites.len() {
                state_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_escape_no_immediate_indirect_matrix_step_pc_{pc}"),
                ));
            }

            let rt_get_callee = cg.declare_runtime_callee_suspend_state_get();
            let get_call = cg.builder.build_call(
                rt_get_callee,
                &[],
                "multi_escape_no_immediate_indirect_matrix_step_callee_state_get",
            )?;
            let callee_state_raw = get_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect-matrix step callee_state_get return",
                    at: span.into(),
                })?
                .into_pointer_value();
            let callee_prefix_ty = cg.llvm_callee_suspend_state_prefix_type();
            let callee_state_ptr_ty = cg.llvm_ptr_type(AddressSpace::default());
            let callee_state_ptr = cg.builder.build_pointer_cast(
                callee_state_raw,
                callee_state_ptr_ty,
                "multi_escape_no_immediate_indirect_matrix_step_callee_state_typed",
            )?;
            let callee_rw_ptr = cg.builder.build_struct_gep(
                callee_prefix_ty,
                callee_state_ptr,
                1,
                "multi_escape_no_immediate_indirect_matrix_step_resume_word_gep",
            )?;
            let _ = cg.builder.build_store(callee_rw_ptr, resume_word)?;
            let callee_rg_ptr = cg.builder.build_struct_gep(
                callee_prefix_ty,
                callee_state_ptr,
                2,
                "multi_escape_no_immediate_indirect_matrix_step_resume_gc_ref_gep",
            )?;
            let wb = cg.declare_runtime_gc_write_barrier();
            let slot_addr = cg.builder.build_pointer_cast(
                callee_rg_ptr,
                i8_ptr_ty,
                "multi_escape_no_immediate_indirect_matrix_step_resume_gc_slot",
            )?;
            let _ = cg.builder.build_call(
                wb,
                &[slot_addr.into(), resume_gc_ref.into()],
                "multi_escape_no_immediate_indirect_matrix_step_resume_gc_store",
            )?;

            cg.builder.build_unconditional_branch(dispatch_bb)?;

            cg.builder.position_at_end(dispatch_bb);
            let pc = cg
                .builder
                .build_load(
                    i32_ty,
                    state_pc_ptr,
                    "multi_escape_no_immediate_indirect_matrix_step_pc",
                )?
                .into_int_value();
            let mut cases = Vec::new();
            for (pc, bb) in state_bbs.iter().enumerate() {
                cases.push((i32_ty.const_int(pc as u64, false), *bb));
            }
            cg.builder.build_switch(pc, bad_state_bb, &cases)?;

            cg.builder.position_at_end(bad_state_bb);
            cg.emit_exit_with_code(span, 3)?;

            for (site_pc, state_bb) in state_bbs.iter().enumerate() {
                let site = &indirect_sites[site_pc];
                cg.builder.position_at_end(*state_bb);

                if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                    for (idx, custom) in custom_siblings.iter().enumerate() {
                        cg.push_effect_unwind_target(
                            &custom.arm.op.op.fqn,
                            step_custom_catch_bbs[idx],
                        );
                    }
                    cg.push_raise_target(step_effect_dispatch_bb);
                }

                for _ in &site.resume_path {
                    cg.env.push_scope();
                }
                cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(site, &body_lift_ids)?;
                if let Some(bb) = cg.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    if matches!(
                        site.resume_path.first(),
                        Some(MixedEscapeDirectFrame::WhileBody { .. })
                    ) {
                        cg.codegen_mixed_escape_matrix_while_tail_after_indirect_site(
                            site_pc,
                            site,
                            &body_lift_ids,
                            |cg, next_pc, next_site| {
                                cg.capture_escape_state_with_pc(
                                    next_site.decl.span,
                                    state_ty,
                                    state_ptr,
                                    &outer_visible_supported,
                                    outer_field_base,
                                    &body_visible_supported,
                                    body_field_base,
                                    2,
                                    next_pc,
                                )?;

                                if step_effect_dispatch_bb.is_some() {
                                    cg.pop_raise_target();
                                }
                                cg.push_raise_target(step_escape_dispatch_bb);
                                cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                    next_site,
                                    &body_lift_ids,
                                )?;
                                cg.pop_raise_target();
                                if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                                    cg.push_raise_target(step_effect_dispatch_bb);
                                }
                                Ok(())
                            },
                        )?;
                    } else {
                        cg.codegen_mixed_escape_matrix_continue_after_indirect_site(
                            site,
                            &body_lift_ids,
                        )?;
                    }
                }

                for (idx, stmt) in handle
                    .body
                    .stmts
                    .iter()
                    .enumerate()
                    .skip(site.top_level_stmt_idx + 1)
                {
                    if let Some(indirect_site_pcs) = if_indirect_site_pcs_by_stmt_idx.get(&idx) {
                        cg.codegen_mixed_escape_matrix_if_stmt_indirect_sites(
                            stmt,
                            indirect_site_pcs,
                            &matrix_escape_sites,
                            &body_lift_ids,
                            |cg, next_pc, next_site| {
                                cg.capture_escape_state_with_pc(
                                    next_site.decl.span,
                                    state_ty,
                                    state_ptr,
                                    &outer_visible_supported,
                                    outer_field_base,
                                    &body_visible_supported,
                                    body_field_base,
                                    2,
                                    next_pc,
                                )?;

                                if step_effect_dispatch_bb.is_some() {
                                    cg.pop_raise_target();
                                }
                                cg.push_raise_target(step_escape_dispatch_bb);
                                cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                    next_site,
                                    &body_lift_ids,
                                )?;
                                cg.pop_raise_target();
                                if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                                    cg.push_raise_target(step_effect_dispatch_bb);
                                }
                                Ok(())
                            },
                        )?;
                        continue;
                    }

                    if let Some(next_pc) = while_indirect_site_pc_by_stmt_idx.get(&idx).copied() {
                        let next_site = &indirect_sites[next_pc];
                        cg.codegen_mixed_escape_matrix_while_stmt_indirect_site(
                            stmt,
                            next_pc,
                            next_site,
                            &body_lift_ids,
                            |cg, next_pc, next_site| {
                                cg.capture_escape_state_with_pc(
                                    next_site.decl.span,
                                    state_ty,
                                    state_ptr,
                                    &outer_visible_supported,
                                    outer_field_base,
                                    &body_visible_supported,
                                    body_field_base,
                                    2,
                                    next_pc,
                                )?;

                                if step_effect_dispatch_bb.is_some() {
                                    cg.pop_raise_target();
                                }
                                cg.push_raise_target(step_escape_dispatch_bb);
                                cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                                    next_site,
                                    &body_lift_ids,
                                )?;
                                cg.pop_raise_target();
                                if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                                    cg.push_raise_target(step_effect_dispatch_bb);
                                }
                                Ok(())
                            },
                        )?;
                        continue;
                    }

                    if let Some(&next_pc) = simple_escape_site_pc_by_stmt_idx.get(&idx) {
                        let next_site = &indirect_sites[next_pc];
                        if !next_site.resume_path.is_empty() {
                            cg.codegen_mixed_escape_matrix_prefix_to_indirect_site(
                                next_site,
                                stmt,
                                &body_lift_ids,
                            )?;
                        }
                        cg.capture_escape_state_with_pc(
                            next_site.decl.span,
                            state_ty,
                            state_ptr,
                            &outer_visible_supported,
                            outer_field_base,
                            &body_visible_supported,
                            body_field_base,
                            2,
                            next_pc,
                        )?;

                        if step_effect_dispatch_bb.is_some() {
                            cg.pop_raise_target();
                        }
                        cg.push_raise_target(step_escape_dispatch_bb);
                        cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                            next_site,
                            &body_lift_ids,
                        )?;
                        cg.pop_raise_target();
                        if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                            cg.push_raise_target(step_effect_dispatch_bb);
                        }
                        if let Some(bb) = cg.builder.get_insert_block()
                            && bb.get_terminator().is_none()
                        {
                            cg.codegen_mixed_escape_matrix_continue_after_indirect_site(
                                next_site,
                                &body_lift_ids,
                            )?;
                        }
                        continue;
                    }

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

                if step_effect_dispatch_bb.is_some() {
                    cg.pop_raise_target();
                    for _ in custom_siblings.iter().rev() {
                        cg.pop_effect_unwind_target();
                    }
                }

                if let Some(bb) = cg.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    let unpin = cg.declare_runtime_gc_unpin();
                    let _ = cg.builder.build_call(
                        unpin,
                        &[state_raw.into()],
                        "multi_escape_no_immediate_indirect_matrix_step_state_unpin",
                    )?;
                    cg.builder.build_return(None)?;
                }
            }

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                let step_effect_dispatch_nomatch_bb = step_effect_dispatch_nomatch_bb
                    .expect("no-immediate indirect-matrix step dispatch_nomatch bb should exist");
                cg.builder.position_at_end(step_effect_dispatch_bb);
                let rt_read_tag = cg.declare_runtime_effect_perform_slot_read_op_tag();
                let tag_call = cg.builder.build_call(
                    rt_read_tag,
                    &[],
                    "multi_escape_no_immediate_indirect_matrix_step_read_op_tag",
                )?;
                let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate indirect-matrix step read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate indirect-matrix step read_op_tag return type",
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
                    "multi_escape_no_immediate_indirect_matrix_step_state_unpin_nomatch",
                )?;
                cg.builder.build_return(None)?;

                if let (Some(raise_arm), Some(step_raise_catch_bb)) =
                    (raise_sibling, step_raise_catch_bb)
                {
                    let binder = &raise_arm.op.binders[0];
                    cg.builder.position_at_end(step_raise_catch_bb);

                    let frame_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        1,
                        "multi_escape_no_immediate_indirect_matrix_step_raise_frame_gep",
                    )?;
                    let prev_ptr = cg.builder.build_struct_gep(
                        handler_frame_ty,
                        frame_ptr,
                        0,
                        "multi_escape_no_immediate_indirect_matrix_step_raise_prev_gep",
                    )?;
                    let prev_raw = cg.builder.build_load(
                        i8_ptr_ty,
                        prev_ptr,
                        "multi_escape_no_immediate_indirect_matrix_step_raise_prev",
                    )?;
                    let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                    let _ = cg.builder.build_call(
                        rt_swap,
                        &[prev_raw.into()],
                        "multi_escape_no_immediate_indirect_matrix_step_raise_detach",
                    )?;

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "multi_escape_no_immediate_indirect_matrix_step_raise_read_slot_len_words",
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
                        "multi_escape_no_immediate_indirect_matrix_step_raise_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_indirect_matrix_step_raise_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_indirect_matrix_step_raise_slot_len_bad_bb",
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
                        "multi_escape_no_immediate_indirect_matrix_step_raise_read_slot_word0",
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
                        "multi_escape_no_immediate_indirect_matrix_step_raise_read_slot_word1",
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
                        "multi_escape_no_immediate_indirect_matrix_step_raise_clear",
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
                                "multi_escape_no_immediate_indirect_matrix_step_raise_kind_is_int",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_indirect_matrix_step_raise_kind_int_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_indirect_matrix_step_raise_kind_int_bad",
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
                                "multi_escape_no_immediate_indirect_matrix_step_raise_kind_is_runtime_error",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_indirect_matrix_step_raise_kind_runtime_error_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_indirect_matrix_step_raise_kind_runtime_error_bad",
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
                                "multi_escape_no_immediate_indirect_matrix_step_runtime_error_tag_i32",
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
                                "multi_escape_no_immediate_indirect_matrix_step_runtime_error_tag",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_word_zero,
                                1,
                                "multi_escape_no_immediate_indirect_matrix_step_runtime_error_payload_word",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_ptr_zero,
                                2,
                                "multi_escape_no_immediate_indirect_matrix_step_runtime_error_payload_ptr",
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
                            "multi_escape_no_immediate_indirect_matrix_step_state_unpin_raise",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }

                for (idx, custom) in custom_siblings.iter().enumerate() {
                    let arm = custom.arm;
                    let binder = &arm.op.binders[0];
                    cg.builder.position_at_end(step_custom_catch_bbs[idx]);

                    let frame_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        1,
                        "multi_escape_no_immediate_indirect_matrix_step_custom_frame_gep",
                    )?;
                    let prev_ptr = cg.builder.build_struct_gep(
                        handler_frame_ty,
                        frame_ptr,
                        0,
                        "multi_escape_no_immediate_indirect_matrix_step_custom_prev_gep",
                    )?;
                    let prev_raw = cg.builder.build_load(
                        i8_ptr_ty,
                        prev_ptr,
                        "multi_escape_no_immediate_indirect_matrix_step_custom_prev",
                    )?;
                    let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                    let _ = cg.builder.build_call(
                        rt_swap,
                        &[prev_raw.into()],
                        "multi_escape_no_immediate_indirect_matrix_step_custom_detach",
                    )?;

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "multi_escape_no_immediate_indirect_matrix_step_custom_read_slot_len_words",
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
                        "multi_escape_no_immediate_indirect_matrix_step_custom_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_indirect_matrix_step_custom_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_indirect_matrix_step_custom_slot_len_bad_bb",
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
                        "multi_escape_no_immediate_indirect_matrix_step_custom_read_slot_word0",
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
                        "multi_escape_no_immediate_indirect_matrix_step_custom_read_slot_gc_ref",
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
                        "multi_escape_no_immediate_indirect_matrix_step_custom_clear",
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
                            "multi_escape_no_immediate_indirect_matrix_step_state_unpin_custom",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }
            }

            cg.builder.position_at_end(step_escape_dispatch_bb);
            let rt_read_tag = cg.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = cg.builder.build_call(
                rt_read_tag,
                &[],
                "multi_escape_no_immediate_indirect_matrix_step_escape_read_op_tag",
            )?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect-matrix step escape read_op_tag return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect-matrix step escape read_op_tag return type",
                    at: span.into(),
                });
            };
            let escape_tag = self.effect_op_tag(&escape_arm.op.op.fqn);
            let tag_matches = cg.builder.build_int_compare(
                IntPredicate::EQ,
                slot_tag,
                i32_ty.const_int(escape_tag as u64, false),
                "multi_escape_no_immediate_indirect_matrix_step_escape_tag_eq",
            )?;
            cg.builder.build_conditional_branch(
                tag_matches,
                step_escape_arm_bb,
                step_escape_fallback_bb,
            )?;

            cg.builder.position_at_end(step_escape_fallback_bb);
            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                cg.builder
                    .build_unconditional_branch(step_effect_dispatch_bb)?;
            } else {
                let unpin = cg.declare_runtime_gc_unpin();
                let _ = cg.builder.build_call(
                    unpin,
                    &[state_raw.into()],
                    "multi_escape_no_immediate_indirect_matrix_step_state_unpin_escape_nomatch",
                )?;
                cg.builder.build_return(None)?;
            }

            cg.builder.position_at_end(step_escape_arm_bb);
            if let Some(slot) = step_escape_binder_slots.first() {
                let rt_read = cg.declare_runtime_effect_perform_slot_read_u64();
                let word_call = cg.builder.build_call(
                    rt_read,
                    &[],
                    "multi_escape_no_immediate_indirect_matrix_step_arm_read_binder_word",
                )?;
                let word_raw = word_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate indirect-matrix step arm read binder return",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(word_u64) = word_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate indirect-matrix step arm read binder type",
                        at: span.into(),
                    });
                };
                let rt_read_gc = cg.declare_runtime_effect_perform_slot_read_gc_ref();
                let gc_call = cg.builder.build_call(
                    rt_read_gc,
                    &[],
                    "multi_escape_no_immediate_indirect_matrix_step_arm_read_binder_gc",
                )?;
                let gc_raw = gc_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate indirect-matrix step arm read binder gc value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(gc_ref_raw) = gc_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate indirect-matrix step arm read binder gc type",
                        at: span.into(),
                    });
                };
                let binder_value =
                    cg.decode_abi_payload_transport(span, word_u64, gc_ref_raw, slot.ty)?;
                let _ = cg.store_local_value(span, slot.ptr, slot.ty, binder_value)?;
            }

            let rt_clear = cg.declare_runtime_effect_clear();
            let _ = cg.builder.build_call(
                rt_clear,
                &[],
                "multi_escape_no_immediate_indirect_matrix_step_arm_effect_clear",
            )?;

            let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
            let step_ptr = step_fn.as_global_value().as_pointer_value();
            let cont_call = cg.builder.build_call(
                rt_cont_alloc,
                &[state_raw.into(), step_ptr.into()],
                "multi_escape_no_immediate_indirect_matrix_step_cont_alloc",
            )?;
            let cont_raw = cont_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect-matrix step continuation alloc return value",
                    at: escape_arm.span.into(),
                })?;
            let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect-matrix step continuation alloc return type",
                    at: escape_arm.span.into(),
                });
            };
            let pin = cg.declare_runtime_gc_pin();
            let _ = cg.builder.build_call(
                pin,
                &[k_raw.into()],
                "multi_escape_no_immediate_indirect_matrix_step_k_pin",
            )?;
            let _ = cg.store_local_value(
                span,
                step_cont_ptr,
                CgTy::Ref,
                CgValue {
                    ty: CgTy::Ref,
                    value: Some(k_raw.into()),
                },
            )?;

            let frame_ptr = cg.builder.build_struct_gep(
                state_ty,
                state_ptr,
                1,
                "multi_escape_no_immediate_indirect_matrix_step_arm_frame_gep",
            )?;
            let prev_ptr = cg.builder.build_struct_gep(
                handler_frame_ty,
                frame_ptr,
                0,
                "multi_escape_no_immediate_indirect_matrix_step_arm_prev_gep",
            )?;
            let prev_raw = cg.builder.build_load(
                i8_ptr_ty,
                prev_ptr,
                "multi_escape_no_immediate_indirect_matrix_step_arm_prev",
            )?;
            let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
            let _ = cg.builder.build_call(
                rt_swap,
                &[prev_raw.into()],
                "multi_escape_no_immediate_indirect_matrix_step_arm_detach",
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
                    ptr: step_cont_ptr,
                    mutable: false,
                },
            );
            if let Some(step_escape_arm_unwind_bb) = step_escape_arm_unwind_bb {
                for custom in &custom_siblings {
                    cg.push_effect_unwind_target(&custom.arm.op.op.fqn, step_escape_arm_unwind_bb);
                }
                cg.push_raise_target(step_escape_arm_unwind_bb);
            }
            let arm_v = cg.codegen_expr_in_expected_context(&escape_arm.body, Some(out_ty))?;
            if step_escape_arm_unwind_bb.is_some() {
                cg.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    cg.pop_effect_unwind_target();
                }
            }
            let _arm_v = if out_ty == CgTy::Unit {
                CgValue::unit()
            } else {
                cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
            };
            cg.env.pop_scope();

            if let Some(bb) = cg.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                let llvm_ref_ty = cg.llvm_basic_type_of(span, CgTy::Ref)?;
                let k_loaded = cg
                    .builder
                    .build_load(
                        llvm_ref_ty,
                        step_cont_ptr,
                        "multi_escape_no_immediate_indirect_matrix_step_k_unpin_load",
                    )?
                    .into_pointer_value();
                let unpin = cg.declare_runtime_gc_unpin();
                let _ = cg.builder.build_call(
                    unpin,
                    &[k_loaded.into()],
                    "multi_escape_no_immediate_indirect_matrix_step_k_unpin",
                )?;
                cg.builder.build_return(None)?;
            }

            if let Some(step_escape_arm_unwind_bb) = step_escape_arm_unwind_bb {
                cg.builder.position_at_end(step_escape_arm_unwind_bb);
                cg.builder.build_return(None)?;
            }

            cg.env.pop_scope();
        }
        self.builder.position_at_end(saved_block);

        let body_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_indirect_matrix_body",
        );
        let escape_dispatch_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_indirect_matrix_dispatch",
        );
        let escape_arm_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_no_immediate_indirect_matrix_arm");
        let done_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_indirect_matrix_done",
        );
        let finally_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_indirect_matrix_finally",
        );
        let finally_unwind_bb = self.context.append_basic_block(
            func,
            "handle_multi_escape_no_immediate_indirect_matrix_finally_unwind",
        );
        let effect_dispatch_bb = if has_sibling_nonresuming {
            Some(self.context.append_basic_block(
                func,
                "handle_multi_escape_no_immediate_indirect_matrix_effect_dispatch",
            ))
        } else {
            None
        };
        let effect_dispatch_nomatch_bb = if has_sibling_nonresuming {
            Some(self.context.append_basic_block(
                func,
                "handle_multi_escape_no_immediate_indirect_matrix_effect_dispatch_nomatch",
            ))
        } else {
            None
        };
        let raise_catch_bb = if raise_sibling.is_some() {
            Some(self.context.append_basic_block(
                func,
                "handle_multi_escape_no_immediate_indirect_matrix_raise_catch",
            ))
        } else {
            None
        };
        let mut custom_catch_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        for (idx, _) in custom_siblings.iter().enumerate() {
            custom_catch_bbs.push(self.context.append_basic_block(
                func,
                &format!("handle_multi_escape_no_immediate_indirect_matrix_custom_catch_{idx}"),
            ));
        }

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(
                span,
                "handle_multi_escape_no_immediate_indirect_matrix_result",
                out_ty,
            )?)
        };
        let continuation_created_ptr = self.create_entry_alloca_raw(
            span,
            "handle_multi_escape_no_immediate_indirect_matrix_cont_created",
            self.context.bool_type().into(),
        )?;
        let _ = self.builder.build_store(
            continuation_created_ptr,
            self.context.bool_type().const_zero(),
        )?;

        let mut escape_binder_slots: Vec<ImmediateResumeBinderSlot<'ctx>> = Vec::new();
        for binder in &escape_arm.op.binders {
            let binder_ty = self
                .cg_ty_of(binder.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape binder type",
                    at: binder.span.into(),
                })?;
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
            &format!("handle_multi_escape_no_immediate_indirect_matrix_k_{seq}"),
            CgTy::Ref,
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
        let state_desc_global_name = format!(
            "__scoop_type_desc_multi_escape_no_immediate_indirect_matrix_state__{func_name}_{seq}"
        );
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
            "multi_escape_no_immediate_indirect_matrix_state_desc_i8",
        )?;
        let alloc_call = self.builder.build_call(
            rt_alloc,
            &[state_desc_i8.into(), size_v.into()],
            "rt_alloc_multi_escape_no_immediate_indirect_matrix_state",
        )?;
        let alloc_raw =
            alloc_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect-matrix alloc return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::PointerValue(state_raw) = alloc_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate indirect-matrix alloc return type",
                at: span.into(),
            });
        };

        let pin = self.declare_runtime_gc_pin();
        let _ = self.builder.build_call(
            pin,
            &[state_raw.into()],
            "multi_escape_no_immediate_indirect_matrix_state_pin",
        )?;

        let state_gc_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let state_gc_ptr = self.builder.build_pointer_cast(
            state_raw,
            state_gc_ptr_ty,
            "multi_escape_no_immediate_indirect_matrix_state_ptr",
        )?;

        let state_pc_ptr = self.builder.build_struct_gep(
            state_ty,
            state_gc_ptr,
            2,
            "multi_escape_no_immediate_indirect_matrix_state_pc_gep",
        )?;
        let _ = self
            .builder
            .build_store(state_pc_ptr, i32_ty.const_zero())?;

        for (idx, cap) in outer_visible_supported.iter().enumerate() {
            let field_idx = outer_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_escape_no_immediate_indirect_matrix_state_outer_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }
        for (idx, cap) in body_visible_supported.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_escape_no_immediate_indirect_matrix_state_body_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }

        let frame_ptr = self.builder.build_struct_gep(
            state_ty,
            state_gc_ptr,
            1,
            "multi_escape_no_immediate_indirect_matrix_state_frame_gep",
        )?;
        let frame_i8 = self.builder.build_address_space_cast(
            frame_ptr,
            i8_ptr_ty,
            "multi_escape_no_immediate_indirect_matrix_frame_i8",
        )?;
        let escape_tag = self.effect_op_tag(&escape_arm.op.op.fqn);
        let escape_tag_i32 = i32_ty.const_int(escape_tag as u64, false);
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), escape_tag_i32.into()],
            "multi_escape_no_immediate_indirect_matrix_push",
        )?;
        let prev_ptr = self.builder.build_struct_gep(
            handler_frame_ty,
            frame_ptr,
            0,
            "multi_escape_no_immediate_indirect_matrix_prev_gep",
        )?;
        let escape_outer_top = self
            .builder
            .build_load(
                i8_ptr_ty,
                prev_ptr,
                "multi_escape_no_immediate_indirect_matrix_outer_top",
            )?
            .into_pointer_value();
        let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
        let main_raise_target = effect_dispatch_bb.unwrap_or(finally_unwind_bb);

        self.builder.build_unconditional_branch(body_bb)?;

        self.builder.position_at_end(body_bb);
        self.env.push_scope();
        for (idx, custom) in custom_siblings.iter().enumerate() {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom_catch_bbs[idx]);
        }
        self.push_raise_target(main_raise_target);

        let mut body_tail: Option<CgValue<'ctx>> = None;
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if let Some(indirect_site_pcs) = if_indirect_site_pcs_by_stmt_idx.get(&idx) {
                self.codegen_mixed_escape_matrix_if_stmt_indirect_sites(
                    stmt,
                    indirect_site_pcs,
                    &matrix_escape_sites,
                    &body_lift_ids,
                    |cg, site_pc, site| {
                        cg.capture_escape_state_with_pc(
                            site.decl.span,
                            state_ty,
                            state_gc_ptr,
                            &outer_visible_supported,
                            outer_field_base,
                            &body_visible_supported,
                            body_field_base,
                            2,
                            site_pc,
                        )?;
                        cg.pop_raise_target();
                        cg.push_raise_target(escape_dispatch_bb);
                        cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                            site,
                            &body_lift_ids,
                        )?;
                        cg.pop_raise_target();
                        cg.push_raise_target(main_raise_target);
                        Ok(())
                    },
                )?;
                body_tail = None;
                continue;
            }

            if let Some(&site_pc) = while_indirect_site_pc_by_stmt_idx.get(&idx) {
                let site = &indirect_sites[site_pc];
                self.codegen_mixed_escape_matrix_while_stmt_indirect_site(
                    stmt,
                    site_pc,
                    site,
                    &body_lift_ids,
                    |cg, site_pc, site| {
                        cg.capture_escape_state_with_pc(
                            site.decl.span,
                            state_ty,
                            state_gc_ptr,
                            &outer_visible_supported,
                            outer_field_base,
                            &body_visible_supported,
                            body_field_base,
                            2,
                            site_pc,
                        )?;
                        cg.pop_raise_target();
                        cg.push_raise_target(escape_dispatch_bb);
                        cg.codegen_mixed_escape_matrix_emit_indirect_site_binding(
                            site,
                            &body_lift_ids,
                        )?;
                        cg.pop_raise_target();
                        cg.push_raise_target(main_raise_target);
                        Ok(())
                    },
                )?;
                body_tail = None;
                continue;
            }

            if let Some(&site_pc) = simple_escape_site_pc_by_stmt_idx.get(&idx) {
                let site = &indirect_sites[site_pc];
                if !site.resume_path.is_empty() {
                    self.codegen_mixed_escape_matrix_prefix_to_indirect_site(
                        site,
                        stmt,
                        &body_lift_ids,
                    )?;
                }
                self.capture_escape_state_with_pc(
                    site.decl.span,
                    state_ty,
                    state_gc_ptr,
                    &outer_visible_supported,
                    outer_field_base,
                    &body_visible_supported,
                    body_field_base,
                    2,
                    site_pc,
                )?;
                self.pop_raise_target();
                self.push_raise_target(escape_dispatch_bb);
                self.codegen_mixed_escape_matrix_emit_indirect_site_binding(site, &body_lift_ids)?;
                self.pop_raise_target();
                self.push_raise_target(main_raise_target);
                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    self.codegen_mixed_escape_matrix_continue_after_indirect_site(
                        site,
                        &body_lift_ids,
                    )?;
                }
                body_tail = None;
                continue;
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
                hir::StmtKind::Return { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "stmt in handle body (indirect perform)",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }
        self.env.pop_scope();

        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            if out_ty != CgTy::Unit
                && let Some(v) = body_tail
            {
                let v = self.coerce_value(handle.body.span, v, out_ty)?;
                if let Some(ptr) = result_ptr {
                    let _ = self.store_local_value(handle.body.span, ptr, out_ty, v)?;
                }
            }
            self.builder.build_unconditional_branch(finally_bb)?;
        }

        if let Some(effect_dispatch_bb) = effect_dispatch_bb {
            let effect_dispatch_nomatch_bb = effect_dispatch_nomatch_bb
                .expect("no-immediate indirect-matrix effect dispatch_nomatch bb should exist");
            self.builder.position_at_end(effect_dispatch_bb);
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = self.builder.build_call(
                rt_read_tag,
                &[],
                "multi_escape_no_immediate_indirect_matrix_dispatch_read_op_tag",
            )?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect-matrix dispatch read_op_tag return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect-matrix dispatch read_op_tag return type",
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
                    "multi_escape_no_immediate_indirect_matrix_raise_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "multi_escape_no_immediate_indirect_matrix_raise_read_slot_len_words",
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
                    "multi_escape_no_immediate_indirect_matrix_raise_slot_len_ok",
                )?;
                let len_ok_bb = self.context.append_basic_block(
                    func,
                    "multi_escape_no_immediate_indirect_matrix_raise_slot_len_ok_bb",
                );
                let len_bad_bb = self.context.append_basic_block(
                    func,
                    "multi_escape_no_immediate_indirect_matrix_raise_slot_len_bad_bb",
                );
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
                    "multi_escape_no_immediate_indirect_matrix_raise_read_slot_word0",
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
                    "multi_escape_no_immediate_indirect_matrix_raise_read_slot_word1",
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
                let _ = self.builder.build_call(
                    rt_clear,
                    &[],
                    "multi_escape_no_immediate_indirect_matrix_raise_clear",
                )?;

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
                            "multi_escape_no_immediate_indirect_matrix_raise_kind_is_int",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_indirect_matrix_raise_kind_int_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_indirect_matrix_raise_kind_int_bad",
                        );
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
                            "multi_escape_no_immediate_indirect_matrix_raise_kind_is_runtime_error",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_indirect_matrix_raise_kind_runtime_error_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_indirect_matrix_raise_kind_runtime_error_bad",
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
                            "multi_escape_no_immediate_indirect_matrix_runtime_error_tag_i32",
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
                            "multi_escape_no_immediate_indirect_matrix_runtime_error_tag",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_word_zero,
                            1,
                            "multi_escape_no_immediate_indirect_matrix_runtime_error_payload_word",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_ptr_zero,
                            2,
                            "multi_escape_no_immediate_indirect_matrix_runtime_error_payload_ptr",
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
                    "multi_escape_no_immediate_indirect_matrix_custom_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "multi_escape_no_immediate_indirect_matrix_custom_read_slot_len_words",
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
                    "multi_escape_no_immediate_indirect_matrix_custom_slot_len_ok",
                )?;
                let len_ok_bb = self.context.append_basic_block(
                    func,
                    "multi_escape_no_immediate_indirect_matrix_custom_slot_len_ok_bb",
                );
                let len_bad_bb = self.context.append_basic_block(
                    func,
                    "multi_escape_no_immediate_indirect_matrix_custom_slot_len_bad_bb",
                );
                self.builder
                    .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                self.builder.position_at_end(len_bad_bb);
                self.emit_exit_with_code(span, 3)?;

                self.builder.position_at_end(len_ok_bb);
                let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
                let value_call = self.builder.build_call(
                    rt_read,
                    &[],
                    "multi_escape_no_immediate_indirect_matrix_custom_read_slot_word0",
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
                    "multi_escape_no_immediate_indirect_matrix_custom_read_slot_gc_ref",
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
                let _ = self.builder.build_call(
                    rt_clear,
                    &[],
                    "multi_escape_no_immediate_indirect_matrix_custom_clear",
                )?;

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

        self.builder.position_at_end(escape_dispatch_bb);
        let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
        let tag_call = self.builder.build_call(
            rt_read_tag,
            &[],
            "multi_escape_no_immediate_indirect_matrix_escape_read_op_tag",
        )?;
        let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate indirect-matrix escape read_op_tag return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate indirect-matrix escape read_op_tag return type",
                at: span.into(),
            });
        };
        let tag_matches = self.builder.build_int_compare(
            IntPredicate::EQ,
            slot_tag,
            escape_tag_i32,
            "multi_escape_no_immediate_indirect_matrix_escape_tag_eq",
        )?;
        let escape_dispatch_fallback_bb = effect_dispatch_bb.unwrap_or(finally_unwind_bb);
        self.builder.build_conditional_branch(
            tag_matches,
            escape_arm_bb,
            escape_dispatch_fallback_bb,
        )?;

        self.builder.position_at_end(escape_arm_bb);
        if let Some(slot) = escape_binder_slots.first() {
            let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
            let word_call = self.builder.build_call(
                rt_read,
                &[],
                "multi_escape_no_immediate_indirect_matrix_arm_read_binder_word",
            )?;
            let word_raw = word_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect-matrix arm read binder return",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(word_u64) = word_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect-matrix arm read binder type",
                    at: span.into(),
                });
            };
            let rt_read_gc = self.declare_runtime_effect_perform_slot_read_gc_ref();
            let gc_call = self.builder.build_call(
                rt_read_gc,
                &[],
                "multi_escape_no_immediate_indirect_matrix_arm_read_binder_gc",
            )?;
            let gc_raw =
                gc_call
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate indirect-matrix arm read binder gc value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::PointerValue(gc_ref_raw) = gc_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate indirect-matrix arm read binder gc type",
                    at: span.into(),
                });
            };
            let binder_value =
                self.decode_abi_payload_transport(span, word_u64, gc_ref_raw, slot.ty)?;
            let _ = self.store_local_value(span, slot.ptr, slot.ty, binder_value)?;
        }

        let rt_clear = self.declare_runtime_effect_clear();
        let _ = self.builder.build_call(
            rt_clear,
            &[],
            "multi_escape_no_immediate_indirect_matrix_arm_effect_clear",
        )?;

        let step_ptr = step_fn.as_global_value().as_pointer_value();
        let cont_call = self.builder.build_call(
            self.declare_runtime_continuation_alloc(),
            &[state_raw.into(), step_ptr.into()],
            "multi_escape_no_immediate_indirect_matrix_cont_alloc",
        )?;
        let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate indirect-matrix continuation alloc return value",
                at: escape_arm.span.into(),
            },
        )?;
        let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate indirect-matrix continuation alloc return type",
                at: escape_arm.span.into(),
            });
        };

        let _ = self.builder.build_call(
            pin,
            &[k_raw.into()],
            "multi_escape_no_immediate_indirect_matrix_k_pin",
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
        let _ = self.builder.build_store(
            continuation_created_ptr,
            self.context.bool_type().const_all_ones(),
        )?;

        let _ = self.builder.build_call(
            rt_swap,
            &[escape_outer_top.into()],
            "multi_escape_no_immediate_indirect_matrix_detach",
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
            "multi_escape_no_immediate_indirect_matrix_finally_unwind_detach",
        )?;
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            let created = self
                .builder
                .build_load(
                    self.context.bool_type(),
                    continuation_created_ptr,
                    "multi_escape_no_immediate_indirect_matrix_unwind_cont_created",
                )?
                .into_int_value();
            let unwind_propagate_bb = self.context.append_basic_block(
                func,
                "multi_escape_no_immediate_indirect_matrix_finally_unwind_propagate",
            );
            let unwind_unpin_bb = self.context.append_basic_block(
                func,
                "multi_escape_no_immediate_indirect_matrix_finally_unwind_unpin",
            );
            self.builder
                .build_conditional_branch(created, unwind_propagate_bb, unwind_unpin_bb)?;

            self.builder.position_at_end(unwind_unpin_bb);
            let unpin = self.declare_runtime_gc_unpin();
            let _ = self.builder.build_call(
                unpin,
                &[state_raw.into()],
                "multi_escape_no_immediate_indirect_matrix_state_unpin_unwind",
            )?;
            self.builder
                .build_unconditional_branch(unwind_propagate_bb)?;

            self.builder.position_at_end(unwind_propagate_bb);
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
            "multi_escape_no_immediate_indirect_matrix_finally_detach",
        )?;
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.builder.build_unconditional_branch(done_bb)?;
        }

        self.builder.position_at_end(done_bb);
        let done_with_k_bb = self.context.append_basic_block(
            func,
            "multi_escape_no_immediate_indirect_matrix_done_with_k",
        );
        let done_without_k_bb = self.context.append_basic_block(
            func,
            "multi_escape_no_immediate_indirect_matrix_done_without_k",
        );
        let done_merge_bb = self
            .context
            .append_basic_block(func, "multi_escape_no_immediate_indirect_matrix_done_merge");
        let created = self
            .builder
            .build_load(
                self.context.bool_type(),
                continuation_created_ptr,
                "multi_escape_no_immediate_indirect_matrix_done_cont_created",
            )?
            .into_int_value();
        self.builder
            .build_conditional_branch(created, done_with_k_bb, done_without_k_bb)?;

        self.builder.position_at_end(done_with_k_bb);
        let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
        let k_loaded = self
            .builder
            .build_load(
                llvm_ref_ty,
                cont_ptr,
                "multi_escape_no_immediate_indirect_matrix_k_unpin_load",
            )?
            .into_pointer_value();
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self.builder.build_call(
            unpin,
            &[k_loaded.into()],
            "multi_escape_no_immediate_indirect_matrix_k_unpin",
        )?;
        self.builder.build_unconditional_branch(done_merge_bb)?;

        self.builder.position_at_end(done_without_k_bb);
        let _ = self.builder.build_call(
            unpin,
            &[state_raw.into()],
            "multi_escape_no_immediate_indirect_matrix_state_unpin_done",
        )?;
        self.builder.build_unconditional_branch(done_merge_bb)?;

        self.builder.position_at_end(done_merge_bb);
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
                let loaded = self.builder.build_load(
                    llvm_ty,
                    ptr,
                    "handle_multi_escape_no_immediate_indirect_matrix_result",
                )?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }

    fn codegen_handle_expr_escape_with_nonresuming_siblings_direct<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        escape: (&'hir hir::HandleArm, hir::SymbolId),
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        #[derive(Clone, Copy)]
        struct CaptureMeta {
            id: hir::SymbolId,
            hir_ty: Option<TypeId>,
            ty: CgTy,
            mutable: bool,
        }

        #[derive(Clone, Copy)]
        struct CustomSiblingArm<'hir> {
            arm: &'hir hir::HandleArm,
            op_tag: u32,
        }

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

        let mut escape_sites =
            self.scan_mixed_escape_direct_sites(handle, &escape_arm.op.op.fqn)?;
        if escape_sites.is_empty() {
            let at = escape_sites
                .first()
                .map(|site| site.decl.span)
                .unwrap_or(span);
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multi-arm without immediate-resume (only top-level or statement-position nested block direct sites supported)",
                at: at.into(),
            });
        }
        for site in &escape_sites {
            if !site.resume_path.is_empty()
                && !Self::mixed_escape_block_only_path_supported(&site.resume_path)
                && !Self::mixed_escape_if_branch_path_supported(&site.resume_path)
                && !Self::mixed_escape_while_nested_path_supported(&site.resume_path)
            {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multi-arm without immediate-resume (only top-level, statement-position nested block, paired if-branch, or while-body direct sites supported)",
                    at: site.decl.span.into(),
                });
            }
        }
        escape_sites.sort_by_key(|site| (site.top_level_stmt_idx, site.decl.span.start));
        let escape_site = &escape_sites[0];

        for site in &escape_sites {
            if escape_arm.op.binders.len() != site.args.len() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape binder arity mismatch",
                    at: escape_arm.op.span.into(),
                });
            }
        }

        let escape_resume_value_ty =
            self.cg_ty_of(escape_site.decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape perform value type",
                    at: escape_site.decl.span.into(),
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

        let mut outer_visible_supported: Vec<CaptureMeta> = Vec::new();
        let mut outer_visible_all: HashMap<hir::SymbolId, CaptureMeta> = HashMap::new();
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
            let meta = CaptureMeta {
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

        let mut body_lift_ids: HashSet<hir::SymbolId> = HashSet::new();
        for site in &escape_sites {
            let Some(&site_order) = body_decl_order.get(&site.id) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation perform binding id",
                    at: site.decl.span.into(),
                });
            };
            let mut used_after: HashSet<hir::SymbolId> = HashSet::new();
            Self::collect_mixed_escape_used_after_site(site, &handle.body.stmts, &mut used_after);
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

        let mut body_visible_supported: Vec<CaptureMeta> = Vec::new();
        for &id in &body_lift_ids {
            let Some(meta) = body_decl_all.get(&id).copied() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape capture local missing",
                    at: escape_site.decl.span.into(),
                });
            };
            body_visible_supported.push(CaptureMeta {
                id: meta.id,
                hir_ty: meta.hir_ty,
                ty: meta.ty,
                mutable: meta.mutable,
            });
        }
        body_visible_supported.sort_by_key(|meta| meta.id.as_u32());
        let matrix_escape_sites: Vec<MatrixEscapeSite<'hir>> = escape_sites
            .iter()
            .map(|site| MatrixEscapeSite {
                stmt_idx: site.top_level_stmt_idx,
                decl: site.decl,
                id: site.id,
                kind: MatrixEscapeSiteKind::Direct { site: site.clone() },
            })
            .collect();
        let mut escape_site_pcs_by_stmt_idx: HashMap<usize, Vec<usize>> = HashMap::new();
        for (pc, site) in escape_sites.iter().enumerate() {
            escape_site_pcs_by_stmt_idx
                .entry(site.top_level_stmt_idx)
                .or_default()
                .push(pc);
        }
        let mut if_direct_site_pcs_by_stmt_idx: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut while_direct_site_pc_by_stmt_idx: HashMap<usize, usize> = HashMap::new();
        let mut simple_escape_site_pc_by_stmt_idx: HashMap<usize, usize> = HashMap::new();
        for (stmt_idx, site_pcs) in &escape_site_pcs_by_stmt_idx {
            let mut then_site_pc: Option<usize> = None;
            let mut else_site_pc: Option<usize> = None;
            let mut while_site_pc: Option<usize> = None;
            let mut simple_site_pc: Option<usize> = None;
            for &pc in site_pcs {
                let site = &escape_sites[pc];
                match site.resume_path.first() {
                    Some(MixedEscapeDirectFrame::IfThen { .. }) => {
                        if then_site_pc.replace(pc).is_some() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-arm without immediate-resume (multiple direct sites in the same if-then branch not yet supported)",
                                at: site.decl.span.into(),
                            });
                        }
                    }
                    Some(MixedEscapeDirectFrame::IfElse { .. }) => {
                        if else_site_pc.replace(pc).is_some() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-arm without immediate-resume (multiple direct sites in the same if-else branch not yet supported)",
                                at: site.decl.span.into(),
                            });
                        }
                    }
                    Some(MixedEscapeDirectFrame::WhileBody { .. }) => {
                        if while_site_pc.replace(pc).is_some() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-arm without immediate-resume (multiple sites in the same while body not yet supported)",
                                at: handle.body.stmts[*stmt_idx].span.into(),
                            });
                        }
                    }
                    Some(MixedEscapeDirectFrame::Block { .. }) | None => {
                        if simple_site_pc.replace(pc).is_some() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-arm without immediate-resume (multiple sites per top-level statement not yet supported)",
                                at: handle.body.stmts[*stmt_idx].span.into(),
                            });
                        }
                    }
                }
            }
            if then_site_pc.is_some() || else_site_pc.is_some() {
                if simple_site_pc.is_some() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-arm without immediate-resume (multiple sites per top-level statement not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
                let (Some(then_pc), Some(else_pc)) = (then_site_pc, else_site_pc) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-arm without immediate-resume (only paired if-branch direct sites supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                };
                if_direct_site_pcs_by_stmt_idx.insert(*stmt_idx, vec![then_pc, else_pc]);
                continue;
            }
            if let Some(site_pc) = while_site_pc {
                if simple_site_pc.is_some() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-arm without immediate-resume (multiple sites per top-level statement not yet supported)",
                        at: handle.body.stmts[*stmt_idx].span.into(),
                    });
                }
                while_direct_site_pc_by_stmt_idx.insert(*stmt_idx, site_pc);
                continue;
            }
            if let Some(site_pc) = simple_site_pc {
                simple_escape_site_pc_by_stmt_idx.insert(*stmt_idx, site_pc);
            }
        }
        let needs_direct_reintercept =
            escape_sites.len() >= 2 || !while_direct_site_pc_by_stmt_idx.is_empty();

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

        let state_ty_name =
            format!("scoop.runtime.MultiEscapeNoImmediateDirectState__{func_name}_{seq}");
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

        let step_name = format!("__scoop_multi_escape_no_immediate_direct_step__{func_name}_{seq}");
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
                    kind: "multi escape no-immediate step state param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_ptr_ty = cg.llvm_ptr_type(cg.gc_address_space());
            let state_ptr = cg.builder.build_pointer_cast(
                state_raw,
                state_ptr_ty,
                "multi_escape_no_immediate_step_state_ptr",
            )?;

            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate step resume_word param",
                    at: span.into(),
                })?
                .into_int_value();
            let resume_gc_ref = step_fn
                .get_nth_param(2)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate step resume_gc_ref param",
                    at: span.into(),
                })?
                .into_pointer_value();

            for (idx, cap) in outer_visible_supported.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "multi_escape_no_immediate_step_outer_gep",
                )?;
                let name = format!("multi_escape_no_immediate_outer_{}", cap.id.as_u32());
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
                    "multi_escape_no_immediate_step_body_gep",
                )?;
                let name = format!("multi_escape_no_immediate_body_{}", cap.id.as_u32());
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
                let binder_ty =
                    cg.cg_ty_of(binder.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape binder type",
                            at: binder.span.into(),
                        })?;
                let ptr = cg.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
                step_escape_binder_slots.push(ImmediateResumeBinderSlot {
                    id: binder.id,
                    hir_ty: binder.ty,
                    ty: binder_ty,
                    ptr,
                });
            }
            let step_cont_ptr = cg.create_entry_alloca(
                span,
                &format!("handle_multi_escape_no_immediate_step_k_{seq}"),
                CgTy::Ref,
            )?;

            let step_has_sibling_dispatch = has_sibling_nonresuming;
            let step_effect_dispatch_bb =
                if step_has_sibling_dispatch {
                    Some(self.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_step_effect_dispatch",
                    ))
                } else {
                    None
                };
            let step_effect_dispatch_nomatch_bb = if step_has_sibling_dispatch {
                Some(self.context.append_basic_block(
                    step_fn,
                    "multi_escape_no_immediate_step_effect_dispatch_nomatch",
                ))
            } else {
                None
            };
            let step_intercept_unwind_bb =
                if step_has_sibling_dispatch {
                    Some(self.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_step_intercept_unwind",
                    ))
                } else {
                    None
                };
            let step_raise_catch_bb = if raise_sibling.is_some() {
                Some(
                    self.context
                        .append_basic_block(step_fn, "multi_escape_no_immediate_step_raise_catch"),
                )
            } else {
                None
            };
            let mut step_custom_catch_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            for (idx, _) in custom_siblings.iter().enumerate() {
                step_custom_catch_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_escape_no_immediate_step_custom_catch_{idx}"),
                ));
            }

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                for (idx, custom) in custom_siblings.iter().enumerate() {
                    cg.push_effect_unwind_target(&custom.arm.op.op.fqn, step_custom_catch_bbs[idx]);
                }
                cg.push_raise_target(step_effect_dispatch_bb);
            }

            let dispatch_bb = self
                .context
                .append_basic_block(step_fn, "multi_escape_no_immediate_step_dispatch");
            let bad_state_bb = self
                .context
                .append_basic_block(step_fn, "multi_escape_no_immediate_step_bad_pc");
            let mut state_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            for pc in 0..escape_sites.len() {
                state_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_escape_no_immediate_step_pc_{pc}"),
                ));
            }
            let intercept_bb = self
                .context
                .append_basic_block(step_fn, "multi_escape_no_immediate_step_intercept");
            let intercept_next_pc_ptr = cg.create_entry_alloca_raw(
                span,
                "multi_escape_no_immediate_step_intercept_next_pc",
                i32_ty.into(),
            )?;

            cg.builder.build_unconditional_branch(dispatch_bb)?;
            cg.builder.position_at_end(dispatch_bb);
            let step_pc_ptr = cg.builder.build_struct_gep(
                state_ty,
                state_ptr,
                2,
                "multi_escape_no_immediate_step_pc_gep",
            )?;
            let current_pc = cg
                .builder
                .build_load(i32_ty, step_pc_ptr, "multi_escape_no_immediate_step_pc")?
                .into_int_value();
            let mut step_cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
                Vec::new();
            for (pc, bb) in state_bbs.iter().enumerate() {
                step_cases.push((i32_ty.const_int(pc as u64, false), *bb));
            }
            cg.builder
                .build_switch(current_pc, bad_state_bb, &step_cases)?;

            cg.builder.position_at_end(bad_state_bb);
            cg.emit_exit_with_code(span, 3)?;

            for (pc, bb) in state_bbs.iter().enumerate() {
                let site = &escape_sites[pc];
                cg.builder.position_at_end(*bb);

                let target_ptr = if let Some(local) = cg.env.get(site.id) {
                    if local.ty != escape_resume_value_ty {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape perform value type mismatch",
                            at: site.decl.span.into(),
                        });
                    }
                    local.ptr
                } else {
                    let local_name = site.decl.name.as_deref().unwrap_or("resume_value");
                    let ptr =
                        cg.create_entry_alloca(site.decl.span, local_name, escape_resume_value_ty)?;
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
                let _ = cg.store_local_value(
                    site.decl.span,
                    target_ptr,
                    escape_resume_value_ty,
                    resume_value,
                )?;
                if matches!(
                    site.resume_path.first(),
                    Some(MixedEscapeDirectFrame::WhileBody { .. })
                ) {
                    cg.codegen_mixed_escape_matrix_while_tail_after_site(
                        &handle.body.stmts[site.top_level_stmt_idx],
                        pc,
                        site,
                        &body_lift_ids,
                        |cg, next_pc, next_site| {
                            for (slot, arg) in
                                step_escape_binder_slots.iter().zip(next_site.args.iter())
                            {
                                let hir::CallArg::Positional(expr) = arg else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "handle mixed-arm escape named perform arg",
                                        at: span.into(),
                                    });
                                };
                                let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                let _ = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                            }
                            let _ = cg.builder.build_store(
                                intercept_next_pc_ptr,
                                i32_ty.const_int(next_pc as u64, false),
                            )?;
                            cg.builder.build_unconditional_branch(intercept_bb)?;
                            Ok(())
                        },
                    )?;
                } else if !site.resume_path.is_empty() {
                    cg.codegen_mixed_escape_matrix_nested_block_tail_after_site(
                        site,
                        &body_lift_ids,
                    )?;
                }

                let mut terminated = false;
                for (idx, stmt) in handle.body.stmts.iter().enumerate() {
                    if idx <= site.top_level_stmt_idx {
                        continue;
                    }
                    if let Some(direct_sites) = if_direct_site_pcs_by_stmt_idx.get(&idx) {
                        cg.codegen_mixed_escape_matrix_if_stmt_direct_sites(
                            stmt,
                            direct_sites,
                            &matrix_escape_sites,
                            &body_lift_ids,
                            |cg, next_pc, next_site| {
                                for (slot, arg) in
                                    step_escape_binder_slots.iter().zip(next_site.args.iter())
                                {
                                    let hir::CallArg::Positional(expr) = arg else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle mixed-arm escape named perform arg",
                                            at: span.into(),
                                        });
                                    };
                                    let v =
                                        cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                    let _ =
                                        cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                }
                                let _ = cg.builder.build_store(
                                    intercept_next_pc_ptr,
                                    i32_ty.const_int(next_pc as u64, false),
                                )?;
                                cg.builder.build_unconditional_branch(intercept_bb)?;
                                Ok(())
                            },
                        )?;
                        continue;
                    }
                    if let Some(next_pc) = while_direct_site_pc_by_stmt_idx.get(&idx).copied() {
                        let next_site = &escape_sites[next_pc];
                        cg.codegen_mixed_escape_matrix_while_stmt_direct_site(
                            stmt,
                            next_pc,
                            next_site,
                            &body_lift_ids,
                            |cg, next_pc, next_site| {
                                for (slot, arg) in
                                    step_escape_binder_slots.iter().zip(next_site.args.iter())
                                {
                                    let hir::CallArg::Positional(expr) = arg else {
                                        return Err(LlvmEmitError::UnsupportedMainBody {
                                            kind: "handle mixed-arm escape named perform arg",
                                            at: span.into(),
                                        });
                                    };
                                    let v =
                                        cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                                    let _ =
                                        cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                                }
                                let _ = cg.builder.build_store(
                                    intercept_next_pc_ptr,
                                    i32_ty.const_int(next_pc as u64, false),
                                )?;
                                cg.builder.build_unconditional_branch(intercept_bb)?;
                                Ok(())
                            },
                        )?;
                        continue;
                    }
                    if let Some(next_pc) = simple_escape_site_pc_by_stmt_idx.get(&idx).copied() {
                        let next_site = &escape_sites[next_pc];
                        if !next_site.resume_path.is_empty() {
                            cg.codegen_mixed_escape_matrix_nested_block_prefix_to_site(
                                next_site,
                                stmt,
                                &body_lift_ids,
                            )?;
                        }
                        for (slot, arg) in
                            step_escape_binder_slots.iter().zip(next_site.args.iter())
                        {
                            let hir::CallArg::Positional(expr) = arg else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape named perform arg",
                                    at: span.into(),
                                });
                            };
                            let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                            let _ = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                        }
                        for _ in 0..next_site.resume_path.len() {
                            cg.env.pop_scope();
                        }
                        let _ = cg.builder.build_store(
                            intercept_next_pc_ptr,
                            i32_ty.const_int(next_pc as u64, false),
                        )?;
                        cg.builder.build_unconditional_branch(intercept_bb)?;
                        terminated = true;
                        break;
                    }
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

                if !terminated
                    && let Some(bb) = cg.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    let unpin = cg.declare_runtime_gc_unpin();
                    let _ = cg.builder.build_call(
                        unpin,
                        &[state_raw.into()],
                        "multi_escape_no_immediate_step_state_unpin",
                    )?;
                    cg.builder.build_return(None)?;
                }
            }

            if step_effect_dispatch_bb.is_some() {
                cg.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    cg.pop_effect_unwind_target();
                }
            }

            if let Some(bb) = cg.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                let unpin = cg.declare_runtime_gc_unpin();
                let _ = cg.builder.build_call(
                    unpin,
                    &[state_raw.into()],
                    "multi_escape_no_immediate_step_state_unpin",
                )?;
                cg.builder.build_return(None)?;
            }

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                let step_effect_dispatch_nomatch_bb = step_effect_dispatch_nomatch_bb
                    .expect("no-immediate escape step dispatch_nomatch bb should exist");
                cg.builder.position_at_end(step_effect_dispatch_bb);
                let rt_read_tag = cg.declare_runtime_effect_perform_slot_read_op_tag();
                let tag_call = cg.builder.build_call(
                    rt_read_tag,
                    &[],
                    "multi_escape_no_immediate_step_read_op_tag",
                )?;
                let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate step read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate step read_op_tag return type",
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
                    "multi_escape_no_immediate_step_state_unpin_nomatch",
                )?;
                cg.builder.build_return(None)?;

                if let (Some(raise_arm), Some(step_raise_catch_bb)) =
                    (raise_sibling, step_raise_catch_bb)
                {
                    let binder = &raise_arm.op.binders[0];
                    cg.builder.position_at_end(step_raise_catch_bb);
                    let frame_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        1,
                        "multi_escape_no_immediate_step_raise_frame_gep",
                    )?;
                    let prev_ptr = cg.builder.build_struct_gep(
                        handler_frame_ty,
                        frame_ptr,
                        0,
                        "multi_escape_no_immediate_step_raise_prev_gep",
                    )?;
                    let prev_raw = cg.builder.build_load(
                        i8_ptr_ty,
                        prev_ptr,
                        "multi_escape_no_immediate_step_raise_prev",
                    )?;
                    let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                    let _ = cg.builder.build_call(
                        rt_swap,
                        &[prev_raw.into()],
                        "multi_escape_no_immediate_step_raise_detach",
                    )?;

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "multi_escape_no_immediate_step_raise_read_slot_len_words",
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
                        "multi_escape_no_immediate_step_raise_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_step_raise_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_step_raise_slot_len_bad_bb",
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
                        "multi_escape_no_immediate_step_raise_read_slot_word0",
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
                        "multi_escape_no_immediate_step_raise_read_slot_word1",
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
                        "multi_escape_no_immediate_step_raise_clear",
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
                                "multi_escape_no_immediate_step_raise_kind_is_int",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_step_raise_kind_int_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_step_raise_kind_int_bad",
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
                                "multi_escape_no_immediate_step_raise_kind_is_runtime_error",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_step_raise_kind_runtime_error_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_no_immediate_step_raise_kind_runtime_error_bad",
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
                                "multi_escape_no_immediate_step_runtime_error_tag_i32",
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
                                "multi_escape_no_immediate_step_runtime_error_tag",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_word_zero,
                                1,
                                "multi_escape_no_immediate_step_runtime_error_payload_word",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_ptr_zero,
                                2,
                                "multi_escape_no_immediate_step_runtime_error_payload_ptr",
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
                            "multi_escape_no_immediate_step_state_unpin_raise",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }

                for (idx, custom) in custom_siblings.iter().enumerate() {
                    let arm = custom.arm;
                    let binder = &arm.op.binders[0];
                    cg.builder.position_at_end(step_custom_catch_bbs[idx]);
                    let frame_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        1,
                        "multi_escape_no_immediate_step_custom_frame_gep",
                    )?;
                    let prev_ptr = cg.builder.build_struct_gep(
                        handler_frame_ty,
                        frame_ptr,
                        0,
                        "multi_escape_no_immediate_step_custom_prev_gep",
                    )?;
                    let prev_raw = cg.builder.build_load(
                        i8_ptr_ty,
                        prev_ptr,
                        "multi_escape_no_immediate_step_custom_prev",
                    )?;
                    let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                    let _ = cg.builder.build_call(
                        rt_swap,
                        &[prev_raw.into()],
                        "multi_escape_no_immediate_step_custom_detach",
                    )?;

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "multi_escape_no_immediate_step_custom_read_slot_len_words",
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
                        "multi_escape_no_immediate_step_custom_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_step_custom_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_no_immediate_step_custom_slot_len_bad_bb",
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
                        "multi_escape_no_immediate_step_custom_read_slot_word0",
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
                        "multi_escape_no_immediate_step_custom_read_slot_gc_ref",
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
                        "multi_escape_no_immediate_step_custom_clear",
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
                            "multi_escape_no_immediate_step_state_unpin_custom",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }
            }

            cg.builder.position_at_end(intercept_bb);
            if !needs_direct_reintercept {
                cg.builder.build_unreachable()?;
            } else {
                for (idx, cap) in outer_visible_supported.iter().enumerate() {
                    let field_idx = outer_field_base.saturating_add(idx as u32);
                    let field_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        field_idx,
                        "multi_escape_no_immediate_step_intercept_outer_gep",
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
                for (idx, cap) in body_visible_supported.iter().enumerate() {
                    let field_idx = body_field_base.saturating_add(idx as u32);
                    let field_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        field_idx,
                        "multi_escape_no_immediate_step_intercept_body_gep",
                    )?;
                    let Some(local) = cg.env.get(cap.id) else {
                        continue;
                    };
                    if local.ty != cap.ty {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "intercept: lift local type mismatch",
                            at: span.into(),
                        });
                    }
                    cg.write_escape_capture_local_to_state(span, field_ptr, local.ptr, cap.ty)?;
                }

                let next_pc_val = cg.builder.build_load(
                    i32_ty,
                    intercept_next_pc_ptr,
                    "multi_escape_no_immediate_step_intercept_next_pc",
                )?;
                let state_pc_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    2,
                    "multi_escape_no_immediate_step_intercept_pc_gep",
                )?;
                let _ = cg.builder.build_store(state_pc_ptr, next_pc_val)?;

                let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                let step_ptr = step_fn.as_global_value().as_pointer_value();
                let call = cg.builder.build_call(
                    rt_cont_alloc,
                    &[state_raw.into(), step_ptr.into()],
                    "multi_escape_no_immediate_step_intercept_cont_alloc",
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

                let pin = cg.declare_runtime_gc_pin();
                let _ = cg.builder.build_call(
                    pin,
                    &[k_raw.into()],
                    "multi_escape_no_immediate_step_intercept_k_pin",
                )?;
                let _ = cg.store_local_value(
                    span,
                    step_cont_ptr,
                    CgTy::Ref,
                    CgValue {
                        ty: CgTy::Ref,
                        value: Some(k_raw.into()),
                    },
                )?;

                let frame_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    1,
                    "multi_escape_no_immediate_step_intercept_frame_gep",
                )?;
                let prev_ptr = cg.builder.build_struct_gep(
                    handler_frame_ty,
                    frame_ptr,
                    0,
                    "multi_escape_no_immediate_step_intercept_prev_gep",
                )?;
                let prev_raw = cg.builder.build_load(
                    i8_ptr_ty,
                    prev_ptr,
                    "multi_escape_no_immediate_step_intercept_prev",
                )?;
                let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                let _ = cg.builder.build_call(
                    rt_swap,
                    &[prev_raw.into()],
                    "multi_escape_no_immediate_step_intercept_detach",
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
                        ptr: step_cont_ptr,
                        mutable: false,
                    },
                );
                if let Some(step_intercept_unwind_bb) = step_intercept_unwind_bb {
                    for custom in &custom_siblings {
                        cg.push_effect_unwind_target(
                            &custom.arm.op.op.fqn,
                            step_intercept_unwind_bb,
                        );
                    }
                    cg.push_raise_target(step_intercept_unwind_bb);
                }
                let arm_v = cg.codegen_expr_in_expected_context(&escape_arm.body, Some(out_ty))?;
                if step_intercept_unwind_bb.is_some() {
                    cg.pop_raise_target();
                    for _ in custom_siblings.iter().rev() {
                        cg.pop_effect_unwind_target();
                    }
                }
                let _arm_v = if out_ty == CgTy::Unit {
                    CgValue::unit()
                } else {
                    cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
                };
                cg.env.pop_scope();

                if let Some(bb) = cg.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    let llvm_ref_ty = cg.llvm_basic_type_of(span, CgTy::Ref)?;
                    let k_loaded = cg
                        .builder
                        .build_load(
                            llvm_ref_ty,
                            step_cont_ptr,
                            "multi_escape_no_immediate_step_intercept_k_unpin_load",
                        )?
                        .into_pointer_value();
                    let unpin = cg.declare_runtime_gc_unpin();
                    let _ = cg.builder.build_call(
                        unpin,
                        &[k_loaded.into()],
                        "multi_escape_no_immediate_step_intercept_k_unpin",
                    )?;
                    cg.builder.build_return(None)?;
                }
            }

            if let Some(step_intercept_unwind_bb) = step_intercept_unwind_bb {
                cg.builder.position_at_end(step_intercept_unwind_bb);
                cg.builder.build_return(None)?;
            }

            cg.env.pop_scope();
        }
        self.builder.position_at_end(saved_block);

        let body_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_no_immediate_body");
        let escape_arm_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_no_immediate_arm");
        let done_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_no_immediate_done");
        let finally_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_no_immediate_finally");
        let finally_unwind_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_no_immediate_finally_unwind");
        let effect_dispatch_bb = if has_sibling_nonresuming {
            Some(
                self.context
                    .append_basic_block(func, "handle_multi_escape_no_immediate_effect_dispatch"),
            )
        } else {
            None
        };
        let effect_dispatch_nomatch_bb = if has_sibling_nonresuming {
            Some(self.context.append_basic_block(
                func,
                "handle_multi_escape_no_immediate_effect_dispatch_nomatch",
            ))
        } else {
            None
        };
        let raise_catch_bb = if raise_sibling.is_some() {
            Some(
                self.context
                    .append_basic_block(func, "handle_multi_escape_no_immediate_raise_catch"),
            )
        } else {
            None
        };
        let mut custom_catch_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        for (idx, _) in custom_siblings.iter().enumerate() {
            custom_catch_bbs.push(self.context.append_basic_block(
                func,
                &format!("handle_multi_escape_no_immediate_custom_catch_{idx}"),
            ));
        }

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(
                span,
                "handle_multi_escape_no_immediate_result",
                out_ty,
            )?)
        };
        let continuation_created_ptr = self.create_entry_alloca_raw(
            span,
            "handle_multi_escape_no_immediate_cont_created",
            self.context.bool_type().into(),
        )?;
        let _ = self.builder.build_store(
            continuation_created_ptr,
            self.context.bool_type().const_zero(),
        )?;

        let mut escape_binder_slots: Vec<ImmediateResumeBinderSlot<'ctx>> = Vec::new();
        for binder in &escape_arm.op.binders {
            let binder_ty = self
                .cg_ty_of(binder.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape binder type",
                    at: binder.span.into(),
                })?;
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
            &format!("handle_multi_escape_no_immediate_k_{seq}"),
            CgTy::Ref,
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
            format!("__scoop_type_desc_multi_escape_no_immediate_state__{func_name}_{seq}");
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
            "multi_escape_no_immediate_state_desc_i8",
        )?;
        let alloc_call = self.builder.build_call(
            rt_alloc,
            &[state_desc_i8.into(), size_v.into()],
            "rt_alloc_multi_escape_no_immediate_state",
        )?;
        let alloc_raw =
            alloc_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate alloc return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::PointerValue(state_raw) = alloc_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape no-immediate alloc return type",
                at: span.into(),
            });
        };

        let pin = self.declare_runtime_gc_pin();
        let _ = self.builder.build_call(
            pin,
            &[state_raw.into()],
            "multi_escape_no_immediate_state_pin",
        )?;

        let state_gc_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let state_gc_ptr = self.builder.build_pointer_cast(
            state_raw,
            state_gc_ptr_ty,
            "multi_escape_no_immediate_state_ptr",
        )?;

        for (idx, cap) in outer_visible_supported.iter().enumerate() {
            let field_idx = outer_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_escape_no_immediate_state_outer_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }
        for (idx, cap) in body_visible_supported.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_escape_no_immediate_state_body_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }

        let frame_ptr = self.builder.build_struct_gep(
            state_ty,
            state_gc_ptr,
            1,
            "multi_escape_no_immediate_state_frame_gep",
        )?;
        let frame_i8 = self.builder.build_address_space_cast(
            frame_ptr,
            i8_ptr_ty,
            "multi_escape_no_immediate_frame_i8",
        )?;
        let escape_tag = self.effect_op_tag(&escape_arm.op.op.fqn);
        let escape_tag_i32 = i32_ty.const_int(escape_tag as u64, false);
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), escape_tag_i32.into()],
            "multi_escape_no_immediate_push",
        )?;
        let prev_ptr = self.builder.build_struct_gep(
            handler_frame_ty,
            frame_ptr,
            0,
            "multi_escape_no_immediate_prev_gep",
        )?;
        let escape_outer_top = self
            .builder
            .build_load(i8_ptr_ty, prev_ptr, "multi_escape_no_immediate_outer_top")?
            .into_pointer_value();
        let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
        let main_raise_target = effect_dispatch_bb.unwrap_or(finally_unwind_bb);

        self.builder.build_unconditional_branch(body_bb)?;

        self.builder.position_at_end(body_bb);
        self.env.push_scope();
        for (idx, custom) in custom_siblings.iter().enumerate() {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom_catch_bbs[idx]);
        }
        self.push_raise_target(main_raise_target);
        let emit_initial_direct_site =
            |cg: &mut Self,
             site_pc: usize,
             direct_site: &MixedEscapeDirectSite<'hir>,
             scopes_to_pop: usize| {
                for (idx, cap) in outer_visible_supported.iter().enumerate() {
                    let field_idx = outer_field_base.saturating_add(idx as u32);
                    let field_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_gc_ptr,
                        field_idx,
                        "multi_escape_no_immediate_capture_outer_gep",
                    )?;
                    let local = cg
                        .env
                        .get(cap.id)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "multi escape no-immediate capture local not found",
                            at: span.into(),
                        })?;
                    if local.ty != cap.ty {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "multi escape no-immediate capture local type mismatch",
                            at: span.into(),
                        });
                    }
                    cg.write_escape_capture_local_to_state(span, field_ptr, local.ptr, cap.ty)?;
                }
                for (idx, cap) in body_visible_supported.iter().enumerate() {
                    let field_idx = body_field_base.saturating_add(idx as u32);
                    let field_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_gc_ptr,
                        field_idx,
                        "multi_escape_no_immediate_capture_body_gep",
                    )?;
                    let Some(local) = cg.env.get(cap.id) else {
                        continue;
                    };
                    if local.ty != cap.ty {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "multi escape no-immediate capture local type mismatch",
                            at: span.into(),
                        });
                    }
                    cg.write_escape_capture_local_to_state(span, field_ptr, local.ptr, cap.ty)?;
                }

                let pc_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_gc_ptr,
                    2,
                    "multi_escape_no_immediate_state_pc_gep",
                )?;
                let _ = cg
                    .builder
                    .build_store(pc_ptr, i32_ty.const_int(site_pc as u64, false))?;

                for (slot, arg) in escape_binder_slots.iter().zip(direct_site.args.iter()) {
                    let hir::CallArg::Positional(expr) = arg else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape named perform arg",
                            at: span.into(),
                        });
                    };
                    let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                    let _ = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                }

                let step_ptr = step_fn.as_global_value().as_pointer_value();
                let cont_call = cg.builder.build_call(
                    cg.declare_runtime_continuation_alloc(),
                    &[state_raw.into(), step_ptr.into()],
                    "multi_escape_no_immediate_cont_alloc",
                )?;
                let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate continuation alloc return value",
                        at: direct_site.decl.span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape no-immediate continuation alloc return type",
                        at: direct_site.decl.span.into(),
                    });
                };

                let _ = cg.builder.build_call(
                    pin,
                    &[k_raw.into()],
                    "multi_escape_no_immediate_k_pin",
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
                let _ = cg.builder.build_store(
                    continuation_created_ptr,
                    cg.context.bool_type().const_all_ones(),
                )?;

                for _ in 0..scopes_to_pop {
                    cg.env.pop_scope();
                }
                let _ = cg.builder.build_call(
                    rt_swap,
                    &[escape_outer_top.into()],
                    "multi_escape_no_immediate_detach",
                )?;
                cg.builder.build_unconditional_branch(escape_arm_bb)?;
                Ok(())
            };

        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if self
                .builder
                .get_insert_block()
                .is_some_and(|bb| bb.get_terminator().is_some())
            {
                break;
            }

            if let Some(direct_sites) = if_direct_site_pcs_by_stmt_idx.get(&idx) {
                self.codegen_mixed_escape_matrix_if_stmt_direct_sites(
                    stmt,
                    direct_sites,
                    &matrix_escape_sites,
                    &body_lift_ids,
                    |cg, site_pc, direct_site| {
                        emit_initial_direct_site(cg, site_pc, direct_site, 0)
                    },
                )?;
                continue;
            }

            if let Some(&site_pc) = while_direct_site_pc_by_stmt_idx.get(&idx) {
                let direct_site = &escape_sites[site_pc];
                self.codegen_mixed_escape_matrix_while_stmt_direct_site(
                    stmt,
                    site_pc,
                    direct_site,
                    &body_lift_ids,
                    |cg, site_pc, direct_site| {
                        emit_initial_direct_site(cg, site_pc, direct_site, 0)
                    },
                )?;
                continue;
            }

            if let Some(&site_pc) = simple_escape_site_pc_by_stmt_idx.get(&idx) {
                let direct_site = &escape_sites[site_pc];
                if !direct_site.resume_path.is_empty() {
                    self.codegen_mixed_escape_matrix_nested_block_prefix_to_site(
                        direct_site,
                        stmt,
                        &body_lift_ids,
                    )?;
                }
                emit_initial_direct_site(
                    self,
                    site_pc,
                    direct_site,
                    direct_site.resume_path.len(),
                )?;
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
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.builder.build_unreachable()?;
        }

        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }
        self.env.pop_scope();

        if let Some(effect_dispatch_bb) = effect_dispatch_bb {
            let effect_dispatch_nomatch_bb = effect_dispatch_nomatch_bb
                .expect("no-immediate escape effect dispatch_nomatch bb should exist");
            self.builder.position_at_end(effect_dispatch_bb);
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = self.builder.build_call(
                rt_read_tag,
                &[],
                "multi_escape_no_immediate_dispatch_read_op_tag",
            )?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate dispatch read_op_tag return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape no-immediate dispatch read_op_tag return type",
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
                    "multi_escape_no_immediate_raise_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "multi_escape_no_immediate_raise_read_slot_len_words",
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
                    "multi_escape_no_immediate_raise_slot_len_ok",
                )?;
                let len_ok_bb = self
                    .context
                    .append_basic_block(func, "multi_escape_no_immediate_raise_slot_len_ok_bb");
                let len_bad_bb = self
                    .context
                    .append_basic_block(func, "multi_escape_no_immediate_raise_slot_len_bad_bb");
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
                    "multi_escape_no_immediate_raise_read_slot_word0",
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
                    "multi_escape_no_immediate_raise_read_slot_word1",
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
                let _ = self.builder.build_call(
                    rt_clear,
                    &[],
                    "multi_escape_no_immediate_raise_clear",
                )?;

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
                            "multi_escape_no_immediate_raise_kind_is_int",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_raise_kind_int_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_raise_kind_int_bad",
                        );
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
                            "multi_escape_no_immediate_raise_kind_is_runtime_error",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_raise_kind_runtime_error_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_no_immediate_raise_kind_runtime_error_bad",
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
                            "multi_escape_no_immediate_runtime_error_tag_i32",
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
                            "multi_escape_no_immediate_runtime_error_tag",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_word_zero,
                            1,
                            "multi_escape_no_immediate_runtime_error_payload_word",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_ptr_zero,
                            2,
                            "multi_escape_no_immediate_runtime_error_payload_ptr",
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
                    "multi_escape_no_immediate_custom_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "multi_escape_no_immediate_custom_read_slot_len_words",
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
                    "multi_escape_no_immediate_custom_slot_len_ok",
                )?;
                let len_ok_bb = self
                    .context
                    .append_basic_block(func, "multi_escape_no_immediate_custom_slot_len_ok_bb");
                let len_bad_bb = self
                    .context
                    .append_basic_block(func, "multi_escape_no_immediate_custom_slot_len_bad_bb");
                self.builder
                    .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                self.builder.position_at_end(len_bad_bb);
                self.emit_exit_with_code(span, 3)?;

                self.builder.position_at_end(len_ok_bb);
                let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
                let value_call = self.builder.build_call(
                    rt_read,
                    &[],
                    "multi_escape_no_immediate_custom_read_slot_word0",
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
                    "multi_escape_no_immediate_custom_read_slot_gc_ref",
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
                let _ = self.builder.build_call(
                    rt_clear,
                    &[],
                    "multi_escape_no_immediate_custom_clear",
                )?;

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

        self.builder.position_at_end(escape_arm_bb);
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
            "multi_escape_no_immediate_finally_unwind_detach",
        )?;
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            let created = self
                .builder
                .build_load(
                    self.context.bool_type(),
                    continuation_created_ptr,
                    "multi_escape_no_immediate_unwind_cont_created",
                )?
                .into_int_value();
            let unwind_propagate_bb = self
                .context
                .append_basic_block(func, "multi_escape_no_immediate_finally_unwind_propagate");
            let unwind_unpin_bb = self
                .context
                .append_basic_block(func, "multi_escape_no_immediate_finally_unwind_unpin");
            self.builder
                .build_conditional_branch(created, unwind_propagate_bb, unwind_unpin_bb)?;

            self.builder.position_at_end(unwind_unpin_bb);
            let unpin = self.declare_runtime_gc_unpin();
            let _ = self.builder.build_call(
                unpin,
                &[state_raw.into()],
                "multi_escape_no_immediate_state_unpin_unwind",
            )?;
            self.builder
                .build_unconditional_branch(unwind_propagate_bb)?;

            self.builder.position_at_end(unwind_propagate_bb);
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
            "multi_escape_no_immediate_finally_detach",
        )?;
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.builder.build_unconditional_branch(done_bb)?;
        }

        self.builder.position_at_end(done_bb);
        let done_with_k_bb = self
            .context
            .append_basic_block(func, "multi_escape_no_immediate_done_with_k");
        let done_without_k_bb = self
            .context
            .append_basic_block(func, "multi_escape_no_immediate_done_without_k");
        let done_merge_bb = self
            .context
            .append_basic_block(func, "multi_escape_no_immediate_done_merge");
        let created = self
            .builder
            .build_load(
                self.context.bool_type(),
                continuation_created_ptr,
                "multi_escape_no_immediate_done_cont_created",
            )?
            .into_int_value();
        self.builder
            .build_conditional_branch(created, done_with_k_bb, done_without_k_bb)?;

        self.builder.position_at_end(done_with_k_bb);
        let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
        let k_loaded = self
            .builder
            .build_load(
                llvm_ref_ty,
                cont_ptr,
                "multi_escape_no_immediate_k_unpin_load",
            )?
            .into_pointer_value();
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self.builder.build_call(
            unpin,
            &[k_loaded.into()],
            "multi_escape_no_immediate_k_unpin",
        )?;
        self.builder.build_unconditional_branch(done_merge_bb)?;

        self.builder.position_at_end(done_without_k_bb);
        let _ = self.builder.build_call(
            unpin,
            &[state_raw.into()],
            "multi_escape_no_immediate_state_unpin_done",
        )?;
        self.builder.build_unconditional_branch(done_merge_bb)?;

        self.builder.position_at_end(done_merge_bb);
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
                let loaded = self.builder.build_load(
                    llvm_ty,
                    ptr,
                    "handle_multi_escape_no_immediate_result",
                )?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }

    fn codegen_handle_expr_immediate_resume_with_escape_sibling<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        immediate: (&'hir hir::HandleArm, hir::SymbolId),
        escape: (&'hir hir::HandleArm, hir::SymbolId),
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (immediate_arm, _) = immediate;
        let (escape_arm, _) = escape;

        let Some(perform_site) =
            self.scan_immediate_resume_site(handle, &immediate_arm.op.op.fqn)?
        else {
            return self.codegen_handle_expr_immediate_resume_with_escape_sibling_direct(
                span,
                handle,
                immediate,
                escape,
                &[],
                out_ty,
            );
        };
        let perform_idx = perform_site.top_level_stmt_idx;

        let mut has_direct_escape_site = false;
        let mut has_direct_escape_site_before_immediate = false;
        let mut has_direct_escape_site_after_immediate = false;
        let mut has_nested_block_direct_escape_site = false;
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if self
                .immediate_resume_stmt_contains_matching_direct_perform(stmt, &escape_arm.op.op.fqn)
            {
                has_direct_escape_site = true;
                if idx < perform_idx {
                    has_direct_escape_site_before_immediate = true;
                }
                if idx > perform_idx {
                    has_direct_escape_site_after_immediate = true;
                }
            }
        }
        if has_direct_escape_site {
            let direct_sites =
                self.scan_mixed_escape_direct_sites(handle, &escape_arm.op.op.fqn)?;
            has_nested_block_direct_escape_site =
                direct_sites.iter().any(|site| !site.resume_path.is_empty());
        }

        let indirect_sites = self.scan_mixed_escape_indirect_sites(handle)?;
        let has_nested_block_indirect_escape_site = indirect_sites
            .iter()
            .any(|site| !site.resume_path.is_empty());
        let has_indirect_escape_site_before_immediate = indirect_sites
            .iter()
            .any(|site| site.top_level_stmt_idx < perform_idx);
        let has_indirect_escape_site_after_immediate = indirect_sites
            .iter()
            .any(|site| site.top_level_stmt_idx > perform_idx);
        let indirect_after_count = indirect_sites
            .iter()
            .filter(|site| site.top_level_stmt_idx > perform_idx)
            .count();

        if has_direct_escape_site_before_immediate
            || has_indirect_escape_site_before_immediate
            || (has_indirect_escape_site_after_immediate
                && (has_direct_escape_site_after_immediate || indirect_after_count > 1))
            || has_nested_block_direct_escape_site
            || has_nested_block_indirect_escape_site
        {
            return self.codegen_handle_expr_immediate_resume_with_escape_sibling_site_matrix(
                span,
                handle,
                immediate,
                escape,
                &[],
                out_ty,
            );
        }

        if has_direct_escape_site {
            return self.codegen_handle_expr_immediate_resume_with_escape_sibling_direct(
                span,
                handle,
                immediate,
                escape,
                &[],
                out_ty,
            );
        }

        if !indirect_sites.is_empty() {
            return self.codegen_handle_expr_immediate_resume_with_escape_sibling_indirect(
                span,
                handle,
                immediate,
                escape,
                &[],
                out_ty,
            );
        }

        self.codegen_handle_expr_immediate_resume_with_escape_sibling_direct(
            span,
            handle,
            immediate,
            escape,
            &[],
            out_ty,
        )
    }

    fn codegen_handle_expr_immediate_resume_with_escape_and_nonresuming_siblings<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        immediate: (&'hir hir::HandleArm, hir::SymbolId),
        escape: (&'hir hir::HandleArm, hir::SymbolId),
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (immediate_arm, _) = immediate;
        let (escape_arm, _) = escape;

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
                kind: "handle mixed-arm escape continuation with sibling non-resuming (only top-level direct single-site supported)",
                at: perform_site.decl.span.into(),
            });
        }

        let direct_sites = self.scan_mixed_escape_direct_sites(handle, &escape_arm.op.op.fqn)?;
        let indirect_sites = self.scan_mixed_escape_indirect_sites(handle)?;
        if direct_sites.len() == 1
            && indirect_sites.is_empty()
            && direct_sites[0].resume_path.is_empty()
            && direct_sites[0].top_level_stmt_idx > perform_site.top_level_stmt_idx
        {
            return self.codegen_handle_expr_immediate_resume_with_escape_sibling_direct(
                span,
                handle,
                immediate,
                escape,
                sibling_nonresuming_arms,
                out_ty,
            );
        }

        if direct_sites.is_empty()
            && indirect_sites.len() == 1
            && indirect_sites[0].resume_path.is_empty()
            && indirect_sites[0].top_level_stmt_idx > perform_site.top_level_stmt_idx
        {
            return self.codegen_handle_expr_immediate_resume_with_escape_sibling_indirect(
                span,
                handle,
                immediate,
                escape,
                sibling_nonresuming_arms,
                out_ty,
            );
        }

        self.codegen_handle_expr_immediate_resume_with_escape_sibling_site_matrix(
            span,
            handle,
            immediate,
            escape,
            sibling_nonresuming_arms,
            out_ty,
        )
    }

    fn codegen_handle_expr_immediate_resume_with_nonresuming_siblings<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        immediate_arm: &'hir hir::HandleArm,
        resume_symbol: hir::SymbolId,
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        #[derive(Clone, Copy)]
        struct MixedCustomSiblingArm<'hir, 'ctx> {
            arm: &'hir hir::HandleArm,
            frame_ptr: PointerValue<'ctx>,
            catch_bb: inkwell::basic_block::BasicBlock<'ctx>,
            op_tag: u32,
        }

        if sibling_nonresuming_arms.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle arm count (only 1 supported)",
                at: span.into(),
            });
        }

        let Some(perform_site) =
            self.scan_immediate_resume_site(handle, &immediate_arm.op.op.fqn)?
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm immediate-resume body (missing direct perform)",
                at: span.into(),
            });
        };
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

        let dispatch_bb = self
            .context
            .append_basic_block(func, "handle_mixed_resume_dispatch");
        let effect_dispatch_bb = self
            .context
            .append_basic_block(func, "handle_mixed_effect_dispatch");
        let dispatch_no_match_bb = self
            .context
            .append_basic_block(func, "handle_mixed_resume_dispatch_nomatch");
        let state0_bb = self
            .context
            .append_basic_block(func, "handle_mixed_resume_state0");
        let state1_bb = self
            .context
            .append_basic_block(func, "handle_mixed_resume_state1");
        let arm_bb = self
            .context
            .append_basic_block(func, "handle_mixed_resume_arm");
        let done_bb = self
            .context
            .append_basic_block(func, "handle_mixed_resume_done");
        let bad_state_bb = self
            .context
            .append_basic_block(func, "handle_mixed_resume_bad_state");
        let finally_bb = self
            .context
            .append_basic_block(func, "handle_mixed_resume_finally");
        let finally_unwind_bb = self
            .context
            .append_basic_block(func, "handle_mixed_resume_finally_unwind");

        let i32_ty = self.context.i32_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let handler_frame_ty = self.llvm_effect_handler_frame_type();

        let state_ptr = self.create_entry_alloca_raw(span, "handle_mixed_state", i32_ty.into())?;
        let resume_used_ptr = self.create_entry_alloca_raw(
            span,
            "handle_mixed_resume_used",
            self.context.bool_type().into(),
        )?;
        let resume_value_ptr = if resume_value_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_mixed_resume_value", resume_value_ty)?)
        };
        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_mixed_result", out_ty)?)
        };

        let mut binder_slots: Vec<ImmediateResumeBinderSlot<'ctx>> = Vec::new();
        for binder in &immediate_arm.op.binders {
            let binder_ty = self
                .cg_ty_of(binder.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume binder type",
                    at: binder.span.into(),
                })?;
            let ptr = self.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
            binder_slots.push(ImmediateResumeBinderSlot {
                id: binder.id,
                hir_ty: binder.ty,
                ty: binder_ty,
                ptr,
            });
        }

        let mut raise_sibling: Option<(
            &'hir hir::HandleArm,
            inkwell::basic_block::BasicBlock<'ctx>,
        )> = None;
        let mut custom_siblings: Vec<MixedCustomSiblingArm<'hir, 'ctx>> = Vec::new();

        for (idx, arm) in sibling_nonresuming_arms.iter().enumerate() {
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
                let catch_bb = self
                    .context
                    .append_basic_block(func, "handle_mixed_raise_catch");
                raise_sibling = Some((arm, catch_bb));
                continue;
            }

            let catch_bb = self
                .context
                .append_basic_block(func, &format!("handle_mixed_custom_catch_{idx}"));
            let frame_ptr = self.create_entry_alloca_raw(
                span,
                &format!("handle_mixed_custom_frame_{idx}"),
                handler_frame_ty.into(),
            )?;
            let op_tag = self.effect_op_tag(&arm.op.op.fqn);
            custom_siblings.push(MixedCustomSiblingArm {
                arm,
                frame_ptr,
                catch_bb,
                op_tag,
            });
        }

        let _ = self.builder.build_store(state_ptr, i32_ty.const_zero())?;
        let _ = self.builder.build_store(
            resume_used_ptr,
            self.context.bool_type().const_int(0, false),
        )?;

        let rt_push = self.declare_runtime_effect_handler_stack_push();
        for custom in &custom_siblings {
            let frame_i8 = self
                .builder
                .build_bit_cast(custom.frame_ptr, i8_ptr_ty, "handle_mixed_custom_frame_i8")?
                .into_pointer_value();
            let tag_i32 = i32_ty.const_int(custom.op_tag as u64, false);
            let _ = self.builder.build_call(
                rt_push,
                &[frame_i8.into(), tag_i32.into()],
                "handle_mixed_custom_push",
            )?;
        }

        let custom_outer_top = if let Some(first) = custom_siblings.first() {
            let prev_ptr = self.builder.build_struct_gep(
                handler_frame_ty,
                first.frame_ptr,
                0,
                "handle_mixed_custom_prev_gep",
            )?;
            Some(
                self.builder
                    .build_load(i8_ptr_ty, prev_ptr, "handle_mixed_custom_outer_top")?
                    .into_pointer_value(),
            )
        } else {
            None
        };
        let custom_restore_top = if let Some(last) = custom_siblings.last() {
            Some(
                self.builder
                    .build_bit_cast(last.frame_ptr, i8_ptr_ty, "handle_mixed_custom_restore_top")?
                    .into_pointer_value(),
            )
        } else {
            None
        };

        let exec_plan = ImmediateResumeExecPlan {
            handle,
            site: &perform_site,
            out_ty,
            result_ptr,
            handler_exit: custom_outer_top
                .map(ImmediateResumeHandlerExit::SwapTop)
                .unwrap_or(ImmediateResumeHandlerExit::None),
            finally_bb,
        };

        self.builder.build_unconditional_branch(dispatch_bb)?;

        self.builder.position_at_end(dispatch_bb);
        let state = self
            .builder
            .build_load(i32_ty, state_ptr, "handle_mixed_state")?
            .into_int_value();
        let cases = [
            (i32_ty.const_int(0, false), state0_bb),
            (i32_ty.const_int(1, false), state1_bb),
        ];
        self.builder.build_switch(state, bad_state_bb, &cases)?;

        self.builder.position_at_end(bad_state_bb);
        self.emit_exit_with_code(span, 3)?;

        self.env.push_scope();

        self.builder.position_at_end(state0_bb);
        for custom in &custom_siblings {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom.catch_bb);
        }
        self.push_raise_target(effect_dispatch_bb);
        let target_ptr = self.codegen_immediate_resume_prefix_to_site(
            exec_plan,
            0,
            &handle.body.stmts,
            &binder_slots,
            resume_used_ptr,
            arm_bb,
        )?;
        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }

        self.builder.position_at_end(arm_bb);
        if let Some(custom_outer_top) = custom_outer_top {
            let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
            let _ = self.builder.build_call(
                rt_swap,
                &[custom_outer_top.into()],
                "handle_mixed_resume_detach",
            )?;
        }

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
            .append_basic_block(func, "handle_mixed_resume_arm_ok");
        let resume_missing_bb = self
            .context
            .append_basic_block(func, "handle_mixed_resume_arm_missing");

        let used = self
            .builder
            .build_load(self.context.bool_type(), resume_used_ptr, "resume_used")?
            .into_int_value();
        self.builder
            .build_conditional_branch(used, resume_ok_bb, resume_missing_bb)?;

        self.builder.position_at_end(resume_missing_bb);
        self.emit_exit_with_code(span, 3)?;

        self.builder.position_at_end(resume_ok_bb);
        if let Some(custom_restore_top) = custom_restore_top {
            let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
            let _ = self.builder.build_call(
                rt_swap,
                &[custom_restore_top.into()],
                "handle_mixed_resume_restore",
            )?;
        }
        self.builder.build_unconditional_branch(dispatch_bb)?;

        self.env.pop_scope();

        self.builder.position_at_end(state1_bb);
        for custom in &custom_siblings {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom.catch_bb);
        }
        self.push_raise_target(effect_dispatch_bb);

        if let Some(ptr) = resume_value_ptr {
            let llvm_ty = self.llvm_basic_type_of(span, resume_value_ty)?;
            let loaded = self.builder.build_load(llvm_ty, ptr, "resume_value")?;
            let v = CgValue {
                ty: resume_value_ty,
                value: Some(loaded),
            };
            let _stored = self.store_local_value(span, target_ptr, resume_value_ty, v)?;
        }

        if perform_site.resume_path.is_empty() {
            self.codegen_immediate_resume_top_level_tail_and_finalize(
                handle,
                perform_idx + 1,
                out_ty,
                result_ptr,
                exec_plan.handler_exit,
                finally_bb,
            )?;
        } else {
            let last_depth = perform_site.resume_path.len() - 1;
            let start_idx = perform_site.resume_path[last_depth].stmt_idx() + 1;
            match &perform_site.resume_path[last_depth] {
                ImmediateResumeFrame::WhileBody {
                    while_cond,
                    while_body,
                    ..
                } => {
                    self.codegen_immediate_resume_while_tail_and_continue(
                        exec_plan,
                        last_depth,
                        start_idx,
                        (while_cond, while_body),
                        target_ptr,
                        ImmediateResumeArmDispatch {
                            binder_slots: &binder_slots,
                            resume_used_ptr,
                            arm_bb,
                        },
                    )?;
                }
                _ => {
                    self.codegen_immediate_resume_frame_tail_and_continue(
                        exec_plan, last_depth, start_idx,
                    )?;
                }
            }
        }
        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }

        self.env.pop_scope();

        self.builder.position_at_end(finally_unwind_bb);
        if let Some(custom_outer_top) = custom_outer_top {
            let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
            let _ = self.builder.build_call(
                rt_swap,
                &[custom_outer_top.into()],
                "handle_mixed_unwind_detach",
            )?;
        }
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
                            kind: "handle mixed finally unwind needs function return type",
                            at: span.into(),
                        })?;
                let v = self.default_value(span, ret_ty)?;
                self.emit_return(span, ret_ty, v)?;
            }
        }

        self.builder.position_at_end(finally_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.builder.build_unconditional_branch(done_bb)?;
        }

        self.builder.position_at_end(effect_dispatch_bb);
        let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
        let tag_call =
            self.builder
                .build_call(rt_read_tag, &[], "handle_mixed_dispatch_read_op_tag")?;
        let tag_raw =
            tag_call
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
        let mut dispatch_cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
            Vec::new();
        if let Some((_, raise_catch_bb)) = raise_sibling {
            let raise_tag = self.effect_op_tag("scoop.core.Raise.raise");
            let raise_tag_i32 = i32_ty.const_int(raise_tag as u64, false);
            dispatch_cases.push((raise_tag_i32, raise_catch_bb));
        }
        for custom in &custom_siblings {
            let tag_i32 = i32_ty.const_int(custom.op_tag as u64, false);
            dispatch_cases.push((tag_i32, custom.catch_bb));
        }
        self.builder
            .build_switch(slot_tag, dispatch_no_match_bb, &dispatch_cases)?;

        self.builder.position_at_end(dispatch_no_match_bb);
        if let Some(custom_outer_top) = custom_outer_top {
            let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
            let _ = self.builder.build_call(
                rt_swap,
                &[custom_outer_top.into()],
                "handle_mixed_dispatch_detach",
            )?;
        }
        self.builder.build_unconditional_branch(finally_unwind_bb)?;

        if let Some((raise_arm, raise_catch_bb)) = raise_sibling {
            let binder = &raise_arm.op.binders[0];
            self.builder.position_at_end(raise_catch_bb);

            if let Some(custom_outer_top) = custom_outer_top {
                let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
                let _ = self.builder.build_call(
                    rt_swap,
                    &[custom_outer_top.into()],
                    "handle_mixed_raise_detach",
                )?;
            }

            let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
            let call = self
                .builder
                .build_call(rt_len, &[], "mixed_raise_read_slot_len_words")?;
            let raw =
                call.try_as_basic_value()
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
                "mixed_raise_slot_len_ok",
            )?;
            let len_ok_bb = self
                .context
                .append_basic_block(func, "mixed_raise_slot_len_ok_bb");
            let len_bad_bb = self
                .context
                .append_basic_block(func, "mixed_raise_slot_len_bad_bb");
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
                "mixed_raise_read_slot_word0",
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
                "mixed_raise_read_slot_word1",
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
            let _ = self
                .builder
                .build_call(rt_clear, &[], "mixed_raise_clear")?;

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
                        "mixed_raise_kind_is_int",
                    )?;
                    let ok_bb = self
                        .context
                        .append_basic_block(func, "mixed_raise_kind_int_ok");
                    let bad_bb = self
                        .context
                        .append_basic_block(func, "mixed_raise_kind_int_bad");
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
                        "mixed_raise_kind_is_runtime_error",
                    )?;
                    let ok_bb = self
                        .context
                        .append_basic_block(func, "mixed_raise_kind_runtime_error_ok");
                    let bad_bb = self
                        .context
                        .append_basic_block(func, "mixed_raise_kind_runtime_error_bad");
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
                        "mixed_raise_runtime_error_tag_i32",
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
                        "mixed_raise_runtime_error_tag",
                    )?;
                    agg = self.builder.build_insert_value(
                        agg,
                        payload_word_zero,
                        1,
                        "mixed_raise_runtime_error_payload_word",
                    )?;
                    agg = self.builder.build_insert_value(
                        agg,
                        payload_ptr_zero,
                        2,
                        "mixed_raise_runtime_error_payload_ptr",
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

            self.push_raise_target(finally_unwind_bb);
            let arm_v = self.codegen_expr_in_expected_context(&raise_arm.body, Some(out_ty))?;
            self.pop_raise_target();
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

        for custom in &custom_siblings {
            let arm = custom.arm;
            let binder = &arm.op.binders[0];
            self.builder.position_at_end(custom.catch_bb);

            if let Some(custom_outer_top) = custom_outer_top {
                let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
                let _ = self.builder.build_call(
                    rt_swap,
                    &[custom_outer_top.into()],
                    "handle_mixed_custom_detach",
                )?;
            }

            let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
            let call = self
                .builder
                .build_call(rt_len, &[], "mixed_custom_read_slot_len_words")?;
            let raw =
                call.try_as_basic_value()
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
                "mixed_custom_slot_len_ok",
            )?;
            let len_ok_bb = self
                .context
                .append_basic_block(func, "mixed_custom_slot_len_ok_bb");
            let len_bad_bb = self
                .context
                .append_basic_block(func, "mixed_custom_slot_len_bad_bb");
            self.builder
                .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

            self.builder.position_at_end(len_bad_bb);
            self.emit_exit_with_code(span, 3)?;

            self.builder.position_at_end(len_ok_bb);

            let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
            let value_call =
                self.builder
                    .build_call(rt_read, &[], "mixed_custom_read_slot_word0")?;
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
            let gc_call =
                self.builder
                    .build_call(rt_read_gc, &[], "mixed_custom_read_slot_gc_ref")?;
            let gc_raw =
                gc_call
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_gc_ref return value",
                        at: span.into(),
                    })?;
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

            let rt_clear = self.declare_runtime_effect_clear();
            let _ = self
                .builder
                .build_call(rt_clear, &[], "mixed_custom_clear")?;

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
            if catch_end.get_terminator().is_none() {
                if let Some(ptr) = result_ptr {
                    let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
                }
                self.builder.build_unconditional_branch(finally_bb)?;
            }
            self.env.pop_scope();
        }

        self.builder.position_at_end(done_bb);
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
                let loaded = self
                    .builder
                    .build_load(llvm_ty, ptr, "handle_mixed_result")?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }

    /// T2003c0b1 / T2003c0b2b1：mixed-arm immediate-resume + sibling escape-continuation 的 direct-site 子集。
    ///
    /// 当前支持：
    /// - 一个 immediate-resume arm + 一个 escape-continuation arm；
    /// - immediate site 是 top-level `val = perform`；
    /// - escape sites 是 immediate site 之后的 1..N 个 top-level `val = perform` direct sites；
    /// - indirect / pre-immediate / nested direct-site 组合继续稳定诊断。
    fn codegen_handle_expr_immediate_resume_with_escape_sibling_direct<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        immediate: (&'hir hir::HandleArm, hir::SymbolId),
        escape: (&'hir hir::HandleArm, hir::SymbolId),
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        #[derive(Debug)]
        struct DirectEscapeSite<'hir> {
            stmt_idx: usize,
            decl: &'hir hir::ValDecl,
            op: &'hir hir::EffectOpRef,
            args: &'hir [hir::CallArg],
            id: hir::SymbolId,
        }

        #[derive(Clone, Copy)]
        struct CaptureMeta {
            id: hir::SymbolId,
            hir_ty: Option<TypeId>,
            ty: CgTy,
            mutable: bool,
        }

        let (immediate_arm, resume_symbol) = immediate;
        let (escape_arm, continuation_symbol) = escape;
        let sibling_plan = self.collect_sibling_nonresuming_plan(sibling_nonresuming_arms)?;
        let raise_sibling = sibling_plan.raise_arm;
        let custom_siblings = sibling_plan.custom_arms.clone();

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

        let mut body_decl_all: HashMap<hir::SymbolId, CaptureMeta> = HashMap::new();
        let mut body_decl_spans: HashMap<hir::SymbolId, crate::span::Span> = HashMap::new();
        let mut body_decl_order: HashMap<hir::SymbolId, usize> = HashMap::new();
        let mut next_decl_order = 0usize;
        let mut escape_sites: Vec<DirectEscapeSite<'hir>> = Vec::new();
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if let hir::StmtKind::Val(decl) = &stmt.kind
                && let Some(id) = decl.id
            {
                let ty = self
                    .cg_ty_of(decl.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape capture local type",
                        at: decl.span.into(),
                    })?;
                let meta = CaptureMeta {
                    id,
                    hir_ty: Some(decl.ty),
                    ty,
                    mutable: decl.mutable,
                };
                body_decl_all.insert(id, meta);
                body_decl_spans.insert(id, decl.span);
                body_decl_order.insert(id, next_decl_order);
                next_decl_order += 1;
            }

            if !self
                .immediate_resume_stmt_contains_matching_direct_perform(stmt, &escape_arm.op.op.fqn)
            {
                continue;
            }

            if idx <= perform_idx {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation (perform before immediate site not yet supported)",
                    at: stmt.span.into(),
                });
            }

            match &stmt.kind {
                hir::StmtKind::Val(decl) => {
                    let Some(init) = decl.init.as_ref() else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (missing perform init)",
                            at: decl.span.into(),
                        });
                    };
                    let hir::ExprKind::Perform { op, args } = &init.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (only top-level val-bound direct perform supported)",
                            at: init.span.into(),
                        });
                    };
                    if op.fqn != escape_arm.op.op.fqn {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation (only top-level val-bound direct perform supported)",
                            at: init.span.into(),
                        });
                    }
                    let Some(id) = decl.id else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape continuation perform binding id",
                            at: decl.span.into(),
                        });
                    };
                    escape_sites.push(DirectEscapeSite {
                        stmt_idx: idx,
                        decl,
                        op,
                        args: args.as_slice(),
                        id,
                    });
                }
                hir::StmtKind::Expr(expr)
                    if matches!(
                        &expr.kind,
                        hir::ExprKind::Perform { op, .. } if op.fqn == escape_arm.op.op.fqn
                    ) =>
                {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (perform must be bound to val)",
                        at: expr.span.into(),
                    });
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (only top-level val-bound direct perform supported)",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        let Some(first_escape_site) = escape_sites.first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (direct top-level perform site required)",
                at: escape_arm.span.into(),
            });
        };
        if !sibling_nonresuming_arms.is_empty() && escape_sites.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation with sibling non-resuming (only top-level direct single-site supported)",
                at: escape_arm.span.into(),
            });
        }

        let mut escape_site_pc_by_stmt_idx: HashMap<usize, usize> = HashMap::new();
        for (pc, site) in escape_sites.iter().enumerate() {
            escape_site_pc_by_stmt_idx.insert(site.stmt_idx, pc);
        }

        for site in &escape_sites {
            if escape_arm.op.binders.len() != site.args.len() {
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

        let mut outer_visible_supported: Vec<CaptureMeta> = Vec::new();
        let mut outer_visible_all: HashMap<hir::SymbolId, CaptureMeta> = HashMap::new();
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
            let meta = CaptureMeta {
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
        for site in &escape_sites {
            let Some(&site_order) = body_decl_order.get(&site.id) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape continuation perform binding id",
                    at: site.decl.span.into(),
                });
            };

            let mut used_after: HashSet<hir::SymbolId> = HashSet::new();
            for stmt in handle.body.stmts.iter().skip(site.stmt_idx + 1) {
                Self::collect_used_locals_in_stmt_static(stmt, &mut used_after);
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

        let mut body_visible_supported: Vec<CaptureMeta> = Vec::new();
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

        let state_ty_name = format!("scoop.runtime.MixedEscapeState__{func_name}_{seq}");
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

        let step_name = format!("__scoop_mixed_escape_step__{func_name}_{seq}");
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
                    kind: "mixed escape step state param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_ptr_ty = cg.llvm_ptr_type(cg.gc_address_space());
            let state_ptr = cg.builder.build_pointer_cast(
                state_raw,
                state_ptr_ty,
                "mixed_escape_step_state_ptr",
            )?;

            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape step resume_word param",
                    at: span.into(),
                })?
                .into_int_value();
            let resume_gc_ref = step_fn
                .get_nth_param(2)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape step resume_gc_ref param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let step_dispatch_pc_ptr =
                cg.builder
                    .build_struct_gep(state_ty, state_ptr, 2, "mixed_escape_step_pc_gep")?;

            for (idx, cap) in outer_visible_supported.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "mixed_escape_step_outer_gep",
                )?;
                let name = format!("mixed_escape_outer_{}", cap.id.as_u32());
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
                    "mixed_escape_step_body_gep",
                )?;
                let name = format!("mixed_escape_body_{}", cap.id.as_u32());
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
                let binder_ty =
                    cg.cg_ty_of(binder.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape binder type",
                            at: binder.span.into(),
                        })?;
                let ptr = cg.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
                step_escape_binder_slots.push(ImmediateResumeBinderSlot {
                    id: binder.id,
                    hir_ty: binder.ty,
                    ty: binder_ty,
                    ptr,
                });
            }
            let cont_ptr =
                cg.create_entry_alloca(span, &format!("handle_mixed_escape_k_{seq}"), CgTy::Ref)?;
            let step_sibling_dispatch = self.build_sibling_nonresuming_dispatch_blocks(
                step_fn,
                "mixed_escape_step",
                &sibling_plan,
            );
            let step_effect_dispatch_bb = step_sibling_dispatch.effect_dispatch_bb;
            let step_effect_dispatch_nomatch_bb =
                step_sibling_dispatch.effect_dispatch_nomatch_bb;
            let step_raise_catch_bb = step_sibling_dispatch.raise_catch_bb;
            let step_custom_catch_bbs = step_sibling_dispatch.custom_catch_bbs;

            let dispatch_bb = self
                .context
                .append_basic_block(step_fn, "mixed_escape_step_dispatch");
            let bad_state_bb = self
                .context
                .append_basic_block(step_fn, "mixed_escape_step_bad_pc");
            let mut state_bbs = Vec::new();
            for pc in 0..escape_sites.len() {
                state_bbs.push(
                    self.context
                        .append_basic_block(step_fn, &format!("mixed_escape_step_pc_{pc}")),
                );
            }

            cg.builder.build_unconditional_branch(dispatch_bb)?;

            cg.builder.position_at_end(dispatch_bb);
            let pc = cg
                .builder
                .build_load(i32_ty, step_dispatch_pc_ptr, "mixed_escape_step_pc")?
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
                        .unwrap_or("mixed_escape_resume_value");
                    let ptr =
                        cg.create_entry_alloca(site.decl.span, name, escape_resume_value_ty)?;
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

                if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                    for (idx, custom) in custom_siblings.iter().enumerate() {
                        cg.push_effect_unwind_target(
                            &custom.arm.op.op.fqn,
                            step_custom_catch_bbs[idx],
                        );
                    }
                    cg.push_raise_target(step_effect_dispatch_bb);
                }

                let mut escaped = false;
                for (idx, stmt) in handle.body.stmts.iter().enumerate().skip(site.stmt_idx + 1) {
                    if let Some(&next_pc) = escape_site_pc_by_stmt_idx.get(&idx) {
                        let next_site = &escape_sites[next_pc];
                        let hir::StmtKind::Val(decl) = &stmt.kind else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (expected perform binding)",
                                at: stmt.span.into(),
                            });
                        };
                        let Some(init) = decl.init.as_ref() else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (missing perform init)",
                                at: decl.span.into(),
                            });
                        };
                        let hir::ExprKind::Perform { op, args } = &init.kind else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation (expected direct perform binding)",
                                at: init.span.into(),
                            });
                        };
                        if op.fqn != next_site.op.fqn {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape op mismatch",
                                at: op.span.into(),
                            });
                        }

                        for (slot, arg) in step_escape_binder_slots.iter().zip(args.iter()) {
                            let hir::CallArg::Positional(expr) = arg else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape named perform arg",
                                    at: stmt.span.into(),
                                });
                            };
                            let v = cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                            let _stored = cg.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                        }

                        for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                            let field_ptr = cg.builder.build_struct_gep(
                                state_ty,
                                state_ptr,
                                outer_field_base.saturating_add(field_idx as u32),
                                "mixed_escape_step_capture_outer_gep",
                            )?;
                            let local =
                                cg.env
                                    .get(cap.id)
                                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                                        kind: "mixed escape capture local not found",
                                        at: decl.span.into(),
                                    })?;
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: decl.span.into(),
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
                                "mixed_escape_step_capture_body_gep",
                            )?;
                            let Some(local) = cg.env.get(cap.id) else {
                                continue;
                            };
                            if local.ty != cap.ty {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape capture local type mismatch",
                                    at: decl.span.into(),
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
                            "mixed_escape_step_pc_store_gep",
                        )?;
                        let _ = cg
                            .builder
                            .build_store(pc_ptr, i32_ty.const_int(next_pc as u64, false))?;

                        let rt_cont_alloc = cg.declare_runtime_continuation_alloc();
                        let step_ptr = step_fn.as_global_value().as_pointer_value();
                        let cont_call = cg.builder.build_call(
                            rt_cont_alloc,
                            &[state_raw.into(), step_ptr.into()],
                            "mixed_escape_step_cont_alloc",
                        )?;
                        let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape continuation alloc return value",
                                at: decl.span.into(),
                            },
                        )?;
                        let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "mixed escape continuation alloc return type",
                                at: decl.span.into(),
                            });
                        };

                        let pin = cg.declare_runtime_gc_pin();
                        let _ = cg.builder.build_call(
                            pin,
                            &[k_raw.into()],
                            "mixed_escape_step_k_pin",
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

                        let frame_ptr = cg.builder.build_struct_gep(
                            state_ty,
                            state_ptr,
                            1,
                            "mixed_escape_step_frame_gep",
                        )?;
                        let prev_ptr = cg.builder.build_struct_gep(
                            handler_frame_ty,
                            frame_ptr,
                            0,
                            "mixed_escape_step_prev_gep",
                        )?;
                        let prev_raw =
                            cg.builder
                                .build_load(i8_ptr_ty, prev_ptr, "mixed_escape_step_prev")?;
                        let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                        let _ = cg.builder.build_call(
                            rt_swap,
                            &[prev_raw.into()],
                            "mixed_escape_step_detach",
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
                        let arm_v =
                            cg.codegen_expr_in_expected_context(&escape_arm.body, Some(out_ty))?;
                        let _arm_v = if out_ty == CgTy::Unit {
                            CgValue::unit()
                        } else {
                            cg.coerce_value(escape_arm.body.span, arm_v, out_ty)?
                        };
                        cg.env.pop_scope();

                        let llvm_ref_ty = cg.llvm_basic_type_of(span, CgTy::Ref)?;
                        let k_loaded = cg
                            .builder
                            .build_load(llvm_ref_ty, cont_ptr, "mixed_escape_step_k_unpin_load")?
                            .into_pointer_value();
                        let unpin = cg.declare_runtime_gc_unpin();
                        let _ = cg.builder.build_call(
                            unpin,
                            &[k_loaded.into()],
                            "mixed_escape_step_k_unpin",
                        )?;
                        cg.builder.build_return(None)?;
                        escaped = true;
                        break;
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
                                let local =
                                    cg.env.get(id).ok_or(LlvmEmitError::UnsupportedMainBody {
                                        kind: "lifted local slot missing",
                                        at: decl.span.into(),
                                    })?;
                                let v = cg.codegen_expr_in_expected_context(init, Some(decl_ty))?;
                                let _stored =
                                    cg.store_local_value(decl.span, local.ptr, decl_ty, v)?;
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

                if step_effect_dispatch_bb.is_some() {
                    cg.pop_raise_target();
                    for _ in custom_siblings.iter().rev() {
                        cg.pop_effect_unwind_target();
                    }
                }

                if step_effect_dispatch_bb.is_some() {
                    cg.pop_raise_target();
                    for _ in custom_siblings.iter().rev() {
                        cg.pop_effect_unwind_target();
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
                        "mixed_escape_state_unpin",
                    )?;
                    cg.builder.build_return(None)?;
                }

                if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                    let step_effect_dispatch_nomatch_bb = step_effect_dispatch_nomatch_bb
                        .expect("mixed escape step dispatch_nomatch bb should exist");
                    cg.builder.position_at_end(step_effect_dispatch_bb);
                    let rt_read_tag = cg.declare_runtime_effect_perform_slot_read_op_tag();
                    let tag_call =
                        cg.builder
                            .build_call(rt_read_tag, &[], "mixed_escape_step_read_op_tag")?;
                    let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "mixed escape step read_op_tag return value",
                            at: span.into(),
                        },
                    )?;
                    let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "mixed escape step read_op_tag return type",
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
                        "mixed_escape_step_state_unpin_nomatch",
                    )?;
                    cg.builder.build_return(None)?;

                    if let (Some(raise_arm), Some(step_raise_catch_bb)) =
                        (raise_sibling, step_raise_catch_bb)
                    {
                        let binder = &raise_arm.op.binders[0];
                        cg.builder.position_at_end(step_raise_catch_bb);

                        let frame_ptr = cg.builder.build_struct_gep(
                            state_ty,
                            state_ptr,
                            1,
                            "mixed_escape_step_raise_frame_gep",
                        )?;
                        let prev_ptr = cg.builder.build_struct_gep(
                            handler_frame_ty,
                            frame_ptr,
                            0,
                            "mixed_escape_step_raise_prev_gep",
                        )?;
                        let prev_raw = cg.builder.build_load(
                            i8_ptr_ty,
                            prev_ptr,
                            "mixed_escape_step_raise_prev",
                        )?;
                        let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                        let _ = cg.builder.build_call(
                            rt_swap,
                            &[prev_raw.into()],
                            "mixed_escape_step_raise_detach",
                        )?;

                        let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                        let call = cg.builder.build_call(
                            rt_len,
                            &[],
                            "mixed_escape_step_raise_read_slot_len_words",
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
                            "mixed_escape_step_raise_slot_len_ok",
                        )?;
                        let len_ok_bb = cg
                            .context
                            .append_basic_block(step_fn, "mixed_escape_step_raise_slot_len_ok_bb");
                        let len_bad_bb = cg
                            .context
                            .append_basic_block(step_fn, "mixed_escape_step_raise_slot_len_bad_bb");
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
                            "mixed_escape_step_raise_read_slot_word0",
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
                            "mixed_escape_step_raise_read_slot_word1",
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
                            "mixed_escape_step_raise_clear",
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
                                    "mixed_escape_step_raise_kind_is_int",
                                )?;
                                let ok_bb = cg.context.append_basic_block(
                                    step_fn,
                                    "mixed_escape_step_raise_kind_int_ok",
                                );
                                let bad_bb = cg.context.append_basic_block(
                                    step_fn,
                                    "mixed_escape_step_raise_kind_int_bad",
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
                                    "mixed_escape_step_raise_kind_is_runtime_error",
                                )?;
                                let ok_bb = cg.context.append_basic_block(
                                    step_fn,
                                    "mixed_escape_step_raise_kind_runtime_error_ok",
                                );
                                let bad_bb = cg.context.append_basic_block(
                                    step_fn,
                                    "mixed_escape_step_raise_kind_runtime_error_bad",
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
                                    "mixed_escape_step_runtime_error_tag_i32",
                                )?;
                                let payload_word_zero =
                                    cg.int_type(cg.enum_payload_ty()).const_int(0, false);
                                let payload_ptr_zero = cg.llvm_gc_i8_ptr_type().const_null();
                                let llvm_enum_ty = cg.llvm_enum_value_type(span, enum_ty)?;
                                let llvm_enum_ty = llvm_enum_ty.into_struct_type();
                                let mut agg: AggregateValueEnum<'ctx> =
                                    llvm_enum_ty.get_undef().into();
                                agg = cg.builder.build_insert_value(
                                    agg,
                                    tag_i32,
                                    0,
                                    "mixed_escape_step_runtime_error_tag",
                                )?;
                                agg = cg.builder.build_insert_value(
                                    agg,
                                    payload_word_zero,
                                    1,
                                    "mixed_escape_step_runtime_error_payload_word",
                                )?;
                                agg = cg.builder.build_insert_value(
                                    agg,
                                    payload_ptr_zero,
                                    2,
                                    "mixed_escape_step_runtime_error_payload_ptr",
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
                        let _ = cg.store_local_value(
                            binder.span,
                            binder_ptr,
                            binder_cg_ty,
                            binder_value,
                        )?;
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
                                "mixed_escape_step_state_unpin_raise",
                            )?;
                            cg.builder.build_return(None)?;
                        }
                    }

                    for (idx, custom) in custom_siblings.iter().enumerate() {
                        let arm = custom.arm;
                        let binder = &arm.op.binders[0];
                        cg.builder.position_at_end(step_custom_catch_bbs[idx]);

                        let frame_ptr = cg.builder.build_struct_gep(
                            state_ty,
                            state_ptr,
                            1,
                            "mixed_escape_step_custom_frame_gep",
                        )?;
                        let prev_ptr = cg.builder.build_struct_gep(
                            handler_frame_ty,
                            frame_ptr,
                            0,
                            "mixed_escape_step_custom_prev_gep",
                        )?;
                        let prev_raw = cg.builder.build_load(
                            i8_ptr_ty,
                            prev_ptr,
                            "mixed_escape_step_custom_prev",
                        )?;
                        let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                        let _ = cg.builder.build_call(
                            rt_swap,
                            &[prev_raw.into()],
                            "mixed_escape_step_custom_detach",
                        )?;

                        let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                        let call = cg.builder.build_call(
                            rt_len,
                            &[],
                            "mixed_escape_step_custom_read_slot_len_words",
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
                            "mixed_escape_step_custom_slot_len_ok",
                        )?;
                        let len_ok_bb = cg
                            .context
                            .append_basic_block(step_fn, "mixed_escape_step_custom_slot_len_ok_bb");
                        let len_bad_bb = cg.context.append_basic_block(
                            step_fn,
                            "mixed_escape_step_custom_slot_len_bad_bb",
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
                            "mixed_escape_step_custom_read_slot_word0",
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
                            "mixed_escape_step_custom_read_slot_gc_ref",
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
                        let _ = cg.store_local_value(
                            binder.span,
                            binder_ptr,
                            binder_cg_ty,
                            binder_value,
                        )?;
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
                            "mixed_escape_step_custom_clear",
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
                                "mixed_escape_step_state_unpin_custom",
                            )?;
                            cg.builder.build_return(None)?;
                        }
                    }
                }
            }

            cg.env.pop_scope();
        }
        self.builder.position_at_end(saved_block);

        let resume_blocks = self.build_mixed_escape_resume_blocks(func, "handle_mixed_escape");
        let dispatch_bb = resume_blocks.dispatch_bb;
        let state0_bb = resume_blocks.state0_bb;
        let state1_bb = resume_blocks.state1_bb;
        let arm_bb = resume_blocks.arm_bb;
        let escape_arm_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_arm");
        let done_bb = resume_blocks.done_bb;
        let bad_state_bb = resume_blocks.bad_state_bb;
        let finally_bb = resume_blocks.finally_bb;
        let finally_unwind_bb = resume_blocks.finally_unwind_bb;
        let sibling_dispatch =
            self.build_sibling_nonresuming_dispatch_blocks(func, "handle_mixed_escape", &sibling_plan);
        let effect_dispatch_bb = sibling_dispatch.effect_dispatch_bb;
        let effect_dispatch_nomatch_bb = sibling_dispatch.effect_dispatch_nomatch_bb;
        let raise_catch_bb = sibling_dispatch.raise_catch_bb;
        let custom_catch_bbs = sibling_dispatch.custom_catch_bbs;

        let state_ptr =
            self.create_entry_alloca_raw(span, "handle_mixed_escape_state", i32_ty.into())?;
        let resume_used_ptr = self.create_entry_alloca_raw(
            span,
            "handle_mixed_escape_resume_used",
            self.context.bool_type().into(),
        )?;
        let resume_value_ptr = if resume_value_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(
                span,
                "handle_mixed_escape_resume_value",
                resume_value_ty,
            )?)
        };
        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_mixed_escape_result", out_ty)?)
        };

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
            let binder_ty = self
                .cg_ty_of(binder.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape binder type",
                    at: binder.span.into(),
                })?;
            let ptr = self.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
            escape_binder_slots.push(ImmediateResumeBinderSlot {
                id: binder.id,
                hir_ty: binder.ty,
                ty: binder_ty,
                ptr,
            });
        }
        let cont_ptr =
            self.create_entry_alloca(span, &format!("handle_mixed_escape_k_{seq}"), CgTy::Ref)?;

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
            format!("__scoop_type_desc_mixed_escape_state__{func_name}_{seq}");
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
            "mixed_escape_state_desc_i8",
        )?;
        let call = self.builder.build_call(
            rt_alloc,
            &[state_desc_i8.into(), size_v.into()],
            "rt_alloc_mixed_escape_state",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "mixed escape alloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(state_raw) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "mixed escape alloc return type",
                at: span.into(),
            });
        };

        let pin = self.declare_runtime_gc_pin();
        let _ = self
            .builder
            .build_call(pin, &[state_raw.into()], "mixed_escape_state_pin")?;

        let state_gc_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let state_gc_ptr = self.builder.build_pointer_cast(
            state_raw,
            state_gc_ptr_ty,
            "mixed_escape_state_ptr",
        )?;

        for (idx, cap) in outer_visible_supported.iter().enumerate() {
            let field_idx = outer_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "mixed_escape_state_outer_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }
        for (idx, cap) in body_visible_supported.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "mixed_escape_state_body_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }

        let frame_ptr = self.builder.build_struct_gep(
            state_ty,
            state_gc_ptr,
            1,
            "mixed_escape_state_frame_gep",
        )?;
        let frame_i8 =
            self.builder
                .build_address_space_cast(frame_ptr, i8_ptr_ty, "mixed_escape_frame_i8")?;
        let escape_tag = self.effect_op_tag(&escape_arm.op.op.fqn);
        let escape_tag_i32 = i32_ty.const_int(escape_tag as u64, false);
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), escape_tag_i32.into()],
            "mixed_escape_push",
        )?;
        let prev_ptr = self.builder.build_struct_gep(
            handler_frame_ty,
            frame_ptr,
            0,
            "mixed_escape_prev_gep",
        )?;
        let escape_outer_top = self
            .builder
            .build_load(i8_ptr_ty, prev_ptr, "mixed_escape_outer_top")?
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
            .build_load(i32_ty, state_ptr, "mixed_escape_state")?
            .into_int_value();
        let cases = [
            (i32_ty.const_int(0, false), state0_bb),
            (i32_ty.const_int(1, false), state1_bb),
        ];
        self.builder.build_switch(state, bad_state_bb, &cases)?;

        self.builder.position_at_end(bad_state_bb);
        self.emit_exit_with_code(span, 3)?;

        self.env.push_scope();

        self.builder.position_at_end(state0_bb);
        for (idx, custom) in custom_siblings.iter().enumerate() {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom_catch_bbs[idx]);
        }
        self.push_raise_target(main_raise_target);
        for stmt in handle.body.stmts.iter().take(perform_idx) {
            self.codegen_immediate_resume_stmt_unit(stmt)?;
        }
        let hir::StmtKind::Val(immediate_decl) = &handle.body.stmts[perform_idx].kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm immediate-resume body (expected perform binding)",
                at: handle.body.stmts[perform_idx].span.into(),
            });
        };
        let target_ptr = self.codegen_immediate_resume_site_binding(
            &perform_site,
            immediate_decl,
            ImmediateResumeArmDispatch {
                binder_slots: &immediate_binder_slots,
                resume_used_ptr,
                arm_bb,
            },
            None,
        )?;
        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }

        self.builder.position_at_end(arm_bb);
        let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
        let _ = self.builder.build_call(
            rt_swap,
            &[escape_outer_top.into()],
            "mixed_escape_detach_for_immediate_arm",
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
        self.push_raise_target(finally_unwind_bb);
        let _ = self.codegen_expr_in_expected_context(&immediate_arm.body, Some(CgTy::Unit))?;
        self.pop_raise_target();
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
            .append_basic_block(arm_func, "handle_mixed_escape_resume_arm_ok");
        let resume_missing_bb = self
            .context
            .append_basic_block(arm_func, "handle_mixed_escape_resume_arm_missing");

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
            "mixed_escape_restore_after_immediate_arm",
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
            let _stored = self.store_local_value(span, target_ptr, resume_value_ty, v)?;
        }

        let mut escaped = false;
        let mut tail_value: CgValue<'ctx> = CgValue::unit();
        for (idx, stmt) in handle.body.stmts.iter().enumerate().skip(perform_idx + 1) {
            if idx == first_escape_site.stmt_idx {
                let hir::StmtKind::Val(decl) = &stmt.kind else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected perform binding)",
                        at: stmt.span.into(),
                    });
                };
                let Some(init) = decl.init.as_ref() else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (missing perform init)",
                        at: decl.span.into(),
                    });
                };
                let hir::ExprKind::Perform { op, args } = &init.kind else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape continuation (expected direct perform binding)",
                        at: init.span.into(),
                    });
                };
                if op.fqn != first_escape_site.op.fqn {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape op mismatch",
                        at: op.span.into(),
                    });
                }

                for (slot, arg) in escape_binder_slots.iter().zip(args.iter()) {
                    let hir::CallArg::Positional(expr) = arg else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape named perform arg",
                            at: stmt.span.into(),
                        });
                    };
                    let v = self.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                    let _stored = self.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
                }

                let pc_ptr = self.builder.build_struct_gep(
                    state_ty,
                    state_gc_ptr,
                    2,
                    "mixed_escape_pc_gep",
                )?;
                let _ = self.builder.build_store(pc_ptr, i32_ty.const_zero())?;

                for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                    let field_ptr = self.builder.build_struct_gep(
                        state_ty,
                        state_gc_ptr,
                        outer_field_base.saturating_add(field_idx as u32),
                        "mixed_escape_capture_outer_gep",
                    )?;
                    let local = self
                        .env
                        .get(cap.id)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "mixed escape capture local not found",
                            at: decl.span.into(),
                        })?;
                    if local.ty != cap.ty {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "mixed escape capture local type mismatch",
                            at: decl.span.into(),
                        });
                    }
                    self.write_escape_capture_local_to_state(span, field_ptr, local.ptr, cap.ty)?;
                }

                for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                    let field_ptr = self.builder.build_struct_gep(
                        state_ty,
                        state_gc_ptr,
                        body_field_base.saturating_add(field_idx as u32),
                        "mixed_escape_capture_body_gep",
                    )?;
                    let Some(local) = self.env.get(cap.id) else {
                        continue;
                    };
                    if local.ty != cap.ty {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "mixed escape capture local type mismatch",
                            at: decl.span.into(),
                        });
                    }
                    self.write_escape_capture_local_to_state(span, field_ptr, local.ptr, cap.ty)?;
                }

                let rt_cont_alloc = self.declare_runtime_continuation_alloc();
                let step_ptr = step_fn.as_global_value().as_pointer_value();
                let cont_call = self.builder.build_call(
                    rt_cont_alloc,
                    &[state_raw.into(), step_ptr.into()],
                    "mixed_escape_cont_alloc",
                )?;
                let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "mixed escape continuation alloc return value",
                        at: decl.span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "mixed escape continuation alloc return type",
                        at: decl.span.into(),
                    });
                };

                let _ = self
                    .builder
                    .build_call(pin, &[k_raw.into()], "mixed_escape_k_pin")?;
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
                    "mixed_escape_detach_for_escape_arm",
                )?;

                self.env.pop_scope();
                self.builder.build_unconditional_branch(escape_arm_bb)?;
                escaped = true;
                break;
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

        if let Some(effect_dispatch_bb) = effect_dispatch_bb {
            let effect_dispatch_nomatch_bb = effect_dispatch_nomatch_bb
                .expect("escape + sibling non-resuming dispatch_nomatch bb should exist");
            self.builder.position_at_end(effect_dispatch_bb);
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call =
                self.builder
                    .build_call(rt_read_tag, &[], "mixed_escape_dispatch_read_op_tag")?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape dispatch read_op_tag return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape dispatch read_op_tag return type",
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
                    "mixed_escape_raise_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "mixed_escape_raise_read_slot_len_words",
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
                    "mixed_escape_raise_slot_len_ok",
                )?;
                let len_ok_bb = self
                    .context
                    .append_basic_block(func, "mixed_escape_raise_slot_len_ok_bb");
                let len_bad_bb = self
                    .context
                    .append_basic_block(func, "mixed_escape_raise_slot_len_bad_bb");
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
                    "mixed_escape_raise_read_slot_word0",
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
                    "mixed_escape_raise_read_slot_word1",
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
                let _ = self
                    .builder
                    .build_call(rt_clear, &[], "mixed_escape_raise_clear")?;

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
                            "mixed_escape_raise_kind_is_int",
                        )?;
                        let ok_bb = self
                            .context
                            .append_basic_block(func, "mixed_escape_raise_kind_int_ok");
                        let bad_bb = self
                            .context
                            .append_basic_block(func, "mixed_escape_raise_kind_int_bad");
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
                            "mixed_escape_raise_kind_is_runtime_error",
                        )?;
                        let ok_bb = self
                            .context
                            .append_basic_block(func, "mixed_escape_raise_kind_runtime_error_ok");
                        let bad_bb = self
                            .context
                            .append_basic_block(func, "mixed_escape_raise_kind_runtime_error_bad");
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
                            "mixed_escape_runtime_error_tag_i32",
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
                            "mixed_escape_runtime_error_tag",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_word_zero,
                            1,
                            "mixed_escape_runtime_error_payload_word",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_ptr_zero,
                            2,
                            "mixed_escape_runtime_error_payload_ptr",
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
                    "mixed_escape_custom_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "mixed_escape_custom_read_slot_len_words",
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
                    "mixed_escape_custom_slot_len_ok",
                )?;
                let len_ok_bb = self
                    .context
                    .append_basic_block(func, "mixed_escape_custom_slot_len_ok_bb");
                let len_bad_bb = self
                    .context
                    .append_basic_block(func, "mixed_escape_custom_slot_len_bad_bb");
                self.builder
                    .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                self.builder.position_at_end(len_bad_bb);
                self.emit_exit_with_code(span, 3)?;

                self.builder.position_at_end(len_ok_bb);

                let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
                let value_call =
                    self.builder
                        .build_call(rt_read, &[], "mixed_escape_custom_read_slot_word0")?;
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
                    "mixed_escape_custom_read_slot_gc_ref",
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
                let _ = self
                    .builder
                    .build_call(rt_clear, &[], "mixed_escape_custom_clear")?;

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

        self.builder.position_at_end(escape_arm_bb);
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
            "mixed_escape_finally_unwind_detach",
        )?;
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
            "mixed_escape_finally_detach",
        )?;
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.builder.build_unconditional_branch(done_bb)?;
        }

        self.builder.position_at_end(done_bb);
        let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
        let k_loaded = self
            .builder
            .build_load(llvm_ref_ty, cont_ptr, "mixed_escape_k_unpin_load")?
            .into_pointer_value();
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self
            .builder
            .build_call(unpin, &[k_loaded.into()], "mixed_escape_k_unpin")?;

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
                let loaded = self
                    .builder
                    .build_load(llvm_ty, ptr, "handle_mixed_escape_result")?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }


    /// T2003c0b2a：mixed-arm immediate-resume + sibling escape-continuation 的单 indirect-site 子集。
    ///
    /// 当前仅支持：
    /// - 一个 immediate-resume arm + 一个 escape-continuation arm；
    /// - immediate site 是 top-level `val = perform`；
    /// - escape site 是 immediate site 之后的单个 top-level `val = f(...)` indirect call site；
    /// - richer mixed 组合（multiple indirect sites、direct+indirect 共存等）继续稳定诊断。
    fn codegen_handle_expr_immediate_resume_with_escape_sibling_indirect<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        immediate: (&'hir hir::HandleArm, hir::SymbolId),
        escape: (&'hir hir::HandleArm, hir::SymbolId),
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        #[derive(Debug)]
        struct IndirectEscapeSite<'hir> {
            stmt_idx: usize,
            decl: &'hir hir::ValDecl,
            init: &'hir hir::Expr,
            id: hir::SymbolId,
        }

        #[derive(Clone, Copy)]
        struct CaptureMeta {
            id: hir::SymbolId,
            hir_ty: Option<TypeId>,
            ty: CgTy,
            mutable: bool,
        }

        fn is_supported_capture_ty(ty: CgTy) -> bool {
            matches!(ty, CgTy::Ref | CgTy::String | CgTy::Bool | CgTy::Int(_))
        }

        let (immediate_arm, resume_symbol) = immediate;
        let (escape_arm, continuation_symbol) = escape;
        let sibling_plan = self.collect_sibling_nonresuming_plan(sibling_nonresuming_arms)?;
        let raise_sibling = sibling_plan.raise_arm;
        let custom_siblings = sibling_plan.custom_arms.clone();

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

        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if !self
                .immediate_resume_stmt_contains_matching_direct_perform(stmt, &escape_arm.op.op.fqn)
            {
                continue;
            }

            let kind = if idx <= perform_idx {
                "handle mixed-arm escape continuation (perform before immediate site not yet supported)"
            } else {
                "handle mixed-arm escape continuation (direct + indirect sites not yet supported)"
            };
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: stmt.span.into(),
            });
        }

        let indirect_sites =
            self.scan_for_indirect_perform_call_sites(&handle.body, &escape_arm.op.op.fqn);
        if indirect_sites
            .iter()
            .any(|site| site.stmt_idx <= perform_idx)
        {
            let at = handle.body.stmts[indirect_sites
                .iter()
                .find(|site| site.stmt_idx <= perform_idx)
                .map(|site| site.stmt_idx)
                .unwrap_or(perform_idx)]
            .span;
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (indirect perform before immediate site not yet supported)",
                at: at.into(),
            });
        }

        let indirect_after: Vec<&IndirectPerformCallSite> = indirect_sites
            .iter()
            .filter(|site| site.stmt_idx > perform_idx)
            .collect();
        if indirect_after.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (single indirect call site required)",
                at: escape_arm.span.into(),
            });
        }
        if indirect_after.len() > 1 {
            let at = handle.body.stmts[indirect_after[1].stmt_idx].span;
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (multiple indirect call sites not yet supported)",
                at: at.into(),
            });
        }

        let indirect_site = indirect_after[0];
        let call_stmt = &handle.body.stmts[indirect_site.stmt_idx];
        let hir::StmtKind::Val(escape_decl) = &call_stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (indirect site must be val-bound)",
                at: call_stmt.span.into(),
            });
        };
        let Some(call_init) = escape_decl.init.as_ref() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (missing indirect call init)",
                at: escape_decl.span.into(),
            });
        };
        if !matches!(&call_init.kind, hir::ExprKind::Call { .. }) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation (indirect site must be call expression)",
                at: call_init.span.into(),
            });
        }
        let Some(escape_id) = escape_decl.id else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape continuation perform binding id",
                at: escape_decl.span.into(),
            });
        };
        let escape_site = IndirectEscapeSite {
            stmt_idx: indirect_site.stmt_idx,
            decl: escape_decl,
            init: call_init,
            id: escape_id,
        };

        let escape_resume_value_ty =
            self.cg_ty_of(escape_site.decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape perform value type",
                    at: escape_site.decl.span.into(),
                })?;

        let mut used_after: HashSet<hir::SymbolId> = HashSet::new();
        Self::collect_used_locals_in_stmt_static(
            &handle.body.stmts[escape_site.stmt_idx],
            &mut used_after,
        );
        for stmt in handle.body.stmts.iter().skip(escape_site.stmt_idx + 1) {
            Self::collect_used_locals_in_stmt_static(stmt, &mut used_after);
        }
        used_after.remove(&escape_site.id);

        let mut outer_visible_supported: Vec<CaptureMeta> = Vec::new();
        let mut outer_visible_all: HashMap<hir::SymbolId, CaptureMeta> = HashMap::new();
        let mut seen_outer: HashSet<hir::SymbolId> = HashSet::new();
        for scope in self.env.scopes.iter().rev() {
            for (&id, &local) in scope.iter() {
                if !seen_outer.insert(id) {
                    continue;
                }
                let meta = CaptureMeta {
                    id,
                    hir_ty: local.hir_ty,
                    ty: local.ty,
                    mutable: local.mutable,
                };
                outer_visible_all.insert(id, meta);
                if is_supported_capture_ty(local.ty) {
                    outer_visible_supported.push(meta);
                }
            }
        }
        outer_visible_supported.sort_by_key(|meta| meta.id.as_u32());

        let mut body_decl_all: HashMap<hir::SymbolId, CaptureMeta> = HashMap::new();
        let mut body_decl_spans: HashMap<hir::SymbolId, crate::span::Span> = HashMap::new();
        let mut body_visible_supported: Vec<CaptureMeta> = Vec::new();
        for stmt in handle.body.stmts.iter().take(escape_site.stmt_idx) {
            if let hir::StmtKind::Val(decl) = &stmt.kind
                && let Some(id) = decl.id
            {
                let ty = self
                    .cg_ty_of(decl.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape capture local type",
                        at: decl.span.into(),
                    })?;
                let meta = CaptureMeta {
                    id,
                    hir_ty: Some(decl.ty),
                    ty,
                    mutable: decl.mutable,
                };
                body_decl_all.insert(id, meta);
                body_decl_spans.insert(id, decl.span);
                if is_supported_capture_ty(ty) {
                    body_visible_supported.push(meta);
                }
            }
        }
        body_visible_supported.sort_by_key(|meta| meta.id.as_u32());

        for id in used_after {
            if let Some(meta) = body_decl_all.get(&id) {
                if !is_supported_capture_ty(meta.ty) {
                    let at = body_decl_spans
                        .get(&id)
                        .copied()
                        .unwrap_or(escape_site.decl.span);
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape capture local type",
                        at: at.into(),
                    });
                }
                continue;
            }
            if let Some(meta) = outer_visible_all.get(&id) {
                if !is_supported_capture_ty(meta.ty) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed-arm escape capture local type",
                        at: escape_site.decl.span.into(),
                    });
                }
                continue;
            }
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm escape capture local missing",
                at: escape_site.decl.span.into(),
            });
        }

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

        let state_ty_name = format!("scoop.runtime.MixedEscapeIndirectState__{func_name}_{seq}");
        let state_ty = if let Some(existing) = self.context.get_struct_type(&state_ty_name) {
            existing
        } else {
            let ty = self.context.opaque_struct_type(&state_ty_name);
            let header_ty = self.llvm_gc_object_header_type();
            let mut fields: Vec<BasicTypeEnum<'ctx>> =
                vec![header_ty.into(), handler_frame_ty.into()];
            for cap in &outer_visible_supported {
                fields.push(match cap.ty {
                    CgTy::Ref | CgTy::String => gc_i8_ptr_ty.into(),
                    CgTy::Bool | CgTy::Int(_) => i64_ty.into(),
                    _ => unreachable!("captures filtered by type"),
                });
            }
            for cap in &body_visible_supported {
                fields.push(match cap.ty {
                    CgTy::Ref | CgTy::String => gc_i8_ptr_ty.into(),
                    CgTy::Bool | CgTy::Int(_) => i64_ty.into(),
                    _ => unreachable!("captures filtered by type"),
                });
            }
            ty.set_body(&fields, false);
            ty
        };

        let step_name = format!("__scoop_mixed_escape_indirect_step__{func_name}_{seq}");
        let step_fn_ty = self.context.void_type().fn_type(
            &[gc_i8_ptr_ty.into(), i64_ty.into(), gc_i8_ptr_ty.into()],
            false,
        );
        let step_fn = self.module.add_function(&step_name, step_fn_ty, None);
        step_fn.set_linkage(Linkage::Internal);
        step_fn.set_gc(super::super::LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE);

        let saved_block = insert_block;
        let outer_field_base = 2u32;
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
                    kind: "mixed escape indirect step state param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_ptr_ty = cg.llvm_ptr_type(cg.gc_address_space());
            let state_ptr = cg.builder.build_pointer_cast(
                state_raw,
                state_ptr_ty,
                "mixed_escape_indirect_step_state_ptr",
            )?;

            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape indirect step resume_word param",
                    at: span.into(),
                })?
                .into_int_value();
            let resume_gc_ref = step_fn
                .get_nth_param(2)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape indirect step resume_gc_ref param",
                    at: span.into(),
                })?
                .into_pointer_value();

            for (idx, cap) in outer_visible_supported.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "mixed_escape_indirect_step_outer_gep",
                )?;
                let name = format!("mixed_escape_indirect_outer_{}", cap.id.as_u32());
                match cap.ty {
                    CgTy::Ref => {
                        let loaded = cg
                            .builder
                            .build_load(
                                gc_i8_ptr_ty,
                                field_ptr,
                                "mixed_escape_indirect_step_load_ref",
                            )?
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
                            .build_load(
                                gc_i8_ptr_ty,
                                field_ptr,
                                "mixed_escape_indirect_step_load_str",
                            )?
                            .into_pointer_value();
                        let casted = cg.builder.build_pointer_cast(
                            loaded,
                            cg.llvm_scoop_string_ptr_type(),
                            "mixed_escape_indirect_step_str",
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
                            .build_load(i64_ty, field_ptr, "mixed_escape_indirect_step_load_bool")?
                            .into_int_value();
                        let b = cg.builder.build_int_compare(
                            IntPredicate::NE,
                            loaded,
                            i64_ty.const_zero(),
                            "mixed_escape_indirect_step_bool",
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
                        let loaded = cg
                            .builder
                            .build_load(i64_ty, field_ptr, "mixed_escape_indirect_step_load_int")?
                            .into_int_value();
                        let to = cg.int_type(int_ty);
                        let v = if int_ty.bits == 64 {
                            loaded
                        } else {
                            cg.builder.build_int_truncate(
                                loaded,
                                to,
                                "mixed_escape_indirect_step_trunc_int",
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

            for (idx, cap) in body_visible_supported.iter().enumerate() {
                let field_idx = body_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "mixed_escape_indirect_step_body_gep",
                )?;
                let name = format!("mixed_escape_indirect_body_{}", cap.id.as_u32());
                match cap.ty {
                    CgTy::Ref => {
                        let loaded = cg
                            .builder
                            .build_load(
                                gc_i8_ptr_ty,
                                field_ptr,
                                "mixed_escape_indirect_step_load_ref",
                            )?
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
                            .build_load(
                                gc_i8_ptr_ty,
                                field_ptr,
                                "mixed_escape_indirect_step_load_str",
                            )?
                            .into_pointer_value();
                        let casted = cg.builder.build_pointer_cast(
                            loaded,
                            cg.llvm_scoop_string_ptr_type(),
                            "mixed_escape_indirect_step_str",
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
                            .build_load(i64_ty, field_ptr, "mixed_escape_indirect_step_load_bool")?
                            .into_int_value();
                        let b = cg.builder.build_int_compare(
                            IntPredicate::NE,
                            loaded,
                            i64_ty.const_zero(),
                            "mixed_escape_indirect_step_bool",
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
                        let loaded = cg
                            .builder
                            .build_load(i64_ty, field_ptr, "mixed_escape_indirect_step_load_int")?
                            .into_int_value();
                        let to = cg.int_type(int_ty);
                        let v = if int_ty.bits == 64 {
                            loaded
                        } else {
                            cg.builder.build_int_truncate(
                                loaded,
                                to,
                                "mixed_escape_indirect_step_trunc_int",
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

            let step_sibling_dispatch = self.build_sibling_nonresuming_dispatch_blocks(
                step_fn,
                "mixed_escape_indirect_step",
                &sibling_plan,
            );
            let step_effect_dispatch_bb = step_sibling_dispatch.effect_dispatch_bb;
            let step_effect_dispatch_nomatch_bb =
                step_sibling_dispatch.effect_dispatch_nomatch_bb;
            let step_raise_catch_bb = step_sibling_dispatch.raise_catch_bb;
            let step_custom_catch_bbs = step_sibling_dispatch.custom_catch_bbs;

            let rt_get_callee = cg.declare_runtime_callee_suspend_state_get();
            let get_call = cg.builder.build_call(
                rt_get_callee,
                &[],
                "mixed_escape_indirect_step_callee_state_get",
            )?;
            let callee_state_raw = get_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape indirect step callee_state_get return",
                    at: span.into(),
                })?
                .into_pointer_value();

            let callee_prefix_ty = cg.llvm_callee_suspend_state_prefix_type();
            let callee_state_ptr_ty = cg.llvm_ptr_type(AddressSpace::default());
            let callee_state_ptr = cg.builder.build_pointer_cast(
                callee_state_raw,
                callee_state_ptr_ty,
                "mixed_escape_indirect_step_callee_state_typed",
            )?;
            let callee_rw_ptr = cg.builder.build_struct_gep(
                callee_prefix_ty,
                callee_state_ptr,
                1,
                "mixed_escape_indirect_step_resume_word_gep",
            )?;
            let _ = cg.builder.build_store(callee_rw_ptr, resume_word)?;

            let callee_rg_ptr = cg.builder.build_struct_gep(
                callee_prefix_ty,
                callee_state_ptr,
                2,
                "mixed_escape_indirect_step_resume_gc_ref_gep",
            )?;
            let wb = cg.declare_runtime_gc_write_barrier();
            let slot_addr = cg.builder.build_pointer_cast(
                callee_rg_ptr,
                i8_ptr_ty,
                "mixed_escape_indirect_step_resume_gc_slot",
            )?;
            let _ = cg.builder.build_call(
                wb,
                &[slot_addr.into(), resume_gc_ref.into()],
                "mixed_escape_indirect_step_resume_gc_store",
            )?;

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                for (idx, custom) in custom_siblings.iter().enumerate() {
                    cg.push_effect_unwind_target(&custom.arm.op.op.fqn, step_custom_catch_bbs[idx]);
                }
                cg.push_raise_target(step_effect_dispatch_bb);
            }

            let call_result = cg
                .codegen_expr_in_expected_context(escape_site.init, Some(escape_resume_value_ty))?;
            let call_result_ptr = cg.create_entry_alloca(
                escape_site.decl.span,
                escape_site
                    .decl
                    .name
                    .as_deref()
                    .unwrap_or("mixed_escape_indirect_result"),
                escape_resume_value_ty,
            )?;
            let _stored = cg.store_local_value(
                escape_site.decl.span,
                call_result_ptr,
                escape_resume_value_ty,
                call_result,
            )?;
            cg.env.insert(
                escape_site.id,
                CgLocal {
                    hir_ty: Some(escape_site.decl.ty),
                    ty: escape_resume_value_ty,
                    ptr: call_result_ptr,
                    mutable: escape_site.decl.mutable,
                },
            );

            for stmt in handle.body.stmts.iter().skip(escape_site.stmt_idx + 1) {
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

            if step_effect_dispatch_bb.is_some() {
                cg.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    cg.pop_effect_unwind_target();
                }
            }

            if let Some(bb) = cg.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                let unpin = cg.declare_runtime_gc_unpin();
                let _ = cg.builder.build_call(
                    unpin,
                    &[state_raw.into()],
                    "mixed_escape_indirect_state_unpin",
                )?;
                cg.builder.build_return(None)?;
            }

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                let step_effect_dispatch_nomatch_bb = step_effect_dispatch_nomatch_bb
                    .expect("mixed escape indirect step dispatch_nomatch bb should exist");
                cg.builder.position_at_end(step_effect_dispatch_bb);
                let rt_read_tag = cg.declare_runtime_effect_perform_slot_read_op_tag();
                let tag_call = cg.builder.build_call(
                    rt_read_tag,
                    &[],
                    "mixed_escape_indirect_step_read_op_tag",
                )?;
                let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "mixed escape indirect step read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "mixed escape indirect step read_op_tag return type",
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
                    "mixed_escape_indirect_step_state_unpin_nomatch",
                )?;
                cg.builder.build_return(None)?;

                if let (Some(raise_arm), Some(step_raise_catch_bb)) =
                    (raise_sibling, step_raise_catch_bb)
                {
                    let binder = &raise_arm.op.binders[0];
                    cg.builder.position_at_end(step_raise_catch_bb);

                    let frame_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        1,
                        "mixed_escape_indirect_step_raise_frame_gep",
                    )?;
                    let prev_ptr = cg.builder.build_struct_gep(
                        handler_frame_ty,
                        frame_ptr,
                        0,
                        "mixed_escape_indirect_step_raise_prev_gep",
                    )?;
                    let prev_raw = cg.builder.build_load(
                        i8_ptr_ty,
                        prev_ptr,
                        "mixed_escape_indirect_step_raise_prev",
                    )?;
                    let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                    let _ = cg.builder.build_call(
                        rt_swap,
                        &[prev_raw.into()],
                        "mixed_escape_indirect_step_raise_detach",
                    )?;

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "mixed_escape_indirect_step_raise_read_slot_len_words",
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
                        "mixed_escape_indirect_step_raise_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "mixed_escape_indirect_step_raise_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "mixed_escape_indirect_step_raise_slot_len_bad_bb",
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
                        "mixed_escape_indirect_step_raise_read_slot_word0",
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
                        "mixed_escape_indirect_step_raise_read_slot_word1",
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
                        "mixed_escape_indirect_step_raise_clear",
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
                                "mixed_escape_indirect_step_raise_kind_is_int",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "mixed_escape_indirect_step_raise_kind_int_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "mixed_escape_indirect_step_raise_kind_int_bad",
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
                                "mixed_escape_indirect_step_raise_kind_is_runtime_error",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "mixed_escape_indirect_step_raise_kind_runtime_error_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "mixed_escape_indirect_step_raise_kind_runtime_error_bad",
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
                                "mixed_escape_indirect_step_runtime_error_tag_i32",
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
                                "mixed_escape_indirect_step_runtime_error_tag",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_word_zero,
                                1,
                                "mixed_escape_indirect_step_runtime_error_payload_word",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_ptr_zero,
                                2,
                                "mixed_escape_indirect_step_runtime_error_payload_ptr",
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
                            "mixed_escape_indirect_step_state_unpin_raise",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }

                for (idx, custom) in custom_siblings.iter().enumerate() {
                    let arm = custom.arm;
                    let binder = &arm.op.binders[0];
                    cg.builder.position_at_end(step_custom_catch_bbs[idx]);

                    let frame_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        1,
                        "mixed_escape_indirect_step_custom_frame_gep",
                    )?;
                    let prev_ptr = cg.builder.build_struct_gep(
                        handler_frame_ty,
                        frame_ptr,
                        0,
                        "mixed_escape_indirect_step_custom_prev_gep",
                    )?;
                    let prev_raw = cg.builder.build_load(
                        i8_ptr_ty,
                        prev_ptr,
                        "mixed_escape_indirect_step_custom_prev",
                    )?;
                    let rt_swap = cg.declare_runtime_effect_handler_stack_swap_top();
                    let _ = cg.builder.build_call(
                        rt_swap,
                        &[prev_raw.into()],
                        "mixed_escape_indirect_step_custom_detach",
                    )?;

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "mixed_escape_indirect_step_custom_read_slot_len_words",
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
                        "mixed_escape_indirect_step_custom_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "mixed_escape_indirect_step_custom_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "mixed_escape_indirect_step_custom_slot_len_bad_bb",
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
                        "mixed_escape_indirect_step_custom_read_slot_word0",
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
                        "mixed_escape_indirect_step_custom_read_slot_gc_ref",
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
                        "mixed_escape_indirect_step_custom_clear",
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
                            "mixed_escape_indirect_step_state_unpin_custom",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }
            }

            cg.env.pop_scope();
            if let Some(bb) = cg.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                let unpin = cg.declare_runtime_gc_unpin();
                let _ = cg.builder.build_call(
                    unpin,
                    &[state_raw.into()],
                    "mixed_escape_indirect_state_unpin",
                )?;
                cg.builder.build_return(None)?;
            }
        }
        self.builder.position_at_end(saved_block);

        let resume_blocks =
            self.build_mixed_escape_resume_blocks(func, "handle_mixed_escape_indirect");
        let dispatch_bb = resume_blocks.dispatch_bb;
        let state0_bb = resume_blocks.state0_bb;
        let state1_bb = resume_blocks.state1_bb;
        let arm_bb = resume_blocks.arm_bb;
        let escape_dispatch_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_indirect_dispatch");
        let escape_arm_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_indirect_arm");
        let done_bb = resume_blocks.done_bb;
        let bad_state_bb = resume_blocks.bad_state_bb;
        let finally_bb = resume_blocks.finally_bb;
        let finally_unwind_bb = resume_blocks.finally_unwind_bb;
        let sibling_dispatch = self.build_sibling_nonresuming_dispatch_blocks(
            func,
            "handle_mixed_escape_indirect",
            &sibling_plan,
        );
        let effect_dispatch_bb = sibling_dispatch.effect_dispatch_bb;
        let effect_dispatch_nomatch_bb = sibling_dispatch.effect_dispatch_nomatch_bb;
        let raise_catch_bb = sibling_dispatch.raise_catch_bb;
        let custom_catch_bbs = sibling_dispatch.custom_catch_bbs;

        let state_ptr = self.create_entry_alloca_raw(
            span,
            "handle_mixed_escape_indirect_state",
            i32_ty.into(),
        )?;
        let resume_used_ptr = self.create_entry_alloca_raw(
            span,
            "handle_mixed_escape_indirect_resume_used",
            self.context.bool_type().into(),
        )?;
        let resume_value_ptr = if resume_value_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(
                span,
                "handle_mixed_escape_indirect_resume_value",
                resume_value_ty,
            )?)
        };
        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_mixed_escape_indirect_result", out_ty)?)
        };

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
            let binder_ty = match self.cg_ty_of(binder.ty) {
                Some(CgTy::Int(int_ty)) => CgTy::Int(int_ty),
                Some(_) | None => CgTy::Int(IntTy {
                    bits: self.host.word_bit_width(),
                    signed: true,
                }),
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
            &format!("handle_mixed_escape_indirect_k_{seq}"),
            CgTy::Ref,
        )?;
        let _ = self
            .builder
            .build_store(cont_ptr, self.llvm_gc_i8_ptr_type().const_null())?;

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
            format!("__scoop_type_desc_mixed_escape_indirect_state__{func_name}_{seq}");
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
            "mixed_escape_indirect_state_desc_i8",
        )?;
        let call = self.builder.build_call(
            rt_alloc,
            &[state_desc_i8.into(), size_v.into()],
            "rt_alloc_mixed_escape_indirect_state",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "mixed escape indirect alloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(state_raw) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "mixed escape indirect alloc return type",
                at: span.into(),
            });
        };

        let pin = self.declare_runtime_gc_pin();
        let _ =
            self.builder
                .build_call(pin, &[state_raw.into()], "mixed_escape_indirect_state_pin")?;

        let state_gc_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let state_gc_ptr = self.builder.build_pointer_cast(
            state_raw,
            state_gc_ptr_ty,
            "mixed_escape_indirect_state_ptr",
        )?;

        for (idx, cap) in outer_visible_supported.iter().enumerate() {
            let field_idx = outer_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "mixed_escape_indirect_state_outer_init_gep",
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
        for (idx, cap) in body_visible_supported.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "mixed_escape_indirect_state_body_init_gep",
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

        let frame_ptr = self.builder.build_struct_gep(
            state_ty,
            state_gc_ptr,
            1,
            "mixed_escape_indirect_state_frame_gep",
        )?;
        let frame_i8 = self.builder.build_address_space_cast(
            frame_ptr,
            i8_ptr_ty,
            "mixed_escape_indirect_frame_i8",
        )?;
        let escape_tag = self.effect_op_tag(&escape_arm.op.op.fqn);
        let escape_tag_i32 = i32_ty.const_int(escape_tag as u64, false);
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), escape_tag_i32.into()],
            "mixed_escape_indirect_push",
        )?;
        let prev_ptr = self.builder.build_struct_gep(
            handler_frame_ty,
            frame_ptr,
            0,
            "mixed_escape_indirect_prev_gep",
        )?;
        let escape_outer_top = self
            .builder
            .build_load(i8_ptr_ty, prev_ptr, "mixed_escape_indirect_outer_top")?
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
            .build_load(i32_ty, state_ptr, "mixed_escape_indirect_state")?
            .into_int_value();
        let cases = [
            (i32_ty.const_int(0, false), state0_bb),
            (i32_ty.const_int(1, false), state1_bb),
        ];
        self.builder.build_switch(state, bad_state_bb, &cases)?;

        self.builder.position_at_end(bad_state_bb);
        self.emit_exit_with_code(span, 3)?;

        self.env.push_scope();

        self.builder.position_at_end(state0_bb);
        for (idx, custom) in custom_siblings.iter().enumerate() {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom_catch_bbs[idx]);
        }
        self.push_raise_target(main_raise_target);
        for stmt in handle.body.stmts.iter().take(perform_idx) {
            self.codegen_immediate_resume_stmt_unit(stmt)?;
        }
        let hir::StmtKind::Val(immediate_decl) = &handle.body.stmts[perform_idx].kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle mixed-arm immediate-resume body (expected perform binding)",
                at: handle.body.stmts[perform_idx].span.into(),
            });
        };
        let target_ptr = self.codegen_immediate_resume_site_binding(
            &perform_site,
            immediate_decl,
            ImmediateResumeArmDispatch {
                binder_slots: &immediate_binder_slots,
                resume_used_ptr,
                arm_bb,
            },
            None,
        )?;
        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }

        self.builder.position_at_end(arm_bb);
        let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
        let _ = self.builder.build_call(
            rt_swap,
            &[escape_outer_top.into()],
            "mixed_escape_indirect_detach_for_immediate_arm",
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
            .append_basic_block(arm_func, "handle_mixed_escape_indirect_resume_arm_ok");
        let resume_missing_bb = self
            .context
            .append_basic_block(arm_func, "handle_mixed_escape_indirect_resume_arm_missing");

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
            "mixed_escape_indirect_restore_after_immediate_arm",
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
            let _stored = self.store_local_value(span, target_ptr, resume_value_ty, v)?;
        }

        let mut tail_value: CgValue<'ctx> = CgValue::unit();
        for (idx, stmt) in handle.body.stmts.iter().enumerate().skip(perform_idx + 1) {
            if idx == escape_site.stmt_idx {
                for (field_idx, cap) in outer_visible_supported.iter().enumerate() {
                    let field_ptr = self.builder.build_struct_gep(
                        state_ty,
                        state_gc_ptr,
                        outer_field_base.saturating_add(field_idx as u32),
                        "mixed_escape_indirect_capture_outer_gep",
                    )?;
                    let local = self
                        .env
                        .get(cap.id)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "mixed escape indirect capture local not found",
                            at: escape_site.decl.span.into(),
                        })?;
                    match cap.ty {
                        CgTy::Ref => {
                            let llvm_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
                            let loaded = self.builder.build_load(
                                llvm_ty,
                                local.ptr,
                                "mixed_escape_indirect_capture_ref",
                            )?;
                            let BasicValueEnum::PointerValue(ptr) = loaded else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape indirect capture ref ptr",
                                    at: escape_site.decl.span.into(),
                                });
                            };
                            let casted = self.builder.build_pointer_cast(
                                ptr,
                                gc_i8_ptr_ty,
                                "mixed_escape_indirect_capture_ref_i8",
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
                                "mixed_escape_indirect_capture_str",
                            )?;
                            let BasicValueEnum::PointerValue(ptr) = loaded else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape indirect capture str ptr",
                                    at: escape_site.decl.span.into(),
                                });
                            };
                            let casted = self.builder.build_pointer_cast(
                                ptr,
                                gc_i8_ptr_ty,
                                "mixed_escape_indirect_capture_str_i8",
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
                        CgTy::Bool | CgTy::Int(_) => {
                            let llvm_ty = self.llvm_basic_type_of(span, cap.ty)?;
                            let loaded = self.builder.build_load(
                                llvm_ty,
                                local.ptr,
                                "mixed_escape_indirect_capture_word",
                            )?;
                            let loaded_v = self.cg_value_from_loaded(span, cap.ty, loaded)?;
                            let word = self.coerce_u64_word(span, loaded_v)?;
                            let _ = self.builder.build_store(field_ptr, word)?;
                        }
                        _ => unreachable!("captures filtered by type"),
                    }
                }

                for (field_idx, cap) in body_visible_supported.iter().enumerate() {
                    let field_ptr = self.builder.build_struct_gep(
                        state_ty,
                        state_gc_ptr,
                        body_field_base.saturating_add(field_idx as u32),
                        "mixed_escape_indirect_capture_body_gep",
                    )?;
                    let local = self
                        .env
                        .get(cap.id)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "mixed escape indirect capture local not found",
                            at: escape_site.decl.span.into(),
                        })?;
                    match cap.ty {
                        CgTy::Ref => {
                            let llvm_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
                            let loaded = self.builder.build_load(
                                llvm_ty,
                                local.ptr,
                                "mixed_escape_indirect_capture_ref",
                            )?;
                            let BasicValueEnum::PointerValue(ptr) = loaded else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape indirect capture ref ptr",
                                    at: escape_site.decl.span.into(),
                                });
                            };
                            let casted = self.builder.build_pointer_cast(
                                ptr,
                                gc_i8_ptr_ty,
                                "mixed_escape_indirect_capture_ref_i8",
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
                                "mixed_escape_indirect_capture_str",
                            )?;
                            let BasicValueEnum::PointerValue(ptr) = loaded else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "mixed escape indirect capture str ptr",
                                    at: escape_site.decl.span.into(),
                                });
                            };
                            let casted = self.builder.build_pointer_cast(
                                ptr,
                                gc_i8_ptr_ty,
                                "mixed_escape_indirect_capture_str_i8",
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
                        CgTy::Bool | CgTy::Int(_) => {
                            let llvm_ty = self.llvm_basic_type_of(span, cap.ty)?;
                            let loaded = self.builder.build_load(
                                llvm_ty,
                                local.ptr,
                                "mixed_escape_indirect_capture_word",
                            )?;
                            let loaded_v = self.cg_value_from_loaded(span, cap.ty, loaded)?;
                            let word = self.coerce_u64_word(span, loaded_v)?;
                            let _ = self.builder.build_store(field_ptr, word)?;
                        }
                        _ => unreachable!("captures filtered by type"),
                    }
                }

                self.pop_raise_target();
                self.push_raise_target(escape_dispatch_bb);
                self.codegen_val_decl(escape_site.decl)?;
                self.pop_raise_target();
                self.push_raise_target(main_raise_target);
                tail_value = CgValue::unit();
                continue;
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

        self.codegen_immediate_resume_finalize_body(
            handle.body.span,
            out_ty,
            tail_value,
            result_ptr,
            ImmediateResumeHandlerExit::None,
            finally_bb,
        )?;
        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }
        self.env.pop_scope();

        self.builder.position_at_end(escape_dispatch_bb);
        let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
        let tag_call = self.builder.build_call(
            rt_read_tag,
            &[],
            "mixed_escape_indirect_dispatch_read_op_tag",
        )?;
        let tag_raw =
            tag_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape indirect dispatch read_op_tag return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "mixed escape indirect dispatch read_op_tag return type",
                at: span.into(),
            });
        };
        let tag_matches = self.builder.build_int_compare(
            IntPredicate::EQ,
            slot_tag,
            escape_tag_i32,
            "mixed_escape_indirect_dispatch_tag_eq",
        )?;
        let escape_dispatch_fallback_bb = effect_dispatch_bb.unwrap_or(finally_unwind_bb);
        self.builder.build_conditional_branch(
            tag_matches,
            escape_arm_bb,
            escape_dispatch_fallback_bb,
        )?;

        self.builder.position_at_end(escape_arm_bb);
        for (slot_idx, slot) in escape_binder_slots.iter().enumerate() {
            if slot_idx != 0 {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed-arm escape binder count (indirect, only 1 supported)",
                    at: escape_arm.op.span.into(),
                });
            }
            let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
            let read_call =
                self.builder
                    .build_call(rt_read, &[], "mixed_escape_indirect_arm_read_binder")?;
            let raw = read_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape indirect arm read binder return",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(binder_u64) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape indirect arm read binder type",
                    at: span.into(),
                });
            };
            let CgTy::Int(int_ty) = slot.ty else {
                unreachable!("mixed indirect binder slots are normalized to Int");
            };
            let to = self.int_type(int_ty);
            let v = if int_ty.bits == 64 {
                binder_u64
            } else {
                self.builder.build_int_truncate(
                    binder_u64,
                    to,
                    "mixed_escape_indirect_arm_binder_trunc",
                )?
            };
            let _ = self.store_local_value(span, slot.ptr, slot.ty, CgValue::int(v, int_ty))?;
        }

        let rt_clear = self.declare_runtime_effect_clear();
        let _ = self
            .builder
            .build_call(rt_clear, &[], "mixed_escape_indirect_arm_effect_clear")?;

        let rt_cont_alloc = self.declare_runtime_continuation_alloc();
        let step_ptr = step_fn.as_global_value().as_pointer_value();
        let cont_call = self.builder.build_call(
            rt_cont_alloc,
            &[state_raw.into(), step_ptr.into()],
            "mixed_escape_indirect_cont_alloc",
        )?;
        let cont_raw =
            cont_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape indirect continuation alloc return value",
                    at: escape_site.decl.span.into(),
                })?;
        let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "mixed escape indirect continuation alloc return type",
                at: escape_site.decl.span.into(),
            });
        };

        let _ = self
            .builder
            .build_call(pin, &[k_raw.into()], "mixed_escape_indirect_k_pin")?;
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
            "mixed_escape_indirect_detach_for_escape_arm",
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

        if let Some(effect_dispatch_bb) = effect_dispatch_bb {
            let effect_dispatch_nomatch_bb = effect_dispatch_nomatch_bb
                .expect("escape + sibling non-resuming indirect dispatch_nomatch bb should exist");
            self.builder.position_at_end(effect_dispatch_bb);
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = self.builder.build_call(
                rt_read_tag,
                &[],
                "mixed_escape_indirect_effect_dispatch_read_op_tag",
            )?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape indirect effect dispatch read_op_tag return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "mixed escape indirect effect dispatch read_op_tag return type",
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
                    "mixed_escape_indirect_raise_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "mixed_escape_indirect_raise_read_slot_len_words",
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
                    "mixed_escape_indirect_raise_slot_len_ok",
                )?;
                let len_ok_bb = self
                    .context
                    .append_basic_block(func, "mixed_escape_indirect_raise_slot_len_ok_bb");
                let len_bad_bb = self
                    .context
                    .append_basic_block(func, "mixed_escape_indirect_raise_slot_len_bad_bb");
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
                    "mixed_escape_indirect_raise_read_slot_word0",
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
                    "mixed_escape_indirect_raise_read_slot_word1",
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
                        .build_call(rt_clear, &[], "mixed_escape_indirect_raise_clear")?;

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
                            "mixed_escape_indirect_raise_kind_is_int",
                        )?;
                        let ok_bb = self
                            .context
                            .append_basic_block(func, "mixed_escape_indirect_raise_kind_int_ok");
                        let bad_bb = self
                            .context
                            .append_basic_block(func, "mixed_escape_indirect_raise_kind_int_bad");
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
                            "mixed_escape_indirect_raise_kind_is_runtime_error",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "mixed_escape_indirect_raise_kind_runtime_error_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "mixed_escape_indirect_raise_kind_runtime_error_bad",
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
                            "mixed_escape_indirect_runtime_error_tag_i32",
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
                            "mixed_escape_indirect_runtime_error_tag",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_word_zero,
                            1,
                            "mixed_escape_indirect_runtime_error_payload_word",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_ptr_zero,
                            2,
                            "mixed_escape_indirect_runtime_error_payload_ptr",
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
                    "mixed_escape_indirect_custom_detach",
                )?;

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "mixed_escape_indirect_custom_read_slot_len_words",
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
                    "mixed_escape_indirect_custom_slot_len_ok",
                )?;
                let len_ok_bb = self
                    .context
                    .append_basic_block(func, "mixed_escape_indirect_custom_slot_len_ok_bb");
                let len_bad_bb = self
                    .context
                    .append_basic_block(func, "mixed_escape_indirect_custom_slot_len_bad_bb");
                self.builder
                    .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                self.builder.position_at_end(len_bad_bb);
                self.emit_exit_with_code(span, 3)?;

                self.builder.position_at_end(len_ok_bb);

                let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
                let value_call = self.builder.build_call(
                    rt_read,
                    &[],
                    "mixed_escape_indirect_custom_read_slot_word0",
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
                    "mixed_escape_indirect_custom_read_slot_gc_ref",
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
                        .build_call(rt_clear, &[], "mixed_escape_indirect_custom_clear")?;

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

        self.builder.position_at_end(finally_unwind_bb);
        let _ = self.builder.build_call(
            rt_swap,
            &[escape_outer_top.into()],
            "mixed_escape_indirect_finally_unwind_detach",
        )?;
        let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
        let k_loaded = self
            .builder
            .build_load(
                llvm_ref_ty,
                cont_ptr,
                "mixed_escape_indirect_k_maybe_unpin_load",
            )?
            .into_pointer_value();
        let k_is_null = self
            .builder
            .build_is_null(k_loaded, "mixed_escape_indirect_k_is_null")?;
        let finally_unwind_state_unpin_bb = self.context.append_basic_block(
            func,
            "handle_mixed_escape_indirect_finally_unwind_state_unpin",
        );
        let finally_unwind_state_keep_bb = self.context.append_basic_block(
            func,
            "handle_mixed_escape_indirect_finally_unwind_state_keep",
        );
        let finally_unwind_state_merge_bb = self.context.append_basic_block(
            func,
            "handle_mixed_escape_indirect_finally_unwind_state_merge",
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
            "mixed_escape_indirect_state_unpin_finally_unwind",
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
            "mixed_escape_indirect_finally_detach",
        )?;
        let k_loaded = self
            .builder
            .build_load(
                llvm_ref_ty,
                cont_ptr,
                "mixed_escape_indirect_k_maybe_unpin_done_load",
            )?
            .into_pointer_value();
        let k_is_null = self
            .builder
            .build_is_null(k_loaded, "mixed_escape_indirect_k_done_is_null")?;
        let finally_state_unpin_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_indirect_finally_state_unpin");
        let finally_state_keep_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_indirect_finally_state_keep");
        let finally_state_merge_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_indirect_finally_state_merge");
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
            "mixed_escape_indirect_state_unpin_finally",
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
            .build_load(llvm_ref_ty, cont_ptr, "mixed_escape_indirect_k_unpin_load")?
            .into_pointer_value();
        let k_is_null = self
            .builder
            .build_is_null(k_loaded, "mixed_escape_indirect_k_unpin_is_null")?;
        let k_unpin_skip_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_indirect_k_unpin_skip");
        let k_unpin_do_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_indirect_k_unpin_do");
        let k_unpin_merge_bb = self
            .context
            .append_basic_block(func, "handle_mixed_escape_indirect_k_unpin_merge");
        self.builder
            .build_conditional_branch(k_is_null, k_unpin_skip_bb, k_unpin_do_bb)?;
        self.builder.position_at_end(k_unpin_do_bb);
        let unpin = self.declare_runtime_gc_unpin();
        let _ =
            self.builder
                .build_call(unpin, &[k_loaded.into()], "mixed_escape_indirect_k_unpin")?;
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
                        .build_load(llvm_ty, ptr, "handle_mixed_escape_indirect_result")?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }

}
