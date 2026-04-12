impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn build_multiple_escape_binder_slots<'hir>(
        &mut self,
        arm: &'hir hir::HandleArm,
        name_prefix: &str,
    ) -> Result<Vec<ImmediateResumeBinderSlot<'ctx>>, LlvmEmitError> {
        let mut slots = Vec::with_capacity(arm.op.binders.len());
        for (idx, binder) in arm.op.binders.iter().enumerate() {
            let binder_ty =
                self.cg_ty_of(binder.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multiple escape-continuation arms binder type",
                        at: binder.span.into(),
                    })?;
            let slot_name = format!("{name_prefix}_{idx}_{}", binder.name);
            let ptr = self.create_entry_alloca(binder.span, &slot_name, binder_ty)?;
            slots.push(ImmediateResumeBinderSlot {
                id: binder.id,
                hir_ty: binder.ty,
                ty: binder_ty,
                ptr,
            });
        }
        Ok(slots)
    }

    fn codegen_handle_expr_multiple_escape_top_level_direct_pure<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        escape_arms: &[(&'hir hir::HandleArm, hir::SymbolId)],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        #[derive(Debug)]
        struct EscapeSitePlan<'hir> {
            site: MixedEscapeDirectSite<'hir>,
            arm: &'hir hir::HandleArm,
            continuation_symbol: hir::SymbolId,
            resume_value_ty: CgTy,
        }

        if handle.finally.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multiple escape-continuation arms (finally not yet supported)",
                at: span.into(),
            });
        }

        let contains_matching_direct_perform = |expr: &hir::Expr| {
            escape_arms.iter().any(|(arm, _)| {
                self.immediate_resume_expr_contains_matching_direct_perform(expr, &arm.op.op.fqn)
            })
        };
        let stmt_contains_matching_direct_perform = |stmt: &hir::Stmt| {
            escape_arms.iter().any(|(arm, _)| {
                self.immediate_resume_stmt_contains_matching_direct_perform(stmt, &arm.op.op.fqn)
            })
        };

        let mut seen_escape_arm = vec![false; escape_arms.len()];
        let mut scanned_sites: Vec<EscapeSitePlan<'hir>> = Vec::new();
        for (top_level_stmt_idx, stmt) in handle.body.stmts.iter().enumerate() {
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    let Some(init) = decl.init.as_ref() else {
                        continue;
                    };
                    if let hir::ExprKind::Perform { op, args } = &init.kind
                        && let Some((arm_idx, (arm, continuation_symbol))) = escape_arms
                            .iter()
                            .enumerate()
                            .find(|(_, (arm, _))| arm.op.op.fqn == op.fqn)
                    {
                        if seen_escape_arm[arm_idx] {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multiple escape-continuation arms (multiple direct perform points for same op not yet supported)",
                                at: decl.span.into(),
                            });
                        }
                        let Some(id) = decl.id else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle mixed-arm escape continuation perform binding id",
                                at: decl.span.into(),
                            });
                        };
                        if arm.op.binders.len() != args.len() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multiple escape-continuation arms binder arity mismatch",
                                at: arm.op.span.into(),
                            });
                        }
                        let resume_value_ty =
                            self.cg_ty_of(decl.ty)
                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle multiple escape-continuation arms perform value type",
                                    at: decl.span.into(),
                                })?;
                        scanned_sites.push(EscapeSitePlan {
                            site: MixedEscapeDirectSite {
                                top_level_stmt_idx,
                                decl,
                                args: args.as_slice(),
                                id,
                                resume_path: Vec::new(),
                            },
                            arm,
                            continuation_symbol: *continuation_symbol,
                            resume_value_ty,
                        });
                        seen_escape_arm[arm_idx] = true;
                        continue;
                    }
                    if contains_matching_direct_perform(init) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multiple escape-continuation arms (only top-level val-bound direct perform supported)",
                            at: init.span.into(),
                        });
                    }
                }
                hir::StmtKind::Assign { lhs, rhs, .. } => {
                    if contains_matching_direct_perform(lhs)
                        || contains_matching_direct_perform(rhs)
                    {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multiple escape-continuation arms (only top-level val-bound direct perform supported)",
                            at: stmt.span.into(),
                        });
                    }
                }
                hir::StmtKind::Expr(expr) => {
                    if let hir::ExprKind::Perform { op, .. } = &expr.kind
                        && escape_arms
                            .iter()
                            .any(|(arm, _)| arm.op.op.fqn == op.fqn)
                    {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multiple escape-continuation arms (perform must be bound to val)",
                            at: expr.span.into(),
                        });
                    }
                    if contains_matching_direct_perform(expr) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multiple escape-continuation arms (only top-level val-bound direct perform supported)",
                            at: expr.span.into(),
                        });
                    }
                }
                hir::StmtKind::While { cond, body } => {
                    if contains_matching_direct_perform(cond)
                        || body
                            .stmts
                            .iter()
                            .any(stmt_contains_matching_direct_perform)
                    {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multiple escape-continuation arms (only top-level val-bound direct perform supported)",
                            at: stmt.span.into(),
                        });
                    }
                }
                hir::StmtKind::Return { value } => {
                    if value
                        .as_ref()
                        .is_some_and(contains_matching_direct_perform)
                    {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multiple escape-continuation arms (only top-level val-bound direct perform supported)",
                            at: stmt.span.into(),
                        });
                    }
                }
                hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement before multiple escape perform",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        let indirect_sites = self.scan_mixed_escape_indirect_sites(handle)?;
        if let Some(first_indirect) = indirect_sites.first() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multiple escape-continuation arms (indirect call site not yet supported)",
                at: first_indirect.decl.span.into(),
            });
        }

        if scanned_sites.is_empty() {
            let body_v = self.codegen_block_value(&handle.body)?;
            return match out_ty {
                CgTy::Unit => Ok(CgValue::unit()),
                CgTy::Never => Ok(CgValue::never()),
                _ => Ok(self.coerce_value(handle.body.span, body_v, out_ty)?),
            };
        }

        scanned_sites.sort_by_key(|plan| (plan.site.top_level_stmt_idx, plan.site.decl.span.start));
        let site_pc_by_stmt_idx: HashMap<usize, usize> = scanned_sites
            .iter()
            .enumerate()
            .map(|(pc, plan)| (plan.site.top_level_stmt_idx, pc))
            .collect();

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
        for plan in &scanned_sites {
            let Some(&site_order) = body_decl_order.get(&plan.site.id) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multiple escape-continuation arms perform binding id",
                    at: plan.site.decl.span.into(),
                });
            };
            let mut used_after: HashSet<hir::SymbolId> = HashSet::new();
            Self::collect_mixed_escape_used_after_site(
                &plan.site,
                &handle.body.stmts,
                &mut used_after,
            );
            for id in used_after {
                if let Some(meta) = body_decl_all.get(&id) {
                    let at = body_decl_spans
                        .get(&id)
                        .copied()
                        .unwrap_or(plan.site.decl.span);
                    if self.escape_capture_storage_kind(at, meta.ty)?.is_none() {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multiple escape-continuation arms capture local type",
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
                        .escape_capture_storage_kind(plan.site.decl.span, meta.ty)?
                        .is_none()
                    {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multiple escape-continuation arms capture local type",
                            at: plan.site.decl.span.into(),
                        });
                    }
                    continue;
                }
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multiple escape-continuation arms capture local missing",
                    at: plan.site.decl.span.into(),
                });
            }
        }

        let mut body_visible_supported: Vec<EscapeCaptureMeta> = Vec::new();
        for &id in &body_lift_ids {
            let Some(meta) = body_decl_all.get(&id).copied() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multiple escape-continuation arms capture local missing",
                    at: span.into(),
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

        let func_name = func.get_name().to_str().unwrap_or("anonymous").to_string();
        let func_name = sanitize_llvm_ident(&func_name);
        let seq = self.escape_continuation_seq;
        self.escape_continuation_seq = self.escape_continuation_seq.saturating_add(1);

        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();

        let state_ty_name =
            format!("scoop.runtime.MultiEscapePureDirectState__{func_name}_{seq}");
        let state_ty = if let Some(existing) = self.context.get_struct_type(&state_ty_name) {
            existing
        } else {
            let ty = self.context.opaque_struct_type(&state_ty_name);
            let header_ty = self.llvm_gc_object_header_type();
            let mut fields: Vec<BasicTypeEnum<'ctx>> = vec![header_ty.into(), i32_ty.into()];
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
        let outer_field_base = 2u32;
        let body_field_base = outer_field_base.saturating_add(outer_visible_supported.len() as u32);
        let pc_field_idx = 1u32;

        let step_name = format!("__scoop_multi_escape_pure_direct_step__{func_name}_{seq}");
        let step_fn_ty = self.context.void_type().fn_type(
            &[gc_i8_ptr_ty.into(), i64_ty.into(), gc_i8_ptr_ty.into()],
            false,
        );
        let step_fn = self.module.add_function(&step_name, step_fn_ty, None);
        step_fn.set_linkage(Linkage::Internal);
        step_fn.set_gc(super::super::LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE);

        let saved_block = insert_block;
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
                    kind: "multi escape pure direct step state param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_ptr_ty = cg.llvm_ptr_type(cg.gc_address_space());
            let state_ptr = cg.builder.build_pointer_cast(
                state_raw,
                state_ptr_ty,
                "multi_escape_pure_direct_step_state_ptr",
            )?;
            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape pure direct step resume_word param",
                    at: span.into(),
                })?
                .into_int_value();
            let resume_gc_ref = step_fn
                .get_nth_param(2)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape pure direct step resume_gc_ref param",
                    at: span.into(),
                })?
                .into_pointer_value();

            for (idx, cap) in outer_visible_supported.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "multi_escape_pure_direct_step_outer_gep",
                )?;
                let name = format!("multi_escape_pure_direct_outer_{}", cap.id.as_u32());
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
                    "multi_escape_pure_direct_step_body_gep",
                )?;
                let name = format!("multi_escape_pure_direct_body_{}", cap.id.as_u32());
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

            let step_cont_ptr =
                cg.create_entry_alloca(span, "multi_escape_pure_direct_step_k", CgTy::Ref)?;
            let dispatch_bb = self
                .context
                .append_basic_block(step_fn, "multi_escape_pure_direct_step_dispatch");
            let bad_state_bb = self
                .context
                .append_basic_block(step_fn, "multi_escape_pure_direct_step_bad_pc");
            let mut state_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            let mut step_arm_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            let mut step_binder_slots_by_site: Vec<Vec<ImmediateResumeBinderSlot<'ctx>>> =
                Vec::new();
            for (site_idx, plan) in scanned_sites.iter().enumerate() {
                state_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_escape_pure_direct_step_state_{site_idx}"),
                ));
                step_arm_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_escape_pure_direct_step_arm_{site_idx}"),
                ));
                let prefix = format!("multi_escape_pure_direct_step_site_{site_idx}");
                step_binder_slots_by_site
                    .push(cg.build_multiple_escape_binder_slots(plan.arm, &prefix)?);
            }

            cg.builder.build_unconditional_branch(dispatch_bb)?;

            cg.builder.position_at_end(dispatch_bb);
            let state_pc_ptr = cg.builder.build_struct_gep(
                state_ty,
                state_ptr,
                pc_field_idx,
                "multi_escape_pure_direct_step_pc_gep",
            )?;
            let pc = cg
                .builder
                .build_load(i32_ty, state_pc_ptr, "multi_escape_pure_direct_step_pc")?
                .into_int_value();
            let cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> = state_bbs
                .iter()
                .enumerate()
                .map(|(site_idx, bb)| (i32_ty.const_int(site_idx as u64, false), *bb))
                .collect();
            cg.builder.build_switch(pc, bad_state_bb, &cases)?;

            cg.builder.position_at_end(bad_state_bb);
            cg.emit_exit_with_code(span, 3)?;

            for (site_idx, state_bb) in state_bbs.iter().enumerate() {
                let plan = &scanned_sites[site_idx];
                cg.builder.position_at_end(*state_bb);

                let target_ptr = if let Some(local) = cg.env.get(plan.site.id) {
                    if local.ty != plan.resume_value_ty {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multiple escape-continuation arms perform value type mismatch",
                            at: plan.site.decl.span.into(),
                        });
                    }
                    local.ptr
                } else {
                    let name = plan.site.decl.name.as_deref().unwrap_or("resume_value");
                    let ptr = cg.create_entry_alloca(plan.site.decl.span, name, plan.resume_value_ty)?;
                    cg.env.insert(
                        plan.site.id,
                        CgLocal {
                            hir_ty: Some(plan.site.decl.ty),
                            ty: plan.resume_value_ty,
                            ptr,
                            mutable: plan.site.decl.mutable,
                        },
                    );
                    ptr
                };
                let resume_value = cg.decode_abi_payload_transport(
                    plan.site.decl.span,
                    resume_word,
                    resume_gc_ref,
                    plan.resume_value_ty,
                )?;
                let _ = cg.store_local_value(
                    plan.site.decl.span,
                    target_ptr,
                    plan.resume_value_ty,
                    resume_value,
                )?;

                let mut terminated = false;
                for (stmt_idx, stmt) in handle.body.stmts.iter().enumerate() {
                    if stmt_idx <= plan.site.top_level_stmt_idx {
                        continue;
                    }
                    if let Some(&next_site_idx) = site_pc_by_stmt_idx.get(&stmt_idx) {
                        let next_plan = &scanned_sites[next_site_idx];
                        cg.capture_escape_state_with_pc(
                            next_plan.site.decl.span,
                            state_ty,
                            state_ptr,
                            &outer_visible_supported,
                            outer_field_base,
                            &body_visible_supported,
                            body_field_base,
                            pc_field_idx,
                            next_site_idx,
                        )?;
                        for (slot, arg) in step_binder_slots_by_site[next_site_idx]
                            .iter()
                            .zip(next_plan.site.args.iter())
                        {
                            let hir::CallArg::Positional(expr) = arg else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle mixed-arm escape named perform arg",
                                    at: span.into(),
                                });
                            };
                            let value =
                                cg.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                            let _ = cg.store_local_value(expr.span, slot.ptr, slot.ty, value)?;
                        }
                        let step_ptr = step_fn.as_global_value().as_pointer_value();
                        let cont_call = cg.builder.build_call(
                            cg.declare_runtime_continuation_alloc(),
                            &[state_raw.into(), step_ptr.into()],
                            "multi_escape_pure_direct_step_cont_alloc",
                        )?;
                        let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "multi escape pure direct step continuation alloc return value",
                                at: next_plan.site.decl.span.into(),
                            },
                        )?;
                        let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "multi escape pure direct step continuation alloc return type",
                                at: next_plan.site.decl.span.into(),
                            });
                        };
                        let pin = cg.declare_runtime_gc_pin();
                        let _ = cg.builder.build_call(
                            pin,
                            &[k_raw.into()],
                            "multi_escape_pure_direct_step_k_pin",
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
                        cg.builder.build_unconditional_branch(step_arm_bbs[next_site_idx])?;
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
                                kind: "`return` inside multiple escape continuation step",
                                at: stmt.span.into(),
                            });
                        }
                        hir::StmtKind::Break { .. }
                        | hir::StmtKind::Continue { .. }
                        | hir::StmtKind::Todo(_) => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "statement inside multiple escape continuation step",
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
                        "multi_escape_pure_direct_step_state_unpin",
                    )?;
                    cg.builder.build_return(None)?;
                }
            }

            for (site_idx, arm_bb) in step_arm_bbs.iter().enumerate() {
                let plan = &scanned_sites[site_idx];
                cg.builder.position_at_end(*arm_bb);
                cg.env.push_scope();
                for slot in &step_binder_slots_by_site[site_idx] {
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
                    plan.continuation_symbol,
                    CgLocal {
                        hir_ty: None,
                        ty: CgTy::Ref,
                        ptr: step_cont_ptr,
                        mutable: false,
                    },
                );
                let arm_v =
                    cg.codegen_expr_in_expected_context(&plan.arm.body, Some(out_ty))?;
                if out_ty != CgTy::Unit && out_ty != CgTy::Never {
                    let _ = cg.coerce_value(plan.arm.body.span, arm_v, out_ty)?;
                }
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
                            "multi_escape_pure_direct_step_k_unpin_load",
                        )?
                        .into_pointer_value();
                    let unpin = cg.declare_runtime_gc_unpin();
                    let _ = cg.builder.build_call(
                        unpin,
                        &[k_loaded.into()],
                        "multi_escape_pure_direct_step_k_unpin",
                    )?;
                    cg.builder.build_return(None)?;
                }
            }

            cg.env.pop_scope();
        }
        self.builder.position_at_end(saved_block);

        let body_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_pure_direct_body");
        let done_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_pure_direct_done");
        let result_ptr = if out_ty == CgTy::Unit || out_ty == CgTy::Never {
            None
        } else {
            Some(self.create_entry_alloca(
                span,
                "handle_multi_escape_pure_direct_result",
                out_ty,
            )?)
        };
        let cont_ptr =
            self.create_entry_alloca(span, "handle_multi_escape_pure_direct_k", CgTy::Ref)?;
        let mut initial_binder_slots_by_site: Vec<Vec<ImmediateResumeBinderSlot<'ctx>>> =
            Vec::new();
        let mut arm_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        for (site_idx, plan) in scanned_sites.iter().enumerate() {
            let prefix = format!("multi_escape_pure_direct_site_{site_idx}");
            initial_binder_slots_by_site
                .push(self.build_multiple_escape_binder_slots(plan.arm, &prefix)?);
            arm_bbs.push(self.context.append_basic_block(
                func,
                &format!("handle_multi_escape_pure_direct_arm_{site_idx}"),
            ));
        }

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
            format!("__scoop_type_desc_multi_escape_pure_direct_state__{func_name}_{seq}");
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
            "multi_escape_pure_direct_state_desc_i8",
        )?;
        let alloc_call = self.builder.build_call(
            rt_alloc,
            &[state_desc_i8.into(), size_v.into()],
            "rt_alloc_multi_escape_pure_direct_state",
        )?;
        let alloc_raw = alloc_call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape pure direct alloc return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::PointerValue(state_raw) = alloc_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi escape pure direct alloc return type",
                at: span.into(),
            });
        };
        let pin = self.declare_runtime_gc_pin();
        let _ = self.builder.build_call(
            pin,
            &[state_raw.into()],
            "multi_escape_pure_direct_state_pin",
        )?;

        let state_gc_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let state_gc_ptr = self.builder.build_pointer_cast(
            state_raw,
            state_gc_ptr_ty,
            "multi_escape_pure_direct_state_ptr",
        )?;
        let pc_ptr = self.builder.build_struct_gep(
            state_ty,
            state_gc_ptr,
            pc_field_idx,
            "multi_escape_pure_direct_state_pc_gep",
        )?;
        let _ = self
            .builder
            .build_store(pc_ptr, i32_ty.const_zero())?;
        for (idx, cap) in outer_visible_supported.iter().enumerate() {
            let field_idx = outer_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_escape_pure_direct_state_outer_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }
        for (idx, cap) in body_visible_supported.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_escape_pure_direct_state_body_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }

        self.builder.build_unconditional_branch(body_bb)?;

        self.builder.position_at_end(body_bb);
        self.env.push_scope();
        for (stmt_idx, stmt) in handle.body.stmts.iter().enumerate() {
            if let Some(&site_idx) = site_pc_by_stmt_idx.get(&stmt_idx) {
                let plan = &scanned_sites[site_idx];
                self.capture_escape_state_with_pc(
                    plan.site.decl.span,
                    state_ty,
                    state_gc_ptr,
                    &outer_visible_supported,
                    outer_field_base,
                    &body_visible_supported,
                    body_field_base,
                    pc_field_idx,
                    site_idx,
                )?;
                for (slot, arg) in initial_binder_slots_by_site[site_idx]
                    .iter()
                    .zip(plan.site.args.iter())
                {
                    let hir::CallArg::Positional(expr) = arg else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle mixed-arm escape named perform arg",
                            at: span.into(),
                        });
                    };
                    let value = self.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
                    let _ = self.store_local_value(expr.span, slot.ptr, slot.ty, value)?;
                }
                let step_ptr = step_fn.as_global_value().as_pointer_value();
                let cont_call = self.builder.build_call(
                    self.declare_runtime_continuation_alloc(),
                    &[state_raw.into(), step_ptr.into()],
                    "multi_escape_pure_direct_cont_alloc",
                )?;
                let cont_raw = cont_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape pure direct continuation alloc return value",
                        at: plan.site.decl.span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi escape pure direct continuation alloc return type",
                        at: plan.site.decl.span.into(),
                    });
                };
                let _ = self.builder.build_call(
                    pin,
                    &[k_raw.into()],
                    "multi_escape_pure_direct_k_pin",
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
                self.builder.build_unconditional_branch(arm_bbs[site_idx])?;
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
                        kind: "statement before multiple escape perform",
                        at: stmt.span.into(),
                    });
                }
            }
        }
        self.env.pop_scope();

        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multiple escape-continuation arms (missing direct perform site)",
                at: span.into(),
            });
        }

        for (site_idx, arm_bb) in arm_bbs.iter().enumerate() {
            let plan = &scanned_sites[site_idx];
            self.builder.position_at_end(*arm_bb);
            self.env.push_scope();
            for slot in &initial_binder_slots_by_site[site_idx] {
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
                plan.continuation_symbol,
                CgLocal {
                    hir_ty: None,
                    ty: CgTy::Ref,
                    ptr: cont_ptr,
                    mutable: false,
                },
            );
            let arm_v = self.codegen_expr_in_expected_context(&plan.arm.body, Some(out_ty))?;
            let arm_v = if out_ty == CgTy::Unit {
                CgValue::unit()
            } else if out_ty == CgTy::Never {
                CgValue::never()
            } else {
                self.coerce_value(plan.arm.body.span, arm_v, out_ty)?
            };
            self.env.pop_scope();

            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                if let Some(ptr) = result_ptr {
                    let _ = self.store_local_value(plan.arm.body.span, ptr, out_ty, arm_v)?;
                }
                self.builder.build_unconditional_branch(done_bb)?;
            }
        }

        self.builder.position_at_end(done_bb);
        let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
        let k_loaded = self
            .builder
            .build_load(
                llvm_ref_ty,
                cont_ptr,
                "multi_escape_pure_direct_k_unpin_load",
            )?
            .into_pointer_value();
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self.builder.build_call(
            unpin,
            &[k_loaded.into()],
            "multi_escape_pure_direct_k_unpin",
        )?;

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
                        kind: "handle multiple escape-continuation arms result slot",
                        at: span.into(),
                    });
                };
                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self
                    .builder
                    .build_load(
                        llvm_ty,
                        ptr,
                        "handle_multiple_escape_continuation_arms_result",
                    )?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }
}
