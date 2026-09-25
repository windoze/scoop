use super::*;

#[derive(Debug)]
pub enum LayoutLinkSymbolUseError {
    Resource(WireError),
    Defined(DefinedLinkSymbolOwnerBuildError),
    Native(lir::CanonicalNativeExternalRequirementBuildError),
    Current(CurrentConeUndefinedRequirementValidationError),
    Dependency(CrossConeStrongRequirementValidationError),
    Terminal(CrossConeLayoutTerminalValidationError),
    Shape(LayoutLinkClosureError),
    Source(SourceExternalRequirementValidationError),
    Runtime(RuntimeAndEhRequirementValidationError),
    Generated(GeneratedCBridgeSemanticValidationError),
    Target(CBridgeTargetSupportRequirementValidationError),
    Unclassified(BuiltinObjectExternalRequirementClosureError),
    Undefined(UndefinedSymbolRequirementFinalizationError),
    OrdinaryProjection(CrossConeLinkClosureSectionValidationError),
    SymbolProjection(LinkSymbolProjectionValidationError),
    RegistrationLeaves(crate::StrongLinkRegistrationLeafFingerprintError),
    RegistrationDependencies(crate::StrongLinkRegistrationDependencyFingerprintError),
    FinalObjects(crate::StrongLinkObjectFinalizationError),
    RuntimeProjection(crate::RuntimeProductionProjectionError),
    RuntimeFingerprintMismatch,
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for LayoutLinkSymbolUseError {
            fn from(value: $source) -> Self {
                Self::$variant(value)
            }
        }
    };
}
from_error!(WireError, Resource);
from_error!(DefinedLinkSymbolOwnerBuildError, Defined);
from_error!(lir::CanonicalNativeExternalRequirementBuildError, Native);
from_error!(CurrentConeUndefinedRequirementValidationError, Current);
from_error!(CrossConeStrongRequirementValidationError, Dependency);
from_error!(CrossConeLayoutTerminalValidationError, Terminal);
from_error!(LayoutLinkClosureError, Shape);
from_error!(SourceExternalRequirementValidationError, Source);
from_error!(RuntimeAndEhRequirementValidationError, Runtime);
from_error!(GeneratedCBridgeSemanticValidationError, Generated);
from_error!(CBridgeTargetSupportRequirementValidationError, Target);
from_error!(BuiltinObjectExternalRequirementClosureError, Unclassified);
from_error!(UndefinedSymbolRequirementFinalizationError, Undefined);
from_error!(
    CrossConeLinkClosureSectionValidationError,
    OrdinaryProjection
);
from_error!(LinkSymbolProjectionValidationError, SymbolProjection);
from_error!(
    crate::StrongLinkRegistrationLeafFingerprintError,
    RegistrationLeaves
);
from_error!(
    crate::StrongLinkRegistrationDependencyFingerprintError,
    RegistrationDependencies
);
from_error!(crate::StrongLinkObjectFinalizationError, FinalObjects);
from_error!(crate::RuntimeProductionProjectionError, RuntimeProjection);

impl std::fmt::Display for LayoutLinkSymbolUseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid layout Link symbol uses: {self:?}")
    }
}

impl std::error::Error for LayoutLinkSymbolUseError {}
