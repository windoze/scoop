//! Capture actual source demands before generating protocol applications.

use super::*;
use std::collections::BTreeSet;

impl Concretizer<'_> {
    pub(in crate::concretize) fn shared_result_types(&self) -> BTreeSet<concrete::TypeId> {
        let mut roots = self.shared_types.clone();
        roots.extend(self.coroutine_result_types());
        for (origin, ty) in self.nominal_results() {
            if matches!(
                origin,
                export::HirNominalIdentity::Source(export::HirSourceNominalIdentity::Generic(_))
            ) {
                roots.insert(ty);
            }
        }
        for (key, function) in self.function_keys.iter().zip(&self.function_slots) {
            let function = function
                .as_ref()
                .expect("the source work queue was drained");
            if (function.receiver.method().is_none() && !function.is_suspend)
                || key.arguments.is_empty()
                || matches!(function.kind, concrete::FunctionKind::Intrinsic(_))
                || !matches!(
                    key.template_owner(),
                    Some(
                        scoop_identity::CallableTemplateOwner::Function(_)
                            | scoop_identity::CallableTemplateOwner::GenericFunction(_)
                            | scoop_identity::CallableTemplateOwner::Accessor(_)
                    )
                )
            {
                continue;
            }
            roots.extend(function.receiver.value_type());
            roots.extend(function.params.iter().map(|parameter| parameter.ty));
            roots.insert(function.return_ty);
        }
        let relations = concrete::ConcreteTypeRelations {
            types: &self.types,
            function_types: &self.function_types,
            structs: &self.structs,
            enums: &self.enums,
            classes: &self.classes,
            interfaces: &self.interfaces,
        };
        let mut pending = roots.iter().copied().collect::<Vec<_>>();
        while let Some(ty) = pending.pop() {
            let result: Result<(), std::convert::Infallible> =
                relations.visit_children(ty, &mut |child| {
                    if roots.insert(child) {
                        pending.push(child);
                    }
                    Ok(())
                });
            let Ok(()) = result;
        }
        roots
    }

    pub(super) fn coroutine_result_types(&self) -> BTreeSet<concrete::TypeId> {
        let mut results = self.coroutine_results.clone();
        for (key, function) in self.function_keys.iter().zip(&self.function_slots) {
            let function = function
                .as_ref()
                .expect("the callable work queue was drained");
            if function.is_suspend
                && matches!(
                    function.kind,
                    concrete::FunctionKind::User(_) | concrete::FunctionKind::Abstract { .. }
                )
            {
                results.insert(function.return_ty);
            }
            if matches!(
                function.kind,
                concrete::FunctionKind::Intrinsic(intrinsic)
                    if matches!(
                        intrinsic.kind,
                        concrete::IntrinsicFunctionKind::CoroutineStart
                            | concrete::IntrinsicFunctionKind::CoroutineSuspend
                    )
            ) {
                results.extend(self.function_key_arguments(key));
            }
        }
        // MIR also needs protocols for suspend function-value variance bridges.
        results.extend(
            self.function_types
                .iter()
                .filter_map(|(_, function)| function.is_suspend.then_some(function.return_type)),
        );
        results.extend(self.interfaces.iter().flat_map(|(_, interface)| {
            interface
                .methods
                .iter()
                .filter_map(|method| method.is_suspend.then_some(method.return_ty))
        }));
        results
    }

    pub(in crate::concretize) fn shared_nominal_roots(&self) -> Vec<export::SourceNominalId> {
        let roots = self.shared_result_types();
        self.nominal_results()
            .filter_map(|(origin, ty)| {
                let source = origin.source()?;
                if source.declaration().origin() != self.source.cone || !roots.contains(&ty) {
                    return None;
                }
                Some(match source {
                    export::HirSourceNominalIdentity::Concrete(record) => {
                        export::SourceNominalId::Concrete(record.id())
                    }
                    export::HirSourceNominalIdentity::Generic(record) => {
                        export::SourceNominalId::GenericTemplate(record.id())
                    }
                })
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub(in crate::concretize) fn owned_result_types(
        &self,
        roots: &BTreeSet<concrete::TypeId>,
    ) -> BTreeSet<concrete::TypeId> {
        self.nominal_results()
            .filter_map(|(origin, ty)| match origin.source()? {
                export::HirSourceNominalIdentity::Concrete(source)
                    if source.key().origin() == self.source.cone && roots.contains(&ty) =>
                {
                    Some(ty)
                }
                _ => None,
            })
            .collect()
    }

    pub(in crate::concretize) fn seed_shape_results(
        &mut self,
        requirements: &export::PublicNominalShapeRequirementsV1,
    ) {
        let sources = requirements
            .roots()
            .iter()
            .map(|root| root.source())
            .collect::<BTreeSet<_>>();
        let results = self
            .nominal_results()
            .filter_map(|(origin, ty)| match origin.source()? {
                export::HirSourceNominalIdentity::Concrete(source)
                    if sources.contains(&source.id()) =>
                {
                    Some(ty)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        self.coroutine_results.extend(results);
        if sources.contains(
            &scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ) {
            let ty = self.intern_type(concrete::TypeKind::Unit, true);
            self.coroutine_results.insert(ty);
        }
    }

    fn nominal_results(
        &self,
    ) -> impl Iterator<Item = (&export::HirNominalIdentity, concrete::TypeId)> {
        self.structs
            .iter()
            .map(|(_, d)| (&d.origin, d.canonical_type))
            .chain(
                self.enums
                    .iter()
                    .map(|(_, d)| (&d.origin, d.canonical_type)),
            )
            .chain(
                self.classes
                    .iter()
                    .map(|(_, d)| (&d.origin, d.canonical_type)),
            )
            .chain(
                self.interfaces
                    .iter()
                    .map(|(_, d)| (&d.origin, d.canonical_type)),
            )
            .chain(
                self.objects
                    .iter()
                    .map(|(_, d)| (&d.origin, self.object_types[d.object_type].canonical_type)),
            )
    }
}
