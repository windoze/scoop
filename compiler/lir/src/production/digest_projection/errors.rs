use super::*;

#[derive(Debug)]
pub enum DigestProjectionError {
    Resource(scoop_wire::WireError),
    Encoding,
    ProducerMismatch {
        module: scoop_identity::ConeIdentity,
        foundation: scoop_identity::ConeIdentity,
    },
    EntryProducerMismatch {
        entry: scoop_identity::ConeIdentity,
        foundation: scoop_identity::ConeIdentity,
    },
    DefinitionIdentity(ObjectDefinitionIdentityError),
    MissingDefinitionPlan(ObjectDefinitionPlanId),
    MissingDefinition {
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    },
    PrimaryAtom {
        plan: ObjectDefinitionPlanId,
        source: DefinitionAtomResolutionError,
    },
    Identity(HashError),
    Node(DigestNodeBuildError),
    Plan(DigestPlanBuildError),
}

impl fmt::Display for DigestProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot project strong digest graph: {self:?}")
    }
}

impl std::error::Error for DigestProjectionError {}

impl From<scoop_wire::WireError> for DigestProjectionError {
    fn from(source: scoop_wire::WireError) -> Self {
        Self::Resource(source)
    }
}
