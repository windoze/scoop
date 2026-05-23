//! Body, call-site, and dispatch-slot resolution.
//!
//! Helpers that translate from late-lowered identifiers (body version keys,
//! site IDs, dispatch tables) to the materialized MIR / interface layout
//! they map to. Used by the carrier and boundary layers to bind dynamic
//! invocations to concrete callees.

use super::*;

impl<'cg, 'a, 'ctx> ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    pub(super) fn physical_class_layout(
        &self,
        layout_key: &crate::hir::ClassInstanceKey,
    ) -> Option<&scoopc_lir_facts::LirClassLayoutFacts> {
        self.lir_facts
            .physical_layout
            .classes
            .get(layout_key.as_str())
    }

    pub(super) fn physical_enum_layout_by_key(
        &self,
        layout_key: &str,
    ) -> Option<&scoopc_lir_facts::LirEnumLayoutFacts> {
        self.lir_facts.physical_layout.enums.get(layout_key)
    }

    pub(super) fn physical_enum_layout_for_option(
        &self,
        types: &TypeStore,
        inner: TypeId,
    ) -> Result<&scoopc_lir_facts::LirEnumLayoutFacts, LlvmEmitError> {
        let key = self.physical_nominal_layout_key("scoop.core.Option", &[inner], types);
        self.physical_enum_layout_by_key(&key).ok_or_else(|| {
            frontend_error(format!(
                "LLVM ABI materialization 缺少 `{key}` 的 LIR enum layout（inner=`{}`）",
                types.display(inner)
            ))
        })
    }

    pub(super) fn physical_enum_layout_for_nominal(
        &self,
        types: &TypeStore,
        nominal: &crate::ty::NominalType,
    ) -> Result<&scoopc_lir_facts::LirEnumLayoutFacts, LlvmEmitError> {
        self.physical_enum_layout_for_nominal_opt(types, nominal)
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 nominal value `{}` 的 LIR enum layout",
                    nominal.fqn
                ))
            })
    }

    pub(super) fn physical_enum_layout_for_nominal_opt(
        &self,
        types: &TypeStore,
        nominal: &crate::ty::NominalType,
    ) -> Option<&scoopc_lir_facts::LirEnumLayoutFacts> {
        let key = self.physical_nominal_layout_key(&nominal.fqn, &nominal.args, types);
        self.physical_enum_layout_by_key(&key)
    }

    fn physical_nominal_layout_key(&self, fqn: &str, args: &[TypeId], types: &TypeStore) -> String {
        if args.is_empty() {
            return fqn.to_string();
        }
        let arg_text = args
            .iter()
            .map(|id| types.display(*id).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("{fqn}<{arg_text}>")
    }

    pub(super) fn callable_facts_for_root(
        &self,
        root_fqn: &str,
    ) -> Result<&LirCallableFacts, LlvmEmitError> {
        self.lir_facts
            .callables
            .values()
            .find(|facts| facts.root_fqn() == root_fqn)
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 callable `{root_fqn}` 的 LIR facts"
                ))
            })
    }

    pub(super) fn callable_facts(
        &self,
        callable: &LateLoweredCallable,
    ) -> Result<&LirCallableFacts, LlvmEmitError> {
        self.callable_facts_for_root(callable.root_fqn())
    }

    pub(super) fn plain_callable_facts(
        &self,
        callable: &LateLoweredCallable,
    ) -> Result<&scoopc_lir_facts::LirPlainCallableFacts, LlvmEmitError> {
        match &self.callable_facts(callable)?.contract {
            LirCallableContract::Plain(plain) => Ok(plain),
            LirCallableContract::EffectStep(_) => Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` 的 LIR facts 不是 plain ABI",
                callable.root_fqn()
            ))),
        }
    }

    pub(super) fn effect_step_callable_facts_for_root(
        &self,
        root_fqn: &str,
    ) -> Result<&scoopc_lir_facts::LirEffectStepCallableFacts, LlvmEmitError> {
        match &self.callable_facts_for_root(root_fqn)?.contract {
            LirCallableContract::EffectStep(effect) => Ok(effect),
            LirCallableContract::Plain(_) => Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{root_fqn}` 的 LIR facts 不是 effect-step ABI"
            ))),
        }
    }

    pub(super) fn dispatch_contract(
        &self,
        key: &scoopc_lir_facts::LirDispatchKey,
    ) -> Result<&scoopc_lir_facts::LirDispatchContract, LlvmEmitError> {
        self.lir_facts.dispatches.get(key).ok_or_else(|| {
            frontend_error(format!(
                "LLVM ABI materialization 缺少 callable `{}` call site {} 的 LIR dispatch contract",
                key.owner_callable.readable_path(),
                key.site_id.as_u32()
            ))
        })
    }

    pub(super) fn lir_target_roots(
        &self,
        contract: &LirDynamicInvokeContract,
    ) -> Result<Vec<String>, LlvmEmitError> {
        contract
            .call
            .target_callables
            .iter()
            .map(|key| {
                self.lir_facts
                    .callables
                    .get(key)
                    .map(|facts| facts.root_fqn.clone())
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "LLVM ABI materialization 的 dynamic-invoke contract 引用了缺失的 target callable `{}`",
                            key.as_str()
                        ))
                    })
            })
            .collect()
    }

    pub(super) fn is_funptr_source_ty(&self, ty: TypeId) -> bool {
        matches!(
            self.source_types.kind(ty),
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.unsafe.FunPtr"
        )
    }

    pub(super) fn validate_published_resume_packing_ids(
        &self,
        owner_label: &str,
        expected_step_schema: StepSchemaId,
        interface_ids: &[ResumeInterfaceId],
    ) -> Result<(), LlvmEmitError> {
        let mut seen = BTreeSet::new();
        for &interface_id in interface_ids {
            if !seen.insert(interface_id) {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 {owner_label} 重复发布 resume packing {}",
                    interface_id.as_u32()
                )));
            }
            let interface = self.program.resume_packing(interface_id).ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 {owner_label} 发布的 resume packing {}",
                    interface_id.as_u32()
                ))
            })?;
            if interface.return_step_schema() != expected_step_schema {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 {owner_label} 发布的 resume packing {} return step schema 为 {}，但当前 step schema 为 {}",
                    interface_id.as_u32(),
                    interface.return_step_schema().as_u32(),
                    expected_step_schema.as_u32()
                )));
            }
        }
        Ok(())
    }
}
