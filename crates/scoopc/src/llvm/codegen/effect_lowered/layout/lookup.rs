//! Body, call-site, and dispatch-slot resolution.
//!
//! Helpers that translate from late-lowered identifiers (body version keys,
//! site IDs, dispatch tables) to the materialized MIR / interface layout
//! they map to. Used by the carrier and boundary layers to bind dynamic
//! invocations to concrete callees.

use super::*;

impl<'cg, 'a, 'ctx> ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    pub(super) fn body_effect_facts(
        &self,
        callable: &LateLoweredCallable,
    ) -> Result<&crate::effect_facts::BodyEffectFacts, LlvmEmitError> {
        self.effect_facts
            .body(callable.instance_key())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{}` 的 BodyEffectFacts，无法发布 source-slice dynamic-invoke contract",
                    callable.root_fqn(),
                ))
            })
    }

    pub(super) fn validate_dynamic_call_site_kind(
        &self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
        facts: &CallSiteEffectFacts,
        call_kind: &MirCallKind,
    ) -> Result<(), LlvmEmitError> {
        let expected_kind = match call_kind {
            MirCallKind::Closure { .. } => CallSiteKind::Closure,
            MirCallKind::FunValue { .. } => CallSiteKind::FunValue,
            MirCallKind::FunPtr { .. } => CallSiteKind::FunPtr,
            MirCallKind::Virtual { .. } => CallSiteKind::Virtual,
            MirCallKind::Interface { .. } => CallSiteKind::Interface,
            other => {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` call site {} 的 canonical MIR kind {other:?} 无法为 {:?} 发布 dynamic-invoke contract",
                    site_id.as_u32(),
                    facts.target_mode(),
                )));
            }
        };
        if facts.kind() != expected_kind {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` call site {} 的 call kind contract 漂移：canonical MIR={call_kind:?}，effect facts={:?}",
                site_id.as_u32(),
                facts.kind(),
            )));
        }
        Ok(())
    }

    pub(super) fn lookup_materialized_callable_body(
        &self,
        owner_root_fqn: &str,
    ) -> Result<&crate::mir::Body, LlvmEmitError> {
        let callable = self.pass_view.callable(owner_root_fqn).ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` 的 canonical MIR body，无法发布 dynamic-invoke contract"
            ))
        })?;
        callable.body.as_ref().ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` 的 canonical MIR body 内容，无法发布 dynamic-invoke contract"
            ))
        })
    }

    pub(super) fn materialized_operand_source_ty(
        &self,
        body: &crate::mir::Body,
        operand: &crate::mir::Operand,
    ) -> Option<TypeId> {
        match operand {
            crate::mir::Operand::Local(local) => {
                body.locals.get(local.as_u32() as usize).map(|decl| decl.ty)
            }
            crate::mir::Operand::Const(_) => None,
        }
    }

    pub(super) fn dynamic_call_carrier_source_ty(
        &self,
        body: &crate::mir::Body,
        kind: &MirCallKind,
    ) -> Option<TypeId> {
        match kind {
            MirCallKind::Closure { callee, .. }
            | MirCallKind::FunValue { callee }
            | MirCallKind::FunPtr { callee } => self.materialized_operand_source_ty(body, callee),
            MirCallKind::Virtual { receiver, .. } | MirCallKind::Interface { receiver, .. } => {
                self.materialized_operand_source_ty(body, receiver)
            }
            MirCallKind::Direct { .. } | MirCallKind::Resume { .. } => None,
        }
    }

    pub(super) fn is_funptr_source_ty(&self, ty: TypeId) -> bool {
        matches!(
            self.source_types.kind(ty),
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.unsafe.FunPtr"
        )
    }

    pub(super) fn lookup_materialized_call_site(
        &self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
    ) -> Result<MaterializedDynamicCallSite, LlvmEmitError> {
        let body = self.lookup_materialized_callable_body(owner_root_fqn)?;
        for block in &body.blocks {
            for stmt in &block.stmts {
                let MirStatementKind::Assign {
                    value:
                        MirRvalue::Call {
                            site_id: stmt_site_id,
                            kind,
                            args,
                            ..
                        },
                    ..
                } = &stmt.kind
                else {
                    continue;
                };
                if *stmt_site_id == site_id {
                    let carrier_source_ty = self.dynamic_call_carrier_source_ty(body, kind);
                    return Ok(MaterializedDynamicCallSite {
                        kind: kind.clone(),
                        arg_count: args.len(),
                        carrier_source_ty,
                    });
                }
            }
        }
        Err(frontend_error(format!(
            "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` call site {} 的 canonical MIR call metadata，无法发布 dynamic-invoke contract",
            site_id.as_u32(),
        )))
    }

    pub(super) fn resolve_virtual_dispatch_slot(
        &self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
        dispatch: &crate::mir::DispatchMetadata,
        explicit_arg_count: usize,
    ) -> Result<u32, LlvmEmitError> {
        let slots = self
            .codegen
            .class_vtables
            .get(&dispatch.owner_fqn)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` virtual call site {} owner `{}` 的 class vtable，无法发布 dispatch slot",
                    site_id.as_u32(),
                    dispatch.owner_fqn,
                ))
            })?;
        let mut candidates = slots.iter().filter(|slot| {
            slot.name == dispatch.member_name && slot.params_len == explicit_arg_count as u32
        });
        let first = candidates.next().ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` virtual call site {} `{}`.`{}`/{} 的 vtable slot",
                site_id.as_u32(),
                dispatch.owner_fqn,
                dispatch.member_name,
                explicit_arg_count,
            ))
        })?;
        if candidates.next().is_some() {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` virtual call site {} `{}`.`{}`/{} 的 vtable slot 多义",
                site_id.as_u32(),
                dispatch.owner_fqn,
                dispatch.member_name,
                explicit_arg_count,
            )));
        }
        Ok(first.slot)
    }

    pub(super) fn resolve_interface_dispatch_slot(
        &self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
        dispatch: &crate::mir::DispatchMetadata,
        explicit_arg_count: usize,
    ) -> Result<(u64, u32), LlvmEmitError> {
        let iface = self
            .codegen
            .interfaces
            .get(&dispatch.owner_fqn)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` interface call site {} owner `{}` 的 interface metadata，无法发布 itable slot",
                    site_id.as_u32(),
                    dispatch.owner_fqn,
                ))
            })?;
        let mut candidates = iface.method_slots.iter().filter(|slot| {
            slot.member_fqn == dispatch.member_fqn && slot.params_len == explicit_arg_count as u32
        });
        let first = candidates.next().ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` interface call site {} `{}` 的 selected itable slot",
                site_id.as_u32(),
                dispatch.member_fqn,
            ))
        })?;
        if candidates.next().is_some() {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` interface call site {} `{}` 的 selected itable slot 多义",
                site_id.as_u32(),
                dispatch.member_fqn,
            )));
        }
        Ok((iface.interface_id, first.slot))
    }

    pub(super) fn lookup_materialized_handle_arms(
        &self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
    ) -> Result<&[MirHandlerArm], LlvmEmitError> {
        let body = self.lookup_materialized_callable_body(owner_root_fqn)?;
        let mut found = None;
        for block in &body.blocks {
            let MirTerminatorKind::Handle {
                site_id: terminator_site,
                arms,
                ..
            } = &block.terminator.kind
            else {
                continue;
            };
            if *terminator_site != site_id {
                continue;
            }
            if found.replace(arms.as_slice()).is_some() {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 在 canonical MIR 中重复出现多个 Handle terminator",
                    site_id.as_u32(),
                )));
            }
        }
        found.ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` handle site {} 的 canonical MIR arm metadata，无法校验 HandleDispatch arm binder contract",
                site_id.as_u32(),
            ))
        })
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
                    "refactor LLVM ABI materialization 发现 {owner_label} 重复发布 resume packing {}",
                    interface_id.as_u32()
                )));
            }
            let interface = self.program.resume_packing(interface_id).ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 {owner_label} 发布的 resume packing {}",
                    interface_id.as_u32()
                ))
            })?;
            if interface.return_step_schema() != expected_step_schema {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 {owner_label} 发布的 resume packing {} return step schema 为 {}，但当前 step schema 为 {}",
                    interface_id.as_u32(),
                    interface.return_step_schema().as_u32(),
                    expected_step_schema.as_u32()
                )));
            }
        }
        Ok(())
    }
}
