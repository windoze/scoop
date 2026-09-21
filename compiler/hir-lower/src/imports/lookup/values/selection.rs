use super::{NonValueTarget, ValueOrigin, ValueTarget};
use crate::{
    Lowerer, NominalTarget,
    imports::lookup::{LookupLayer, LookupResult},
    imports::{CurrentUnitBindingId, CurrentUnitTarget, ImportLookupLayer},
    namespace::{TopLevelLookupLayer, TopLevelTypeTarget},
};
use scoop_ast as ast;
use scoop_hir as hir;

impl Lowerer {
    pub(super) fn non_value_origin(origin: &ValueOrigin) -> Option<NonValueTarget> {
        match origin {
            ValueOrigin::NonValue { target, .. } | ValueOrigin::CoreNonValue(target) => {
                Some(*target)
            }
            ValueOrigin::Dependency(binding) => match binding.target() {
                hir::ImportedTarget::Property(_) => None,
                target => Some(NonValueTarget::ImportedDependency(target)),
            },
            ValueOrigin::CurrentUnit(_)
            | ValueOrigin::Core(_)
            | ValueOrigin::RejectedFunction(_) => None,
        }
    }

    fn value_binding_origin(&self, binding: CurrentUnitBindingId) -> ValueOrigin {
        let target = match self.imports.binding(binding).target {
            CurrentUnitTarget::Function(id) => NonValueTarget::Function(id),
            CurrentUnitTarget::Property(id)
                if matches!(self.properties[id].owner, hir::PropertyOwner::Extension(_)) =>
            {
                NonValueTarget::ExtensionProperty(id)
            }
            CurrentUnitTarget::SourceProperty(id)
                if self.imports.source_extension_properties.contains(&id) =>
            {
                NonValueTarget::SourceExtensionProperty(id)
            }
            target
                if target.type_target().is_some()
                    && !matches!(target, CurrentUnitTarget::Object(_)) =>
            {
                NonValueTarget::Type(
                    target
                        .type_target()
                        .expect("a type binding has a typed target"),
                )
            }
            CurrentUnitTarget::Property(_)
            | CurrentUnitTarget::SourceProperty(_)
            | CurrentUnitTarget::Object(_)
            | CurrentUnitTarget::EnumVariant(_)
            | CurrentUnitTarget::SourceVariant(_) => return ValueOrigin::CurrentUnit(binding),
            _ => unreachable!("all nominal bindings carry a type target"),
        };
        ValueOrigin::NonValue { binding, target }
    }

    pub(crate) fn named_call_value_origin(
        &self,
        binding: &super::super::calls::NamedCallBinding,
    ) -> ValueOrigin {
        match &binding.origin {
            super::super::calls::NamedCallOrigin::CurrentUnit(id) => self.value_binding_origin(*id),
            super::super::calls::NamedCallOrigin::Core(target) => match *target {
                super::super::calls::NamedCallTarget::Function(id) => {
                    ValueOrigin::CoreNonValue(NonValueTarget::Function(id))
                }
                super::super::calls::NamedCallTarget::ImportedDependency(_) => {
                    unreachable!("core bindings cannot carry ordinary dependency targets")
                }
                super::super::calls::NamedCallTarget::Type(target) => {
                    ValueOrigin::CoreNonValue(NonValueTarget::Type(target))
                }
                super::super::calls::NamedCallTarget::Value(value) => ValueOrigin::Core(value),
                super::super::calls::NamedCallTarget::ExtensionProperty(id) => {
                    ValueOrigin::CoreNonValue(NonValueTarget::ExtensionProperty(id))
                }
            },
            super::super::calls::NamedCallOrigin::Dependency(binding) => {
                ValueOrigin::Dependency(binding.clone())
            }
        }
    }

    fn core_value_candidates(&self, name: &str) -> Vec<ValueOrigin> {
        let mut result = self
            .top_level_namespaces
            .property_layers(self.current_file, name)
            .into_iter()
            .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
            .flat_map(|layer| layer.candidates)
            .map(|id| ValueOrigin::Core(ValueTarget::Property(id)))
            .collect::<Vec<_>>();
        result.extend(
            self.top_level_namespaces
                .type_layers(self.current_file, name)
                .into_iter()
                .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
                .flat_map(|layer| layer.candidates)
                .map(|target| match target {
                    TopLevelTypeTarget::Nominal(NominalTarget::Object(id)) => {
                        ValueOrigin::Core(ValueTarget::Object(id))
                    }
                    target => ValueOrigin::CoreNonValue(NonValueTarget::Type(target)),
                }),
        );
        let (functions, _) = self.core_named_callable_partition(name);
        result.extend(
            functions
                .into_iter()
                .map(|id| ValueOrigin::CoreNonValue(NonValueTarget::Function(id))),
        );
        result.extend(
            self.top_level_namespaces
                .extension_property_layers(self.current_file, name)
                .into_iter()
                .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
                .flat_map(|layer| layer.candidates)
                .map(|id| ValueOrigin::CoreNonValue(NonValueTarget::ExtensionProperty(id))),
        );
        result.extend(
            self.core_prelude_variant_refs(name)
                .iter()
                .copied()
                .map(|target| ValueOrigin::Core(ValueTarget::Variant(target))),
        );
        result.extend(
            self.imports
                .prelude_value_bindings(name)
                .iter()
                .cloned()
                .map(ValueOrigin::Dependency),
        );
        result
    }

