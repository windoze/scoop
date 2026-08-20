//! Errors from object planning, verification, and fingerprint production.

use super::*;

#[derive(Debug)]
pub enum BuiltinObjectProductionError {
    ReadObject {
        producer: BuiltinObjectProducerV1,
        path: PathBuf,
        source: std::io::Error,
    },
    GeneratedBridgePlanMismatch,
    CBridgeEnvelopes(CBridgeProductionEnvelopeValidationError),
    StrongSymbolPlan(StrongObjectSymbolPlanningError),
    StrongRelocations(BuiltinObjectSetValidationError),
    DigestPatchSites(DigestPatchSiteValidationError),
    Stackmaps(ScoopLirStackmapValidationError),
    SafepointRegistrations(StrongSafepointRegistrationValidationError),
    CallableRegistrations(StrongCallableRegistrationValidationError),
    TypeRegistrations(StrongTypeRegistrationValidationError),
    ImmortalObjectRegistrations(StrongImmortalObjectRegistrationValidationError),
    StaticStorageRegistrations(StrongStaticStorageRegistrationValidationError),
    InitializationRegistrations(StrongInitializationRegistrationValidationError),
    SafepointFingerprints(StrongSafepointFingerprintError),
    DefinedSymbols(DefinedLinkSymbolOwnerBuildError),
    CurrentConeRequirements(CurrentConeUndefinedRequirementValidationError),
    NativeRequirementSurface(CanonicalNativeExternalRequirementBuildError),
    DependencyRequirements(CrossConeStrongRequirementValidationError),
    SourceExternalRequirements(SourceExternalRequirementValidationError),
    RuntimeAndEhRequirements(RuntimeAndEhRequirementValidationError),
    GeneratedBridgeSemantics(GeneratedCBridgeSemanticValidationError),
    CBridgeTargetSupportRequirements(CBridgeTargetSupportRequirementValidationError),
    UnclassifiedExternalRequirement(BuiltinObjectExternalRequirementClosureError),
    UndefinedSymbols(UndefinedSymbolRequirementFinalizationError),
    CallableBodyFingerprints(StrongCallableBodyFingerprintError),
    CallableFingerprints(StrongCallableFingerprintError),
    TypeFingerprints(StrongTypeFingerprintError),

    ImmortalObjectFingerprints(StrongImmortalObjectFingerprintError),

    StaticStorageShapeFingerprints(StrongStaticStorageShapeFingerprintError),
    StaticStorageFingerprints(StrongStaticStorageFingerprintError),

    InitializationFingerprints(StrongInitializationFingerprintError),
    ConeImage(ConeImageValidationError),
    EntryProduction(EntryProductionValidationError),
    RegistrationPatch(StrongRegistrationPatchError),
    Compatibility(scoop_wire::HashError),
    RuntimeImageFingerprint(RuntimeImageFingerprintError),
    RuntimeImagePatch(RuntimeImagePatchError),
    EntryPatch(EntryPatchError),
    MissingFinalMemberPlan(SlibMemberId),
    FinalMember(SlibMemberRecordError),
    FinalMemberIdMismatch {
        expected: SlibMemberId,
        actual: SlibMemberId,
    },
    CodeLinkObjects(CodeLinkObjectMemberValidationError),
    ProductionCodeProjection(ProductionCodeProjectionError),
    CodeFingerprint(CodeFingerprintError),
    Units {
        producer: BuiltinObjectProducerV1,
        source: ObjectUnitSetError,
    },
    MemberPlan(LinkObjectMemberSetPlanError),
    EmptyObjectUnits,
    UnassignedObjectDefinition(ObjectDefinitionPlanId),
    UnassignedGeneratedBridgeUnit(GeneratedBridgeUnitId),
    SplitObjectUnits(SlibMemberId),
    MissingPlannedMember(SlibMemberId),
    UnassignedPatchDefinition(ObjectDefinitionPlanId),
    PatchMemberMismatch {
        definition: ObjectDefinitionPlanId,
        expected: SlibMemberId,
        actual: SlibMemberId,
    },
    DuplicateDigestIntent(scoop_identity::DigestPatchIntentId),
    EmptyDigestMaterializationSet,
}

impl fmt::Display for BuiltinObjectProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid built-in object production: {self:?}")
    }
}

impl std::error::Error for BuiltinObjectProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadObject { source, .. } => Some(source),
            Self::CBridgeEnvelopes(source) => Some(source),
            Self::StrongSymbolPlan(source) => Some(source),
            Self::StrongRelocations(source) => Some(source),
            Self::DigestPatchSites(source) => Some(source),
            Self::Stackmaps(source) => Some(source),
            Self::SafepointRegistrations(source) => Some(source),
            Self::CallableRegistrations(source) => Some(source),
            Self::TypeRegistrations(source) => Some(source),
            Self::ImmortalObjectRegistrations(source) => Some(source),
            Self::StaticStorageRegistrations(source) => Some(source),
            Self::InitializationRegistrations(source) => Some(source),
            Self::SafepointFingerprints(source) => Some(source),
            Self::DefinedSymbols(source) => Some(source),
            Self::CurrentConeRequirements(source) => Some(source),
            Self::NativeRequirementSurface(source) => Some(source),
            Self::DependencyRequirements(source) => Some(source),
            Self::SourceExternalRequirements(source) => Some(source),
            Self::RuntimeAndEhRequirements(source) => Some(source),
            Self::GeneratedBridgeSemantics(source) => Some(source),
            Self::CBridgeTargetSupportRequirements(source) => Some(source),
            Self::UnclassifiedExternalRequirement(source) => Some(source),
            Self::UndefinedSymbols(source) => Some(source),
            Self::CallableBodyFingerprints(source) => Some(source),
            Self::CallableFingerprints(source) => Some(source),
            Self::TypeFingerprints(source) => Some(source),

            Self::ImmortalObjectFingerprints(source) => Some(source),

            Self::StaticStorageShapeFingerprints(source) => Some(source),
            Self::StaticStorageFingerprints(source) => Some(source),

            Self::InitializationFingerprints(source) => Some(source),
            Self::ConeImage(source) => Some(source),
            Self::EntryProduction(source) => Some(source),
            Self::RegistrationPatch(source) => Some(source),
            Self::Compatibility(source) => Some(source),
            Self::RuntimeImageFingerprint(source) => Some(source),
            Self::RuntimeImagePatch(source) => Some(source),
            Self::EntryPatch(source) => Some(source),
            Self::FinalMember(source) => Some(source),
            Self::CodeLinkObjects(source) => Some(source),
            Self::ProductionCodeProjection(source) => Some(source),
            Self::CodeFingerprint(source) => Some(source),
            Self::Units { source, .. } => Some(source),
            Self::MemberPlan(source) => Some(source),
            _ => None,
        }
    }
}
