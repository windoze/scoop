use super::*;

#[derive(Debug)]
pub enum StrongDigestProjectionError {
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
    Safepoints(StrongSafepointSemanticPlanError),
    Types(StrongTypeDescriptorSemanticPlanBuildError),
    ImmortalObjects(StrongImmortalObjectSemanticPlanBuildError),
    InitializationUnits(StrongInitializationUnitSemanticPlanBuildError),
    DefinitionIdentity(ObjectDefinitionIdentityError),
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
    Plan(StrongDigestPlanBuildError),
}

impl fmt::Display for StrongDigestProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot project strong digest graph: {self:?}")
    }
}

impl std::error::Error for StrongDigestProjectionError {}

impl From<scoop_wire::WireError> for StrongDigestProjectionError {
    fn from(source: scoop_wire::WireError) -> Self {
        Self::Resource(source)
    }
}
