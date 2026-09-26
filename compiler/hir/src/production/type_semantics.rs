//! Projection of ordinary HIR into the M23-6 type-semantics transport.

use std::fmt;

use scoop_identity::PersistentExactTypeId;

use crate::{
    CanonicalPersistentIdsV1, CrossConeTypeSemanticsSectionV1, DependencyHirOutput,
    ExactTypeFactShapeV1, NominalInheritanceEdgesV1, SourceNominalId, TypeSectionDependencyFactV1,
};

mod authority;
pub use authority::*;
mod facts;
pub(in crate::production) mod inheritance;
mod nested_sources;
mod protected_sources;
pub use protected_sources::*;
mod nominal_callables;
mod nominal_constructors;
mod nominal_parameters;
pub use nested_sources::*;
pub(in crate::production) use nominals::declaration_access_for_subject;
mod nominals;
mod source_parameter_shapes;

/// The transport plus the independently projected inventories needed by the
/// semantic validator. Keeping both products together prevents a driver from
/// deriving validation authority from the candidate section itself.
#[derive(Clone, Debug)]
pub struct CrossConeTypeSemanticsProductionV1 {
    section: CrossConeTypeSemanticsSectionV1,
    foundation: CrossConeTypeSemanticsFoundationV1,
    inheritance_inventory: crate::CanonicalSourceInheritanceInventoriesV1,
    interface_sources: crate::CanonicalInterfaceSourceDispatchesV1,
    slot_selections: crate::CanonicalInheritanceSourceSlotSelectionsV1,
    source_callables: crate::CanonicalInheritanceSourceCallablesV1,
    source_constructors: crate::CanonicalInheritanceSourceConstructorsV1,
    source_protected_callables: crate::CanonicalInheritanceSourceProtectedCallablesV1,
    source_properties: crate::CanonicalInheritanceSourcePropertiesV1,
    source_parameters: crate::CanonicalInheritanceSourceParameterProtocolsV1,
    source_nominals: crate::CanonicalNominalSourceContractsV1,
}

