//! Typed declaration origins for named calls and extension scopes.

use super::values::{ValueOrigin, ValueTarget};
use super::*;
use scoop_hir as hir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NamedCallTarget {
    Function(hir::FunctionId),
    Type(TopLevelTypeTarget),
    Value(ValueTarget),
    ExtensionProperty(hir::PropertyId),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum NamedCallOrigin {
    CurrentUnit(CurrentUnitBindingId),
    Core(NamedCallTarget),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NamedCallBinding {
    pub(crate) target: NamedCallTarget,
    pub(crate) origin: NamedCallOrigin,
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
    fn named_extension_role_layers(&self) -> Vec<LookupLayer<hir::FunctionId>> {
        let core = LookupLayer {
            kind: ImportLookupLayer::CorePrelude,
            candidates: self
                .top_level_namespaces
                .all_extension_layers(self.current_file)
                .into_iter()
                .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
                .flat_map(|layer| layer.candidates)
                .collect(),
        };
        let mut layers = Vec::new();
        if let TopLevelLookupLayer::CurrentPackage(package) = self
            .top_level_namespaces
            .source_namespace(self.current_file)
        {
            let imports = &self.imports.files[self.current_file];
            let origins = [
                (
                    ImportLookupLayer::Exact,
                    imports
                        .exact
                        .iter()
                        .flat_map(|import| import.targets.iter().map(|target| target.binding))
                        .collect::<Vec<_>>(),
                ),
                (
                    ImportLookupLayer::CurrentPackage(package),
                    self.imports
                        .namespaces
                        .get(&ResolvedNamespace::Package(package))
                        .into_iter()
                        .flat_map(|namespace| namespace.values())
                        .flatten()
                        .copied()
                        .collect(),
                ),
                (
                    ImportLookupLayer::Star,
                    imports
                        .stars
                        .iter()
                        .flat_map(|import| import.snapshot.values())
                        .flat_map(|bindings| bindings.iter().map(|binding| binding.binding))
                        .collect(),
                ),
            ];
            for (kind, origins) in origins {
                let candidates = origins
                    .into_iter()
                    .filter_map(|id| {
                        let binding = self.imports.binding(id);
                        let CurrentUnitTarget::Function(function) = binding.target else {
                            return None;
                        };
                        (self.extension_receivers.contains_key(&function)
                            && self.access_domain_allows(&binding.access.0, None))
                        .then_some(function)
                    })
                    .collect();
                layers.push(LookupLayer { kind, candidates });
            }
        }
        layers.push(core);
        for layer in &mut layers {
            layer
                .candidates
                .retain(|function| self.function_is_accessible(*function, None));
            layer
                .candidates
                .sort_by_key(|function| function.into_raw().into_u32());
            layer.candidates.dedup();
        }
        layers
    }

    pub(crate) fn named_extension_operator_layers(
        &self,
        operator: hir::OperatorKind,
    ) -> Vec<LookupLayer<hir::FunctionId>> {
        self.named_extension_role_layers()
            .into_iter()
            .map(|mut layer| {
                layer.candidates.retain(|function| {
                    self.signatures[function].modifiers.operator == Some(operator)
                });
                layer
            })
            .collect()
    }

    pub(crate) fn named_extension_delegate_operator_layers(
        &self,
        role: hir::PropertyDelegateOperatorKind,
    ) -> Vec<LookupLayer<hir::FunctionId>> {
        self.named_extension_role_layers()
            .into_iter()
            .map(|mut layer| {
                layer.candidates.retain(|function| {
                    self.signatures[function]
                        .modifiers
                        .property_delegate_operator
                        == Some(role)
                });
                layer
            })
            .collect()
    }

    fn current_named_call_target(&self, binding: CurrentUnitBindingId) -> NamedCallTarget {
        match self.imports.binding(binding).target {
            CurrentUnitTarget::Function(id) => NamedCallTarget::Function(id),
            CurrentUnitTarget::Property(_)
            | CurrentUnitTarget::SourceProperty(_)
            | CurrentUnitTarget::Object(_)
            | CurrentUnitTarget::EnumVariant(_)
            | CurrentUnitTarget::SourceVariant(_) => {
                let value = self
                    .materialized_value_target(ValueOrigin::CurrentUnit(binding))
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

    fn core_named_call_targets(&self, name: &str) -> Vec<NamedCallTarget> {
        let mut targets = self
            .top_level_namespaces
            .named_callable_layers(self.current_file, name)
            .into_iter()
            .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
            .flat_map(|layer| layer.candidates)
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
        targets
    }

    fn named_call_binding_accessible(&self, binding: NamedCallBinding) -> bool {
        match binding.origin {
            NamedCallOrigin::CurrentUnit(id) => {
                self.access_domain_allows(&self.imports.binding(id).access.0, None)
            }
            NamedCallOrigin::Core(target) => match target {
                NamedCallTarget::Function(id) => self.function_is_accessible(id, None),
                NamedCallTarget::Type(target) => self.top_level_type_target_is_accessible(target),
                NamedCallTarget::Value(ValueTarget::Property(id))
                | NamedCallTarget::ExtensionProperty(id) => {
                    self.access_domain_allows(&self.properties[id].access.lookup.0, None)
                }
                NamedCallTarget::Value(ValueTarget::Object(id)) => {
                    self.access_domain_allows(&self.objects[id].access.lookup.0, None)
                }
                NamedCallTarget::Value(ValueTarget::Variant(target)) => self
                    .access_domain_allows(&self.enums[target.enumeration()].access.lookup.0, None),
            },
        }
    }

    pub(crate) fn named_call_layers(&self, name: &str) -> Vec<LookupLayer<NamedCallBinding>> {
        let core = || LookupLayer {
            kind: ImportLookupLayer::CorePrelude,
            candidates: self
                .core_named_call_targets(name)
                .into_iter()
                .map(|target| NamedCallBinding {
                    target,
                    origin: NamedCallOrigin::Core(target),
                })
                .collect(),
        };
        let layers = match self
            .top_level_namespaces
            .source_namespace(self.current_file)
        {
            TopLevelLookupLayer::CorePrelude => vec![core()],
            TopLevelLookupLayer::CurrentPackage(package) => self
                .imports
                .layers(self.current_file, package, name)
                .into_iter()
                .map(|layer| {
                    if layer.kind == ImportLookupLayer::CorePrelude {
                        core()
                    } else {
                        LookupLayer {
                            kind: layer.kind,
                            candidates: layer
                                .bindings
                                .into_iter()
                                .map(|id| NamedCallBinding {
                                    target: self.current_named_call_target(id),
                                    origin: NamedCallOrigin::CurrentUnit(id),
                                })
                                .collect(),
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
                    if self.named_call_binding_accessible(binding)
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
                }
            })
            .collect()
    }

    pub(crate) fn named_extension_call_layers(
        &self,
        name: &str,
    ) -> Vec<LookupLayer<hir::FunctionId>> {
        self.named_call_layers(name)
            .into_iter()
            .map(|layer| LookupLayer {
                kind: layer.kind,
                candidates: layer
                    .candidates
                    .into_iter()
                    .filter_map(|binding| match binding.target {
                        NamedCallTarget::Function(id)
                            if self.extension_receivers.contains_key(&id) =>
                        {
                            Some(id)
                        }
                        _ => None,
                    })
                    .collect(),
            })
            .collect()
    }

    /// The declaration candidates visible to an unqualified callable
    /// reference. Ordinary functions and extension functions intentionally
    /// remain in the same scope layer: the latter are interpreted as unbound
    /// references by the callable-reference resolver.
    pub(crate) fn named_callable_reference_layers(
        &self,
        name: &str,
    ) -> Vec<LookupLayer<hir::FunctionId>> {
        self.named_call_layers(name)
            .into_iter()
            .map(|layer| LookupLayer {
                kind: layer.kind,
                candidates: layer
                    .candidates
                    .into_iter()
                    .filter_map(|binding| match binding.target {
                        NamedCallTarget::Function(id) if !self.function_owner.contains_key(&id) => {
                            Some(id)
                        }
                        _ => None,
                    })
                    .collect(),
            })
            .collect()
    }

    pub(crate) fn named_extension_property_layers(
        &self,
        name: &str,
    ) -> Vec<LookupLayer<hir::PropertyId>> {
        self.named_call_layers(name)
            .into_iter()
            .map(|layer| LookupLayer {
                kind: layer.kind,
                candidates: layer
                    .candidates
                    .into_iter()
                    .filter_map(|binding| match binding.target {
                        NamedCallTarget::ExtensionProperty(id) => Some(id),
                        _ => None,
                    })
                    .collect(),
            })
            .collect()
    }
}
