//! Extension lookup by the ordinary closed operator roles.

use super::*;
use crate::callable_modifiers::{CallableOperatorKind, callable_operator_kind};
use crate::imports::{
    CurrentUnitTarget, ImportLookupLayer, ImportedTargetBinding, ResolvedNamespace,
};
use crate::namespace::TopLevelLookupLayer;

impl Lowerer {
    fn named_extension_role_layers(
        &self,
        role: CallableOperatorKind,
    ) -> Vec<LookupLayer<hir::FunctionId>> {
        let core = LookupLayer {
            unavailable: Vec::new(),
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
                let unavailable = self
                    .unavailable_bindings_matching(&origins, |kind| {
                        matches!(
                            kind,
                            ast::DeclarationHeaderKind::Function {
                                is_extension: true,
                                is_operator: true,
                                ..
                            }
                        )
                    })
                    .into_iter()
                    .filter(|&binding| {
                        callable_operator_kind(&self.imports.binding(binding).name).ok()
                            == Some(role)
                    })
                    .collect();
                let candidates = origins
                    .into_iter()
                    .filter_map(|id| {
                        let binding = self.imports.binding(id);
                        let CurrentUnitTarget::Function(function) = binding.target else {
                            return None;
                        };
                        (self.declaration_source_in_scope(binding.file)
                            && self.extension_receivers.contains_key(&function)
                            && self.access_domain_allows(&binding.access.0))
                        .then_some(function)
                    })
                    .collect();
                layers.push(LookupLayer {
                    unavailable,
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

    pub(crate) fn named_executable_extension_operator_layers(
        &self,
        operator: hir::OperatorKind,
    ) -> Vec<LookupLayer<ExtensionCallTarget>> {
        self.named_executable_extension_role_layers(
            CallableOperatorKind::Ordinary(operator),
            |target| self.extension_target_has_operator(target, operator),
        )
    }

    fn named_executable_extension_role_layers(
        &self,
        role: CallableOperatorKind,
        matches_role: impl Fn(&ExtensionCallTarget) -> bool,
    ) -> Vec<LookupLayer<ExtensionCallTarget>> {
        let mut layers = self
            .named_extension_role_layers(role)
            .into_iter()
            .map(|layer| LookupLayer {
                unavailable: layer.unavailable,
                kind: layer.kind,
                candidates: layer
                    .candidates
                    .into_iter()
                    .map(ExtensionCallTarget::Current)
                    .filter(&matches_role)
                    .collect(),
                suppressed_callables: layer.suppressed_callables,
            })
            .collect::<Vec<_>>();
        let TopLevelLookupLayer::CurrentPackage(package) = self
            .top_level_namespaces
            .source_namespace(self.current_file)
        else {
            return layers;
        };
        let imports = &self.imports.files[self.current_file];
        let dependency_origins = [
            (
                ImportLookupLayer::Exact,
                imports
                    .exact
                    .iter()
                    .flat_map(|import| import.targets.iter())
                    .filter_map(ImportedTargetBinding::direct_binding)
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
            (
                ImportLookupLayer::CurrentPackage(package),
                imports
                    .current_package_dependencies
                    .values()
                    .flatten()
                    .cloned()
                    .collect(),
            ),
            (
                ImportLookupLayer::Star,
                imports
                    .stars
                    .iter()
                    .flat_map(|import| import.snapshot.values())
                    .flat_map(ast::NonEmptyVec::iter)
                    .filter_map(ImportedTargetBinding::direct_binding)
                    .cloned()
                    .collect(),
            ),
        ];
        for (kind, bindings) in dependency_origins {
            let layer = layers
                .iter_mut()
                .find(|layer| layer.kind == kind)
                .expect("current sources have every non-core extension layer");
            layer.candidates.extend(
                self.imports
                    .canonicalize_dependencies(bindings)
                    .into_iter()
                    .map(ExtensionCallTarget::Dependency)
                    .filter(|target| {
                        self.imported_dependency_callable_is_extension(match target {
                            ExtensionCallTarget::Dependency(binding) => binding,
                            ExtensionCallTarget::Current(_) => {
                                unreachable!("the appended targets are dependencies")
                            }
                        }) && matches_role(target)
                    }),
            );
        }
        layers
    }

    pub(crate) fn named_extension_delegate_operator_layers(
        &self,
        role: hir::PropertyDelegateOperatorKind,
    ) -> Vec<LookupLayer<ExtensionCallTarget>> {
        let wire_role = match role {
            hir::PropertyDelegateOperatorKind::ProvideDelegate => {
                hir::PropertyDelegateOperatorV1::ProvideDelegate
            }
            hir::PropertyDelegateOperatorKind::GetValue => {
                hir::PropertyDelegateOperatorV1::GetValue
            }
            hir::PropertyDelegateOperatorKind::SetValue => {
                hir::PropertyDelegateOperatorV1::SetValue
            }
        };
        self.named_executable_extension_role_layers(
            CallableOperatorKind::PropertyDelegate(role),
            |target| match target {
                ExtensionCallTarget::Current(function) => {
                    self.signatures[function]
                        .modifiers
                        .property_delegate_operator
                        == Some(role)
                }
                ExtensionCallTarget::Dependency(binding) => self
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.callable_candidate(binding).ok())
                    .is_some_and(|candidate| {
                        candidate.interface().effects().operator_role()
                            == hir::CallableOperatorRoleV1::PropertyDelegate(wire_role)
                    }),
            },
        )
    }
}
