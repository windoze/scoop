//! Boundary operand layouts and contract validation.
//!
//! Every boundary (call / perform / resume / handle / runtime-error) carries
//! an operand contract that names a frame layout slot for each value flowing
//! into or out of the boundary. This module materializes those operand
//! layouts and validates that the published source-slice consumption agrees
//! with the contract.

use super::carrier::expected_source_types_for_carrier;
use super::*;

fn boundary_source_consumption(
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
) -> Option<LateLoweredBoundarySourceConsumption> {
    match boundary.lowering()? {
        LateLoweredBoundaryLowering::Call(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::ClassCtor(lowering) => Some(lowering.source_consumption()),
        LateLoweredBoundaryLowering::Perform(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::Resume(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::RuntimeError(_) | LateLoweredBoundaryLowering::Handle(_) => {
            None
        }
    }
}

impl<'cg, 'a, 'ctx> ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    pub(super) fn materialize_boundary_operand_layouts(
        &mut self,
        dynamic_invoke_layouts: &BTreeMap<
            (StepSchemaId, crate::mir::SiteId),
            DynamicInvokeLayout<'ctx>,
        >,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeLayout<'ctx>,
        >,
        surface_resume_dispatch_layouts: &BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeDispatchLayout<'ctx>,
        >,
    ) -> Result<BoundaryOperandLayoutSets, LlvmEmitError> {
        let mut call_layouts = BTreeMap::new();
        let mut perform_layouts = BTreeMap::new();
        let mut resume_layouts = BTreeMap::new();

        for callable in self.program.callables() {
            if !callable.has_control_body() {
                continue;
            }
            for boundary in callable.boundary_map().entries() {
                match (boundary.source(), boundary.lowering()) {
                    (
                        LateLoweredBoundarySource::Site {
                            site_id,
                            kind: BoundarySiteKind::Call,
                        },
                        Some(LateLoweredBoundaryLowering::Call(lowering)),
                    ) => {
                        self.validate_call_boundary_operand_contract(
                            callable,
                            boundary,
                            site_id,
                            lowering,
                            dynamic_invoke_layouts,
                            surface_resume_layouts,
                        )?;
                        let key = (callable.step_schema(), site_id);
                        if call_layouts.contains_key(&key) {
                            return Err(frontend_error(format!(
                                "LLVM ABI materialization 发现 owner step schema {} call site {} 的 boundary operand contract 重复发布",
                                callable.step_schema().as_u32(),
                                site_id.as_u32(),
                            )));
                        }
                        call_layouts.insert(
                            key,
                            CallBoundaryOperandLayout::new(
                                callable.step_schema(),
                                site_id,
                                lowering.operand_contract().clone(),
                            ),
                        );
                    }
                    (
                        LateLoweredBoundarySource::Site {
                            site_id,
                            kind: BoundarySiteKind::Perform,
                        },
                        Some(LateLoweredBoundaryLowering::Perform(lowering)),
                    ) => {
                        self.validate_perform_boundary_operand_contract(
                            callable, boundary, site_id, lowering,
                        )?;
                        let key = (callable.step_schema(), site_id);
                        if perform_layouts.contains_key(&key) {
                            return Err(frontend_error(format!(
                                "LLVM ABI materialization 发现 owner step schema {} perform site {} 的 boundary operand contract 重复发布",
                                callable.step_schema().as_u32(),
                                site_id.as_u32(),
                            )));
                        }
                        perform_layouts.insert(
                            key,
                            PerformBoundaryOperandLayout::new(
                                callable.step_schema(),
                                site_id,
                                lowering.operand_contract().clone(),
                            ),
                        );
                    }
                    (
                        LateLoweredBoundarySource::Site {
                            site_id,
                            kind: BoundarySiteKind::Resume,
                        },
                        Some(LateLoweredBoundaryLowering::Resume(lowering)),
                    ) => {
                        self.validate_resume_boundary_operand_contract(
                            callable,
                            boundary,
                            site_id,
                            lowering,
                            surface_resume_layouts,
                            surface_resume_dispatch_layouts,
                        )?;
                        let key = (callable.step_schema(), site_id);
                        if resume_layouts.contains_key(&key) {
                            return Err(frontend_error(format!(
                                "LLVM ABI materialization 发现 owner step schema {} resume site {} 的 boundary operand contract 重复发布",
                                callable.step_schema().as_u32(),
                                site_id.as_u32(),
                            )));
                        }
                        resume_layouts.insert(
                            key,
                            ResumeBoundaryOperandLayout::new(
                                callable.step_schema(),
                                site_id,
                                lowering.operand_contract().clone(),
                            ),
                        );
                    }
                    _ => {}
                }
            }
        }

        Ok((call_layouts, perform_layouts, resume_layouts))
    }

    pub(super) fn validate_boundary_source_consumption(
        &self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
        kind: &'static str,
        owner_slices: &[LateLoweredStateSlice],
        consumption: LateLoweredBoundarySourceConsumption,
        expect_statement: bool,
    ) -> Result<(), LlvmEmitError> {
        let source_slice = consumption.source_slice();
        if !owner_slices.contains(&source_slice) {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 published anchor slice {:?} 不属于 owner state source_slices",
                site_id.as_u32(),
                source_slice,
            )));
        }
        match consumption {
            LateLoweredBoundarySourceConsumption::Statement {
                source_slice,
                statement_index,
                consumes_last_statement,
            } => {
                if !expect_statement {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 错误地发布了 statement anchor",
                        site_id.as_u32(),
                    )));
                }
                if statement_index < source_slice.start_statement_index()
                    || statement_index >= source_slice.end_statement_index()
                {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 statement anchor {} 越界于 published source slice {:?}",
                        site_id.as_u32(),
                        statement_index,
                        source_slice,
                    )));
                }
                let expected_last =
                    statement_index.saturating_add(1) == source_slice.end_statement_index();
                if consumes_last_statement != expected_last {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 consumes_last_statement 漂移：published={}，expected={expected_last}",
                        site_id.as_u32(),
                        consumes_last_statement,
                    )));
                }
            }
            LateLoweredBoundarySourceConsumption::Terminator { source_slice } => {
                if expect_statement {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 错误地发布了 terminator anchor",
                        site_id.as_u32(),
                    )));
                }
                if !source_slice.includes_terminator() {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 terminator anchor 所在 source slice 没有包含 terminator",
                        site_id.as_u32(),
                    )));
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_source_statement_classifications(&self) -> Result<(), LlvmEmitError> {
        for callable in self.program.callables() {
            if !callable.has_control_body() {
                continue;
            }
            let mut expected = BTreeSet::<(BasicBlockId, u32)>::new();
            for state in callable.state_graph().states() {
                for slice in state.source_slices() {
                    let start = slice.start_statement_index() as usize;
                    let end = slice.end_statement_index() as usize;
                    if start > end {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` state st{} source slice [{}..{}) 非法",
                            callable.root_fqn(),
                            state.state_id().as_u32(),
                            slice.start_statement_index(),
                            slice.end_statement_index(),
                        )));
                    }
                    for stmt_index in slice.start_statement_index()..slice.end_statement_index() {
                        expected.insert((slice.block_id(), stmt_index));
                    }
                }
            }

            let mut classified =
                BTreeMap::<(BasicBlockId, u32), LateLoweredSourceStatementClassificationKind>::new(
                );
            for classification in callable.source_statement_classifications() {
                let key = (
                    classification.source_slice().block_id(),
                    classification.statement_index(),
                );
                if !expected.contains(&key) {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` 的 source-slice statement classification bb{} stmt{} 不属于任何 published source_slices",
                        callable.root_fqn(),
                        key.0.as_u32(),
                        key.1,
                    )));
                }
                if classified.insert(key, classification.kind()).is_some() {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` 的 source-slice statement classification bb{} stmt{} 重复发布",
                        callable.root_fqn(),
                        key.0.as_u32(),
                        key.1,
                    )));
                }
            }
            if classified.len() != expected.len() {
                let missing = expected
                    .iter()
                    .find(|key| !classified.contains_key(key))
                    .expect("classified length drift should expose a missing key");
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` source-slice statement bb{} stmt{} 缺少 classification",
                    callable.root_fqn(),
                    missing.0.as_u32(),
                    missing.1,
                )));
            }

            self.validate_boundary_anchor_classifications(callable, &classified)?;
            self.validate_resume_payload_classifications(callable, &classified)?;
        }
        Ok(())
    }

    pub(super) fn validate_boundary_anchor_classifications(
        &self,
        callable: &LateLoweredCallable,
        classified: &BTreeMap<(BasicBlockId, u32), LateLoweredSourceStatementClassificationKind>,
    ) -> Result<(), LlvmEmitError> {
        for boundary in callable.boundary_map().entries() {
            let Some(LateLoweredBoundarySourceConsumption::Statement {
                source_slice,
                statement_index,
                ..
            }) = boundary_source_consumption(boundary)
            else {
                continue;
            };
            let key = (source_slice.block_id(), statement_index);
            match classified.get(&key) {
                Some(LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor {
                    boundary_id,
                }) if *boundary_id == boundary.boundary_id() => {}
                other => {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 consumed anchor bb{} stmt{} classification 漂移：{:?}",
                        callable.root_fqn(),
                        boundary.boundary_id().as_u32(),
                        key.0.as_u32(),
                        key.1,
                        other,
                    )));
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_resume_payload_classifications(
        &self,
        callable: &LateLoweredCallable,
        classified: &BTreeMap<(BasicBlockId, u32), LateLoweredSourceStatementClassificationKind>,
    ) -> Result<(), LlvmEmitError> {
        for kind in classified.values() {
            let LateLoweredSourceStatementClassificationKind::ResumePayloadInjection {
                boundary_id,
                resume_state,
                consumer_local,
            } = kind
            else {
                continue;
            };
            let Some(binding) = callable.frame_schema().resume_payload_binding(*boundary_id) else {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` boundary bd{} resume payload injection classification 缺少对应 binding",
                    callable.root_fqn(),
                    boundary_id.as_u32(),
                )));
            };
            if binding.resume_state() != *resume_state
                || binding.consumer_local() != *consumer_local
            {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` boundary bd{} resume payload injection classification 漂移",
                    callable.root_fqn(),
                    boundary_id.as_u32(),
                )));
            }
        }
        Ok(())
    }

    pub(super) fn validate_boundary_operand_source_layout(
        &mut self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
        kind: &'static str,
        label: &'static str,
        source: &LateLoweredOperandSource,
    ) -> Result<(), LlvmEmitError> {
        self.source_value_layout(source.source_ty()).map(|_| ()).map_err(|err| {
            frontend_error(format!(
                "LLVM ABI materialization 缺少 callable `{owner_root_fqn}` {kind} site {} {label} source type t{} 的 ABI value lowering contract：{err}",
                site_id.as_u32(),
                source.source_ty().as_u32(),
            ))
        })
    }

    pub(super) fn validate_ordered_boundary_sources(
        &mut self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
        kind: &'static str,
        label: &'static str,
        sources: &[LateLoweredOperandSource],
        expected_tuple_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        if let [source] = sources
            && source.source_ty() == expected_tuple_ty
        {
            self.validate_boundary_operand_source_layout(
                owner_root_fqn,
                site_id,
                kind,
                label,
                source,
            )?;
            return Ok(());
        }
        let expected_components =
            expected_source_types_for_carrier(self.source_types, expected_tuple_ty, sources.len())
                .map_err(|detail| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 {label} contract 非法：{detail}",
                        site_id.as_u32(),
                    ))
                })?;
        if sources.len() != expected_components.len() {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 {label} 数量({}) 与 published carrier t{} 的 component 数量({}) 不一致",
                site_id.as_u32(),
                sources.len(),
                expected_tuple_ty.as_u32(),
                expected_components.len(),
            )));
        }
        for (index, (source, expected_ty)) in sources.iter().zip(expected_components).enumerate() {
            if source.source_ty() != expected_ty {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 {label}[{}] source_ty 漂移：published=t{}，expected=t{}",
                    site_id.as_u32(),
                    index,
                    source.source_ty().as_u32(),
                    expected_ty.as_u32(),
                )));
            }
            self.validate_boundary_operand_source_layout(
                owner_root_fqn,
                site_id,
                kind,
                label,
                source,
            )?;
        }
        Ok(())
    }

    pub(super) fn validate_call_boundary_operand_contract(
        &mut self,
        callable: &LateLoweredCallable,
        boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
        site_id: crate::mir::SiteId,
        lowering: &crate::effect_lowered::ir::LateLoweredCallBoundaryLowering,
        dynamic_invoke_layouts: &BTreeMap<
            (StepSchemaId, crate::mir::SiteId),
            DynamicInvokeLayout<'ctx>,
        >,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<(), LlvmEmitError> {
        let owner_state = callable.state_graph().state(boundary.owner_state()).ok_or_else(|| {
            frontend_error(format!(
                "LLVM ABI materialization 缺少 callable `{}` call site {} owner state st{}，无法发布 boundary operand contract",
                callable.root_fqn(),
                site_id.as_u32(),
                boundary.owner_state().as_u32(),
            ))
        })?;
        self.validate_boundary_source_consumption(
            callable.root_fqn(),
            site_id,
            "call",
            owner_state.source_slices(),
            lowering.operand_contract().source_consumption(),
            true,
        )?;
        self.validate_ordered_boundary_sources(
            callable.root_fqn(),
            site_id,
            "call",
            "ordered args",
            lowering.operand_contract().arg_sources(),
            lowering.facts().invoke_args_tuple_ty(),
        )?;
        match lowering.facts().kind() {
            CallSiteKind::Direct => {
                if lowering.operand_contract().carrier_source().is_some() {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` call site {} 的 direct call boundary 错误地发布了 carrier source",
                        callable.root_fqn(),
                        site_id.as_u32(),
                    )));
                }
            }
            CallSiteKind::Closure
            | CallSiteKind::FunValue
            | CallSiteKind::FunPtr
            | CallSiteKind::Virtual
            | CallSiteKind::Interface => {
                let carrier = lowering.operand_contract().carrier_source().ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` call site {} 的 non-KnownInstance boundary 缺少 carrier source contract",
                        callable.root_fqn(),
                        site_id.as_u32(),
                    ))
                })?;
                self.validate_boundary_operand_source_layout(
                    callable.root_fqn(),
                    site_id,
                    "call",
                    "carrier",
                    carrier,
                )?;
                if lowering.facts().target_mode() != CallTargetMode::KnownInstance {
                    let layout = dynamic_invoke_layouts.get(&(callable.step_schema(), site_id)).ok_or_else(|| {
                        frontend_error(format!(
                            "LLVM ABI materialization 缺少 callable `{}` call site {} 的 dynamic-invoke contract，无法校验 carrier source",
                            callable.root_fqn(),
                            site_id.as_u32(),
                        ))
                    })?;
                    let source_layout = self.source_value_layout(carrier.source_ty())?;
                    if source_layout.abi().is_elided()
                        != layout.carrier().receiver_abi().is_elided()
                        || source_layout.abi().llvm_ty()
                            != layout.carrier().receiver_abi().llvm_ty()
                    {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` call site {} 的 carrier source ABI 与 published dynamic-invoke carrier 漂移",
                            callable.root_fqn(),
                            site_id.as_u32(),
                        )));
                    }
                }
            }
        }
        self.validate_call_boundary_continuation_compositions(
            callable,
            boundary,
            site_id,
            lowering,
            surface_resume_layouts,
        )?;
        Ok(())
    }

    pub(super) fn validate_call_boundary_continuation_compositions(
        &self,
        callable: &LateLoweredCallable,
        boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
        site_id: crate::mir::SiteId,
        lowering: &crate::effect_lowered::ir::LateLoweredCallBoundaryLowering,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<(), LlvmEmitError> {
        let input_step = self
            .program
            .step_type(lowering.dispatch().input_step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` call site {} 的 continuation composition 缺少 input StepSchema s{}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    lowering.dispatch().input_step_schema().as_u32(),
                ))
            })?;
        let mut seen_input_cases = BTreeSet::new();
        for composition in lowering.continuation_compositions() {
            if !seen_input_cases.insert(composition.input_case_tag()) {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` call site {} 对 input case c{} 重复发布 call-boundary continuation composition",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    composition.input_case_tag().as_u32(),
                )));
            }
            self.validate_call_boundary_continuation_composition(
                callable,
                boundary,
                site_id,
                lowering,
                input_step,
                composition,
                surface_resume_layouts,
            )?;
        }
        for forwarding in lowering.dispatch().outward_cases() {
            if lowering
                .continuation_composition_for_input_case(forwarding.input_case_tag())
                .is_none()
            {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` call site {} outward case c{} 缺少 call-boundary continuation composition contract",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    forwarding.input_case_tag().as_u32(),
                )));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_call_boundary_continuation_composition(
        &self,
        callable: &LateLoweredCallable,
        boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
        site_id: crate::mir::SiteId,
        lowering: &crate::effect_lowered::ir::LateLoweredCallBoundaryLowering,
        input_step: &LateLoweredStepType,
        composition: &LateLoweredCallBoundaryContinuationComposition,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<(), LlvmEmitError> {
        if composition.boundary_id() != boundary.boundary_id()
            || composition.input_step_schema() != lowering.dispatch().input_step_schema()
            || composition.caller_resume_state() != boundary.resume_state()
            || composition.caller_result_local() != lowering.result_local()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition 与 boundary/result contract 漂移：composition={:?}",
                callable.root_fqn(),
                site_id.as_u32(),
                composition,
            )));
        }
        let binding = callable
            .frame_schema()
            .resume_payload_binding(boundary.boundary_id())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition 缺少 result home binding",
                    callable.root_fqn(),
                    site_id.as_u32(),
                ))
            })?;
        if binding.resume_state() != composition.caller_resume_state()
            || binding.consumer_local() != composition.caller_result_local()
            || binding.consumer_frame_slot() != composition.caller_result_frame_slot()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition result home 漂移：binding={:?} composition={:?}",
                callable.root_fqn(),
                site_id.as_u32(),
                binding,
                composition,
            )));
        }
        if let Some(frame_slot) = composition.caller_result_frame_slot() {
            let slot = callable
                .frame_schema()
                .slots()
                .iter()
                .find(|slot| slot.slot_id() == frame_slot)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition result frame fs{} 不存在",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        frame_slot.as_u32(),
                    ))
                })?;
            if slot.ty() != composition.caller_result_ty() {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition result frame fs{} 类型 t{} 与 result t{} 不一致",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    frame_slot.as_u32(),
                    slot.ty().as_u32(),
                    composition.caller_result_ty().as_u32(),
                )));
            }
        }
        let input_case = input_step
            .case(composition.input_case_tag())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition 引用缺失 input case c{}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    composition.input_case_tag().as_u32(),
                ))
            })?;
        if input_case.continuation_contract() != composition.callee_continuation_contract()
            || input_step.complete_ty() != composition.caller_result_ty()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition callee contract 漂移：composition={:?}",
                callable.root_fqn(),
                site_id.as_u32(),
                composition,
            )));
        }
        let forwarding = lowering
            .dispatch()
            .outward_cases()
            .iter()
            .find(|forwarding| forwarding.input_case_tag() == composition.input_case_tag())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition 没有对应 dispatch forwarding c{}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    composition.input_case_tag().as_u32(),
                ))
            })?;
        if forwarding.emission().case_tag() != composition.output_case_tag()
            || forwarding.emission().continuation_contract()
                != composition.caller_continuation_contract()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition caller contract 漂移：composition={:?}",
                callable.root_fqn(),
                site_id.as_u32(),
                composition,
            )));
        }
        if input_case.resume_tuple_ty()
            != forwarding
                .emission()
                .continuation_contract()
                .resume_tuple_ty()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition resume payload type 漂移：callee=t{} caller=t{}",
                callable.root_fqn(),
                site_id.as_u32(),
                input_case.resume_tuple_ty().as_u32(),
                forwarding
                    .emission()
                    .continuation_contract()
                    .resume_tuple_ty()
                    .as_u32(),
            )));
        }
        let callee_surface = surface_resume_layouts
            .get(&composition.callee_continuation_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition 缺少 callee continuation schema k{} surface resume ABI",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    composition.callee_continuation_schema().as_u32(),
                ))
            })?;
        if callee_surface.resume_tuple_ty()
            != composition.callee_continuation_contract().resume_tuple_ty()
            || callee_surface.return_step_schema()
                != composition.callee_continuation_contract().out_step_schema()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition callee surface ABI 漂移：composition={:?}",
                callable.root_fqn(),
                site_id.as_u32(),
                composition,
            )));
        }
        Ok(())
    }

    pub(super) fn validate_perform_boundary_operand_contract(
        &mut self,
        callable: &LateLoweredCallable,
        boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
        site_id: crate::mir::SiteId,
        lowering: &crate::effect_lowered::ir::LateLoweredPerformBoundaryLowering,
    ) -> Result<(), LlvmEmitError> {
        let owner_state = callable.state_graph().state(boundary.owner_state()).ok_or_else(|| {
            frontend_error(format!(
                "LLVM ABI materialization 缺少 callable `{}` perform site {} owner state st{}，无法发布 boundary operand contract",
                callable.root_fqn(),
                site_id.as_u32(),
                boundary.owner_state().as_u32(),
            ))
        })?;
        self.validate_boundary_source_consumption(
            callable.root_fqn(),
            site_id,
            "perform",
            owner_state.source_slices(),
            lowering.operand_contract().source_consumption(),
            false,
        )?;
        self.validate_ordered_boundary_sources(
            callable.root_fqn(),
            site_id,
            "perform",
            "payload sources",
            lowering.operand_contract().payload_sources(),
            if layout_type_is_any(self.source_types, lowering.facts().payload_tuple_ty()) {
                lowering.emitted_step().payload_tuple_ty()
            } else {
                lowering.facts().payload_tuple_ty()
            },
        )
    }

    pub(super) fn validate_resume_boundary_operand_contract(
        &mut self,
        callable: &LateLoweredCallable,
        boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
        site_id: crate::mir::SiteId,
        lowering: &crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeLayout<'ctx>,
        >,
        surface_resume_dispatch_layouts: &BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeDispatchLayout<'ctx>,
        >,
    ) -> Result<(), LlvmEmitError> {
        let owner_state = callable.state_graph().state(boundary.owner_state()).ok_or_else(|| {
            frontend_error(format!(
                "LLVM ABI materialization 缺少 callable `{}` resume site {} owner state st{}，无法发布 boundary operand contract",
                callable.root_fqn(),
                site_id.as_u32(),
                boundary.owner_state().as_u32(),
            ))
        })?;
        self.validate_boundary_source_consumption(
            callable.root_fqn(),
            site_id,
            "resume",
            owner_state.source_slices(),
            lowering.operand_contract().source_consumption(),
            true,
        )?;
        self.validate_boundary_operand_source_layout(
            callable.root_fqn(),
            site_id,
            "resume",
            "continuation",
            lowering.operand_contract().continuation_source(),
        )?;
        self.validate_ordered_boundary_sources(
            callable.root_fqn(),
            site_id,
            "resume",
            "ordered args",
            lowering.operand_contract().arg_sources(),
            lowering.facts().resume_tuple_ty(),
        )?;
        let route = lowering.operand_contract().underlying_continuation_route();
        let inventory = self
            .program
            .surface_resume_dispatch(route.continuation_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 callable `{}` resume site {} underlying continuation schema k{} 的 published surface-resume dispatch inventory",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    route.continuation_schema().as_u32(),
                ))
            })?;
        if !inventory.publications().contains(route.publication()) {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` resume site {} 的 underlying continuation route 漂移：schema k{} 缺少 publication {:?}",
                callable.root_fqn(),
                site_id.as_u32(),
                route.continuation_schema().as_u32(),
                route.publication(),
            )));
        }
        let surface_layout = surface_resume_layouts
            .get(&lowering.facts().continuation_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 callable `{}` resume site {} continuation schema k{} 的 surface-resume layout",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    lowering.facts().continuation_schema().as_u32(),
                ))
            })?;
        if surface_layout.resume_tuple_ty() != lowering.facts().resume_tuple_ty()
            || surface_layout.answer_ty() != lowering.facts().answer_ty()
            || surface_layout.return_step_schema() != lowering.facts().out_step_schema()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` resume site {} 的 surface-resume layout 与 published facts 漂移",
                callable.root_fqn(),
                site_id.as_u32(),
            )));
        }
        if !surface_resume_dispatch_layouts.contains_key(&lowering.facts().continuation_schema()) {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 缺少 callable `{}` resume site {} continuation schema k{} 的 surface-resume owner dispatch contract",
                callable.root_fqn(),
                site_id.as_u32(),
                lowering.facts().continuation_schema().as_u32(),
            )));
        }
        Ok(())
    }
}
