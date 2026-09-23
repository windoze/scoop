use scoop_identity::{
    ConeIdentity, IdentityReferenceError, PersistentExactTypeId, PersistentTypeId,
};
use scoop_wire::WireError;

use crate::{ExactTypeFactsSemanticError, NominalMaterializationClosureError};

#[derive(Debug)]
pub enum SharedTypeMetadataError {
    Resource(WireError),
    Identity(IdentityReferenceError),
    Key(String),
    Materialization(NominalMaterializationClosureError),
    MissingProvider(ConeIdentity),
    DuplicateProvider(ConeIdentity),
    CurrentProviderDependency(ConeIdentity),
    MissingNominal(PersistentTypeId),
    MissingFact(PersistentExactTypeId),
    ForeignFact(PersistentExactTypeId),
    FactInventory,
    ByValueCycle(PersistentExactTypeId),
    NonConcreteSignature,
    GenericFact(PersistentExactTypeId),
    IntrinsicNominal(PersistentTypeId),
    NominalOwner(PersistentTypeId),
    RepresentationInventory,
    Representation(PersistentTypeId),
    DeclarationSource(PersistentTypeId),
    ClassBase(PersistentTypeId),
    Facts(Box<ExactTypeFactsSemanticError<SharedTypeMetadataError>>),
}

impl From<WireError> for SharedTypeMetadataError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl From<IdentityReferenceError> for SharedTypeMetadataError {
    fn from(error: IdentityReferenceError) -> Self {
        Self::Identity(error)
    }
}

impl std::fmt::Display for SharedTypeMetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared HIR type foundation: {self:?}")
    }
}

impl std::error::Error for SharedTypeMetadataError {}
