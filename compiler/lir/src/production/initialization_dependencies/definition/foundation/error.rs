use scoop_identity::{
    DigestNodeId, DigestNodeKey, ObjectDefinitionIdentityError, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PersistentInitializationUnitId, PersistentSymbolError,
    PersistentSymbolRequest,
};

#[derive(Debug)]
pub enum InitializationDefinitionResolutionErrorV2 {
    Resource(scoop_wire::WireError),
    MissingRegistrationIdentity(PersistentInitializationUnitId),
    DefinitionKey(ObjectDefinitionIdentityError),
    MissingDefinition(ObjectDefinitionPlanKey),
    Symbol(PersistentSymbolError),
    MissingSymbol(PersistentSymbolRequest),
    PrimaryAtoms(ObjectDefinitionPlanId),
    AssociatedAtoms(ObjectDefinitionPlanId),
    RegistrationDefinition {
        expected: ObjectDefinitionPlanId,
        actual: ObjectDefinitionPlanId,
    },
    MissingFingerprint(DigestNodeKey),
    RegistrationFingerprint {
        expected: DigestNodeId,
        actual: DigestNodeId,
    },
}
impl From<scoop_wire::WireError> for InitializationDefinitionResolutionErrorV2 {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for InitializationDefinitionResolutionErrorV2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid initialization definition reference: {self:?}")
    }
}
impl std::error::Error for InitializationDefinitionResolutionErrorV2 {}
