//! Projection of ordinary HIR into the M23-6 type-semantics transport.

use std::fmt;

use scoop_identity::{CallableTemplateOrigin, PersistentExactTypeId};

use crate::{
    CanonicalPersistentIdsV1, CrossConeHirInterfaceSectionV1, CrossConeTypeSemanticsSectionV1,
    ExactTypeFactShapeV1, NominalInheritanceEdgesV1, OrdinaryHirOutput, SourceNominalId,
    TypeSectionDependencyFactV1,
};

mod authority;
pub use authority::*;
mod facts;
mod inheritance;
mod nominals;

/// The transport plus the independently projected inventories needed by the
/// semantic validator. Keeping both products together prevents a driver from
/// deriving validation authority from the candidate section itself.
#[derive(Clone, Debug)]
pub struct CrossConeTypeSemanticsProductionV1 {
    section: CrossConeTypeSemanticsSectionV1,
    foundation: CrossConeTypeSemanticsFoundationV1,
    inheritance_inventory: crate::CanonicalSourceInheritanceInventoriesV1,
}

impl CrossConeTypeSemanticsProductionV1 {
    /// Projects the M23-6 HIR payload from the sealed Export/LocalConcrete
    /// pair and the M23-5 public interface produced from that same output.
    /// Unsupported dispatch, protected/default constructor sources, and
    /// generic materialization return typed capability errors before a
    /// partial section can be observed. M23-5 narrow dependency selections
    /// remain in their existing partition and do not populate field 8.
    pub fn from_ordinary_hir(
        output: &OrdinaryHirOutput<'_>,
        public: &CrossConeHirInterfaceSectionV1,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<Self, CrossConeTypeSemanticsProductionError> {
        nominals::produce(output, public, meter)
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
    ) {
        (self.section, self.foundation, self.inheritance_inventory)
    }

    pub const fn foundation(&self) -> &CrossConeTypeSemanticsFoundationV1 {
        &self.foundation
    }

    pub const fn inheritance_inventory(&self) -> &crate::CanonicalSourceInheritanceInventoriesV1 {
        &self.inheritance_inventory
    }

    pub fn source_roots(&self) -> &[SourceNominalId] {
        self.foundation.source_roots()
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
    UnsupportedDispatch(SourceNominalId),
    UnsupportedProtectedNominal(SourceNominalId),
    UnsupportedProtectedConstructor(SourceNominalId),
    MissingConstructor(PersistentExactTypeId),
    MissingSourceInterface(CallableTemplateOrigin),
    DefaultTemplateAuthorityRequired(CallableTemplateOrigin),
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
            Self::UnsupportedDispatch(owner) => write!(
                f,
                "nominal {owner:?} requires dispatch production not present in this HIR projection"
            ),
            Self::UnsupportedProtectedNominal(owner) => write!(
                f,
                "protected nominal {owner:?} requires protected nested-source production"
            ),
            Self::UnsupportedProtectedConstructor(owner) => write!(
                f,
                "nominal {owner:?} has a protected constructor that requires protected-source production"
            ),
            Self::MissingConstructor(owner) => {
                write!(f, "public constructor metadata for {owner} is missing")
            }
            Self::MissingSourceInterface(owner) => {
                write!(f, "source-call interface for {owner:?} is missing")
            }
            Self::DefaultTemplateAuthorityRequired(owner) => write!(
                f,
                "source-call interface {owner:?} requires M23-6 protected-default authority"
            ),
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
