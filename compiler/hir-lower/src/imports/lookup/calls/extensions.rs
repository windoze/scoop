//! Current-unit extension role lookup derived from named call layers.

mod operators;

use super::{NamedCallOrigin, NamedCallTarget};
use crate::Lowerer;
use crate::imports::lookup::LookupLayer;
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
    pub(crate) fn named_executable_extension_call_layers(
        &self,
        name: &str,
    ) -> Vec<LookupLayer<ExtensionCallTarget>> {
        self.named_call_layers(name)
            .into_iter()
            .map(|layer| LookupLayer {
                unavailable: self.unavailable_bindings_matching(&layer.unavailable, |kind| {
                    matches!(
                        kind,
                        ast::DeclarationHeaderKind::Function {
                            is_extension: true,
                            ..
                        }
                    )
                }),
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
                unavailable: layer.unavailable,
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
                            | hir::ImportedTarget::GenericFunction(_)
                            | hir::ImportedTarget::Type(_)
                            | hir::ImportedTarget::GenericType(_)
                            | hir::ImportedTarget::TypeAlias(_),
                        ) => true,
                        NamedCallTarget::Type(_)
                        | NamedCallTarget::Value(
                            crate::imports::lookup::values::ValueTarget::Object(_),
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
                unavailable: self.unavailable_bindings_matching(&layer.unavailable, |kind| {
                    matches!(
                        kind,
                        ast::DeclarationHeaderKind::Property { is_extension: true }
                    )
                }),
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
