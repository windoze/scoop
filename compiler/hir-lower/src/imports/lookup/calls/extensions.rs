//! Current-unit extension role lookup derived from named call layers.

use super::NamedCallTarget;
use crate::Lowerer;
use crate::imports::lookup::LookupLayer;
use crate::imports::{
    CurrentUnitTarget, ImportLookupLayer, ImportedTargetBinding, ResolvedNamespace,
};
use crate::namespace::TopLevelLookupLayer;
use scoop_hir as hir;

#[derive(Debug, Clone)]
pub(crate) enum ExtensionPropertyTarget {
    Current(hir::PropertyId),
    Dependency(hir::DirectImportedTargetBinding),
}

impl ExtensionPropertyTarget {
    pub(crate) const fn identity(&self) -> ExtensionPropertyIdentity {
        match self {
            Self::Current(property) => ExtensionPropertyIdentity::Current(*property),
            Self::Dependency(binding) => ExtensionPropertyIdentity::Dependency(binding.target()),
        }
    }
}

impl PartialEq for ExtensionPropertyTarget {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}

impl Eq for ExtensionPropertyTarget {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExtensionPropertyIdentity {
    Current(hir::PropertyId),
    Dependency(hir::ImportedTarget),
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
            suppressed_callables: Vec::new(),
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
                        .flat_map(|import| {
                            import
                                .targets
                                .iter()
                                .filter_map(ImportedTargetBinding::current_binding)
                        })
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
                        .flat_map(|bindings| {
                            bindings
                                .iter()
                                .filter_map(ImportedTargetBinding::current_binding)
                        })
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
                layers.push(LookupLayer {
                    kind,
                    candidates,
                    suppressed_callables: Vec::new(),
                });
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

    pub(crate) fn named_extension_call_layers(
        &self,
        name: &str,
    ) -> Vec<LookupLayer<hir::FunctionId>> {
        let mut layers = self
            .named_call_layers(name)
            .into_iter()
            .map(|layer| LookupLayer {
                kind: layer.kind,
                suppressed_callables: layer
                    .suppressed_callables
                    .into_iter()
                    .filter(|function| self.extension_receivers.contains_key(function))
                    .collect(),
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
            .collect::<Vec<_>>();
        if let Some(index) = layers
            .iter()
            .position(|layer| !layer.suppressed_callables.is_empty())
        {
            // Rejection is terminal only after the raw callable layer has
            // been narrowed to the extension role. Keep the rejecting layer
            // itself so valid peers in that same layer are still resolved.
            layers.truncate(index + 1);
        }
        layers
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
                suppressed_callables: layer
                    .suppressed_callables
                    .into_iter()
                    .filter(|function| !self.function_owner.contains_key(function))
                    .collect(),
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
    ) -> Vec<LookupLayer<ExtensionPropertyTarget>> {
        self.named_call_layers(name)
            .into_iter()
            .map(|layer| LookupLayer {
                kind: layer.kind,
                suppressed_callables: Vec::new(),
                candidates: layer
                    .candidates
                    .into_iter()
                    .filter_map(|binding| match (binding.target, binding.origin) {
                        (NamedCallTarget::ExtensionProperty(id), _) => {
                            Some(ExtensionPropertyTarget::Current(id))
                        }
                        (
                            NamedCallTarget::ImportedDependency(
                                hir::ImportedTarget::ExtensionProperty(_),
                            ),
                            super::NamedCallOrigin::Dependency(binding),
                        ) => Some(ExtensionPropertyTarget::Dependency(binding)),
                        _ => None,
                    })
                    .collect(),
            })
            .collect()
    }
}
