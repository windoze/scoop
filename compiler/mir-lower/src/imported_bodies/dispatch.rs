//! Concrete semantic conformances also describe provider-owned box dispatch.
use super::*;

pub(crate) type IntrinsicDispatch = HashMap<(mir::ClassId, mir::InterfaceId, u32), IntrinsicTarget>;

#[derive(Clone)]
pub(crate) struct IntrinsicTarget {
    pub callee: mir::DispatchCallee,
    pub unbox: bool,
}

impl Lowerer {
    pub(crate) fn intrinsic_dispatch(&mut self, module: &hir::Module) -> IntrinsicDispatch {
        let mut targets = IntrinsicDispatch::new();
        let boxes = self
            .boxed
            .entries
            .iter()
            .map(|entry| (entry.class, entry.payload.clone()))
            .collect::<Vec<_>>();
        for (class, payload) in boxes {
            let implementations = if let mir::Type::Enum(id, _) = payload {
                module.enums[self.enums.hir_ids[&id]]
                    .interface_implementations
                    .clone()
            } else {
                let Some(exact) = self.source_exact_types.get(&payload) else {
                    continue;
                };
                let Some(source) = module.structs.values().find(|source| {
                    module.exact_type_identities[source.canonical_type].id()
                        == exact.identity_record().id()
                }) else {
                    continue;
                };
                source.interface_implementations.clone()
            };
            self.intrinsic_conformance_targets(module, class, &implementations, true, &mut targets);
        }
        for (source, declaration) in module.classes.iter() {
            self.intrinsic_conformance_targets(
                module,
                self.class_map[&source],
                &declaration.interface_implementations,
                false,
                &mut targets,
            );
        }
        targets
    }

    fn intrinsic_conformance_targets(
        &mut self,
        module: &hir::Module,
        class: mir::ClassId,
        implementations: &[hir::InterfaceImplementation],
        unbox: bool,
        targets: &mut IntrinsicDispatch,
    ) {
        for implementation in implementations {
            let interface = self.interfaces.mir_id(implementation.interface);
            for method in &implementation.methods {
                let callee = match method.target {
                    hir::InterfaceImplementationTarget::Imported(source)
                        if module.imported_intrinsics.contains_key(&source) =>
                    {
                        let Some(target) = self.imported_dependency_callable_map.get(&source)
                        else {
                            continue;
                        };
                        mir::DispatchCallee::External(target.scoop_entry())
                    }
                    hir::InterfaceImplementationTarget::Method(source)
                        if matches!(
                            module.functions[source].kind,
                            hir::FunctionKind::Intrinsic(_)
                        ) =>
                    {
                        mir::DispatchCallee::User(self.interface_method_entry(module, source))
                    }
                    _ => continue,
                };
                targets.insert(
                    (class, interface, method.slot.into_raw()),
                    IntrinsicTarget { callee, unbox },
                );
            }
        }
    }
}