impl CrossConeTypeSemanticsProductionV1 {
    /// Projects the M23-6 HIR payload from the sealed Export/LocalConcrete
    /// pair and the M23-5 public interface produced from that same output.
    /// Members, slots and default bodies share the resolved source projection.
    /// Declarations with generic machine dependencies remain source-only;
    /// closed nominal roots retain complete representation and inheritance.
    /// Actual generic materialization still requires M23-7. Type requirements
    /// use the same shared declaration metadata as the Compile reader.
    pub fn from_dependency_hir(
        output: &DependencyHirOutput,
        metadata: crate::SharedTypeMetadataV1<'_>,
        dependencies: &[crate::SharedTypeMetadataV1<'_>],
    ) -> Result<Self, CrossConeTypeSemanticsProductionError> {
        nominals::produce(output, metadata, dependencies)
    }

    pub const fn section(&self) -> &CrossConeTypeSemanticsSectionV1 {
        &self.section
    }

    pub fn into_parts(
        self,
    ) -> (
        CrossConeTypeSemanticsSectionV1,
        CrossConeTypeSemanticsFoundationV1,
        crate::CanonicalSourceInheritanceInventoriesV1,
        crate::CanonicalInterfaceSourceDispatchesV1,
        crate::CanonicalInheritanceSourceSlotSelectionsV1,
        crate::CanonicalInheritanceSourceCallablesV1,
        crate::CanonicalInheritanceSourceConstructorsV1,
        crate::CanonicalInheritanceSourceProtectedCallablesV1,
        crate::CanonicalInheritanceSourcePropertiesV1,
        crate::CanonicalInheritanceSourceParameterProtocolsV1,
        crate::CanonicalNominalSourceContractsV1,
    ) {
        (
            self.section,
            self.foundation,
            self.inheritance_inventory,
            self.interface_sources,
            self.slot_selections,
            self.source_callables,
            self.source_constructors,
            self.source_protected_callables,
            self.source_properties,
            self.source_parameters,
            self.source_nominals,
        )
    }

    pub const fn foundation(&self) -> &CrossConeTypeSemanticsFoundationV1 {
        &self.foundation
    }

    pub const fn inheritance_inventory(&self) -> &crate::CanonicalSourceInheritanceInventoriesV1 {
        &self.inheritance_inventory
    }

    pub const fn interface_sources(&self) -> &crate::CanonicalInterfaceSourceDispatchesV1 {
        &self.interface_sources
    }

    pub const fn slot_selections(&self) -> &crate::CanonicalInheritanceSourceSlotSelectionsV1 {
        &self.slot_selections
    }

    pub const fn source_callables(&self) -> &crate::CanonicalInheritanceSourceCallablesV1 {
        &self.source_callables
    }

    pub const fn source_constructors(&self) -> &crate::CanonicalInheritanceSourceConstructorsV1 {
        &self.source_constructors
    }

    pub const fn source_protected_callables(
        &self,
    ) -> &crate::CanonicalInheritanceSourceProtectedCallablesV1 {
        &self.source_protected_callables
    }

    pub fn source_roots(&self) -> &[SourceNominalId] {
        self.foundation.source_roots()
    }

    pub const fn source_properties(&self) -> &crate::CanonicalInheritanceSourcePropertiesV1 {
        &self.source_properties
    }

    pub const fn source_parameters(
        &self,
    ) -> &crate::CanonicalInheritanceSourceParameterProtocolsV1 {
        &self.source_parameters
    }

    pub const fn source_nominals(&self) -> &crate::CanonicalNominalSourceContractsV1 {
        &self.source_nominals
    }

    pub const fn local_exact_facts(&self) -> &CanonicalPersistentIdsV1<PersistentExactTypeId> {
        self.foundation.local_exact_facts()
    }

    pub fn dependency_facts(&self) -> &[TypeSectionDependencyFactV1] {
        self.foundation.dependency_facts()
    }

    pub fn local_inheritance_edges(&self) -> &[NominalInheritanceEdgesV1] {
        self.foundation.local_inheritance_edges()
    }

    pub fn fact_shape(&self, exact: PersistentExactTypeId) -> Option<&ExactTypeFactShapeV1> {
        self.foundation.fact_shape(exact)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeSemanticsNominalKind {
    Struct,
    Enum,
    Class,
    Interface,
    Object,
}

#[derive(Debug)]
pub enum CrossConeTypeSemanticsProductionError {
    SourceInventory(crate::SourceInventoryError),
    SharedTypeMetadata(Box<crate::SharedTypeMetadataError>),
    PublicInterface(String),
    MissingNominalIdentity {
        kind: TypeSemanticsNominalKind,
        index: u32,
    },
    GeneratedPublicNominal {
        kind: TypeSemanticsNominalKind,
        index: u32,
    },
    MissingDefinitionOrigin(scoop_identity::DefinitionOriginSubject),
    InvalidLexicalOwner(scoop_identity::DefinitionOwnerAtom),
    InvalidSourceDeclaration(String),
    MissingExactIdentity,
    ExactIdentityMismatch(PersistentExactTypeId),
    MissingConcreteType(PersistentExactTypeId),
    MissingLocalSupport(PersistentExactTypeId),
    GenericOdrRequired(PersistentExactTypeId),
    MissingConstructor(PersistentExactTypeId),
    InvalidSourceShape {
        declaration: SourceNominalId,
        reason: String,
    },
    InvalidFact {
        exact: PersistentExactTypeId,
        reason: String,
    },
    InvalidRepresentation {
        declaration: SourceNominalId,
        reason: String,
    },
    InvalidInheritance {
        exact: PersistentExactTypeId,
        reason: String,
    },
    InvalidTable {
        table: &'static str,
        reason: String,
    },
}

impl fmt::Display for CrossConeTypeSemanticsProductionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceInventory(error) => error.fmt(f),
            Self::SharedTypeMetadata(error) => error.fmt(f),
            Self::PublicInterface(reason) => {
                write!(f, "cannot project the public interface: {reason}")
            }
            Self::MissingNominalIdentity { kind, index } => {
                write!(f, "missing {kind:?} identity at arena index {index}")
            }
            Self::GeneratedPublicNominal { kind, index } => {
                write!(
                    f,
                    "public {kind:?} at arena index {index} is compiler-generated"
                )
            }
            Self::MissingDefinitionOrigin(subject) => {
                write!(f, "missing definition origin for {subject:?}")
            }
            Self::InvalidLexicalOwner(owner) => write!(f, "invalid lexical owner {owner:?}"),
            Self::InvalidSourceDeclaration(reason) => {
                write!(f, "invalid persistent source declaration: {reason}")
            }
            Self::MissingExactIdentity => {
                f.write_str("a parameter-free source nominal has no exact identity")
            }
            Self::ExactIdentityMismatch(exact) => write!(
                f,
                "source nominal exact identity {exact} disagrees between Export and LocalConcrete HIR"
            ),
            Self::MissingConcreteType(exact) => {
                write!(f, "exact type {exact} is absent from LocalConcrete HIR")
            }
            Self::MissingLocalSupport(exact) => write!(
                f,
                "exact type {exact} needs local source support that is not exported"
            ),
            Self::GenericOdrRequired(exact) => write!(
                f,
                "exact type {exact} requires generic ODR materialization from M23-7"
            ),
            Self::MissingConstructor(owner) => {
                write!(f, "source constructor metadata for {owner} is missing")
            }
            Self::InvalidSourceShape {
                declaration,
                reason,
            } => write!(f, "invalid source shape for {declaration:?}: {reason}"),
            Self::InvalidFact { exact, reason } => {
                write!(f, "invalid exact facts for {exact}: {reason}")
            }
            Self::InvalidRepresentation {
                declaration,
                reason,
            } => write!(f, "invalid representation for {declaration:?}: {reason}"),
            Self::InvalidInheritance { exact, reason } => {
                write!(f, "invalid inheritance interface for {exact}: {reason}")
            }
            Self::InvalidTable { table, reason } => write!(f, "invalid {table} table: {reason}"),
        }
    }
}

impl std::error::Error for CrossConeTypeSemanticsProductionError {}
