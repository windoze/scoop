//! Typed declaration origins for named calls and extension scopes.

use super::values::{ValueOrigin, ValueTarget};
use super::*;
use scoop_hir as hir;

mod extensions;
mod qualifiers;

pub(crate) use extensions::{
    ExtensionCallTarget, ExtensionPropertyIdentity, ExtensionPropertyTarget, wire_operator,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NamedCallTarget {
    Function(hir::FunctionId),
    ImportedDependency(hir::ImportedTarget),
    Type(TopLevelTypeTarget),
    Value(ValueTarget),
    ExtensionProperty(hir::PropertyId),
}

#[derive(Debug, Clone)]
pub(crate) enum NamedCallOrigin {
    CurrentUnit(CurrentUnitBindingId),
    Core(NamedCallTarget),
    Dependency(hir::DirectImportedTargetBinding),
}

#[derive(Debug, Clone)]
pub(crate) struct NamedCallBinding {
    pub(crate) target: NamedCallTarget,
    pub(crate) origin: NamedCallOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExpressionQualifierTarget {
    Type(TopLevelTypeTarget),
    DependencyType(hir::ImportedTarget),
    Object(hir::ObjectId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExpressionQualifierLookup {
    Missing,
    Value,
    Unique(ExpressionQualifierTarget),
    Inaccessible(ExpressionQualifierTarget),
    Ambiguous,
}

impl ImportLookupLayer {
    pub(crate) fn call_rank(self) -> usize {
        match self {
            Self::Exact => 0,
            Self::CurrentPackage(_) => 1,
            Self::Star => 2,
            Self::CorePrelude => 3,
        }
    }

    pub(crate) fn call_name(self) -> &'static str {
        match self {
            Self::Exact => "exact import",
            Self::CurrentPackage(_) => "current package",
            Self::Star => "star import",
            Self::CorePrelude => "core prelude",
        }
    }
}

impl Lowerer {
    fn current_named_call_target(&self, binding: CurrentUnitBindingId) -> NamedCallTarget {
        match self.imports.binding(binding).target {
            CurrentUnitTarget::Function(id) => NamedCallTarget::Function(id),
            CurrentUnitTarget::Property(_)
            | CurrentUnitTarget::SourceProperty(_)
            | CurrentUnitTarget::Object(_)
            | CurrentUnitTarget::EnumVariant(_)
            | CurrentUnitTarget::SourceVariant(_) => {
                let value = self
                    .materialized_value_target(&ValueOrigin::CurrentUnit(binding))
                    .expect("named body calls follow complete declaration materialization");
                match value {
                    ValueTarget::Property(id)
                        if matches!(
                            self.properties[id].owner,
                            hir::PropertyOwner::Extension(_)
                        ) =>
                    {
                        NamedCallTarget::ExtensionProperty(id)
                    }
                    value => NamedCallTarget::Value(value),
                }
            }
            target => NamedCallTarget::Type(
                target
                    .type_target()
                    .expect("a nominal call binding carries a type target"),
            ),
        }
    }

    pub(super) fn core_named_callable_partition(
        &self,
        name: &str,
    ) -> (Vec<hir::FunctionId>, Vec<hir::FunctionId>) {
        self.top_level_namespaces
            .named_callable_layers(self.current_file, name)
            .into_iter()
            .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
            .flat_map(|layer| layer.candidates)
            .filter(|function| self.function_is_accessible(*function, None))
            .partition(|function| !self.declaration_surface.rejects_function(*function))
    }

    pub(super) fn core_named_call_targets(
        &self,
        name: &str,
    ) -> (Vec<NamedCallTarget>, Vec<hir::FunctionId>) {
        let (functions, suppressed_callables) = self.core_named_callable_partition(name);
        let mut targets = functions
            .into_iter()
            .map(NamedCallTarget::Function)
            .collect::<Vec<_>>();
        targets.extend(
            self.top_level_namespaces
                .type_layers(self.current_file, name)
                .into_iter()
                .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
                .flat_map(|layer| layer.candidates)
                .map(|target| match target {
                    TopLevelTypeTarget::Nominal(NominalTarget::Object(id)) => {
                        NamedCallTarget::Value(ValueTarget::Object(id))
                    }
                    target => NamedCallTarget::Type(target),
                }),
        );
        targets.extend(
            self.top_level_namespaces
                .property_layers(self.current_file, name)
                .into_iter()
                .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
                .flat_map(|layer| layer.candidates)
                .map(|id| NamedCallTarget::Value(ValueTarget::Property(id))),
        );
        targets.extend(
            self.top_level_namespaces
                .extension_property_layers(self.current_file, name)
                .into_iter()
                .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
                .flat_map(|layer| layer.candidates)
                .map(NamedCallTarget::ExtensionProperty),
        );
        targets.extend(
            self.core_prelude_variant_refs(name)
                .iter()
                .copied()
                .map(|target| NamedCallTarget::Value(ValueTarget::Variant(target))),
        );
        (targets, suppressed_callables)
    }

    pub(super) fn named_call_binding_accessible(&self, binding: &NamedCallBinding) -> bool {
        match &binding.origin {
            NamedCallOrigin::CurrentUnit(id) => {
                self.access_domain_allows(&self.imports.binding(*id).access.0)
            }
            NamedCallOrigin::Core(target) => match *target {
                NamedCallTarget::Function(id) => self.function_is_accessible(id, None),
                NamedCallTarget::ImportedDependency(_) => {
                    unreachable!("core bindings cannot carry ordinary dependency targets")
                }
                NamedCallTarget::Type(target) => self.top_level_type_target_is_accessible(target),
                NamedCallTarget::Value(ValueTarget::Property(id))
                | NamedCallTarget::ExtensionProperty(id) => {
                    self.access_domain_allows(&self.properties[id].access.lookup.0)
                }
                NamedCallTarget::Value(ValueTarget::Object(id)) => {
                    self.access_domain_allows(&self.objects[id].access.lookup.0)
                }
                NamedCallTarget::Value(ValueTarget::Variant(target)) => {
                    self.access_domain_allows(&self.enums[target.enumeration()].access.lookup.0)
                }
            },
            NamedCallOrigin::Dependency(_) => true,
        }
    }

    pub(crate) fn named_call_layers(&self, name: &str) -> Vec<LookupLayer<NamedCallBinding>> {
        let core = || {
            let (targets, suppressed_callables) = self.core_named_call_targets(name);
            LookupLayer {
                kind: ImportLookupLayer::CorePrelude,
                suppressed_callables,
                candidates: targets
                    .into_iter()
                    .map(|target| NamedCallBinding {
                        target,
                        origin: NamedCallOrigin::Core(target),
                    })
                    .chain(
                        self.imports
                            .prelude_bindings(scoop_identity::BindingNamespace::Value, name)
                            .iter()
                            .cloned()
                            .map(|binding| NamedCallBinding {
                                target: NamedCallTarget::ImportedDependency(binding.target()),
                                origin: NamedCallOrigin::Dependency(binding),
                            }),
                    )
                    .collect(),
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
                        let mut candidates = layer
                            .bindings
                            .into_iter()
                            .map(|id| NamedCallBinding {
                                target: self.current_named_call_target(id),
                                origin: NamedCallOrigin::CurrentUnit(id),
                            })
                            .collect::<Vec<_>>();
                        candidates.extend(layer.dependency_bindings.into_iter().map(|binding| {
                            NamedCallBinding {
                                target: NamedCallTarget::ImportedDependency(binding.target()),
                                origin: NamedCallOrigin::Dependency(binding),
                            }
                        }));
                        LookupLayer {
                            kind: layer.kind,
                            suppressed_callables: layer.suppressed_callables,
                            candidates,
                        }
                    }
                })
                .collect(),
        };
        layers
            .into_iter()
            .map(|layer| {
                let mut candidates = Vec::new();
                for binding in layer.candidates {
                    if self.named_call_binding_accessible(&binding)
                        && !candidates
                            .iter()
                            .any(|other: &NamedCallBinding| other.target == binding.target)
                    {
                        candidates.push(binding);
                    }
                }
                LookupLayer {
                    kind: layer.kind,
                    candidates,
                    suppressed_callables: layer.suppressed_callables,
                }
            })
            .collect()
    }
}
