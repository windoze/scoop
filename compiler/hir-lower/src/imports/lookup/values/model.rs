use crate::{imports::CurrentUnitBindingId, namespace::TopLevelTypeTarget};
use scoop_hir as hir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueTarget {
    Property(hir::PropertyId),
    Object(hir::ObjectId),
    Variant(hir::EnumVariantRef),
}

#[derive(Debug, Clone)]
pub(crate) enum ResolvedValueTarget {
    Materialized(ValueTarget),
    Dependency(hir::DirectImportedTargetBinding),
}

/// A declaration-side origin is usable even during static-image preflight,
/// before every source property has been allocated. It is never an HIR value.
#[derive(Debug, Clone)]
pub(crate) enum ValueOrigin {
    CurrentUnit(CurrentUnitBindingId),
    NonValue {
        binding: CurrentUnitBindingId,
        target: NonValueTarget,
    },
    Core(ValueTarget),
    CoreNonValue(NonValueTarget),
    Dependency(hir::DirectImportedTargetBinding),
    /// A duplicate-signature declaration owns this value spelling but cannot
    /// be exposed as a semantic value/callable candidate.
    RejectedFunction(hir::FunctionId),
}

impl PartialEq for ValueOrigin {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::CurrentUnit(left), Self::CurrentUnit(right)) => left == right,
            (
                Self::NonValue {
                    binding: left_binding,
                    target: left_target,
                },
                Self::NonValue {
                    binding: right_binding,
                    target: right_target,
                },
            ) => left_binding == right_binding && left_target == right_target,
            (Self::Core(left), Self::Core(right)) => left == right,
            (Self::CoreNonValue(left), Self::CoreNonValue(right)) => left == right,
            (Self::Dependency(left), Self::Dependency(right)) => {
                left.binding_target() == right.binding_target() && left.target() == right.target()
            }
            (Self::RejectedFunction(left), Self::RejectedFunction(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for ValueOrigin {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NonValueTarget {
    Function(hir::FunctionId),
    ImportedDependency(hir::ImportedTarget),
    Type(TopLevelTypeTarget),
    ExtensionProperty(hir::PropertyId),
    SourceExtensionProperty(crate::imports::SourcePropertyId),
}

pub(crate) enum NamedPropertyReceiver {
    None,
    Singleton {
        owner: hir::MethodOwnerApplication,
        value: hir::Expr,
    },
}

impl NamedPropertyReceiver {
    pub(crate) fn parts(self) -> (Option<hir::MethodOwnerApplication>, Option<hir::Expr>) {
        match self {
            Self::None => (None, None),
            Self::Singleton { owner, value } => (Some(owner), Some(value)),
        }
    }
}
