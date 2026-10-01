//! Finite result roots are captured before generating protocol applications.

use super::*;
use std::collections::BTreeSet;

impl Concretizer<'_> {
    pub(super) fn seed_shared_coroutine_results(&mut self) {
        let mut roots = self.shared_types.clone();
        let mut owned = BTreeSet::new();
        for (builtin, kind) in [
            (
                scoop_identity::CoreBuiltinNominal::Unit,
                concrete::TypeKind::Unit,
            ),
            (
                scoop_identity::CoreBuiltinNominal::Any,
                concrete::TypeKind::Any,
            ),
        ] {
            let source = builtin.identity_record();
            if source.key().origin() == self.source.cone {
                let gc_free = matches!(kind, concrete::TypeKind::Unit);
                let ty = self.intern_type(kind, gc_free);
                owned.insert(ty);
                if self.shape_sources.contains(&source.id()) {
                    roots.insert(ty);
                }
            }
        }
        let nominals = self
            .structs
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
            );
        for (origin, ty) in nominals {
            match origin {
                export::HirNominalIdentity::Source(export::HirSourceNominalIdentity::Concrete(
                    source,
                )) => {
                    if source.key().origin() == self.source.cone {
                        owned.insert(ty);
                        if self.shape_sources.contains(&source.id()) {
                            roots.insert(ty);
                        }
                    }
                }
                export::HirNominalIdentity::Source(export::HirSourceNominalIdentity::Generic(
                    _,
                )) => {
                    roots.insert(ty);
                }
                export::HirNominalIdentity::Generated(_) => continue,
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
        self.coroutine_results
            .extend(roots.intersection(&owned).copied());
    }
}
