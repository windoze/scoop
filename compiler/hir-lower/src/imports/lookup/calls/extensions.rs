//! Current-unit extension role lookup derived from named call layers.

use super::{NamedCallOrigin, NamedCallTarget};
use crate::Lowerer;
use crate::imports::lookup::LookupLayer;
use crate::imports::{
    CurrentUnitTarget, ImportLookupLayer, ImportedTargetBinding, ResolvedNamespace,
};
use crate::namespace::TopLevelLookupLayer;
use scoop_ast as ast;
use scoop_hir as hir;

#[derive(Debug, Clone)]
pub(crate) enum ExtensionCallTarget {
    Current(hir::FunctionId),
    Dependency(hir::DirectImportedTargetBinding),
}

impl ExtensionCallTarget {
    const fn identity(&self) -> ExtensionCallIdentity {
        match self {
            Self::Current(function) => ExtensionCallIdentity::Current(*function),
            Self::Dependency(binding) => ExtensionCallIdentity::Dependency(binding.target()),
        }
    }
}

impl PartialEq for ExtensionCallTarget {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}

impl Eq for ExtensionCallTarget {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtensionCallIdentity {
    Current(hir::FunctionId),
    Dependency(hir::ImportedTarget),
}

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
                            && self.access_domain_allows(&binding.access.0))
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

    pub(crate) fn named_executable_extension_operator_layers(
        &self,
        operator: hir::OperatorKind,
    ) -> Vec<LookupLayer<ExtensionCallTarget>> {
        self.named_executable_extension_role_layers(|target| {
            self.extension_target_has_operator(target, operator)
        })
    }

    fn named_executable_extension_role_layers(
        &self,
        matches_role: impl Fn(&ExtensionCallTarget) -> bool,
    ) -> Vec<LookupLayer<ExtensionCallTarget>> {
        let mut layers = self
            .named_extension_role_layers()
            .into_iter()
            .map(|layer| LookupLayer {
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
        self.named_executable_extension_role_layers(|target| match target {
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
        })
    }

    pub(crate) fn named_executable_extension_call_layers(
        &self,
        name: &str,
    ) -> Vec<LookupLayer<ExtensionCallTarget>> {
        self.named_call_layers(name)
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
                    .filter_map(|binding| match (binding.target, binding.origin) {
                        (NamedCallTarget::Function(function), _)
                            if self.extension_receivers.contains_key(&function) =>
                        {
                            Some(ExtensionCallTarget::Current(function))
                        }
                        (
                            NamedCallTarget::ImportedDependency(
                                hir::ImportedTarget::Function(_)
                                | hir::ImportedTarget::GenericFunction(_),
                            ),
                            NamedCallOrigin::Dependency(binding),
                        ) if self.imported_dependency_callable_is_extension(&binding) => {
                            Some(ExtensionCallTarget::Dependency(binding))
                        }
                        _ => None,
                    })
                    .collect(),
            })
            .collect()
    }

    fn imported_dependency_callable_is_extension(
        &self,
        binding: &hir::DirectImportedTargetBinding,
    ) -> bool {
        self.dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.callable_candidate(binding).ok())
            .is_some_and(|candidate| {
                candidate.interface().owner() == hir::PublicDeclarationOwnerV1::Extension
            })
    }

    fn extension_target_has_operator(
        &self,
        target: &ExtensionCallTarget,
        operator: hir::OperatorKind,
    ) -> bool {
        match target {
            ExtensionCallTarget::Current(function) => {
                self.signatures[function].modifiers.operator == Some(operator)
            }
            ExtensionCallTarget::Dependency(binding) => self
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.callable_candidate(binding).ok())
                .is_some_and(|candidate| {
                    candidate.interface().effects().operator_role()
                        == hir::CallableOperatorRoleV1::Language(wire_operator(operator))
                }),
        }
    }

    /// The declaration candidates visible to an unqualified callable
    /// reference. Ordinary functions and extension functions intentionally
    /// remain in the same scope layer: the latter are interpreted as unbound
    /// references by the callable-reference resolver.
    pub(crate) fn named_callable_reference_layers(
        &self,
        name: &str,
    ) -> Vec<LookupLayer<super::NamedCallBinding>> {
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
                    .filter(|binding| match binding.target {
                        NamedCallTarget::Function(id) => !self.function_owner.contains_key(&id),
                        NamedCallTarget::ImportedDependency(
                            hir::ImportedTarget::Function(_)
                            | hir::ImportedTarget::GenericFunction(_),
                        ) => true,
                        _ => false,
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

pub(crate) const fn wire_operator(operator: hir::OperatorKind) -> hir::CallableOperatorV1 {
    match operator {
        hir::OperatorKind::UnaryPlus => hir::CallableOperatorV1::UnaryPlus,
        hir::OperatorKind::UnaryMinus => hir::CallableOperatorV1::UnaryMinus,
        hir::OperatorKind::Not => hir::CallableOperatorV1::Not,
        hir::OperatorKind::Inc => hir::CallableOperatorV1::Inc,
        hir::OperatorKind::Dec => hir::CallableOperatorV1::Dec,
        hir::OperatorKind::Plus => hir::CallableOperatorV1::Plus,
        hir::OperatorKind::Minus => hir::CallableOperatorV1::Minus,
        hir::OperatorKind::Times => hir::CallableOperatorV1::Times,
        hir::OperatorKind::Div => hir::CallableOperatorV1::Div,
        hir::OperatorKind::Rem => hir::CallableOperatorV1::Rem,
        hir::OperatorKind::RangeTo => hir::CallableOperatorV1::RangeTo,
        hir::OperatorKind::RangeUntil => hir::CallableOperatorV1::RangeUntil,
        hir::OperatorKind::Contains => hir::CallableOperatorV1::Contains,
        hir::OperatorKind::Get => hir::CallableOperatorV1::Get,
        hir::OperatorKind::Set => hir::CallableOperatorV1::Set,
        hir::OperatorKind::Invoke => hir::CallableOperatorV1::Invoke,
        hir::OperatorKind::PlusAssign => hir::CallableOperatorV1::PlusAssign,
        hir::OperatorKind::MinusAssign => hir::CallableOperatorV1::MinusAssign,
        hir::OperatorKind::TimesAssign => hir::CallableOperatorV1::TimesAssign,
        hir::OperatorKind::DivAssign => hir::CallableOperatorV1::DivAssign,
        hir::OperatorKind::RemAssign => hir::CallableOperatorV1::RemAssign,
        hir::OperatorKind::CompareTo => hir::CallableOperatorV1::CompareTo,
        hir::OperatorKind::Equals => hir::CallableOperatorV1::Equals,
        hir::OperatorKind::Component { index } => hir::CallableOperatorV1::Component { index },
        hir::OperatorKind::Iterator => hir::CallableOperatorV1::Iterator,
    }
}
