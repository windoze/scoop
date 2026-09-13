use std::fmt;

use scoop_identity::{
    GeneratedBridgeAtomId, GeneratedBridgeUnitId, NativeExternalContractFingerprint,
    ObjectDefinitionAtomId, ObjectDefinitionPlanId,
};
use scoop_lir::{CBridgeTargetSupportRegistryError, RuntimeRequirementRegistryError};

use crate::SlibMemberId;

#[derive(Debug)]
pub enum GeneratedCBridgeSemanticValidationError {
    BridgeProducerMismatch {
        object: scoop_identity::ConeIdentity,
        bridge: scoop_identity::ConeIdentity,
    },
    NativeProducerMismatch {
        object: scoop_identity::ConeIdentity,
        native: scoop_identity::ConeIdentity,
    },
    ProductionMismatch,
    TargetSupportRegistry(CBridgeTargetSupportRegistryError),
    RuntimeRegistry(RuntimeRequirementRegistryError),
    MissingUnitMember(GeneratedBridgeUnitId),
    MissingExpectedUnit(GeneratedBridgeUnitId),
    MissingObservedUnitState(GeneratedBridgeUnitId),
    InvalidBridgeAtomIdentity(GeneratedBridgeAtomId),
    InvalidObjectAtomIdentity(ObjectDefinitionPlanId),
    DuplicateBridgeDefinition(ObjectDefinitionPlanId),
    MultipleSignatureDescriptors(GeneratedBridgeUnitId),
    UnexpectedSignatureDescriptor(GeneratedBridgeUnitId),
    SignatureDescriptorMismatch(GeneratedBridgeUnitId),
    MissingSignatureDescriptor(GeneratedBridgeUnitId),
    DefinitionCoverageMismatch {
        member: SlibMemberId,
    },
    DefinitionAtomShapeMismatch {
        member: SlibMemberId,
        definition: ObjectDefinitionPlanId,
    },
    MissingActualDefinition {
        member: SlibMemberId,
        definition: ObjectDefinitionPlanId,
    },
    UnexpectedBridgeRelocationAtom {
        member: SlibMemberId,
        atom: ObjectDefinitionAtomId,
    },
    BridgeAtomMemberMismatch {
        atom: ObjectDefinitionAtomId,
        expected: SlibMemberId,
        actual: SlibMemberId,
    },
    AssociatedAtomRelocation {
        unit: GeneratedBridgeUnitId,
        atom: ObjectDefinitionAtomId,
    },
    CrossAtomLocalRelocation {
        unit: GeneratedBridgeUnitId,
        source: ObjectDefinitionAtomId,
        target: ObjectDefinitionAtomId,
    },
    SectionBaseRelocation {
        unit: GeneratedBridgeUnitId,
        atom: ObjectDefinitionAtomId,
    },
    MissingNativeContract {
        unit: GeneratedBridgeUnitId,
        contract: NativeExternalContractFingerprint,
    },
    NativeContractKindMismatch(GeneratedBridgeUnitId),
    InvalidStaticStorageBridgeIdentity(GeneratedBridgeUnitId),
    UnexpectedPrimaryRelocation {
        unit: GeneratedBridgeUnitId,
        atom: ObjectDefinitionAtomId,
        symbol: Vec<u8>,
    },
    MissingRequiredRelocation(GeneratedBridgeUnitId),
}

impl fmt::Display for GeneratedCBridgeSemanticValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid generated C bridge semantics: {self:?}")
    }
}

impl std::error::Error for GeneratedCBridgeSemanticValidationError {}
