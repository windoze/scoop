//! Stable capability gates shared by all ordinary dependency consumers.

use scoop_hir as hir;
use scoop_identity::{CallableTemplateOrigin, Effect};

use crate::Lowerer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImportedCapabilityRequirement {
    Layout,
    Dispatch,
    Generic,
    Native,
}

impl ImportedCapabilityRequirement {
    pub(crate) const fn code(self) -> &'static str {
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

    pub(crate) fn diagnostic(self, subject: &str) -> String {
        format!("{}: {subject} requires {}", self.code(), self.description())
    }
}

pub(crate) fn callable_requirement(
    candidate: &dyn hir::ImportedCallableSource,
    has_vararg: bool,
) -> ImportedCapabilityRequirement {
    let interface = candidate.interface();
    if matches!(
        interface.effects().implementation(),
        hir::CallableImplementationV1::Intrinsic(hir::IntrinsicFunctionKind::Integer(
            hir::IntegerIntrinsicKind::ManagedOperation { .. }
        ))
    ) {
        ImportedCapabilityRequirement::Layout
    } else if matches!(interface.owner(), hir::PublicDeclarationOwnerV1::Nominal(_))
        || interface.access() == hir::PublicLookupAccessV1::PublicSlot
    {
        ImportedCapabilityRequirement::Dispatch
    } else if !interface.type_parameters().is_empty()
        || matches!(
            interface.declaration(),
            CallableTemplateOrigin::GenericFunction(_)
        )
        || interface.effects().execution() == Effect::Suspend
        || has_vararg
    {
        ImportedCapabilityRequirement::Generic
    } else if interface.effects().implementation() != hir::CallableImplementationV1::Scoop {
        ImportedCapabilityRequirement::Native
    } else {
        ImportedCapabilityRequirement::Layout
    }
}

impl Lowerer {
    pub(crate) fn imported_dependency_capability_error(
        &mut self,
        candidate: &dyn hir::ImportedCallableSource,
        has_vararg: bool,
        subject: &str,
        span: scoop_ast::Span,
    ) {
        let requirement = callable_requirement(candidate, has_vararg);
        self.error(span, requirement.diagnostic(subject));
    }
}
