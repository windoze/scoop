use super::*;

#[derive(Debug)]
pub enum LayoutLinkObjectContentsError {
    CompileView,
    Resource(scoop_wire::WireError),
    ObjectEnvelope {
        member: crate::SlibMemberId,
        source: ObjectEnvelopeValidationError,
    },
    CBridgeProduction(lir::CBridgeProductionValidationError),
    CBridgeEnvelopes(CBridgeProductionEnvelopeValidationError),
    DigestInputs(LinkDigestPatchInputValidationError),
    Normalization(FinalObjectNormalizationError),
    SymbolPlan(StrongObjectSymbolPlanningError),
    Objects(BuiltinObjectSetValidationError),
    DigestSites(DigestPatchSiteValidationError),
    ObjectProjection(LinkObjectProjectionValidationError),
    Stackmaps(ScoopLirStackmapValidationError),
    Safepoints(StrongSafepointRegistrationValidationError),
    Callables(StrongCallableRegistrationValidationError),
    Types(StrongTypeRegistrationValidationError),
    Immortals(StrongImmortalObjectRegistrationValidationError),
    Storages(StrongStaticStorageRegistrationValidationError),
    Initializations(StrongInitializationRegistrationValidationError),
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for LayoutLinkObjectContentsError {
            fn from(value: $source) -> Self {
                Self::$variant(value)
            }
        }
    };
}
from_error!(scoop_wire::WireError, Resource);
from_error!(lir::CBridgeProductionValidationError, CBridgeProduction);
from_error!(CBridgeProductionEnvelopeValidationError, CBridgeEnvelopes);
from_error!(LinkDigestPatchInputValidationError, DigestInputs);
from_error!(FinalObjectNormalizationError, Normalization);
from_error!(StrongObjectSymbolPlanningError, SymbolPlan);
from_error!(BuiltinObjectSetValidationError, Objects);
from_error!(DigestPatchSiteValidationError, DigestSites);
from_error!(LinkObjectProjectionValidationError, ObjectProjection);
from_error!(ScoopLirStackmapValidationError, Stackmaps);
from_error!(StrongSafepointRegistrationValidationError, Safepoints);
from_error!(StrongCallableRegistrationValidationError, Callables);
from_error!(StrongTypeRegistrationValidationError, Types);
from_error!(StrongImmortalObjectRegistrationValidationError, Immortals);
from_error!(StrongStaticStorageRegistrationValidationError, Storages);
from_error!(
    StrongInitializationRegistrationValidationError,
    Initializations
);

impl std::fmt::Display for LayoutLinkObjectContentsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid layout Link object contents: {self:?}")
    }
}

impl std::error::Error for LayoutLinkObjectContentsError {}
