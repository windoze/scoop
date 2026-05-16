//! Per-callable frame management: frame root allocation, frame slot lookup, effect-context slot ops, effect-handler-node allocation, and the GC pointer rooting helpers used to keep stack pointers alive across runtime calls.

use super::*;

impl<'cg, 'a, 'ctx> CallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn initialize_new_frame(&mut self) -> Result<(), LlvmEmitError> {
        let frame_ptr = self.codegen.alloc_gc_struct(
            self.mir_fun.span,
            self.frame_layout.llvm_ty(),
            self.frame_layout.layout_anchor_name(),
            "frame",
        )?;
        self.store_frame_root(frame_ptr)?;
        self.initialize_frame_effect_ctx_root()
    }

    pub(super) fn store_frame_root(
        &mut self,
        frame_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let frame_gc = self.codegen.cast_ptr(
            frame_ptr,
            self.codegen.llvm_gc_i8_ptr_type(),
            "frame_root_value",
        )?;
        self.codegen.store_gc_root_slot(
            self.mir_fun.span,
            self.frame_root_slot,
            frame_gc,
            "frame_root",
        )?;
        Ok(())
    }

    pub(super) fn clear_frame_root(&mut self) -> Result<(), LlvmEmitError> {
        let null = self.codegen.llvm_gc_i8_ptr_type().const_null();
        self.codegen.store_gc_root_slot(
            self.mir_fun.span,
            self.frame_root_slot,
            null,
            "frame_root",
        )
    }

    pub(super) fn release_frame_root_for_frame_free_tail(
        &mut self,
        resume_state: StateId,
    ) -> Result<(), LlvmEmitError> {
        if !matches!(self.return_mode, CallableReturnMode::Plain { .. })
            || self.callable.needs_reentry()
            || self.return_projection.is_some()
            || self.surface_resume_handle_sites.is_some()
            || self.callable_has_handle_or_composed_resume_boundary()
            || !self.reachable_tail_is_frame_free(resume_state)
        {
            return Ok(());
        }
        self.clear_frame_root()
    }

    pub(super) fn callable_has_handle_or_composed_resume_boundary(&self) -> bool {
        if self.callable.state_graph().states().iter().any(|state| {
            matches!(
                state.terminator(),
                LateLoweredStateTerminator::HandleDispatch { .. }
            )
        }) {
            return true;
        }
        self.callable
            .boundary_map()
            .entries()
            .iter()
            .any(|boundary| match boundary.lowering() {
                Some(LateLoweredBoundaryLowering::Resume(_))
                | Some(LateLoweredBoundaryLowering::RuntimeError(_)) => true,
                Some(LateLoweredBoundaryLowering::Call(lowering)) => {
                    !lowering.continuation_compositions().is_empty()
                }
                None => false,
                Some(_) => true,
            })
    }

    pub(super) fn reachable_tail_is_frame_free(&self, start: StateId) -> bool {
        let mut seen = BTreeSet::new();
        let mut stack = vec![start];
        while let Some(state_id) = stack.pop() {
            if !seen.insert(state_id) {
                continue;
            }
            let Some(state) = self.callable.state_graph().state(state_id) else {
                return false;
            };
            if matches!(state.role(), LateLoweredStateRole::Cleanup) {
                return false;
            }
            if matches!(
                state.terminator(),
                LateLoweredStateTerminator::Suspend { .. }
                    | LateLoweredStateTerminator::HandleDispatch { .. }
            ) {
                return false;
            }
            stack.extend(state.successors().iter().copied());
        }
        true
    }

    pub(super) fn current_frame_ptr(&mut self) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let frame_gc = self.codegen.load_gc_root_slot(
            self.mir_fun.span,
            self.frame_root_slot,
            "frame_root",
        )?;
        self.codegen.cast_ptr(
            frame_gc,
            self.codegen.llvm_ptr_type(self.codegen.gc_address_space()),
            "frame_current",
        )
    }

    pub(super) fn current_frame_gc_ref(
        &mut self,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let frame_ptr = self.current_frame_ptr()?;
        self.codegen
            .cast_ptr(frame_ptr, self.codegen.llvm_gc_i8_ptr_type(), name)
    }

    pub(super) fn frame_slot_id_for_kind(
        &self,
        kind: LateLoweredFrameSlotKind,
    ) -> Result<FrameSlotId, LlvmEmitError> {
        self.callable
            .frame_schema()
            .slot_for_kind(kind)
            .map(|slot| slot.slot_id())
            .ok_or_else(|| frontend_error(format!("frame schema 缺少 slot kind {kind:?}")))
    }

    pub(super) fn frame_gc_ref_slot_ptr(
        &mut self,
        slot_id: FrameSlotId,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let field_index = self
            .frame_layout
            .field_index_for_slot(slot_id)
            .ok_or_else(|| {
                frontend_error(format!(
                    "frame layout 缺少 slot{} field index",
                    slot_id.as_u32()
                ))
            })?;
        self.frame_field_ptr(field_index, name)
    }

    pub(super) fn load_gc_ref_from_frame_slot_id(
        &mut self,
        slot_id: FrameSlotId,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let field_ptr = self.frame_gc_ref_slot_ptr(slot_id, name)?;
        Ok(self
            .codegen
            .builder
            .build_load(self.codegen.llvm_gc_i8_ptr_type(), field_ptr, name)?
            .into_pointer_value())
    }

    pub(super) fn store_gc_ref_to_frame_slot_id(
        &mut self,
        slot_id: FrameSlotId,
        value: PointerValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let field_ptr = self.frame_gc_ref_slot_ptr(slot_id, name)?;
        self.codegen
            .store_gc_pointer_slot_with_write_barrier(self.mir_fun.span, field_ptr, value)
    }

    pub(super) fn current_effect_ctx_slot_ptr(
        &mut self,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let field_index = self
            .frame_layout
            .field_index_for_system(SystemSlotKind::CurrentEffectCtx)
            .ok_or_else(|| {
                frontend_error(
                    "frame layout 缺少 CurrentEffectCtx system field".to_string(),
                )
            })?;
        self.frame_field_ptr(field_index, name)
    }

    pub(super) fn current_state_tag_slot_ptr(
        &mut self,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let field_index = self
            .frame_layout
            .field_index_for_system(SystemSlotKind::StateTag)
            .ok_or_else(|| {
                frontend_error("frame layout 缺少 StateTag system field".to_string())
            })?;
        self.frame_field_ptr(field_index, name)
    }

    pub(super) fn load_current_effect_ctx(
        &mut self,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let field_ptr = self.current_effect_ctx_slot_ptr(name)?;
        Ok(self
            .codegen
            .builder
            .build_load(self.codegen.llvm_gc_i8_ptr_type(), field_ptr, name)?
            .into_pointer_value())
    }

    pub(super) fn store_current_effect_ctx(
        &mut self,
        effect_ctx: PointerValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let field_ptr = self.current_effect_ctx_slot_ptr(name)?;
        self.codegen.store_gc_pointer_slot_with_write_barrier(
            self.mir_fun.span,
            field_ptr,
            effect_ctx,
        )
    }

    pub(super) fn load_current_state_tag(
        &mut self,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let field_ptr = self.current_state_tag_slot_ptr(name)?;
        Ok(self
            .codegen
            .builder
            .build_load(self.codegen.context.i32_type(), field_ptr, name)?
            .into_int_value())
    }

    pub(super) fn store_current_state_tag(
        &mut self,
        state_tag: IntValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let field_ptr = self.current_state_tag_slot_ptr(name)?;
        self.codegen.builder.build_store(field_ptr, state_tag)?;
        Ok(())
    }

    pub(super) fn current_effect_outcome_ptr(&self) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.codegen
            .function_cx
            .current_effect_outcome_ptr
            .ok_or_else(|| {
                frontend_error(format!(
                    "callable `{}` 缺少当前 explicit effect outcome 指针",
                    self.callable.root_fqn()
                ))
            })
    }

    pub(super) fn handle_saved_effect_ctx_slot_id(
        &self,
        site_id: SiteId,
    ) -> Result<FrameSlotId, LlvmEmitError> {
        self.frame_slot_id_for_kind(LateLoweredFrameSlotKind::HandleSavedEffectCtx { site_id })
    }

    pub(super) fn handle_arm_effect_ctx_slot_id(
        &self,
        site_id: SiteId,
        arm_ordinal: u32,
    ) -> Result<FrameSlotId, LlvmEmitError> {
        self.frame_slot_id_for_kind(LateLoweredFrameSlotKind::HandleArmEffectCtx {
            site_id,
            arm_ordinal,
        })
    }

    pub(super) fn load_handle_saved_effect_ctx(
        &mut self,
        site_id: SiteId,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.load_gc_ref_from_frame_slot_id(self.handle_saved_effect_ctx_slot_id(site_id)?, name)
    }

    pub(super) fn store_handle_saved_effect_ctx(
        &mut self,
        site_id: SiteId,
        value: PointerValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        self.store_gc_ref_to_frame_slot_id(
            self.handle_saved_effect_ctx_slot_id(site_id)?,
            value,
            name,
        )
    }

    pub(super) fn clear_handle_saved_effect_ctx(
        &mut self,
        site_id: SiteId,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        self.store_gc_ref_to_frame_slot_id(
            self.handle_saved_effect_ctx_slot_id(site_id)?,
            self.codegen.llvm_gc_i8_ptr_type().const_null(),
            name,
        )
    }

    pub(super) fn load_handle_arm_effect_ctx(
        &mut self,
        site_id: SiteId,
        arm_ordinal: u32,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.load_gc_ref_from_frame_slot_id(
            self.handle_arm_effect_ctx_slot_id(site_id, arm_ordinal)?,
            name,
        )
    }

    pub(super) fn store_handle_arm_effect_ctx(
        &mut self,
        site_id: SiteId,
        arm_ordinal: u32,
        value: PointerValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        self.store_gc_ref_to_frame_slot_id(
            self.handle_arm_effect_ctx_slot_id(site_id, arm_ordinal)?,
            value,
            name,
        )
    }

    pub(super) fn clear_handle_arm_effect_ctx(
        &mut self,
        site_id: SiteId,
        arm_ordinal: u32,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        self.store_gc_ref_to_frame_slot_id(
            self.handle_arm_effect_ctx_slot_id(site_id, arm_ordinal)?,
            self.codegen.llvm_gc_i8_ptr_type().const_null(),
            name,
        )
    }

    pub(super) fn handle_arm_ordinals_for_site(
        &self,
        site_id: SiteId,
    ) -> Result<Vec<u32>, LlvmEmitError> {
        let mut ordinals = BTreeSet::new();
        for state in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id: dispatch_site,
                contract,
                ..
            } = state.terminator()
            else {
                continue;
            };
            if *dispatch_site != site_id {
                continue;
            }
            for arm in contract.handled_arms() {
                ordinals.insert(arm.arm_ordinal());
            }
        }
        if ordinals.is_empty() {
            return Err(frontend_error(format!(
                "HandleDispatch site{} 缺少 handled arm metadata",
                site_id.as_u32(),
            )));
        }
        Ok(ordinals.into_iter().collect())
    }

    pub(super) fn clear_handle_effect_ctx_slots(
        &mut self,
        site_id: SiteId,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        self.clear_handle_saved_effect_ctx(site_id, &format!("{name}_saved"))?;
        for arm_ordinal in self.handle_arm_ordinals_for_site(site_id)? {
            self.clear_handle_arm_effect_ctx(
                site_id,
                arm_ordinal,
                &format!("{name}_arm{arm_ordinal}"),
            )?;
        }
        Ok(())
    }

    pub(super) fn restore_and_clear_handle_effect_ctx_slots(
        &mut self,
        site_id: SiteId,
        restore_name: &str,
        clear_name: &str,
    ) -> Result<(), LlvmEmitError> {
        self.restore_handle_saved_effect_ctx(site_id, restore_name)?;
        self.clear_handle_effect_ctx_slots(site_id, clear_name)
    }

    pub(super) fn cast_gc_ref_to_effect_ctx_ptr(
        &mut self,
        value: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.codegen.cast_ptr(
            value,
            self.codegen.llvm_ptr_type(self.codegen.gc_address_space()),
            name,
        )
    }

    pub(super) fn cast_gc_ref_to_effect_handler_node_ptr(
        &mut self,
        value: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.codegen.cast_ptr(
            value,
            self.codegen.llvm_ptr_type(self.codegen.gc_address_space()),
            name,
        )
    }

    pub(super) fn alloc_effect_ctx_with_handler_top(
        &mut self,
        handler_top: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let ctx_ptr = self.codegen.alloc_gc_struct(
            self.mir_fun.span,
            self.codegen.llvm_effect_ctx_object_type(),
            self.codegen.effect_ctx_layout_anchor_name(),
            name,
        )?;
        let ctx_root_slot = self.capture_gc_pointer_root_slot(ctx_ptr, &format!("{name}_root"))?;
        let ctx_ptr =
            self.reload_gc_pointer_from_root_slot(ctx_root_slot, &format!("{name}_root"))?;
        self.codegen
            .store_effect_ctx_handler_top(self.mir_fun.span, ctx_ptr, handler_top, name)?;
        let ctx_ptr =
            self.reload_gc_pointer_from_root_slot(ctx_root_slot, &format!("{name}_root"))?;
        self.clear_root_gc_slot(ctx_root_slot, &format!("{name}_root_clear"))?;
        self.codegen.cast_ptr(
            ctx_ptr,
            self.codegen.llvm_gc_i8_ptr_type(),
            &format!("{name}_gc"),
        )
    }

    pub(super) fn alloc_empty_effect_ctx(
        &mut self,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.alloc_effect_ctx_with_handler_top(
            self.codegen.llvm_gc_i8_ptr_type().const_null(),
            name,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn alloc_effect_handler_node(
        &mut self,
        prev_ref_root_slot: PointerValue<'ctx>,
        op_tag: u32,
        flags: u32,
        site_id: SiteId,
        arm_ordinal: u32,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let node_ptr = self.codegen.alloc_gc_struct(
            self.mir_fun.span,
            self.codegen.llvm_effect_handler_node_object_type(),
            self.codegen.effect_handler_node_layout_anchor_name(),
            name,
        )?;
        let node_root_slot =
            self.capture_gc_pointer_root_slot(node_ptr, &format!("{name}_root"))?;
        let node_ptr =
            self.reload_gc_pointer_from_root_slot(node_root_slot, &format!("{name}_root"))?;
        let prev_ref = self.codegen.load_gc_root_slot(
            self.mir_fun.span,
            prev_ref_root_slot,
            &format!("{name}_prev_ref_reload"),
        )?;
        self.codegen
            .store_effect_handler_prev_ref(self.mir_fun.span, node_ptr, prev_ref, name)?;
        self.codegen.store_effect_handler_op_tag(
            node_ptr,
            self.codegen
                .context
                .i32_type()
                .const_int(u64::from(op_tag), false),
            name,
        )?;
        self.codegen.store_effect_handler_flags(
            node_ptr,
            self.codegen
                .context
                .i32_type()
                .const_int(u64::from(flags), false),
            name,
        )?;
        let node_ptr =
            self.reload_gc_pointer_from_root_slot(node_root_slot, &format!("{name}_root"))?;
        let owner_frame_ref = self.current_frame_gc_ref(&format!("{name}_owner_frame_reload"))?;
        self.codegen.store_effect_handler_owner_frame_ref(
            self.mir_fun.span,
            node_ptr,
            owner_frame_ref,
            name,
        )?;
        self.codegen.store_effect_handler_dispatch_identity(
            node_ptr,
            self.codegen
                .effect_handler_dispatch_identity_const(site_id, arm_ordinal),
            name,
        )?;
        let node_ptr =
            self.reload_gc_pointer_from_root_slot(node_root_slot, &format!("{name}_root"))?;
        self.clear_root_gc_slot(node_root_slot, &format!("{name}_root_clear"))?;
        self.codegen.cast_ptr(
            node_ptr,
            self.codegen.llvm_gc_i8_ptr_type(),
            &format!("{name}_gc"),
        )
    }

    pub(super) fn handle_case_op_tag(&mut self, case_tag: CaseTag) -> Result<u32, LlvmEmitError> {
        let case_layout = self.step_layout.case_layout(case_tag).ok_or_else(|| {
            frontend_error(format!(
                "step schema s{} 缺少 handle case c{} layout",
                self.abi_step_schema.as_u32(),
                case_tag.as_u32()
            ))
        })?;
        Ok(self
            .codegen
            .effect_op_tag(case_layout.concrete_op_key().effect_family().effect_fqn()))
    }

    pub(super) fn restore_handle_saved_effect_ctx(
        &mut self,
        site_id: SiteId,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let outer = self.load_handle_saved_effect_ctx(site_id, &format!("{name}_saved"))?;
        self.store_current_effect_ctx(outer, &format!("{name}_restore"))
    }

    pub(super) fn initialize_frame_effect_ctx_root(&mut self) -> Result<(), LlvmEmitError> {
        let empty_ctx = self.alloc_empty_effect_ctx("effect_ctx_root")?;
        self.store_current_effect_ctx(empty_ctx, "effect_ctx_root")
    }

    pub(super) fn enter_handle_dispatch_effect_ctx(
        &mut self,
        site_id: SiteId,
        contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
    ) -> Result<(), LlvmEmitError> {
        let outer_ctx = self.load_current_effect_ctx("handle_outer_ctx")?;
        self.store_handle_saved_effect_ctx(site_id, outer_ctx, "handle_saved_ctx")?;
        let outer_ctx = self.load_current_effect_ctx("handle_outer_ctx_reload")?;
        let outer_ctx_ptr =
            self.cast_gc_ref_to_effect_ctx_ptr(outer_ctx, "handle_outer_ctx_ptr")?;
        let outer_handler_top = self
            .codegen
            .load_effect_ctx_handler_top(outer_ctx_ptr, "handle_outer_top")?;
        let outer_handler_top_root_slot = self
            .codegen
            .create_gc_root_slot(self.mir_fun.span, "handle_outer_top_root")?;
        let _ = self.root_gc_pointer_in_slot(
            outer_handler_top_root_slot,
            outer_handler_top,
            "handle_outer_top_root",
        )?;
        let active_top_root_slot = self
            .codegen
            .create_gc_root_slot(self.mir_fun.span, "handle_active_top_root")?;
        let body_ctx_root_slot = self
            .codegen
            .create_gc_root_slot(self.mir_fun.span, "handle_body_ctx_root")?;
        let active_flag = self.codegen.effect_handler_active_flag();

        let mut arm_metas = Vec::with_capacity(contract.handled_arms().len());
        for arm in contract.handled_arms() {
            arm_metas.push((
                arm.arm_ordinal(),
                self.handle_case_op_tag(arm.handled_case())?,
            ));
        }

        let body_ctx =
            self.alloc_empty_effect_ctx(&format!("handle{}_body_ctx", site_id.as_u32()))?;
        let body_ctx = self.root_gc_pointer_in_slot(
            body_ctx_root_slot,
            body_ctx,
            &format!("handle{}_body_ctx_root", site_id.as_u32()),
        )?;
        self.store_current_effect_ctx(body_ctx, "handle_body_ctx")?;

        let mut active_prev_root_slot = outer_handler_top_root_slot;
        for (arm_ordinal, op_tag) in arm_metas.iter().rev().copied() {
            let active_top = self.alloc_effect_handler_node(
                active_prev_root_slot,
                op_tag,
                active_flag,
                site_id,
                arm_ordinal,
                &format!(
                    "handle{}_active_arm{}_node",
                    site_id.as_u32(),
                    arm_ordinal
                ),
            )?;
            let active_top = self.root_gc_pointer_in_slot(
                active_top_root_slot,
                active_top,
                &format!(
                    "handle{}_active_arm{}_node_root",
                    site_id.as_u32(),
                    arm_ordinal
                ),
            )?;
            active_prev_root_slot = active_top_root_slot;
            let body_ctx = self.load_current_effect_ctx(&format!(
                "handle{}_body_ctx_reload",
                site_id.as_u32()
            ))?;
            let body_ctx_ptr = self.cast_gc_ref_to_effect_ctx_ptr(
                body_ctx,
                &format!("handle{}_body_ctx_ptr", site_id.as_u32()),
            )?;
            self.codegen.store_effect_ctx_handler_top(
                self.mir_fun.span,
                body_ctx_ptr,
                active_top,
                &format!("handle{}_body_ctx_top", site_id.as_u32()),
            )?;
        }
        self.clear_root_gc_slot(
            active_top_root_slot,
            "handle_active_top_root_clear",
        )?;

        for (target_arm_ordinal, _) in &arm_metas {
            let derived_ctx_root_slot = self.codegen.create_gc_root_slot(
                self.mir_fun.span,
                &format!(
                    "handle{}_derived_arm{}_ctx_root",
                    site_id.as_u32(),
                    target_arm_ordinal
                ),
            )?;
            let derived_ctx = self.alloc_empty_effect_ctx(&format!(
                "handle{}_derived_arm{}_ctx",
                site_id.as_u32(),
                target_arm_ordinal
            ))?;
            let derived_ctx = self.root_gc_pointer_in_slot(
                derived_ctx_root_slot,
                derived_ctx,
                &format!(
                    "handle{}_derived_arm{}_ctx_root",
                    site_id.as_u32(),
                    target_arm_ordinal
                ),
            )?;
            self.store_handle_arm_effect_ctx(
                site_id,
                *target_arm_ordinal,
                derived_ctx,
                "handle_arm_effect_ctx",
            )?;
            let derived_top_root_slot = self.codegen.create_gc_root_slot(
                self.mir_fun.span,
                &format!(
                    "handle{}_derived_arm{}_root",
                    site_id.as_u32(),
                    target_arm_ordinal
                ),
            )?;
            let mut derived_prev_root_slot = outer_handler_top_root_slot;
            for (arm_ordinal, op_tag) in arm_metas.iter().rev().copied() {
                let flags = if arm_ordinal == *target_arm_ordinal {
                    0
                } else {
                    active_flag
                };
                let derived_top = self.alloc_effect_handler_node(
                    derived_prev_root_slot,
                    op_tag,
                    flags,
                    site_id,
                    arm_ordinal,
                    &format!(
                        "handle{}_derived_arm{}_clone{}",
                        site_id.as_u32(),
                        target_arm_ordinal,
                        arm_ordinal
                    ),
                )?;
                derived_prev_root_slot = derived_top_root_slot;
                let derived_top = self.root_gc_pointer_in_slot(
                    derived_top_root_slot,
                    derived_top,
                    &format!(
                        "handle{}_derived_arm{}_clone{}_root",
                        site_id.as_u32(),
                        target_arm_ordinal,
                        arm_ordinal
                    ),
                )?;
                let derived_ctx = self.load_handle_arm_effect_ctx(
                    site_id,
                    *target_arm_ordinal,
                    &format!(
                        "handle{}_derived_arm{}_ctx_reload",
                        site_id.as_u32(),
                        target_arm_ordinal
                    ),
                )?;
                let derived_ctx_ptr = self.cast_gc_ref_to_effect_ctx_ptr(
                    derived_ctx,
                    &format!(
                        "handle{}_derived_arm{}_ctx_ptr",
                        site_id.as_u32(),
                        target_arm_ordinal
                    ),
                )?;
                self.codegen.store_effect_ctx_handler_top(
                    self.mir_fun.span,
                    derived_ctx_ptr,
                    derived_top,
                    &format!(
                        "handle{}_derived_arm{}_ctx_top",
                        site_id.as_u32(),
                        target_arm_ordinal
                    ),
                )?;
            }
            self.clear_root_gc_slot(
                derived_top_root_slot,
                &format!(
                    "handle{}_derived_arm{}_root_clear",
                    site_id.as_u32(),
                    target_arm_ordinal
                ),
            )?;
            self.clear_root_gc_slot(
                derived_ctx_root_slot,
                &format!(
                    "handle{}_derived_arm{}_ctx_root_clear",
                    site_id.as_u32(),
                    target_arm_ordinal
                ),
            )?;
        }
        self.clear_root_gc_slot(body_ctx_root_slot, "handle_body_ctx_root_clear")?;
        self.clear_root_gc_slot(
            outer_handler_top_root_slot,
            "handle_outer_top_root_clear",
        )?;
        Ok(())
    }

    pub(super) fn handle_boundary_site_id(boundary: &LateLoweredBoundary) -> Option<SiteId> {
        let LateLoweredBoundarySource::Site {
            site_id,
            kind: BoundarySiteKind::Handle,
        } = boundary.source()
        else {
            return None;
        };
        Some(site_id)
    }

    pub(super) fn root_gc_pointer(
        &mut self,
        value: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let slot = self
            .codegen
            .create_gc_root_slot(self.mir_fun.span, name)?;
        let value = self.codegen.cast_ptr(
            value,
            self.codegen.llvm_gc_i8_ptr_type(),
            &format!("{name}_value"),
        )?;
        self.codegen
            .store_gc_root_slot(self.mir_fun.span, slot, value, name)?;
        self.codegen
            .load_gc_root_slot(self.mir_fun.span, slot, name)
    }

    pub(super) fn root_gc_pointer_in_slot(
        &mut self,
        slot: PointerValue<'ctx>,
        value: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let value = self.codegen.cast_ptr(
            value,
            self.codegen.llvm_gc_i8_ptr_type(),
            &format!("{name}_value"),
        )?;
        self.codegen
            .store_gc_root_slot(self.mir_fun.span, slot, value, name)?;
        self.codegen
            .load_gc_root_slot(self.mir_fun.span, slot, name)
    }

    // Fresh GC objects and loaded GC refs in effect lowering can cross write-barrier,
    // allocation, and payload-materialization windows. Keep a stable reloadable home slot so
    // later uses do not keep stale pre-GC SSA pointers.
    pub(super) fn capture_gc_pointer_root_slot(
        &mut self,
        value: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let slot = self
            .codegen
            .create_gc_root_slot(self.mir_fun.span, name)?;
        let _ = self.root_gc_pointer_in_slot(slot, value, name)?;
        Ok(slot)
    }

    pub(super) fn reload_gc_pointer_from_root_slot(
        &mut self,
        slot: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.codegen
            .load_gc_root_slot(self.mir_fun.span, slot, name)
    }

    pub(super) fn clear_root_gc_slot(
        &mut self,
        slot: PointerValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        self.codegen.store_gc_root_slot(
            self.mir_fun.span,
            slot,
            self.codegen.llvm_gc_i8_ptr_type().const_null(),
            name,
        )
    }
}
