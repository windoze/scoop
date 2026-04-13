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

    fn codegen_handle_expr_multiple_escape_top_level_direct<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        state_machine_plan: &HandleStateMachinePlan,
        escape_arms: &[(&'hir hir::HandleArm, hir::SymbolId)],
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        #[derive(Debug)]
        struct EscapeSitePlan<'hir> {
            site: MixedEscapeDirectSite<'hir>,
            arm: &'hir hir::HandleArm,
            continuation_symbol: hir::SymbolId,
            resume_value_ty: CgTy,
        }

        let sibling_plan = self.collect_sibling_nonresuming_plan(sibling_nonresuming_arms)?;
        let raise_sibling = sibling_plan.raise_arm;
        let custom_siblings = sibling_plan.custom_arms.clone();
        let has_sibling_nonresuming = sibling_plan.has_any();
        let has_finally = handle.finally.is_some();
        let outer_raise_target = self.current_raise_target();

        if has_finally && has_sibling_nonresuming {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multiple escape-continuation arms with sibling non-resuming and finally not yet supported",
                at: span.into(),
            });
        }

        let escape_arm_plans = escape_arms
            .iter()
            .map(|(arm, continuation_symbol)| {
                let Some(arm_id) = handle
                    .arms
                    .iter()
                    .position(|candidate| std::ptr::eq(candidate, *arm))
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle arm dispatch (multiple escape-continuation arm id)",
                        at: arm.span.into(),
                    });
                };
                Ok((*arm, arm_id as ArmPlanId, *continuation_symbol))
            })
            .collect::<Result<Vec<_>, LlvmEmitError>>()?;
        let plan_escape_arms = escape_arm_plans
            .iter()
            .map(|(arm, arm_id, _)| (*arm, *arm_id))
            .collect::<Vec<_>>();
        let ResolvedPlanMixedEscapeDirectSites {
            direct_sites: resolved_direct_sites,
            mut capture_ids,
        } = Self::resolve_mixed_escape_direct_sites_from_plan(
            handle,
            state_machine_plan,
            plan_escape_arms.as_slice(),
        )?;
        for (_, arm_id, _) in &escape_arm_plans {
            capture_ids.extend(state_machine_plan.arm_capture_locals(*arm_id).iter().copied());
        }

        let mut seen_escape_arm: HashSet<ArmPlanId> = HashSet::new();
        let mut scanned_sites: Vec<EscapeSitePlan<'hir>> =
            Vec::with_capacity(resolved_direct_sites.len());
        for resolved in resolved_direct_sites {
            if !seen_escape_arm.insert(resolved.arm_id) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multiple escape-continuation arms (multiple direct perform points for same op not yet supported)",
                    at: resolved.site.decl.span.into(),
                });
            }
            if !resolved.site.resume_path.is_empty() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multiple escape-continuation arms (only top-level val-bound direct perform supported)",
                    at: resolved.site.decl.span.into(),
                });
            }
            let Some((arm, _, continuation_symbol)) = escape_arm_plans
                .iter()
                .find(|(_, arm_id, _)| *arm_id == resolved.arm_id)
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle arm dispatch (multiple escape-continuation arm id)",
                    at: resolved.site.decl.span.into(),
                });
            };
            if arm.op.binders.len() != resolved.site.args.len() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multiple escape-continuation arms binder arity mismatch",
                    at: arm.op.span.into(),
                });
            }
            let resume_value_ty =
                self.cg_ty_of(resolved.site.decl.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multiple escape-continuation arms perform value type",
                        at: resolved.site.decl.span.into(),
                    })?;
            scanned_sites.push(EscapeSitePlan {
                site: resolved.site,
                arm,
                continuation_symbol: *continuation_symbol,
                resume_value_ty,
            });
        }

        let ResolvedPlanMixedEscapeIndirectSites { indirect_sites, .. } =
            Self::resolve_mixed_escape_indirect_sites_from_plan(handle, state_machine_plan)?;
        if let Some(first_indirect) = indirect_sites.first() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multiple escape-continuation arms (indirect call site not yet supported)",
                at: first_indirect.decl.span.into(),
            });
        }

        if scanned_sites.is_empty() {
            if has_sibling_nonresuming {
                return self.codegen_handle_expr_nonresuming_multi_arm(
                    span,
                    handle,
                    sibling_nonresuming_arms,
                    out_ty,
                );
            }
            if has_finally {
                return self.codegen_handle_expr_no_perform(span, handle, out_ty);
            }
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

        let (outer_visible_supported, body_visible_supported) =
            self.collect_escape_capture_metas_from_plan(
                span,
                handle,
                &capture_ids,
                "handle multiple escape-continuation arms capture local type",
                "handle multiple escape-continuation arms capture local missing",
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
            let step_sibling_dispatch = cg.build_sibling_nonresuming_dispatch_blocks(
                step_fn,
                "multi_escape_pure_direct_step",
                &sibling_plan,
            );
            let step_effect_dispatch_bb = step_sibling_dispatch.effect_dispatch_bb;
            let step_effect_dispatch_nomatch_bb =
                step_sibling_dispatch.effect_dispatch_nomatch_bb;
            let step_raise_catch_bb = step_sibling_dispatch.raise_catch_bb;
            let step_custom_catch_bbs = step_sibling_dispatch.custom_catch_bbs;
            let dispatch_bb = self
                .context
                .append_basic_block(step_fn, "multi_escape_pure_direct_step_dispatch");
            let bad_state_bb = self
                .context
                .append_basic_block(step_fn, "multi_escape_pure_direct_step_bad_pc");
            let mut state_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            let mut step_arm_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            let mut step_arm_unwind_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
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
                step_arm_unwind_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_escape_pure_direct_step_arm_unwind_{site_idx}"),
                ));
                let prefix = format!("multi_escape_pure_direct_step_site_{site_idx}");
                step_binder_slots_by_site
                    .push(cg.build_multiple_escape_binder_slots(plan.arm, &prefix)?);
            }

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                for (idx, custom) in custom_siblings.iter().enumerate() {
                    cg.push_effect_unwind_target(
                        &custom.arm.op.op.fqn,
                        step_custom_catch_bbs[idx],
                    );
                }
                cg.push_raise_target(step_effect_dispatch_bb);
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

            if step_effect_dispatch_bb.is_some() {
                cg.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    cg.pop_effect_unwind_target();
                }
            }

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                let step_effect_dispatch_nomatch_bb = step_effect_dispatch_nomatch_bb
                    .expect("multiple escape step dispatch_nomatch bb should exist");
                cg.builder.position_at_end(step_effect_dispatch_bb);
                let rt_read_tag = cg.declare_runtime_effect_perform_slot_read_op_tag();
                let tag_call = cg.builder.build_call(
                    rt_read_tag,
                    &[],
                    "multi_escape_pure_direct_step_read_op_tag",
                )?;
                let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "multiple escape step read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multiple escape step read_op_tag return type",
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
                    "multi_escape_pure_direct_step_state_unpin_nomatch",
                )?;
                cg.builder.build_return(None)?;

                if let (Some(raise_arm), Some(step_raise_catch_bb)) =
                    (raise_sibling, step_raise_catch_bb)
                {
                    let binder = &raise_arm.op.binders[0];
                    cg.builder.position_at_end(step_raise_catch_bb);

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "multi_escape_pure_direct_step_raise_read_slot_len_words",
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
                        "multi_escape_pure_direct_step_raise_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_pure_direct_step_raise_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_pure_direct_step_raise_slot_len_bad_bb",
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
                        "multi_escape_pure_direct_step_raise_read_slot_word0",
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
                        "multi_escape_pure_direct_step_raise_read_slot_word1",
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
                        "multi_escape_pure_direct_step_raise_clear",
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
                                "multi_escape_pure_direct_step_raise_kind_is_int",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_pure_direct_step_raise_kind_int_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_pure_direct_step_raise_kind_int_bad",
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
                                "multi_escape_pure_direct_step_raise_kind_is_runtime_error",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_pure_direct_step_raise_kind_runtime_error_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_escape_pure_direct_step_raise_kind_runtime_error_bad",
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
                                "multi_escape_pure_direct_step_runtime_error_tag_i32",
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
                                "multi_escape_pure_direct_step_runtime_error_tag",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_word_zero,
                                1,
                                "multi_escape_pure_direct_step_runtime_error_payload_word",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_ptr_zero,
                                2,
                                "multi_escape_pure_direct_step_runtime_error_payload_ptr",
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
                    if out_ty != CgTy::Unit && out_ty != CgTy::Never {
                        let _ = cg.coerce_value(raise_arm.body.span, arm_v, out_ty)?;
                    }
                    cg.env.pop_scope();

                    if let Some(bb) = cg.builder.get_insert_block()
                        && bb.get_terminator().is_none()
                    {
                        let unpin = cg.declare_runtime_gc_unpin();
                        let _ = cg.builder.build_call(
                            unpin,
                            &[state_raw.into()],
                            "multi_escape_pure_direct_step_state_unpin_raise",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }

                for (idx, custom) in custom_siblings.iter().enumerate() {
                    let arm = custom.arm;
                    let binder = &arm.op.binders[0];
                    cg.builder.position_at_end(step_custom_catch_bbs[idx]);

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "multi_escape_pure_direct_step_custom_read_slot_len_words",
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
                        "multi_escape_pure_direct_step_custom_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_pure_direct_step_custom_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_escape_pure_direct_step_custom_slot_len_bad_bb",
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
                        "multi_escape_pure_direct_step_custom_read_slot_word0",
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
                        "multi_escape_pure_direct_step_custom_read_slot_gc_ref",
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
                        "multi_escape_pure_direct_step_custom_clear",
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
                    if out_ty != CgTy::Unit && out_ty != CgTy::Never {
                        let _ = cg.coerce_value(arm.body.span, arm_v, out_ty)?;
                    }
                    cg.env.pop_scope();

                    if let Some(bb) = cg.builder.get_insert_block()
                        && bb.get_terminator().is_none()
                    {
                        let unpin = cg.declare_runtime_gc_unpin();
                        let _ = cg.builder.build_call(
                            unpin,
                            &[state_raw.into()],
                            "multi_escape_pure_direct_step_state_unpin_custom",
                        )?;
                        cg.builder.build_return(None)?;
                    }
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
                if has_sibling_nonresuming {
                    for custom in &custom_siblings {
                        cg.push_effect_unwind_target(
                            &custom.arm.op.op.fqn,
                            step_arm_unwind_bbs[site_idx],
                        );
                    }
                    cg.push_raise_target(step_arm_unwind_bbs[site_idx]);
                }
                let arm_v =
                    cg.codegen_expr_in_expected_context(&plan.arm.body, Some(out_ty))?;
                if has_sibling_nonresuming {
                    cg.pop_raise_target();
                    for _ in custom_siblings.iter().rev() {
                        cg.pop_effect_unwind_target();
                    }
                }
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

                if has_sibling_nonresuming {
                    cg.builder.position_at_end(step_arm_unwind_bbs[site_idx]);
                    let llvm_ref_ty = cg.llvm_basic_type_of(span, CgTy::Ref)?;
                    let k_loaded = cg
                        .builder
                        .build_load(
                            llvm_ref_ty,
                            step_cont_ptr,
                            "multi_escape_pure_direct_step_k_unpin_load_unwind",
                        )?
                        .into_pointer_value();
                    let unpin = cg.declare_runtime_gc_unpin();
                    let _ = cg.builder.build_call(
                        unpin,
                        &[k_loaded.into()],
                        "multi_escape_pure_direct_step_k_unpin_unwind",
                    )?;
                    cg.builder.build_return(None)?;
                }
            }

            if !has_sibling_nonresuming {
                for unwind_bb in &step_arm_unwind_bbs {
                    cg.builder.position_at_end(*unwind_bb);
                    cg.builder.build_unreachable()?;
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
        let finally_bb = if has_finally {
            Some(
                self.context
                    .append_basic_block(func, "handle_multi_escape_pure_direct_finally"),
            )
        } else {
            None
        };
        let finally_unwind_bb = if has_finally {
            Some(
                self.context.append_basic_block(
                    func,
                    "handle_multi_escape_pure_direct_finally_unwind",
                ),
            )
        } else {
            None
        };
        let sibling_dispatch = self.build_sibling_nonresuming_dispatch_blocks(
            func,
            "handle_multi_escape_pure_direct",
            &sibling_plan,
        );
        let effect_dispatch_bb = sibling_dispatch.effect_dispatch_bb;
        let effect_dispatch_nomatch_bb = sibling_dispatch.effect_dispatch_nomatch_bb;
        let raise_catch_bb = sibling_dispatch.raise_catch_bb;
        let custom_catch_bbs = sibling_dispatch.custom_catch_bbs;
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
        let continuation_created_ptr = self.create_entry_alloca_raw(
            span,
            "handle_multi_escape_pure_direct_cont_created",
            self.context.bool_type().into(),
        )?;
        let _ = self.builder.build_store(
            continuation_created_ptr,
            self.context.bool_type().const_zero(),
        )?;
        let mut initial_binder_slots_by_site: Vec<Vec<ImmediateResumeBinderSlot<'ctx>>> =
            Vec::new();
        let mut arm_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        let mut arm_unwind_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        for (site_idx, plan) in scanned_sites.iter().enumerate() {
            let prefix = format!("multi_escape_pure_direct_site_{site_idx}");
            initial_binder_slots_by_site
                .push(self.build_multiple_escape_binder_slots(plan.arm, &prefix)?);
            arm_bbs.push(self.context.append_basic_block(
                func,
                &format!("handle_multi_escape_pure_direct_arm_{site_idx}"),
            ));
            arm_unwind_bbs.push(self.context.append_basic_block(
                func,
                &format!("handle_multi_escape_pure_direct_arm_unwind_{site_idx}"),
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
        if let Some(effect_dispatch_bb) = effect_dispatch_bb {
            for (idx, custom) in custom_siblings.iter().enumerate() {
                self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom_catch_bbs[idx]);
            }
            self.push_raise_target(effect_dispatch_bb);
        } else if let Some(finally_unwind_bb) = finally_unwind_bb {
            self.push_raise_target(finally_unwind_bb);
        }
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
                let _ = self.builder.build_store(
                    continuation_created_ptr,
                    self.context.bool_type().const_all_ones(),
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
        if effect_dispatch_bb.is_some() {
            self.pop_raise_target();
            for _ in custom_siblings.iter().rev() {
                self.pop_effect_unwind_target();
            }
        } else if finally_unwind_bb.is_some() {
            self.pop_raise_target();
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

        if let Some(effect_dispatch_bb) = effect_dispatch_bb {
            let effect_dispatch_nomatch_bb = effect_dispatch_nomatch_bb
                .expect("multiple escape dispatch_nomatch bb should exist");
            self.builder.position_at_end(effect_dispatch_bb);
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = self.builder.build_call(
                rt_read_tag,
                &[],
                "multi_escape_pure_direct_dispatch_read_op_tag",
            )?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "multiple escape dispatch read_op_tag return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multiple escape dispatch read_op_tag return type",
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
            let unpin = self.declare_runtime_gc_unpin();
            let _ = self.builder.build_call(
                unpin,
                &[state_raw.into()],
                "multi_escape_pure_direct_state_unpin_nomatch",
            )?;
            if let Some(target) = outer_raise_target {
                self.builder.build_unconditional_branch(target)?;
            } else {
                let ret_ty =
                    self.current_fun_return_ty
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "multiple escape dispatch unwind needs function return type",
                            at: span.into(),
                        })?;
                let v = self.default_value(span, ret_ty)?;
                self.emit_return(span, ret_ty, v)?;
            }

            if let (Some(raise_arm), Some(raise_catch_bb)) = (raise_sibling, raise_catch_bb) {
                let binder = &raise_arm.op.binders[0];
                self.builder.position_at_end(raise_catch_bb);

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "multi_escape_pure_direct_raise_read_slot_len_words",
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
                    "multi_escape_pure_direct_raise_slot_len_ok",
                )?;
                let len_ok_bb = self
                    .context
                    .append_basic_block(func, "multi_escape_pure_direct_raise_slot_len_ok_bb");
                let len_bad_bb = self
                    .context
                    .append_basic_block(func, "multi_escape_pure_direct_raise_slot_len_bad_bb");
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
                    "multi_escape_pure_direct_raise_read_slot_word0",
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
                    "multi_escape_pure_direct_raise_read_slot_word1",
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
                    "multi_escape_pure_direct_raise_clear",
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
                            "multi_escape_pure_direct_raise_kind_is_int",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_pure_direct_raise_kind_int_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_pure_direct_raise_kind_int_bad",
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
                            "multi_escape_pure_direct_raise_kind_is_runtime_error",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_pure_direct_raise_kind_runtime_error_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "multi_escape_pure_direct_raise_kind_runtime_error_bad",
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
                            "multi_escape_pure_direct_runtime_error_tag_i32",
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
                            "multi_escape_pure_direct_runtime_error_tag",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_word_zero,
                            1,
                            "multi_escape_pure_direct_runtime_error_payload_word",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_ptr_zero,
                            2,
                            "multi_escape_pure_direct_runtime_error_payload_ptr",
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
                let _ = self.store_local_value(binder.span, binder_ptr, binder_cg_ty, binder_value)?;
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
                    self.push_effect_unwind_target(
                        &custom.arm.op.op.fqn,
                        effect_dispatch_nomatch_bb,
                    );
                }
                self.push_raise_target(effect_dispatch_nomatch_bb);
                let arm_v = self.codegen_expr_in_expected_context(&raise_arm.body, Some(out_ty))?;
                self.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    self.pop_effect_unwind_target();
                }
                let arm_v = if out_ty == CgTy::Unit {
                    CgValue::unit()
                } else if out_ty == CgTy::Never {
                    CgValue::never()
                } else {
                    self.coerce_value(raise_arm.body.span, arm_v, out_ty)?
                };
                self.env.pop_scope();

                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    if let Some(ptr) = result_ptr {
                        let _ = self.store_local_value(raise_arm.body.span, ptr, out_ty, arm_v)?;
                    }
                    self.builder.build_unconditional_branch(done_bb)?;
                }
            }

            for (idx, custom) in custom_siblings.iter().enumerate() {
                let arm = custom.arm;
                let binder = &arm.op.binders[0];
                self.builder.position_at_end(custom_catch_bbs[idx]);

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "multi_escape_pure_direct_custom_read_slot_len_words",
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
                    "multi_escape_pure_direct_custom_slot_len_ok",
                )?;
                let len_ok_bb = self
                    .context
                    .append_basic_block(func, "multi_escape_pure_direct_custom_slot_len_ok_bb");
                let len_bad_bb = self
                    .context
                    .append_basic_block(func, "multi_escape_pure_direct_custom_slot_len_bad_bb");
                self.builder
                    .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                self.builder.position_at_end(len_bad_bb);
                self.emit_exit_with_code(span, 3)?;

                self.builder.position_at_end(len_ok_bb);
                let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
                let value_call = self.builder.build_call(
                    rt_read,
                    &[],
                    "multi_escape_pure_direct_custom_read_slot_word0",
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
                    "multi_escape_pure_direct_custom_read_slot_gc_ref",
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
                    "multi_escape_pure_direct_custom_clear",
                )?;

                for custom in &custom_siblings {
                    self.push_effect_unwind_target(
                        &custom.arm.op.op.fqn,
                        effect_dispatch_nomatch_bb,
                    );
                }
                self.push_raise_target(effect_dispatch_nomatch_bb);
                let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
                self.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    self.pop_effect_unwind_target();
                }
                let arm_v = if out_ty == CgTy::Unit {
                    CgValue::unit()
                } else if out_ty == CgTy::Never {
                    CgValue::never()
                } else {
                    self.coerce_value(arm.body.span, arm_v, out_ty)?
                };
                self.env.pop_scope();

                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    if let Some(ptr) = result_ptr {
                        let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
                    }
                    self.builder.build_unconditional_branch(done_bb)?;
                }
            }
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
            if has_sibling_nonresuming {
                for custom in &custom_siblings {
                    self.push_effect_unwind_target(
                        &custom.arm.op.op.fqn,
                        arm_unwind_bbs[site_idx],
                    );
                }
                self.push_raise_target(arm_unwind_bbs[site_idx]);
            } else if let Some(finally_unwind_bb) = finally_unwind_bb {
                self.push_raise_target(finally_unwind_bb);
            }
            let arm_v = self.codegen_expr_in_expected_context(&plan.arm.body, Some(out_ty))?;
            if has_sibling_nonresuming {
                self.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    self.pop_effect_unwind_target();
                }
            } else if finally_unwind_bb.is_some() {
                self.pop_raise_target();
            }
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
                let target = finally_bb.unwrap_or(done_bb);
                self.builder.build_unconditional_branch(target)?;
            }

            if has_sibling_nonresuming {
                self.builder.position_at_end(arm_unwind_bbs[site_idx]);
                let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
                let k_loaded = self
                    .builder
                    .build_load(
                        llvm_ref_ty,
                        cont_ptr,
                        "multi_escape_pure_direct_k_unpin_load_unwind",
                    )?
                    .into_pointer_value();
                let unpin = self.declare_runtime_gc_unpin();
                let _ = self.builder.build_call(
                    unpin,
                    &[k_loaded.into()],
                    "multi_escape_pure_direct_k_unpin_unwind",
                )?;
                if let Some(target) = outer_raise_target {
                    self.builder.build_unconditional_branch(target)?;
                } else {
                    let ret_ty =
                        self.current_fun_return_ty
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "multiple escape arm unwind needs function return type",
                                at: span.into(),
                            })?;
                    let v = self.default_value(span, ret_ty)?;
                    self.emit_return(span, ret_ty, v)?;
                }
            }
        }

        if !has_sibling_nonresuming {
            for unwind_bb in &arm_unwind_bbs {
                self.builder.position_at_end(*unwind_bb);
                self.builder.build_unreachable()?;
            }
        }

        if let Some(finally_unwind_bb) = finally_unwind_bb {
            self.builder.position_at_end(finally_unwind_bb);
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
                        "multi_escape_pure_direct_unwind_cont_created",
                    )?
                    .into_int_value();
                let unwind_propagate_bb = self.context.append_basic_block(
                    func,
                    "handle_multi_escape_pure_direct_finally_unwind_propagate",
                );
                let unwind_unpin_bb = self.context.append_basic_block(
                    func,
                    "handle_multi_escape_pure_direct_finally_unwind_unpin",
                );
                self.builder
                    .build_conditional_branch(created, unwind_propagate_bb, unwind_unpin_bb)?;

                self.builder.position_at_end(unwind_unpin_bb);
                let unpin = self.declare_runtime_gc_unpin();
                let _ = self.builder.build_call(
                    unpin,
                    &[state_raw.into()],
                    "multi_escape_pure_direct_state_unpin_unwind",
                )?;
                self.builder
                    .build_unconditional_branch(unwind_propagate_bb)?;

                self.builder.position_at_end(unwind_propagate_bb);
                if let Some(target) = outer_raise_target {
                    self.builder.build_unconditional_branch(target)?;
                } else {
                    let ret_ty =
                        self.current_fun_return_ty
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multiple escape-continuation arms finally unwind needs function return type",
                                at: span.into(),
                            })?;
                    let v = self.default_value(span, ret_ty)?;
                    self.emit_return(span, ret_ty, v)?;
                }
            }
        }

        if let Some(finally_bb) = finally_bb {
            self.builder.position_at_end(finally_bb);
            if let Some(finally) = handle.finally.as_ref() {
                let _ = self.codegen_block_value(finally)?;
            }
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                self.builder.build_unconditional_branch(done_bb)?;
            }
        }

        self.builder.position_at_end(done_bb);
        let done_unpin_k_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_pure_direct_done_unpin_k");
        let done_unpin_state_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_pure_direct_done_unpin_state");
        let done_merge_bb = self
            .context
            .append_basic_block(func, "handle_multi_escape_pure_direct_done_merge");
        let created = self
            .builder
            .build_load(
                self.context.bool_type(),
                continuation_created_ptr,
                "multi_escape_pure_direct_done_cont_created",
            )?
            .into_int_value();
        self.builder
            .build_conditional_branch(created, done_unpin_k_bb, done_unpin_state_bb)?;

        self.builder.position_at_end(done_unpin_k_bb);
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
        self.builder.build_unconditional_branch(done_merge_bb)?;

        self.builder.position_at_end(done_unpin_state_bb);
        let _ = self.builder.build_call(
            unpin,
            &[state_raw.into()],
            "multi_escape_pure_direct_state_unpin_done",
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
