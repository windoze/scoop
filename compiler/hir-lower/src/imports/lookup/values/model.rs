use crate::{imports::CurrentUnitBindingId, namespace::TopLevelTypeTarget};
use scoop_hir as hir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueTarget {
    Property(hir::PropertyId),
    Object(hir::ObjectId),
    Variant(hir::EnumVariantRef),
}

/// A declaration-side origin is usable even during static-image preflight,
/// before every source property has been allocated. It is never an HIR value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueOrigin {
    CurrentUnit(CurrentUnitBindingId),
    NonValue {
        binding: CurrentUnitBindingId,
        target: NonValueTarget,
    },
    Core(ValueTarget),
    CoreNonValue(NonValueTarget),
    DependencyNonValue(hir::ImportedTarget),
    /// A duplicate-signature declaration owns this value spelling but cannot
    /// be exposed as a semantic value/callable candidate.
    RejectedFunction(hir::FunctionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NonValueTarget {
    Function(hir::FunctionId),
    ImportedCoreCallable(hir::ImportedCorePreludeRef),
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
