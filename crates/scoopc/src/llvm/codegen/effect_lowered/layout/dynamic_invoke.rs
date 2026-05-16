//! Dynamic invoke ABI layouts.
//!
//! Dynamic invocations cover closure / virtual / interface dispatch where the
//! callee identity is decided at runtime. This module materializes the per-
//! site layout (carrier source slice, target callable layout) for every
//! dynamic invoke boundary and validates the source-slice shape.

use super::*;

impl<'cg, 'a, 'ctx> ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    pub(super) fn materialize_dynamic_invoke_layouts(
        &mut self,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
    ) -> Result<
        BTreeMap<(StepSchemaId, crate::mir::SiteId), DynamicInvokeLayout<'ctx>>,
        LlvmEmitError,
    > {
        let mut layouts = BTreeMap::new();
        for callable in self.program.callables() {
            if !callable.has_control_body() {
                continue;
            }
            self.publish_boundary_dynamic_invoke_layouts(callable, step_layouts, &mut layouts)?;
            self.publish_source_slice_dynamic_invoke_layouts(callable, step_layouts, &mut layouts)?;
        }
        Ok(layouts)
    }

    pub(super) fn publish_boundary_dynamic_invoke_layouts(
        &mut self,
        callable: &LateLoweredCallable,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
        layouts: &mut BTreeMap<(StepSchemaId, crate::mir::SiteId), DynamicInvokeLayout<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        for boundary in callable.boundary_map().entries() {
            let (
                LateLoweredBoundarySource::Site {
                    site_id,
                    kind: BoundarySiteKind::Call,
                },
                Some(LateLoweredBoundaryLowering::Call(lowering)),
            ) = (boundary.source(), boundary.lowering())
            else {
                continue;
            };
            if lowering.facts().target_mode() == CallTargetMode::KnownInstance {
                continue;
            }
            let call_site = self.lookup_materialized_call_site(callable.root_fqn(), site_id)?;

            self.publish_dynamic_invoke_layout(
                callable,
                site_id,
                lowering.facts(),
                &call_site.kind,
                call_site.carrier_source_ty,
                call_site.arg_count,
                step_layouts,
                layouts,
            )?;
        }
        Ok(())
    }

    pub(super) fn publish_source_slice_dynamic_invoke_layouts(
        &mut self,
        callable: &LateLoweredCallable,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
        layouts: &mut BTreeMap<(StepSchemaId, crate::mir::SiteId), DynamicInvokeLayout<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        let boundary_call_sites = callable
            .boundary_map()
            .entries()
            .iter()
            .filter_map(|boundary| match boundary.source() {
                LateLoweredBoundarySource::Site {
                    site_id,
                    kind: BoundarySiteKind::Call,
                } => Some(site_id),
                LateLoweredBoundarySource::RuntimeError { .. }
                | LateLoweredBoundarySource::Site { .. } => None,
            })
            .collect::<BTreeSet<_>>();

        let source_slice_sites = {
            let body = self.lookup_materialized_callable_body(callable.root_fqn())?;
            let body_facts = self.body_effect_facts(callable)?;
            let mut sites = Vec::new();
            for state in callable.state_graph().states() {
                for slice in state.source_slices() {
                    let Some(block) = body.blocks.get(slice.block_id().as_u32() as usize) else {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` 的 source slice 指向缺失的 canonical MIR block bb{}",
                            callable.root_fqn(),
                            slice.block_id().as_u32(),
                        )));
                    };
                    let start = slice.start_statement_index() as usize;
                    let end = slice.end_statement_index() as usize;
                    if start > end || end > block.stmts.len() {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` 的 source slice [{start}..{end}) 越界于 canonical MIR block bb{}（stmt_count={}）",
                            callable.root_fqn(),
                            slice.block_id().as_u32(),
                            block.stmts.len(),
                        )));
                    }
                    for stmt in &block.stmts[start..end] {
                        let MirStatementKind::Assign {
                            value:
                                MirRvalue::Call {
                                    site_id,
                                    kind,
                                    args,
                                    ..
                                },
                            ..
                        } = &stmt.kind
                        else {
                            continue;
                        };
                        if boundary_call_sites.contains(site_id) {
                            continue;
                        }
                        if !matches!(
                            kind,
                            MirCallKind::FunValue { .. }
                                | MirCallKind::FunPtr { .. }
                                | MirCallKind::Closure { .. }
                                | MirCallKind::Virtual { .. }
                                | MirCallKind::Interface { .. }
                        ) {
                            continue;
                        }

                        let site = body_facts.site(*site_id).ok_or_else(|| {
                            frontend_error(format!(
                                "LLVM ABI materialization 缺少 callable `{}` source-slice call site {} 的 published effect facts，无法发布 non-boundary dynamic-invoke contract",
                                callable.root_fqn(),
                                site_id.as_u32(),
                            ))
                        })?;
                        let SiteEffectFacts::Call(facts) = site else {
                            return Err(frontend_error(format!(
                                "LLVM ABI materialization 发现 callable `{}` source-slice call site {} 的 canonical MIR kind {:?} 不是普通 Call site，而 published facts 为 {site:?}",
                                callable.root_fqn(),
                                site_id.as_u32(),
                                kind,
                            )));
                        };
                        if !facts.resolved_cases().is_empty() {
                            return Err(frontend_error(format!(
                                "LLVM ABI materialization 发现 callable `{}` source-slice dynamic call site {} 仍暴露 outward cases，但 late-lowered handoff 没有对应 call boundary",
                                callable.root_fqn(),
                                site_id.as_u32(),
                            )));
                        }
                        // Control-body 内的 plain dynamic call 继续走普通 ABI lowering，
                        // 只有真正返回 Step_F 的 effect-step call 才需要 dynamic-invoke contract。
                        if facts.callee_step_schema().is_none() {
                            continue;
                        }
                        let carrier_source_ty = self.dynamic_call_carrier_source_ty(body, kind);
                        sites.push((
                            *site_id,
                            kind.clone(),
                            carrier_source_ty,
                            args.len(),
                            facts.clone(),
                        ));
                    }
                }
            }
            sites
        };

        for (site_id, kind, carrier_source_ty, arg_count, facts) in source_slice_sites {
            self.publish_dynamic_invoke_layout(
                callable,
                site_id,
                &facts,
                &kind,
                carrier_source_ty,
                arg_count,
                step_layouts,
                layouts,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn publish_dynamic_invoke_layout(
        &mut self,
        callable: &LateLoweredCallable,
        site_id: crate::mir::SiteId,
        facts: &CallSiteEffectFacts,
        call_kind: &MirCallKind,
        carrier_source_ty: Option<TypeId>,
        arg_count: usize,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
        layouts: &mut BTreeMap<(StepSchemaId, crate::mir::SiteId), DynamicInvokeLayout<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        let key = (callable.step_schema(), site_id);
        if layouts.contains_key(&key) {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 owner step schema {} call site {} 的 dynamic-invoke contract 重复发布",
                callable.step_schema().as_u32(),
                site_id.as_u32(),
            )));
        }
        let layout = self.materialize_dynamic_invoke_layout(
            callable.root_fqn(),
            callable.step_schema(),
            site_id,
            facts,
            call_kind,
            carrier_source_ty,
            arg_count,
            step_layouts,
        )?;
        layouts.insert(key, layout);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn materialize_dynamic_invoke_layout(
        &mut self,
        owner_root_fqn: &str,
        owner_step_schema: StepSchemaId,
        site_id: crate::mir::SiteId,
        facts: &CallSiteEffectFacts,
        call_kind: &MirCallKind,
        carrier_source_ty: Option<TypeId>,
        arg_count: usize,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
    ) -> Result<DynamicInvokeLayout<'ctx>, LlvmEmitError> {
        self.validate_dynamic_call_site_kind(owner_root_fqn, site_id, facts, call_kind)?;
        let step_ty = step_layouts
            .get(&facts.callee_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 callable `{owner_root_fqn}` call site {} dynamic-invoke return step schema {} 的 step layout",
                    site_id.as_u32(),
                    facts.callee_schema().as_u32(),
                ))
            })?
            .llvm_ty();
        let args_layout = self.source_value_layout(facts.invoke_args_tuple_ty())?;
        let args_abi = *args_layout.abi();
        let carrier = match call_kind {
            MirCallKind::FunValue { .. }
            | MirCallKind::FunPtr { .. }
            | MirCallKind::Closure { .. } => {
                if !matches!(
                    facts.target_mode(),
                    CallTargetMode::DynamicFallback | CallTargetMode::KnownInstance
                ) {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{owner_root_fqn}` call site {} 的 {:?} lowering 只能绑定 KnownInstance/DynamicFallback，但实际 target_mode 为 {:?}",
                        site_id.as_u32(),
                        call_kind,
                        facts.target_mode(),
                    )));
                }
                let carrier_source_ty = carrier_source_ty.ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 缺少 callable `{owner_root_fqn}` call site {} 的 callable carrier source type",
                        site_id.as_u32(),
                    ))
                })?;
                let receiver_abi = *self.source_value_layout(carrier_source_ty)?.abi();
                if self.is_funptr_source_ty(carrier_source_ty) {
                    DynamicInvokeCarrierLayout::FunPtr(receiver_abi)
                } else {
                    DynamicInvokeCarrierLayout::ClosureObject(ClosureCarrierLayout::new(
                        self.codegen.llvm_closure_object_type(),
                        receiver_abi,
                        1,
                        2,
                    ))
                }
            }
            MirCallKind::Virtual { dispatch, .. } => {
                let method_slot = self.resolve_virtual_dispatch_slot(
                    owner_root_fqn,
                    site_id,
                    dispatch,
                    arg_count,
                )?;
                if let crate::effect_facts::CallSiteTarget::CandidateSet(targets) = facts.target() {
                    for target in targets {
                        if self.program.callable(&target.template.fqn).is_none() {
                            return Err(frontend_error(format!(
                                "LLVM ABI materialization 缺少 callable `{owner_root_fqn}` call site {} CandidateSet target `{}` 的 published callable shell",
                                site_id.as_u32(),
                                target.template.fqn,
                            )));
                        }
                    }
                }
                DynamicInvokeCarrierLayout::VirtualReceiver(DispatchReceiverLayout::new(
                    dispatch.receiver_ty,
                    *self.source_value_layout(dispatch.receiver_ty)?.abi(),
                    dispatch.owner_fqn.clone(),
                    dispatch.member_name.clone(),
                    method_slot,
                    None,
                ))
            }
            MirCallKind::Interface { dispatch, .. } => {
                let (interface_id, method_slot) = self.resolve_interface_dispatch_slot(
                    owner_root_fqn,
                    site_id,
                    dispatch,
                    arg_count,
                )?;
                if let crate::effect_facts::CallSiteTarget::CandidateSet(targets) = facts.target() {
                    for target in targets {
                        if self.program.callable(&target.template.fqn).is_none() {
                            return Err(frontend_error(format!(
                                "LLVM ABI materialization 缺少 callable `{owner_root_fqn}` call site {} CandidateSet target `{}` 的 published callable shell",
                                site_id.as_u32(),
                                target.template.fqn,
                            )));
                        }
                    }
                }
                DynamicInvokeCarrierLayout::InterfaceReceiver(DispatchReceiverLayout::new(
                    dispatch.receiver_ty,
                    *self.source_value_layout(dispatch.receiver_ty)?.abi(),
                    dispatch.owner_fqn.clone(),
                    dispatch.member_name.clone(),
                    method_slot,
                    Some(interface_id),
                ))
            }
            other => {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{owner_root_fqn}` call site {} 的 canonical MIR kind {other:?} 无法为 {:?} 发布 dynamic-invoke contract",
                    site_id.as_u32(),
                    facts.target_mode(),
                )));
            }
        };

        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> =
            vec![carrier.receiver_abi().llvm_ty().into()];
        if !args_abi.is_elided() {
            params.push(args_abi.llvm_ty().into());
        }
        let llvm_ty = step_ty.fn_type(&params, false);

        let candidate_targets = match facts.target() {
            crate::effect_facts::CallSiteTarget::CandidateSet(targets) => targets
                .iter()
                .map(|target| target.template.fqn.clone())
                .collect::<Vec<_>>(),
            crate::effect_facts::CallSiteTarget::KnownInstance(target) => {
                vec![target.template.fqn.clone()]
            }
            crate::effect_facts::CallSiteTarget::DynamicFallback => Vec::new(),
        };

        Ok(DynamicInvokeLayout::new(
            owner_step_schema,
            site_id,
            facts.target_mode(),
            facts.invoke_args_tuple_ty(),
            llvm_ty,
            params.len(),
            args_abi,
            facts.callee_schema(),
            carrier,
            candidate_targets,
        ))
    }
}
