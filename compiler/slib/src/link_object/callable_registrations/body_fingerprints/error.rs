use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongCallableBodyFingerprintError {
    InvalidAssociatedAtom {
        body: PersistentCallableBodyId,
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    MissingDefinitionAssignment {
        body: PersistentCallableBodyId,
    },
    MissingBodyDefinition {
        body: PersistentCallableBodyId,
    },
    MissingBodyPrimaryAtom {
        body: PersistentCallableBodyId,
    },
    InvalidBodyPrimaryAtomRange {
        body: PersistentCallableBodyId,
    },
    MissingObject(SlibMemberId),
    BodyRange {
        body: PersistentCallableBodyId,
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
            Self::StackmapNodeIdentity { source, .. } | Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}
