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
    TypeUseRelations(Box<crate::HirDependencyTypeRelationError>),
    CallSignature {
        position: crate::concrete::ExecutableExpressionPosition,
        source: Box<crate::HirDependencyCallSignatureError>,
    },
    CallReceiver {
        position: Box<crate::concrete::ExecutableExpressionPosition>,
        receiver: crate::SourceCallReceiver<PersistentExactTypeId>,
        expected: PersistentExactTypeId,
    },
    RuntimeConstructor(Box<crate::HirRuntimeConstructorError>),
    TypeUseInventory,
    SourceOnlyNominal(PersistentTypeId),
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
    CallableClassification(crate::NominalCallableClassificationError),
    ConstructorInventory(PersistentExactTypeId),
    ObjectInitializationOwner(PersistentTypeId),
    DuplicateObjectInitialization(PersistentTypeId),
    ProtectedMemberInventory(PersistentExactTypeId),
    ProtectedMember(ProtectedDeclarationRefV1),
    ProtectedInventory(ConeIdentity),
    PropertyContract(PersistentPropertyId),
    NestedContract(SourceNominalId),
    DefinitionSourceLocation(crate::DefinitionSourceLocationValidationError),
    DispatchDeclarations(crate::NominalDispatchDeclarationError),
    SlotSource(PersistentDispatchSlotId),
    SlotOrder(PersistentExactTypeId),
    SlotSchemas(Box<crate::InheritanceSlotSchemaSemanticError<SharedTypeMetadataError>>),
    SlotCallable(crate::InheritanceCallableDeclarationV1),
    SlotSelectionInventory(PersistentExactTypeId),
    SlotSelection {
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    },
    SlotContracts(Box<crate::InheritanceInterfaceSemanticError<SharedTypeMetadataError>>),
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

impl From<NominalMaterializationClosureError> for SharedTypeMetadataError {
    fn from(error: NominalMaterializationClosureError) -> Self {
        Self::Materialization(error)
    }
}

impl std::fmt::Display for SharedTypeMetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared HIR type foundation: {self:?}")
    }
}

impl std::error::Error for SharedTypeMetadataError {}
