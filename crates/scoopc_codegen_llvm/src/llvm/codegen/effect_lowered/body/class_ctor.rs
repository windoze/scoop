//! Class constructor and task-transport boundary lowering: hidden init bridges, class-ctor outcome payload assembly, and the dynamic-resume adapter used when a task transport delegates resume to a callee.

use super::*;
use scoopc_lir_facts::LirGlobalRootKind;

impl<'cg, 'a, 'ctx> CallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn lower_class_ctor_boundary(
        &mut self,
        boundary: &LateLoweredBoundary,
        lowering: &crate::effect_lowered::ir::LateLoweredClassCtorBoundaryLowering,
    ) -> Result<(), LlvmEmitError> {
        let site_id = boundary_site(boundary, "ClassCtor")?;
        let source = self.class_ctor_boundary_statement(boundary, lowering, site_id)?;
        match &source {
            ClassCtorBoundarySource::ClassCtor { span, ctor, args } => {
                let class_layout_key = self.class_ctor_layout_key(
                    *span,
                    lowering.class_fqn(),
                    lowering.result_local(),
                )?;
                let slots = self.slots.clone();
                let args = args.to_vec();
                let result = self
                    .codegen
                    .with_active_suspend_site_any_effect_outcome_capture(
                        site_id.as_u32(),
                        |cg| {
                            cg.with_ordinary_effect_propagation_suppressed(|cg| {
                                cg.codegen_lir_class_ctor_call(
                                    *span,
                                    site_id,
                                    &class_layout_key,
                                    ctor,
                                    &args,
                                    &slots,
                                )
                            })
                        },
                    )?;
                let outcome_slot = self
                    .codegen
                    .take_suspend_site_explicit_effect_outcome(site_id.as_u32())
                    .or(self.codegen.function_cx.current_effect_outcome_ptr);
                let Some(outcome_slot) = outcome_slot else {
                    let _ = self.store_local_value(*span, lowering.result_local(), result)?;
                    return self.branch_to_state(boundary.resume_state());
                };

                let active_bb = self
                    .codegen
                    .context
                    .append_basic_block(self.function, "class_ctor_hidden_effect_active");
                let inactive_bb = self
                    .codegen
                    .context
                    .append_basic_block(self.function, "class_ctor_hidden_effect_inactive");
                let is_propagating = self.codegen.effect_outcome_is_propagating(
                    *span,
                    outcome_slot,
                    "class_ctor_hidden_effect",
                )?;
                self.codegen.builder.build_conditional_branch(
                    is_propagating,
                    active_bb,
                    inactive_bb,
                )?;

                self.codegen.builder.position_at_end(active_bb);
                let emission = match lowering.emitted_steps() {
                    [single] => single,
                    [] => {
                        return Err(frontend_error(format!(
                            "class ctor boundary bd{} 缺少 hidden effect emission",
                            boundary.boundary_id().as_u32()
                        )));
                    }
                    many => {
                        return Err(frontend_error(format!(
                            "class ctor boundary bd{} 发布了 {} 个 hidden effect emission；当前 runtime outcome lowering 需要唯一 ordinary effect case",
                            boundary.boundary_id().as_u32(),
                            many.len()
                        )));
                    }
                };
                let payload = self
                    .lower_class_ctor_outcome_payload(outcome_slot, emission.payload_tuple_ty())?;
                let cleared_outcome = self.codegen.build_zero_complete_effect_outcome()?;
                self.codegen
                    .builder
                    .build_store(outcome_slot, cleared_outcome)?;
                self.emit_or_consume_outward_case(
                    boundary,
                    emission.case_tag(),
                    payload,
                    emission.payload_tuple_ty(),
                    None,
                    None,
                )?;

                self.codegen.builder.position_at_end(inactive_bb);
                let _ = self.store_local_value(*span, lowering.result_local(), result)?;
                self.branch_to_state(boundary.resume_state())
            }
            ClassCtorBoundarySource::ObjectProperty { span, fqn } => {
                let object_fqn = self
                    .codegen
                    .lookup_object_property_by_fqn(fqn)
                    .map(|(object, _prop)| object.fqn.clone())
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "class ctor boundary site{} hidden object property `{fqn}` 缺少 metadata",
                            site_id.as_u32()
                        ))
                    })?;
                let bridge = self
                    .codegen
                    .ensure_object_init_bridge_defined(&object_fqn)?;
                let outcome_slot =
                    self.call_hidden_init_bridge(*span, bridge, "hidden_object_init_bridge")?;
                let prop_fqn = (*fqn).to_string();
                self.lower_hidden_init_boundary_from_bridge(
                    boundary,
                    lowering,
                    *span,
                    outcome_slot,
                    move |cg| {
                        cg.codegen
                            .load_initialized_object_property_value(*span, &prop_fqn)
                    },
                )
            }
            ClassCtorBoundarySource::TopLevelRef { span, fqn } => {
                if self
                    .codegen
                    .lir_global_root_has_kind(fqn, LirGlobalRootKind::ObjectSingleton)
                {
                    let object_fqn = (*fqn).to_string();
                    let bridge = self
                        .codegen
                        .ensure_object_init_bridge_defined(&object_fqn)?;
                    let outcome_slot =
                        self.call_hidden_init_bridge(*span, bridge, "hidden_object_init_bridge")?;
                    self.lower_hidden_init_boundary_from_bridge(
                        boundary,
                        lowering,
                        *span,
                        outcome_slot,
                        move |cg| cg.codegen.load_initialized_object_value(*span, &object_fqn),
                    )
                } else if let Some(value) =
                    self.codegen.top_level_immutable_values.get(*fqn).cloned()
                {
                    let bridge = self
                        .codegen
                        .ensure_top_level_immutable_value_init_bridge_defined(&value.fqn)?;
                    let outcome_slot = self.call_hidden_init_bridge(
                        *span,
                        bridge,
                        "hidden_top_level_init_bridge",
                    )?;
                    self.lower_hidden_init_boundary_from_bridge(
                        boundary,
                        lowering,
                        *span,
                        outcome_slot,
                        move |cg| {
                            cg.codegen
                                .load_initialized_top_level_immutable_value(*span, &value)
                        },
                    )
                } else {
                    Err(frontend_error(format!(
                        "class ctor boundary site{} hidden top-level ref `{fqn}` 不是 object/top-level immutable init",
                        site_id.as_u32()
                    )))
                }
            }
        }
    }

    pub(super) fn call_hidden_init_bridge(
        &mut self,
        span: crate::span::Span,
        bridge: FunctionValue<'ctx>,
        label: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let outcome_slot = self.codegen.alloc_effect_outcome_slot(span, label)?;
        self.codegen
            .with_conservative_gc_local_root_spills(span, |cg| {
                let call = cg.builder.build_call(bridge, &[], label)?;
                let outcome = call.try_as_basic_value().basic().ok_or_else(|| {
                    frontend_error(format!(
                        "hidden-init bridge `{label}` 未返回 explicit outcome aggregate"
                    ))
                })?;
                cg.builder.build_store(outcome_slot, outcome)?;
                Ok(())
            })?;
        Ok(outcome_slot)
    }

    pub(super) fn lower_hidden_init_boundary_from_bridge<F>(
        &mut self,
        boundary: &LateLoweredBoundary,
        lowering: &crate::effect_lowered::ir::LateLoweredClassCtorBoundaryLowering,
        span: crate::span::Span,
        outcome_slot: PointerValue<'ctx>,
        load_result: F,
    ) -> Result<(), LlvmEmitError>
    where
        F: FnOnce(&mut Self) -> Result<CgValue<'ctx>, LlvmEmitError>,
    {
        let emission = match lowering.emitted_steps() {
            [single] => single,
            [] => {
                return Err(frontend_error(format!(
                    "class ctor boundary bd{} 缺少 hidden effect emission",
                    boundary.boundary_id().as_u32()
                )));
            }
            many => {
                return Err(frontend_error(format!(
                    "class ctor boundary bd{} 发布了 {} 个 hidden effect emission；当前 hidden-init bridge 需要唯一 ordinary effect case",
                    boundary.boundary_id().as_u32(),
                    many.len()
                )));
            }
        };

        let active_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "class_ctor_hidden_effect_active");
        let inactive_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "class_ctor_hidden_effect_inactive");
        let dispatch_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "class_ctor_hidden_effect_dispatch");
        let complete_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "class_ctor_hidden_effect_complete");
        let case_bb = self.codegen.context.append_basic_block(
            self.function,
            &format!(
                "class_ctor_hidden_effect_case{}",
                emission.case_tag().as_u32()
            ),
        );
        let unmatched_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "class_ctor_hidden_effect_unmatched");

        let is_propagating = self.codegen.effect_outcome_is_propagating(
            span,
            outcome_slot,
            "class_ctor_hidden_effect",
        )?;
        self.codegen
            .builder
            .build_conditional_branch(is_propagating, active_bb, inactive_bb)?;

        self.codegen.builder.position_at_end(active_bb);
        self.codegen
            .builder
            .build_unconditional_branch(dispatch_bb)?;
        let active_end = self.codegen.builder.get_insert_block().ok_or_else(|| {
            frontend_error("hidden-init active branch 缺少 insert block".to_string())
        })?;

        self.codegen.builder.position_at_end(inactive_bb);
        self.codegen
            .builder
            .build_unconditional_branch(dispatch_bb)?;
        let inactive_end = self.codegen.builder.get_insert_block().ok_or_else(|| {
            frontend_error("hidden-init inactive branch 缺少 insert block".to_string())
        })?;

        self.codegen.builder.position_at_end(dispatch_bb);
        let complete_tag = self.codegen.context.i32_type().const_zero();
        let outward_tag = self.codegen.context.i32_type().const_int(
            u64::from(emission.case_tag().as_u32().saturating_add(1)),
            false,
        );
        let step_tag = self
            .codegen
            .builder
            .build_phi(self.codegen.context.i32_type(), "step_tag")?;
        step_tag.add_incoming(&[(&outward_tag, active_end), (&complete_tag, inactive_end)]);
        let step_tag = step_tag.as_basic_value().into_int_value();
        self.codegen.builder.build_switch(
            step_tag,
            unmatched_bb,
            &[(complete_tag, complete_bb), (outward_tag, case_bb)],
        )?;

        self.codegen.builder.position_at_end(complete_bb);
        let result = load_result(self)?;
        let _ = self.store_local_value(span, lowering.result_local(), result)?;
        self.branch_to_state(boundary.resume_state())?;

        self.codegen.builder.position_at_end(case_bb);
        let payload =
            self.lower_class_ctor_outcome_payload(outcome_slot, emission.payload_tuple_ty())?;
        let cleared_outcome = self.codegen.build_zero_complete_effect_outcome()?;
        self.codegen
            .builder
            .build_store(outcome_slot, cleared_outcome)?;
        self.emit_or_consume_outward_case(
            boundary,
            emission.case_tag(),
            payload,
            emission.payload_tuple_ty(),
            None,
            None,
        )?;

        self.codegen.builder.position_at_end(unmatched_bb);
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    pub(super) fn class_ctor_layout_key(
        &self,
        span: crate::span::Span,
        class_fqn: &str,
        result_local: LocalId,
    ) -> Result<crate::effect_lowered::source::ClassInstanceKey, LlvmEmitError> {
        let Some(target_ty) = self
            .lir_body
            .locals()
            .get(result_local.as_u32() as usize)
            .map(|local| local.ty())
        else {
            return Err(frontend_error(format!(
                "class ctor boundary `{class_fqn}` at {span:?} result local{} missing typed nominal result",
                result_local.as_u32()
            )));
        };
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.source_types.kind(target_ty) else {
            return Err(frontend_error(format!(
                "class ctor boundary `{class_fqn}` at {span:?} result local{} has non-nominal result type t{}",
                result_local.as_u32(),
                target_ty.as_u32()
            )));
        };
        if nominal.fqn != class_fqn {
            return Err(frontend_error(format!(
                "class ctor boundary `{class_fqn}` at {span:?} result local{} has mismatched nominal `{}`",
                result_local.as_u32(),
                nominal.fqn
            )));
        }
        let layout = self.abi.class_instance_layout(target_ty)?;
        if layout.base_fqn() != class_fqn {
            return Err(frontend_error(format!(
                "class ctor boundary `{class_fqn}` result local{} resolved to mismatched layout `{}`",
                result_local.as_u32(),
                layout.base_fqn()
            )));
        }
        Ok(layout.class_key().clone())
    }

    pub(super) fn class_ctor_boundary_statement(
        &self,
        boundary: &LateLoweredBoundary,
        lowering: &crate::effect_lowered::ir::LateLoweredClassCtorBoundaryLowering,
        site_id: SiteId,
    ) -> Result<ClassCtorBoundarySource<'a>, LlvmEmitError> {
        let Some(statement_index) = lowering.source_consumption().statement_index() else {
            return Err(frontend_error(format!(
                "class ctor boundary site{} source consumption 不是 statement anchor",
                site_id.as_u32()
            )));
        };
        let source_slice = lowering.source_consumption().source_slice();
        let (stmt, _) = self.lir_statement_for_source_position(
            boundary.owner_state(),
            source_slice,
            statement_index,
            "class ctor boundary",
        )?;
        match &stmt.kind {
            LirStatementKind::Assign {
                value:
                    LirRvalue::ClassCtor {
                        site_id: stmt_site,
                        ctor,
                        args,
                        ..
                    },
                ..
            } if *stmt_site == site_id => Ok(ClassCtorBoundarySource::ClassCtor {
                span: stmt.span,
                ctor,
                args,
            }),
            LirStatementKind::Assign {
                value: LirRvalue::TopLevelRef(top_level),
                ..
            } if top_level.site_id == Some(site_id) && !top_level.hidden_effects.is_pure() => {
                match &top_level.target {
                    LirTopLevelRefTarget::Global(root) => {
                        Ok(ClassCtorBoundarySource::TopLevelRef {
                            span: stmt.span,
                            fqn: root.as_str(),
                        })
                    }
                    LirTopLevelRefTarget::Callable(_) => Err(frontend_error(format!(
                        "class ctor boundary site{} hidden top-level source 是 callable ref，不是 global init",
                        site_id.as_u32()
                    ))),
                }
            }
            LirStatementKind::Assign {
                value:
                    LirRvalue::MemberAccess {
                        site_id: Some(stmt_site),
                        member,
                        ..
                    },
                ..
            } if *stmt_site == site_id && !member.hidden_effects.is_pure() => {
                let LirMemberTarget::Value { member } = &member.resolved else {
                    return Err(frontend_error(format!(
                        "class ctor boundary site{} hidden member source 不是 resolved value member",
                        site_id.as_u32()
                    )));
                };
                Ok(ClassCtorBoundarySource::ObjectProperty {
                    span: stmt.span,
                    fqn: member.as_str(),
                })
            }
            _ => Err(frontend_error(format!(
                "class ctor boundary site{} source anchor 不是 ClassCtor/hidden member statement",
                site_id.as_u32()
            ))),
        }
    }

    pub(super) fn lower_class_ctor_outcome_payload(
        &mut self,
        outcome_slot: PointerValue<'ctx>,
        payload_ty: TypeId,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let layout = self.abi.source_value_layout(payload_ty)?;
        if layout.abi().is_elided() {
            return Ok(None);
        }
        let payload_cg = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, payload_ty)
            .ok_or_else(|| {
                frontend_error(format!(
                    "class ctor hidden effect payload t{} 缺少 codegen type",
                    payload_ty.as_u32()
                ))
            })?;
        let transport = self.codegen.effect_outcome_payload_transport(
            self.mir_fun.span,
            outcome_slot,
            "class_ctor_hidden_effect_payload",
        )?;
        let decoded = self.codegen.decode_effect_transport_value_as(
            self.mir_fun.span,
            Some(payload_ty),
            transport.word,
            transport.gc_ref,
            payload_cg,
        )?;
        decoded
            .value
            .ok_or_else(|| {
                frontend_error(format!(
                    "class ctor hidden effect payload t{} decoded to elided value despite non-elided ABI",
                    payload_ty.as_u32()
                ))
            })
            .map(Some)
    }

    pub(super) fn should_use_task_transport_dynamic_resume(
        &mut self,
        site_id: SiteId,
        surface: &ContinuationSurfaceResumeLayout<'ctx>,
        lowering: &crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering,
    ) -> Result<bool, LlvmEmitError> {
        // These continuations are stored in heap state and later resumed from helper paths, so
        // their concrete owner route is recovered from the continuation object descriptor.
        if !self.is_task_transport_tuple_ty(surface.resume_tuple_ty())? {
            return Ok(false);
        }
        let route = lowering.operand_contract().underlying_continuation_route();
        Ok(matches!(
            route.publication(),
            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                owner_version_key,
                site_id: route_site,
                ..
            } if owner_version_key == self.callable.body_version_key() && *route_site == site_id
        ))
    }

    pub(super) fn lower_task_transport_dynamic_resume_boundary(
        &mut self,
        boundary: &LateLoweredBoundary,
        lowering: &crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering,
        surface: &ContinuationSurfaceResumeLayout<'ctx>,
        cont_ptr: PointerValue<'ctx>,
        args_payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<bool, LlvmEmitError> {
        let payload = args_payload.ok_or_else(|| {
            frontend_error(format!(
                "task transport resume bd{} 需要 non-elided payload",
                boundary.boundary_id().as_u32()
            ))
        })?;
        let candidates =
            self.task_transport_resume_candidates(lowering, surface.resume_tuple_ty())?;
        if candidates.is_empty() {
            return Ok(false);
        }

        let current_desc = self.load_gc_object_type_desc(cont_ptr, "task_resume_cont_desc")?;
        let word_ty = self.codegen.context.i64_type();
        let current_desc_int = self.codegen.builder.build_ptr_to_int(
            current_desc,
            word_ty,
            "task_resume_cont_desc_int",
        )?;
        let first_check = self
            .codegen
            .context
            .append_basic_block(self.function, "task_resume_check0");
        self.codegen
            .builder
            .build_unconditional_branch(first_check)?;

        let mut check_bb = first_check;
        for (index, candidate) in candidates.into_iter().enumerate() {
            let next_bb = self
                .codegen
                .context
                .append_basic_block(self.function, &format!("task_resume_check{}", index + 1));
            let hit_bb = self.codegen.context.append_basic_block(
                self.function,
                &format!(
                    "task_resume_hit_s{}",
                    candidate.callable.step_schema().as_u32()
                ),
            );
            self.codegen.builder.position_at_end(check_bb);
            let target_desc_int = self.codegen.builder.build_ptr_to_int(
                candidate.type_desc_i8,
                word_ty,
                "task_resume_target_desc_int",
            )?;
            let is_match = self.codegen.builder.build_int_compare(
                inkwell::IntPredicate::EQ,
                current_desc_int,
                target_desc_int,
                "task_resume_desc_match",
            )?;
            self.codegen
                .builder
                .build_conditional_branch(is_match, hit_bb, next_bb)?;

            self.codegen.builder.position_at_end(hit_bb);
            let args = vec![cont_ptr.into(), payload.into()];
            let call = self.codegen.build_call_preserving_gc_local_roots(
                self.mir_fun.span,
                candidate.adapter,
                &args,
                "task_transport_resume",
            )?;
            let owner_step = call.try_as_basic_value().basic().ok_or_else(|| {
                frontend_error(format!(
                    "task transport resume adapter `{}` 未返回 Step_F",
                    candidate.adapter.get_name().to_str().unwrap_or("<invalid>")
                ))
            })?;
            self.dispatch_boundary_step(
                boundary,
                candidate.callable.step_schema(),
                owner_step,
                &candidate.dispatch_plan,
                None,
                None,
            )?;
            check_bb = next_bb;
        }

        self.codegen.builder.position_at_end(check_bb);
        self.codegen.builder.build_unreachable()?;
        Ok(true)
    }

    pub(super) fn task_transport_resume_candidates(
        &mut self,
        lowering: &crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering,
        transport_ty: TypeId,
    ) -> Result<Vec<TaskTransportResumeCandidate<'a, 'ctx>>, LlvmEmitError> {
        let mut candidates = Vec::new();
        for callable in self.program.callables() {
            if !callable.has_control_body()
                || callable.frame_schema().resume_payload_bindings().is_empty()
            {
                continue;
            }
            let Some(dispatch_plan) = self
                .task_transport_owner_dispatch_plan(callable.step_schema(), lowering.dispatch())?
            else {
                continue;
            };
            if !self.callable_accepts_task_transport_resume(callable, transport_ty)? {
                continue;
            }
            let continuation_layout = self
                .abi
                .continuation_layout(callable.continuation_object())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "task transport resume 缺少 callable `{}` continuation layout",
                        callable.root_fqn()
                    ))
                })?;
            let type_desc = self.codegen.get_or_create_gc_type_descriptor(
                self.mir_fun.span,
                continuation_layout.llvm_ty(),
                continuation_layout.layout_anchor_name(),
            )?;
            let type_desc_i8 = self.codegen.builder.build_pointer_cast(
                type_desc.as_pointer_value(),
                self.codegen.llvm_i8_ptr_type(),
                "task_resume_candidate_type_desc",
            )?;
            let adapter = self.ensure_task_transport_resume_adapter(callable, transport_ty)?;
            candidates.push(TaskTransportResumeCandidate {
                callable,
                adapter,
                type_desc_i8,
                dispatch_plan,
            });
        }
        Ok(candidates)
    }

    pub(super) fn callable_accepts_task_transport_resume(
        &mut self,
        callable: &LateLoweredCallable,
        transport_ty: TypeId,
    ) -> Result<bool, LlvmEmitError> {
        for binding in callable.frame_schema().resume_payload_bindings() {
            let Some(body) = callable.executable_body() else {
                continue;
            };
            let local_ty = body.locals()[binding.consumer_local().as_u32() as usize].ty();
            if local_ty != transport_ty || self.is_task_transport_tuple_ty(local_ty)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn task_transport_owner_dispatch_plan(
        &self,
        owner_step_schema: StepSchemaId,
        wrapper_dispatch: &LateLoweredStepDispatchPlan,
    ) -> Result<Option<LateLoweredStepDispatchPlan>, LlvmEmitError> {
        let owner_step = self.program.step_type(owner_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "task transport resume 缺少 owner step schema s{}",
                owner_step_schema.as_u32()
            ))
        })?;
        if owner_step.complete_ty() != wrapper_dispatch.complete().answer_ty() {
            return Ok(None);
        }
        let wrapper_step = self
            .program
            .step_type(wrapper_dispatch.input_step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "task transport resume 缺少 wrapper step schema s{}",
                    wrapper_dispatch.input_step_schema().as_u32()
                ))
            })?;
        let mut outward_cases = Vec::new();
        for wrapper_forwarding in wrapper_dispatch.outward_cases() {
            let Some(wrapper_case) = wrapper_step.case(wrapper_forwarding.input_case_tag()) else {
                return Ok(None);
            };
            let Some(owner_case) = owner_step.cases().iter().find(|candidate| {
                candidate.concrete_op_key() == wrapper_case.concrete_op_key()
                    && candidate.payload_tuple_ty() == wrapper_case.payload_tuple_ty()
            }) else {
                return Ok(None);
            };
            outward_cases.push(LateLoweredStepCaseForwarding::new(
                owner_case.case_tag(),
                owner_case.concrete_op_key().clone(),
                wrapper_forwarding.emission().clone(),
            ));
        }
        Ok(Some(LateLoweredStepDispatchPlan::new(
            owner_step_schema,
            wrapper_dispatch.complete().clone(),
            outward_cases,
        )))
    }

    pub(super) fn ensure_task_transport_resume_adapter(
        &mut self,
        callable: &'a LateLoweredCallable,
        transport_ty: TypeId,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let step_layout = self
            .abi
            .step_layout(callable.step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "task transport resume 缺少 callable `{}` step layout s{}",
                    callable.root_fqn(),
                    callable.step_schema().as_u32()
                ))
            })?;
        let payload_layout = self.abi.source_value_layout(transport_ty)?;
        let payload_abi = *payload_layout.abi();
        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> =
            vec![self.codegen.llvm_gc_i8_ptr_type().into()];
        if !payload_abi.is_elided() {
            params.push(payload_abi.llvm_ty().into());
        }
        let fn_ty = step_layout.llvm_ty().fn_type(&params, false);
        let transport_type_key = canonical_type_text(
            self.source_types,
            transport_ty,
            self.codegen.stable_type_param_resolver(),
        )
        .map_err(|err| {
            frontend_error(format!(
                "task transport resume `{}` 计算 canonical type text 失败（t{}）: {err}",
                callable.root_fqn(),
                transport_ty.as_u32()
            ))
        })?;
        let symbol_name = stable_naming::private_name_from_key_text(
            "task_transport_resume",
            &canonical_record(
                "task_transport_resume",
                [
                    step_layout.stable_effect_key_text().to_string(),
                    transport_type_key,
                ],
            ),
        );
        let function = self.codegen.declare_compiler_private_helper_function(
            &symbol_name,
            fn_ty,
            Linkage::Internal,
        );
        if function.count_basic_blocks() > 0 {
            return Ok(function);
        }

        let saved_block = self.codegen.builder.get_insert_block();
        let mut child = self.codegen.fresh_child_codegen();
        let (mir_fun, _body) = callable_source_body(callable, "task transport resume adapter")?;
        let entry = child.context.append_basic_block(function, "entry");
        child.builder.position_at_end(entry);
        child.begin_function_explicit_frame_layout(function)?;
        CallableEmitter::new(
            &mut child,
            self.program,
            self.source_types,
            self.abi,
            callable,
            mir_fun,
            function,
            None,
            None,
            None,
            HandleCompletionMode::ReturnFromFunction,
        )?
        .emit_resume_entry(transport_ty)?;
        child.finish_function_explicit_frame_layout(mir_fun.span)?;
        if let Some(block) = saved_block {
            self.codegen.builder.position_at_end(block);
        }
        Ok(function)
    }
}
