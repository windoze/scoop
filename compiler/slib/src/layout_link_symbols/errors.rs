use super::*;

#[derive(Debug)]
pub enum LayoutLinkSymbolUseError {
    LinkSupport(crate::LinkSupportError),
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
    FinalMembers(CodeLinkObjectMemberValidationError),
    FinalProjection(LinkFinalObjectProjectionError),
    OrdinaryCoverage(CrossConeLinkClosureBuildError),
    CodeInput(Box<crate::SingleConeLinkSectionDecodeError>),
    ProductionProjection(crate::ProductionCodeProjectionError),
    CodeProjection(crate::CodeProductionProjectionError),
    CodeFingerprint(LayoutCodeFingerprintError),
    CodeFingerprintMismatch,
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
from_error!(crate::LinkSupportError, LinkSupport);
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
from_error!(CodeLinkObjectMemberValidationError, FinalMembers);
from_error!(LinkFinalObjectProjectionError, FinalProjection);
from_error!(CrossConeLinkClosureBuildError, OrdinaryCoverage);
from_error!(crate::ProductionCodeProjectionError, ProductionProjection);
from_error!(crate::CodeProductionProjectionError, CodeProjection);
from_error!(LayoutCodeFingerprintError, CodeFingerprint);

impl std::fmt::Display for LayoutLinkSymbolUseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Self::FinalMembers(source) = self {
            return write!(f, "invalid layout Link members: {source}");
        }
        write!(f, "invalid layout Link symbol uses: {self:?}")
    }
}

impl std::error::Error for LayoutLinkSymbolUseError {}
