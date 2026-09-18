use std::fmt;

use scoop_identity::{
    BindableEntity, CanonicalIdentifierError, ConeIdentity, DefinitionOwnerAtom,
    NominalDeclarationOwner, PersistentConstructorId, PersistentExportBindingId,
    SourceDeclarationKind,
};

use crate::{
    CanonicalPersistentIdSetBuildError, EnumSourceVariantBuildError,
    HirInterfaceSignatureProjectionError, NominalInterfaceRecordBuildError,
    NominalInterfaceSetBuildError, NominalSourceShapeBuildError, PublicMemberRefBuildError,
    PublicNominalKindV1, SignatureTypeSetBuildError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalArenaKind {
    Class,
    Interface,
    Struct,
    Enum,
    Object,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalConstructorProjectionError {
    Unknown(u32),
    OwnerMismatch {
        actual: u32,
    },
    MissingIdentity(u32),
    OrphanGeneratedAdapter {
        adapter: u32,
        source: u32,
    },
    ForeignDeclaration {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    InvalidDeclarationScope,
    PersistentOwnerMismatch {
        expected: DefinitionOwnerAtom,
        actual: Option<DefinitionOwnerAtom>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalMemberProjectionError {
    UnknownFunction(u32),
    MissingFunctionIdentity(u32),
    NonSourceFunction(u32),
    UnknownProperty(u32),
    MissingPropertyIdentity(u32),
    ExtensionProperty(u32),
    UnknownInterfaceMethod(u32),
    InterfaceMethodOwner {
        expected: u32,
        actual: u32,
    },
    InterfacePropertyAccessor(u32),
    MissingMethodOwner(u32),
    HirMethodOwnerMismatch {
        function: u32,
        actual: Option<NominalDeclarationOwner>,
    },
    HirPropertyOwnerMismatch {
        property: u32,
        actual: Option<NominalDeclarationOwner>,
    },
    ForeignDeclaration {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    InvalidDeclarationScope,
    PersistentOwnerMismatch {
        expected: DefinitionOwnerAtom,
        actual: Option<DefinitionOwnerAtom>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalNestedBindingProjectionError {
    MissingIdentity { kind: NominalArenaKind, index: u32 },
    GeneratedIdentity { kind: NominalArenaKind, index: u32 },
    MissingBinding(BindableEntity),
    NonCurrentBinding(PersistentExportBindingId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalSourceProjectionError {
    TooManyStructFields,
    MissingStructFieldIdentity { field: u32 },
    TooManyEnumVariants,
    MissingEnumVariantIdentity { variant: u32 },
    TooManyEnumFields { variant: u32 },
    MissingEnumFieldIdentity { variant: u32, field: u32 },
    MissingObjectValueIdentity(u32),
    ObjectValueOwner { expected: u32, actual: u32 },
    EnumVariant(EnumSourceVariantBuildError),
    Shape(NominalSourceShapeBuildError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalInterfaceBuildError {
    UnknownPublicNominal {
        kind: NominalArenaKind,
        index: u32,
    },
    MissingNominalIdentity {
        kind: NominalArenaKind,
        index: u32,
    },
    GeneratedNominalIdentity {
        kind: NominalArenaKind,
        index: u32,
    },
    ForeignDeclaration {
        declaration: NominalDeclarationOwner,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    InvalidDeclarationScope(NominalDeclarationOwner),
    DeclarationKind {
        declaration: NominalDeclarationOwner,
        expected: PublicNominalKindV1,
        actual: SourceDeclarationKind,
    },
    InvalidDeclarationName {
        kind: NominalArenaKind,
        index: u32,
        source: CanonicalIdentifierError,
    },
    DeclarationNameMismatch(NominalDeclarationOwner),
    TypeParameterArity {
        declaration: NominalDeclarationOwner,
        expected: u32,
        actual: u32,
    },
    InvalidPublicAccess(NominalDeclarationOwner),
    UnknownLexicalOwner {
        declaration: NominalDeclarationOwner,
        owner: NominalArenaKind,
        index: u32,
    },
    GeneratedLexicalOwner {
        declaration: NominalDeclarationOwner,
        owner: NominalArenaKind,
        index: u32,
    },
    LexicalOwnerMismatch {
        declaration: NominalDeclarationOwner,
        expected: Option<DefinitionOwnerAtom>,
        actual: Option<DefinitionOwnerAtom>,
    },
    Signature {
        declaration: NominalDeclarationOwner,
        source: HirInterfaceSignatureProjectionError,
    },
    Supertypes {
        declaration: NominalDeclarationOwner,
        source: SignatureTypeSetBuildError,
    },
    Constructor {
        declaration: NominalDeclarationOwner,
        detail: NominalConstructorProjectionError,
    },
    Constructors {
        declaration: NominalDeclarationOwner,
        source: CanonicalPersistentIdSetBuildError<PersistentConstructorId>,
    },
    Member {
        declaration: NominalDeclarationOwner,
        detail: NominalMemberProjectionError,
    },
    Members {
        declaration: NominalDeclarationOwner,
        source: PublicMemberRefBuildError,
    },
    NestedBinding {
        declaration: NominalDeclarationOwner,
        detail: NominalNestedBindingProjectionError,
    },
    NestedBindings {
        declaration: NominalDeclarationOwner,
        source: CanonicalPersistentIdSetBuildError<PersistentExportBindingId>,
    },
    SourceShape {
        declaration: NominalDeclarationOwner,
        detail: NominalSourceProjectionError,
    },
    Record {
        declaration: NominalDeclarationOwner,
        source: NominalInterfaceRecordBuildError,
    },
    Table(NominalInterfaceSetBuildError),
}

impl fmt::Display for NominalInterfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPublicNominal { kind, index } => {
                write!(
                    formatter,
                    "public {kind:?} id {index} is outside the HIR arena"
                )
            }
            Self::MissingNominalIdentity { kind, index } => {
                write!(
                    formatter,
                    "public {kind:?} id {index} has no persistent identity"
                )
            }
            Self::GeneratedNominalIdentity { kind, index } => {
                write!(
                    formatter,
                    "public {kind:?} id {index} has a generated identity"
                )
            }
            Self::ForeignDeclaration {
                declaration,
                expected,
                actual,
            } => write!(
                formatter,
                "nominal interface {declaration:?} belongs to Cone {actual}, not {expected}"
            ),
            Self::InvalidDeclarationScope(declaration) => write!(
                formatter,
                "nominal interface {declaration:?} does not have ConeWide scope"
            ),
            Self::DeclarationKind {
                declaration,
                expected,
                actual,
            } => write!(
                formatter,
                "nominal interface {declaration:?} has source kind {actual:?}, expected {expected:?}"
            ),
            Self::InvalidDeclarationName {
                kind,
                index,
                source,
            } => write!(
                formatter,
                "public {kind:?} {index} has an invalid canonical name: {source}"
            ),
            Self::DeclarationNameMismatch(declaration) => write!(
                formatter,
                "nominal interface {declaration:?} disagrees with its HIR declaration name"
            ),
            Self::TypeParameterArity {
                declaration,
                expected,
                actual,
            } => write!(
                formatter,
                "nominal interface {declaration:?} declares {actual} type parameters, identity expects {expected}"
            ),
            Self::InvalidPublicAccess(declaration) => write!(
                formatter,
                "nominal interface {declaration:?} lacks a foreign-public lookup domain"
            ),
            Self::UnknownLexicalOwner {
                declaration,
                owner,
                index,
            } => write!(
                formatter,
                "nominal interface {declaration:?} references unknown {owner:?} owner {index}"
            ),
            Self::GeneratedLexicalOwner {
                declaration,
                owner,
                index,
            } => write!(
                formatter,
                "nominal interface {declaration:?} references generated {owner:?} owner {index}"
            ),
            Self::LexicalOwnerMismatch {
                declaration,
                expected,
                actual,
            } => write!(
                formatter,
                "nominal interface {declaration:?} has persistent owner {actual:?}, expected {expected:?}"
            ),
            Self::Signature {
                declaration,
                source,
            } => write!(
                formatter,
                "cannot project nominal {declaration:?}: {source}"
            ),
            Self::Supertypes {
                declaration,
                source,
            } => write!(
                formatter,
                "invalid supertypes for nominal {declaration:?}: {source}"
            ),
            Self::Constructor {
                declaration,
                detail,
            } => write!(
                formatter,
                "invalid constructor of nominal {declaration:?}: {detail:?}"
            ),
            Self::Constructors {
                declaration,
                source,
            } => write!(
                formatter,
                "invalid constructor set for nominal {declaration:?}: {source}"
            ),
            Self::Member {
                declaration,
                detail,
            } => write!(
                formatter,
                "invalid member of nominal {declaration:?}: {detail:?}"
            ),
            Self::Members {
                declaration,
                source,
            } => write!(
                formatter,
                "invalid member set for nominal {declaration:?}: {source}"
            ),
            Self::NestedBinding {
                declaration,
                detail,
            } => write!(
                formatter,
                "invalid nested binding of nominal {declaration:?}: {detail:?}"
            ),
            Self::NestedBindings {
                declaration,
                source,
            } => write!(
                formatter,
                "invalid nested binding set for nominal {declaration:?}: {source}"
            ),
            Self::SourceShape {
                declaration,
                detail,
            } => write!(
                formatter,
                "invalid source shape for nominal {declaration:?}: {detail:?}"
            ),
            Self::Record {
                declaration,
                source,
            } => write!(
                formatter,
                "invalid nominal interface {declaration:?}: {source}"
            ),
            Self::Table(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for NominalInterfaceBuildError {}