    fn value_origin_accessible(&self, origin: &ValueOrigin) -> bool {
        if matches!(
            origin,
            ValueOrigin::RejectedFunction(_) | ValueOrigin::Dependency(_)
        ) {
            return true;
        }
        if let ValueOrigin::CoreNonValue(target) = origin {
            return match *target {
                NonValueTarget::Function(id) => self.function_is_accessible(id, None),
                NonValueTarget::ImportedDependency(_) => {
                    unreachable!("core blockers cannot carry ordinary dependency targets")
                }
                NonValueTarget::Type(target) => self.top_level_type_target_is_accessible(target),
                NonValueTarget::ExtensionProperty(id) => {
                    self.access_domain_allows(&self.properties[id].access.lookup.0)
                }
                NonValueTarget::SourceExtensionProperty(_) => {
                    unreachable!("core has no provisional import properties")
                }
            };
        }
        let domain = match origin {
            ValueOrigin::CurrentUnit(id) | ValueOrigin::NonValue { binding: id, .. } => {
                &self.imports.binding(*id).access.0
            }
            ValueOrigin::Core(ValueTarget::Property(id)) => &self.properties[*id].access.lookup.0,
            ValueOrigin::Core(ValueTarget::Object(id)) => &self.objects[*id].access.lookup.0,
            ValueOrigin::Core(ValueTarget::Variant(target)) => {
                &self.enums[target.enumeration()].access.lookup.0
            }
            ValueOrigin::RejectedFunction(_) => {
                unreachable!("rejected-function blockers are accessible by construction")
            }
            ValueOrigin::Dependency(_) => {
                unreachable!("dependency values are accessible by construction")
            }
            ValueOrigin::CoreNonValue(_) => unreachable!("core blocker access was checked above"),
        };
        self.access_domain_allows(domain)
    }

    pub(crate) fn lookup_value_origin(&self, name: &str) -> LookupResult<ValueOrigin> {
        let core = || {
            let (_, suppressed_callables) = self.core_named_callable_partition(name);
            LookupLayer {
                kind: ImportLookupLayer::CorePrelude,
                suppressed_callables,
                candidates: self.core_value_candidates(name),
            }
        };
        let layers = match self
            .top_level_namespaces
            .source_namespace(self.current_file)
        {
            TopLevelLookupLayer::CorePrelude => vec![core()],
            TopLevelLookupLayer::CurrentPackage(package) => self
                .expression_import_layers(package, name)
                .into_iter()
                .map(|layer| {
                    if layer.kind == ImportLookupLayer::CorePrelude {
                        core()
                    } else {
                        LookupLayer {
                            kind: layer.kind,
                            suppressed_callables: layer.suppressed_callables,
                            candidates: layer
                                .bindings
                                .into_iter()
                                .map(|binding| self.value_binding_origin(binding))
                                .chain(
                                    layer
                                        .dependency_bindings
                                        .into_iter()
                                        .map(ValueOrigin::Dependency),
                                )
                                .collect(),
                        }
                    }
                })
                .collect(),
        };
        let mut inaccessible = Vec::new();
        for layer in layers {
            let mut visible = Vec::new();
            for origin in layer.candidates {
                let candidates = if self.value_origin_accessible(&origin) {
                    &mut visible
                } else {
                    &mut inaccessible
                };
                if !candidates.contains(&origin) {
                    candidates.push(origin);
                }
            }
            let has_value = visible
                .iter()
                .any(|origin| Self::non_value_origin(origin).is_none());
            if has_value {
                visible.retain(|origin| Self::non_value_origin(origin).is_none());
            } else if let Some(&function) = layer.suppressed_callables.first() {
                return LookupResult::Unique(ValueOrigin::RejectedFunction(function));
            }
            match visible.as_slice() {
                [] => {}
                [one] => return LookupResult::Unique(one.clone()),
                [first, rest @ ..] => {
                    return LookupResult::Ambiguous {
                        layer: layer.kind,
                        candidates: ast::NonEmptyVec::new(first.clone(), rest.to_vec()),
                    };
                }
            }
        }
        match inaccessible.as_slice() {
            [] => LookupResult::Missing,
            [first, rest @ ..] => {
                LookupResult::Inaccessible(ast::NonEmptyVec::new(first.clone(), rest.to_vec()))
            }
        }
    }
}
