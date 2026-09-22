use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongCallableBodyFingerprintError {
    InvalidConstantAtom {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    ObjectProofMismatch,
    UndefinedRequirementProofMismatch,
    ObjectValidation(StrongCallableRegistrationValidationError),
    ProofCoverageMismatch,
    MissingDefinitionAssignment {
        body: PersistentCallableBodyId,
    },
    MissingBodyDefinition {
        body: PersistentCallableBodyId,
    },
    BodyPrimaryAtomMismatch {
        body: PersistentCallableBodyId,
    },
    MissingBodyPrimaryAtom {
        body: PersistentCallableBodyId,
    },
    MissingRuntimeScanPlan {
        body: PersistentCallableBodyId,
    },
    RuntimeScanAtomSetMismatch {
        body: PersistentCallableBodyId,
        expected: Vec<scoop_identity::ObjectDefinitionAtomId>,
        actual: Vec<scoop_identity::ObjectDefinitionAtomId>,
    },
    MissingRuntimeScanAtom {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    InvalidRuntimeScanAtomRange {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    RuntimeScanSectionMismatch {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
        actual: BuiltinObjectSectionRoleV1,
    },
    RuntimeScanBytesMismatch {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    RuntimeScanRelocationMismatch {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    RuntimeScanPlanTree {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    InvalidBodyPrimaryAtomRange {
        body: PersistentCallableBodyId,
    },
    BodyPrimarySectionMismatch {
        body: PersistentCallableBodyId,
        actual: BuiltinObjectSectionRoleV1,
    },
    MissingObject(SlibMemberId),
    BodyRange {
        body: PersistentCallableBodyId,
    },
    MissingBodyDigestNode {
        body: PersistentCallableBodyId,
    },
    BodyDirectInputMismatch {
        body: PersistentCallableBodyId,
    },
    UnsupportedBodyDirectInput {
        body: PersistentCallableBodyId,
        kind: DigestKind,
    },
    MissingStackmapInput {
        body: PersistentCallableBodyId,
        node: DigestNodeId,
    },
    StackmapNodeIdentity {
        site: scoop_identity::PersistentSafepointSiteId,
        source: HashError,
    },
    Relocation {
        body: PersistentCallableBodyId,
        kind: ObjectDefinitionRelocationFailureV1,
    },
    Hash {
        body: PersistentCallableBodyId,
        source: HashError,
    },
}

impl fmt::Display for StrongCallableBodyFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong callable body object fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongCallableBodyFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ObjectValidation(source) => Some(source),
            Self::StackmapNodeIdentity { source, .. } | Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}
