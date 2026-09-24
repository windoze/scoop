use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DefinitionOriginSubject, IdentityReferenceError,
    PersistentDispatchSlotId, PersistentExactTypeId, PersistentPropertyId, PersistentTypeId,
};
use scoop_wire::WireError;

use crate::{
    AccessDomainSemanticError, ExactTypeFactsSemanticError, InheritanceGraphError,
    NominalMaterializationClosureError, ProtectedDeclarationRefV1, SourceNominalId,
};

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
    InheritanceSource(SourceNominalId),
    InheritanceOrigin,
    InheritanceInventory(ConeIdentity),
    InheritanceEdges(PersistentExactTypeId),
    InheritanceGraph(Box<InheritanceGraphError<SharedTypeMetadataError>>),
    InheritanceDomains(AccessDomainSemanticError),
    DeclarationMetadata(DefinitionOriginSubject),
    CallableContract(CallableTemplateOrigin),
    ConstructorInventory(PersistentExactTypeId),
    ProtectedMemberInventory(PersistentExactTypeId),
    ProtectedMember(ProtectedDeclarationRefV1),
    ProtectedInventory(ConeIdentity),
    PropertyContract(PersistentPropertyId),
    NestedContract(SourceNominalId),
    NestedSupport(Box<crate::NestedSourceSemanticError<SharedTypeMetadataError>>),
    SourceProtocolInventory(ConeIdentity),
    SourceProtocolContract(CallableTemplateOrigin),
    SourceProtocols(Box<crate::ProtectedSourceClosureError<std::convert::Infallible>>),
    DefaultContract(crate::ProtectedDefaultTemplateKeyV1),
    DefaultReferences(crate::ProtectedDefaultTemplateKeyV1),
    DefaultWitness(crate::ProtectedDefaultTemplateKeyV1),
    DefaultBody(Box<crate::ProtectedDefaultBodyClosureError<SharedTypeMetadataError>>),
    DefinitionSources(Box<crate::TypeDefinitionSourceClosureError<SharedTypeMetadataError>>),
    DefinitionSourceLocation(crate::DefinitionSourceLocationValidationError),
    DispatchDeclarations(crate::NominalDispatchDeclarationError),
    SlotSource(PersistentDispatchSlotId),
    SlotOrder(PersistentExactTypeId),
    SlotSchemas(Box<crate::InheritanceSlotSchemaSemanticError<SharedTypeMetadataError>>),
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
