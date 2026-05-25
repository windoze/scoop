//! State-machine layouts: per-step types, resume-packing layouts, surface
//! resume site discovery, and per-callable frame layouts.
//!
//! These methods materialize the ABI shape of every state in the late-lowered
//! state graph. The outputs feed the callable / continuation / surface resume
//! materializers that follow.

use super::*;

impl<'cg, 'a, 'ctx> ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    pub(super) fn materialize_step_layout(
        &mut self,
        step_type: &LateLoweredStepType,
    ) -> Result<StepLayout<'ctx>, LlvmEmitError> {
        let stable_effect_key_text = stable_naming::effect_schema_key_text(
            self.stable_cone_key,
            self.source_types,
            self.codegen.stable_type_param_resolver(),
            self.program,
            step_type,
            &format!("step schema {}", step_type.step_schema().as_u32()),
        )?;
        let step_type_name = stable_naming::private_type_name_from_key_text(
            "Step",
            "step_type",
            &stable_effect_key_text,
        )?;
        let storage_type_name = stable_naming::private_type_name_from_key_text(
            "StepStorage",
            "step_storage",
            &stable_effect_key_text,
        )?;
        let step_anchor_name =
            stable_naming::private_name_from_key_text("step_layout", &stable_effect_key_text);
        let complete_tag_name = stable_naming::private_name_from_key_text(
            "step_case_tag_complete",
            &stable_effect_key_text,
        );
        let complete_payload_name = stable_naming::private_type_name_from_key_text(
            "StepComplete",
            "step_complete_type",
            &stable_effect_key_text,
        )?;
        let complete_payload_anchor = stable_naming::private_name_from_key_text(
            "step_variant_payload_complete",
            &stable_effect_key_text,
        );

        let complete_payload_layout = self.source_value_layout(step_type.complete_ty())?;
        let complete_payload_abi = *complete_payload_layout.abi();
        let complete_fields = if complete_payload_abi.is_elided() {
            Vec::new()
        } else {
            vec![complete_payload_abi.llvm_ty()]
        };
        let complete_payload_ty =
            self.define_named_struct(&complete_payload_name, &complete_fields);
        self.ensure_struct_anchor(&complete_payload_anchor, complete_payload_ty);
        self.ensure_case_tag_constant(&complete_tag_name, 0);

        let complete_variant = StepVariantLayout::new(
            0,
            step_type.complete_ty(),
            complete_payload_ty,
            usize::from(!complete_payload_abi.is_elided()),
            complete_payload_anchor,
            complete_payload_abi.is_elided(),
        );

        let mut case_layouts = BTreeMap::new();
        let mut payload_tys = vec![complete_payload_ty];
        for case in step_type.cases() {
            let case_key_text = stable_naming::step_case_key_text(
                self.stable_cone_key,
                self.source_types,
                self.codegen.stable_type_param_resolver(),
                self.program,
                case,
                &format!(
                    "step schema {} case {}",
                    step_type.step_schema().as_u32(),
                    case.case_tag().as_u32()
                ),
            )?;
            let case_payload_name = stable_naming::private_type_name_from_key_text(
                "StepCase",
                "step_case_type",
                &case_key_text,
            )?;
            let case_payload_anchor = stable_naming::private_name_from_key_text(
                "step_variant_payload_case",
                &case_key_text,
            );
            let case_tag_name =
                stable_naming::private_name_from_key_text("step_case_tag_case", &case_key_text);
            let payload_layout = self.source_value_layout(case.payload_tuple_ty())?;
            let payload_abi = *payload_layout.abi();
            let mut case_fields = Vec::new();
            if !payload_abi.is_elided() {
                case_fields.push(payload_abi.llvm_ty());
            }
            case_fields.push(self.codegen.llvm_gc_i8_ptr_type().into());
            let case_payload_ty = self.define_named_struct(&case_payload_name, &case_fields);
            self.ensure_struct_anchor(&case_payload_anchor, case_payload_ty);
            let tag_value = case.case_tag().as_u32().saturating_add(1);
            self.ensure_case_tag_constant(&case_tag_name, tag_value);

            payload_tys.push(case_payload_ty);
            case_layouts.insert(
                case.case_tag(),
                StepCaseLayout::new(
                    case.case_tag(),
                    case.concrete_op_key().clone(),
                    case.payload_tuple_ty(),
                    case_tag_name,
                    StepVariantLayout::new(
                        tag_value,
                        case.payload_tuple_ty(),
                        case_payload_ty,
                        case_fields.len(),
                        case_payload_anchor,
                        payload_abi.is_elided(),
                    ),
                ),
            );
        }

        let storage_ty = self.define_union_storage_type(&storage_type_name, &payload_tys);
        let step_ty = self.define_named_struct(
            &step_type_name,
            &[self.codegen.context.i32_type().into(), storage_ty.into()],
        );
        self.ensure_struct_anchor(&step_anchor_name, step_ty);

        Ok(StepLayout::new(
            step_type.step_schema(),
            stable_effect_key_text,
            step_ty,
            step_anchor_name,
            complete_tag_name,
            complete_variant,
            case_layouts,
        ))
    }

    pub(super) fn materialize_resume_packing_layout(
        &mut self,
        interface: &LateLoweredResumeInterface,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
    ) -> Result<ResumeInterfaceLayout<'ctx>, LlvmEmitError> {
        let step_schema = interface.return_step_schema();
        let return_step_ty = step_layouts
            .get(&step_schema)
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 resume packing {} 的 return step schema {}",
                    interface.interface_id().as_u32(),
                    step_schema.as_u32()
                ))
            })?
            .llvm_ty();
        let step_type = self.program.step_type(step_schema).ok_or_else(|| {
            frontend_error(format!(
                "LLVM ABI materialization 缺少 resume packing {} 的 step type {}",
                interface.interface_id().as_u32(),
                step_schema.as_u32()
            ))
        })?;
        let step_layout = step_layouts.get(&step_schema).ok_or_else(|| {
            frontend_error(format!(
                "LLVM ABI materialization 缺少 resume packing {} 的 step layout {}",
                interface.interface_id().as_u32(),
                step_schema.as_u32()
            ))
        })?;
        let vtable_key_text = canonical_record(
            "resume_vtable",
            [
                step_layout.stable_effect_key_text().to_string(),
                interface.effect_family().effect_fqn().to_string(),
            ],
        );
        let vtable_type_name = stable_naming::private_type_name_from_key_text(
            "ResumeVtable",
            "resume_vtable_type",
            &vtable_key_text,
        )?;
        let vtable_anchor_name =
            stable_naming::private_name_from_key_text("resume_vtable_layout", &vtable_key_text);
        let expected_case_tags = step_type
            .cases()
            .iter()
            .filter(|case| case.concrete_op_key().effect_family() == interface.effect_family())
            .map(|case| case.case_tag())
            .collect::<BTreeSet<_>>();

        let mut methods = BTreeMap::new();
        let mut vtable_fields = Vec::new();
        let mut published_case_tags = BTreeSet::new();
        for (index, method) in interface.methods().iter().enumerate() {
            let step_case = step_type.case(method.case_tag()).ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 发现 resume packing {} method case {} 在 step schema {} 中不存在",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32(),
                    step_schema.as_u32()
                ))
            })?;
            if !published_case_tags.insert(method.case_tag()) {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 resume packing {} 重复发布 case {}",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32()
                )));
            }
            if method.concrete_op_key().effect_family() != interface.effect_family() {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 resume packing {} method case {} 的 effect family `{}` 与 packing family `{}` 不一致",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32(),
                    method.concrete_op_key().effect_family().effect_fqn(),
                    interface.effect_family().effect_fqn()
                )));
            }
            if step_case.concrete_op_key() != method.concrete_op_key() {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 resume packing {} method case {} 的 concrete op `{}` 与 step shell 发布 `{}` 不一致",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32(),
                    method.concrete_op_key().instance_key().template.fqn,
                    step_case.concrete_op_key().instance_key().template.fqn,
                )));
            }
            if step_case.concrete_op_key().effect_family() != interface.effect_family() {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 resume packing {} method case {} 指向的 step case family `{}` 与 packing family `{}` 不一致",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32(),
                    step_case.concrete_op_key().effect_family().effect_fqn(),
                    interface.effect_family().effect_fqn()
                )));
            }
            if method.out_step_schema() != step_schema {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 resume packing {} method case {} 的 out step schema {} 与 packing return step schema {} 不一致",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32(),
                    method.out_step_schema().as_u32(),
                    step_schema.as_u32()
                )));
            }
            if step_case.continuation_contract() != method.continuation_contract() {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 resume packing {} method case {} 的 continuation contract 与 step shell 不一致",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32()
                )));
            }

            let step_case = step_type.case(method.case_tag()).ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 resume packing {} method case {} 的 authoritative step case",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32(),
                ))
            })?;
            let symbol_name = stable_naming::private_name_from_key_text(
                "resume",
                &canonical_record(
                    "resume_method",
                    [
                        step_layout.stable_effect_key_text().to_string(),
                        stable_naming::step_case_key_text(
                            self.stable_cone_key,
                            self.source_types,
                            self.codegen.stable_type_param_resolver(),
                            self.program,
                            step_case,
                            &format!(
                                "resume packing {} method case {}",
                                interface.interface_id().as_u32(),
                                method.case_tag().as_u32()
                            ),
                        )?,
                    ],
                ),
            );
            let payload_abi = self.resume_surface_abi_value(method.resume_tuple_ty())?;
            let _answer_abi = self.resume_surface_abi_value(method.answer_ty())?;
            let mut params: Vec<BasicMetadataTypeEnum<'ctx>> =
                vec![self.codegen.llvm_gc_i8_ptr_type().into()];
            if !payload_abi.is_elided() {
                params.push(payload_abi.llvm_ty().into());
            }
            let fn_ty = return_step_ty.fn_type(&params, false);
            self.ensure_declared_compiler_private_helper_function(&symbol_name, fn_ty);
            vtable_fields.push(self.codegen.llvm_i8_ptr_type().into());
            methods.insert(
                method.case_tag(),
                ResumeMethodLayout::new(
                    interface.interface_id(),
                    method.case_tag(),
                    symbol_name,
                    fn_ty,
                    params.len(),
                    index as u32,
                    payload_abi,
                    step_schema,
                ),
            );
        }
        let missing_case_tags = expected_case_tags
            .difference(&published_case_tags)
            .map(|case_tag| case_tag.as_u32().to_string())
            .collect::<Vec<_>>();
        if !missing_case_tags.is_empty() {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 resume packing {} 在 step schema {} 的 effect family `{}` 下缺少 authoritative method cases [{}]",
                interface.interface_id().as_u32(),
                step_schema.as_u32(),
                interface.effect_family().effect_fqn(),
                missing_case_tags.join(", ")
            )));
        }

        let vtable_ty = self.define_named_struct(&vtable_type_name, &vtable_fields);
        self.ensure_struct_anchor(&vtable_anchor_name, vtable_ty);
        Ok(ResumeInterfaceLayout::new(
            interface.interface_id(),
            interface.effect_family().effect_fqn().to_string(),
            vtable_ty,
            vtable_anchor_name,
            methods,
        ))
    }

    pub(super) fn materialize_surface_resume_layouts(
        &mut self,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
    ) -> Result<BTreeMap<ContinuationSchemaId, ContinuationSurfaceResumeLayout<'ctx>>, LlvmEmitError>
    {
        let mut layouts = BTreeMap::new();
        for entry in self.program.surface_resume_dispatch_inventory() {
            let contract = entry.contract();
            self.register_surface_resume_layout(
                &mut layouts,
                entry.continuation_schema(),
                entry.source_kind(),
                contract.resume_tuple_ty(),
                contract.answer_ty(),
                contract.out_step_schema(),
                &format!(
                    "surface-resume dispatch inventory k{}",
                    entry.continuation_schema().as_u32()
                ),
                step_layouts,
            )?;
        }
        for callable in self.program.callables() {
            if !callable.has_control_body() {
                continue;
            }
            for boundary in callable.boundary_map().entries() {
                let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering() else {
                    continue;
                };
                for composition in lowering.continuation_compositions() {
                    let contract = composition.callee_continuation_contract();
                    self.register_surface_resume_layout(
                        &mut layouts,
                        composition.callee_continuation_schema(),
                        crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::Unreachable,
                        contract.resume_tuple_ty(),
                        contract.answer_ty(),
                        contract.out_step_schema(),
                        &format!(
                            "call-boundary callee continuation composition k{}",
                            composition.callee_continuation_schema().as_u32()
                        ),
                        step_layouts,
                    )?;
                }
            }
        }
        Ok(layouts)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn register_surface_resume_layout(
        &mut self,
        layouts: &mut BTreeMap<ContinuationSchemaId, ContinuationSurfaceResumeLayout<'ctx>>,
        continuation_schema: ContinuationSchemaId,
        dispatch_source_kind: crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind,
        resume_tuple_ty: crate::ty::TypeId,
        answer_ty: crate::ty::TypeId,
        return_step_schema: StepSchemaId,
        source_label: &str,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        let layout = self.materialize_surface_resume_layout(
            continuation_schema,
            dispatch_source_kind,
            resume_tuple_ty,
            answer_ty,
            return_step_schema,
            step_layouts,
        )?;
        match layouts.entry(continuation_schema) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(layout);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                let existing = entry.get();
                if existing.resume_tuple_ty() != layout.resume_tuple_ty()
                    || existing.answer_ty() != layout.answer_ty()
                    || existing.return_step_schema() != layout.return_step_schema()
                    || existing.param_count() != layout.param_count()
                    || existing.resume_payload_abi().is_elided()
                        != layout.resume_payload_abi().is_elided()
                {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation schema k{} 的 surface-resume contract 漂移：已发布为 resume_tuple_ty={} answer_ty={} out_step_schema={}，但 {source_label} 重新发布为 resume_tuple_ty={} answer_ty={} out_step_schema={}",
                        continuation_schema.as_u32(),
                        existing.resume_tuple_ty().as_u32(),
                        existing.answer_ty().as_u32(),
                        existing.return_step_schema().as_u32(),
                        layout.resume_tuple_ty().as_u32(),
                        layout.answer_ty().as_u32(),
                        layout.return_step_schema().as_u32(),
                    )));
                }
            }
        }
        Ok(())
    }

    pub(super) fn materialize_surface_resume_layout(
        &mut self,
        continuation_schema: ContinuationSchemaId,
        dispatch_source_kind: crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind,
        resume_tuple_ty: crate::ty::TypeId,
        answer_ty: crate::ty::TypeId,
        step_schema: StepSchemaId,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
    ) -> Result<ContinuationSurfaceResumeLayout<'ctx>, LlvmEmitError> {
        let return_step_ty = step_layouts
            .get(&step_schema)
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 continuation schema k{} 的 surface-resume return step schema {}",
                    continuation_schema.as_u32(),
                    step_schema.as_u32()
                ))
            })?
            .llvm_ty();
        let surface_resume_contract = LateLoweredSurfaceResumeContract::new(
            continuation_schema,
            resume_tuple_ty,
            answer_ty,
            step_schema,
        );
        let stable_continuation_key_text = stable_naming::surface_resume_contract_key_text(
            self.stable_cone_key,
            self.source_types,
            self.codegen.stable_type_param_resolver(),
            self.program,
            surface_resume_contract,
            &format!("continuation schema {}", continuation_schema.as_u32()),
        )?;
        let symbol_name = stable_naming::private_name_from_key_text(
            "surface_resume",
            &stable_continuation_key_text,
        );
        let payload_abi = self.resume_surface_abi_value(resume_tuple_ty)?;
        let _answer_abi = self.resume_surface_abi_value(answer_ty)?;
        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> =
            vec![self.codegen.llvm_gc_i8_ptr_type().into()];
        if !payload_abi.is_elided() {
            params.push(payload_abi.llvm_ty().into());
        }
        let fn_ty = return_step_ty.fn_type(&params, false);
        self.ensure_declared_compiler_private_helper_function(&symbol_name, fn_ty);
        Ok(ContinuationSurfaceResumeLayout::new(
            continuation_schema,
            dispatch_source_kind,
            stable_continuation_key_text,
            symbol_name,
            fn_ty,
            params.len(),
            resume_tuple_ty,
            answer_ty,
            payload_abi,
            step_schema,
        ))
    }

    pub(super) fn validate_resume_site_surface_contracts(
        &self,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<(), LlvmEmitError> {
        for callable in self.program.callables() {
            if !callable.has_control_body() {
                continue;
            }
            for boundary in callable.boundary_map().entries() {
                let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering()
                else {
                    continue;
                };
                let site_id = match boundary.source() {
                    LateLoweredBoundarySource::Site {
                        site_id,
                        kind: BoundarySiteKind::Resume,
                    } => site_id,
                    other => {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` 的 resume lowering 绑定到了非 Resume boundary source {other:?}",
                            callable.root_fqn(),
                        )));
                    }
                };
                let facts = lowering.facts();
                let layout = surface_resume_layouts.get(&facts.continuation_schema()).ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 缺少 callable `{}` resume site {} 所需的 continuation schema k{} surface-resume layout",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        facts.continuation_schema().as_u32(),
                    ))
                })?;
                if layout.resume_tuple_ty() != facts.resume_tuple_ty()
                    || layout.answer_ty() != facts.answer_ty()
                    || layout.return_step_schema() != facts.out_step_schema()
                {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` resume site {} 的 continuation schema k{} surface-resume contract 与 ResumeSiteEffectFacts 漂移：layout=(resume_tuple_ty={}, answer_ty={}, out_step_schema={})，facts=(resume_tuple_ty={}, answer_ty={}, out_step_schema={})",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        facts.continuation_schema().as_u32(),
                        layout.resume_tuple_ty().as_u32(),
                        layout.answer_ty().as_u32(),
                        layout.return_step_schema().as_u32(),
                        facts.resume_tuple_ty().as_u32(),
                        facts.answer_ty().as_u32(),
                        facts.out_step_schema().as_u32(),
                    )));
                }
                if matches!(
                    layout.dispatch_source_kind(),
                    crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
                        | crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::Unreachable
                ) {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` resume site {} 的 continuation schema k{} dispatch source kind 为 {:?}，无法作为 authoritative resume-site surface source",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        facts.continuation_schema().as_u32(),
                        layout.dispatch_source_kind(),
                    )));
                }
            }
        }
        Ok(())
    }

    pub(super) fn materialize_frame_layout(
        &mut self,
        callable: &LateLoweredCallable,
    ) -> Result<FrameLayout<'ctx>, LlvmEmitError> {
        let stable_callable_key_text = stable_naming::callable_version_key_text(
            self.stable_cone_key,
            self.source_types,
            self.codegen.stable_type_param_resolver(),
            self.program,
            callable.body_version_key(),
            &format!("frame callable `{}`", callable.root_fqn()),
        )?;
        let frame_type_name = stable_naming::private_type_name_from_key_text(
            "Frame",
            "frame_type",
            &stable_callable_key_text,
        )?;
        let frame_anchor_name =
            stable_naming::private_name_from_key_text("frame_layout", &stable_callable_key_text);
        let header_ty = self.codegen.llvm_gc_object_header_type();
        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = vec![header_ty.into()];
        let mut fields = vec![FrameFieldLayout::new(
            0,
            FrameFieldKind::Header,
            header_ty.into(),
        )];
        let mut slot_field_indices = BTreeMap::new();
        let mut system_field_indices = BTreeMap::new();

        for slot in callable.frame_schema().slots() {
            let field_index = llvm_fields.len() as u32;
            let slot_abi = match slot.kind() {
                LateLoweredFrameSlotKind::ResumePayload { .. } => {
                    self.resume_surface_abi_value(slot.ty())?
                }
                _ => self.abi_value(slot.ty())?,
            };
            llvm_fields.push(slot_abi.llvm_ty());
            fields.push(FrameFieldLayout::new(
                field_index,
                FrameFieldKind::Slot(slot.slot_id()),
                slot_abi.llvm_ty(),
            ));
            slot_field_indices.insert(slot.slot_id(), field_index);
            if let LateLoweredFrameSlotKind::System(kind) = slot.kind() {
                system_field_indices.insert(kind, field_index);
            }
        }

        let frame_ty = self.define_named_struct(&frame_type_name, &llvm_fields);
        self.ensure_struct_anchor(&frame_anchor_name, frame_ty);
        Ok(FrameLayout::new(
            callable.step_schema(),
            frame_ty,
            frame_anchor_name,
            fields,
            slot_field_indices,
            system_field_indices,
        ))
    }
}
