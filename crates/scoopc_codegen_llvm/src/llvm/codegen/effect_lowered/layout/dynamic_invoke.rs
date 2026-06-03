//! Dynamic invoke ABI layouts.
//!
//! Dynamic invocations cover closure / virtual / interface dispatch where the
//! callee identity is decided at runtime. The backend-neutral contract is now
//! read from `LirFacts`; this module only turns that contract into LLVM function
//! and carrier layouts.

use super::*;

impl<'cg, 'a, 'ctx> ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    pub(super) fn materialize_dynamic_invoke_layouts(
        &mut self,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
    ) -> Result<BTreeMap<(StepSchemaId, SiteId), DynamicInvokeLayout<'ctx>>, LlvmEmitError> {
        let mut layouts = BTreeMap::new();
        for contract in self.lir_facts.dynamic_invokes.values() {
            let Some(owner_step_schema) = contract.owner_step_schema else {
                continue;
            };
            let owner_step_schema = StepSchemaId::new(owner_step_schema.as_u32());
            let site_id = SiteId::from_raw(contract.site_id.as_u32());
            let key = (owner_step_schema, site_id);
            if layouts.contains_key(&key) {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 owner step schema {} call site {} 的 LIR dynamic-invoke contract 重复发布",
                    owner_step_schema.as_u32(),
                    site_id.as_u32(),
                )));
            }
            let layout = self.materialize_dynamic_invoke_layout(
                owner_step_schema,
                site_id,
                contract,
                step_layouts,
            )?;
            layouts.insert(key, layout);
        }
        Ok(layouts)
    }

    pub(super) fn materialize_dynamic_invoke_layout(
        &mut self,
        owner_step_schema: StepSchemaId,
        site_id: SiteId,
        contract: &LirDynamicInvokeContract,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
    ) -> Result<DynamicInvokeLayout<'ctx>, LlvmEmitError> {
        let callee_schema = contract.call.callee_step_schema.ok_or_else(|| {
            frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{:?}` call site {} 的 LIR dynamic-invoke contract 缺少 callee step schema",
                contract.owner_callable,
                site_id.as_u32(),
            ))
        })?;
        let callee_schema = StepSchemaId::new(callee_schema.as_u32());
        let step_ty = step_layouts
            .get(&callee_schema)
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 callable `{:?}` call site {} dynamic-invoke return step schema {} 的 step layout",
                    contract.owner_callable,
                    site_id.as_u32(),
                    callee_schema.as_u32(),
                ))
            })?
            .llvm_ty();
        let args_layout = self.source_value_layout(contract.call.invoke_args_tuple_ty)?;
        let args_abi = *args_layout.abi();
        let carrier = match contract.carrier.kind {
            LirDynamicInvokeCarrierKind::ClosureObject | LirDynamicInvokeCarrierKind::FunPtr => {
                if !matches!(
                    contract.call.target_mode,
                    LirCallTargetMode::DynamicFallback
                        | LirCallTargetMode::KnownInstance
                        | LirCallTargetMode::CandidateSet
                ) {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{:?}` call site {} 的 callable-carrier lowering 只能绑定 KnownInstance/CandidateSet/DynamicFallback，但实际 target_mode 为 {:?}",
                        contract.owner_callable,
                        site_id.as_u32(),
                        contract.call.target_mode,
                    )));
                }
                let carrier_source_ty = contract.carrier.source_ty.ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 缺少 callable `{:?}` call site {} 的 callable carrier source type",
                        contract.owner_callable,
                        site_id.as_u32(),
                    ))
                })?;
                let receiver_abi = *self.source_value_layout(carrier_source_ty)?.abi();
                match contract.carrier.kind {
                    LirDynamicInvokeCarrierKind::FunPtr => {
                        DynamicInvokeCarrierLayout::FunPtr(receiver_abi)
                    }
                    LirDynamicInvokeCarrierKind::ClosureObject => {
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
                    LirDynamicInvokeCarrierKind::VirtualReceiver
                    | LirDynamicInvokeCarrierKind::InterfaceReceiver => {
                        unreachable!("outer match already selected callable carrier kinds")
                    }
                }
            }
            LirDynamicInvokeCarrierKind::VirtualReceiver => {
                let dispatch_key = contract.carrier.dispatch.as_ref().ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 缺少 callable `{:?}` call site {} 的 virtual dispatch key",
                        contract.owner_callable,
                        site_id.as_u32(),
                    ))
                })?;
                let dispatch = self.dispatch_contract(dispatch_key)?;
                let receiver_ty = dispatch.receiver_ty;
                let owner_fqn = dispatch.owner_fqn.clone();
                let member_name = dispatch.member_name.clone();
                let method_slot = dispatch.method_slot;
                let receiver_abi = *self.source_value_layout(receiver_ty)?.abi();
                DynamicInvokeCarrierLayout::VirtualReceiver(DispatchReceiverLayout::new(
                    receiver_ty,
                    receiver_abi,
                    owner_fqn,
                    member_name,
                    method_slot,
                    None,
                ))
            }
            LirDynamicInvokeCarrierKind::InterfaceReceiver => {
                let dispatch_key = contract.carrier.dispatch.as_ref().ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 缺少 callable `{:?}` call site {} 的 interface dispatch key",
                        contract.owner_callable,
                        site_id.as_u32(),
                    ))
                })?;
                let dispatch = self.dispatch_contract(dispatch_key)?;
                let receiver_ty = dispatch.receiver_ty;
                let owner_fqn = dispatch.owner_fqn.clone();
                let member_name = dispatch.member_name.clone();
                let method_slot = dispatch.method_slot;
                let interface_id = dispatch.interface_id.ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{:?}` call site {} 的 interface dispatch contract 缺少 interface id",
                        contract.owner_callable,
                        site_id.as_u32(),
                    ))
                })?;
                let receiver_abi = *self.source_value_layout(receiver_ty)?.abi();
                DynamicInvokeCarrierLayout::InterfaceReceiver(DispatchReceiverLayout::new(
                    receiver_ty,
                    receiver_abi,
                    owner_fqn,
                    member_name,
                    method_slot,
                    Some(interface_id),
                ))
            }
        };

        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> =
            vec![carrier.receiver_abi().llvm_ty().into()];
        if !args_abi.is_elided() {
            params.push(args_abi.llvm_ty().into());
        }
        let llvm_ty = step_ty.fn_type(&params, false);
        Ok(DynamicInvokeLayout::new(
            owner_step_schema,
            site_id,
            lir_target_mode(contract.call.target_mode),
            contract.call.invoke_args_tuple_ty,
            llvm_ty,
            params.len(),
            args_abi,
            callee_schema,
            carrier,
            self.lir_target_roots(contract)?,
        ))
    }
}

fn lir_target_mode(mode: LirCallTargetMode) -> CallTargetMode {
    match mode {
        LirCallTargetMode::KnownInstance => CallTargetMode::KnownInstance,
        LirCallTargetMode::CandidateSet => CallTargetMode::CandidateSet,
        LirCallTargetMode::DynamicFallback => CallTargetMode::DynamicFallback,
    }
}
