//! Stable HIR capability gates shared by dependency calls and accessors.

use scoop_hir as hir;
use scoop_identity::{CallableTemplateOrigin, Effect};

use super::named_calls::imported_dependency::ImportedArgumentMap;
use crate::Lowerer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::expr) enum ImportedCapabilityRequirement {
    Layout,
    Dispatch,
    Generic,
    Native,
}

impl ImportedCapabilityRequirement {
    pub(in crate::expr) const fn code(self) -> &'static str {
        match self {
            Self::Layout => "SCOOP_HIR_CROSS_CONE_LAYOUT_REQUIRED",
            Self::Dispatch => "SCOOP_HIR_CROSS_CONE_DISPATCH_REQUIRED",
            Self::Generic => "SCOOP_HIR_CROSS_CONE_GENERIC_REQUIRED",
            Self::Native => "SCOOP_HIR_CROSS_CONE_NATIVE_REQUIRED",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Layout => "layout/ABI capability from M23-6",
            Self::Dispatch => "member/dispatch capability from M23-6",
            Self::Generic => "generic/ODR capability from M23-7",
            Self::Native => "native closure capability from M23-10",
        }
    }

    pub(in crate::expr) fn diagnostic(self, subject: &str) -> String {
        format!("{}: {subject} requires {}", self.code(), self.description())
    }
}

pub(in crate::expr) fn callable_requirement(
    candidate: &hir::ImportedDependencyCallableCandidate,
    arguments: Option<&ImportedArgumentMap>,
) -> ImportedCapabilityRequirement {
    let interface = candidate.interface();
    if matches!(interface.owner(), hir::PublicDeclarationOwnerV1::Nominal(_))
        || interface.access() == hir::PublicLookupAccessV1::PublicSlot
    {
        ImportedCapabilityRequirement::Dispatch
    } else if !interface.type_parameters().is_empty()
        || matches!(
            interface.declaration(),
            CallableTemplateOrigin::GenericFunction(_)
        )
        || interface.effects().execution() == Effect::Suspend
        || arguments.is_some_and(ImportedArgumentMap::has_vararg)
    {
        ImportedCapabilityRequirement::Generic
    } else if interface.effects().implementation() != hir::CallableImplementationV1::Scoop {
        ImportedCapabilityRequirement::Native
    } else {
        ImportedCapabilityRequirement::Layout
    }
}

impl Lowerer {
    pub(in crate::expr) fn imported_dependency_capability_error(
        &mut self,
        candidate: &hir::ImportedDependencyCallableCandidate,
        arguments: Option<&ImportedArgumentMap>,
        subject: &str,
        span: scoop_ast::Span,
    ) {
        let requirement = callable_requirement(candidate, arguments);
        self.error(span, requirement.diagnostic(subject));
    }
}
