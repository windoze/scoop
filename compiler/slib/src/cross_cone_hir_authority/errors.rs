use std::fmt;

use scoop_hir::{PublicDeclarationOwnerV1, PublicNominalKindV1, SourceNominalId};
use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DefinitionOriginSubject, DefinitionOwnerAtom,
    DuplicateSignatureKey, IdentityReferenceError, PersistentEnumVariantId,
    PersistentExportBindingId, PersistentTypeAliasId, PropertyOwner, SourceDeclarationKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeHirNominalAuthorityError {
    Identity(IdentityReferenceError),
    ForeignDeclaration {
        entity: &'static str,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    UnreachableProvider {
        origin: ConeIdentity,
    },
    MissingNominalInterface {
        origin: ConeIdentity,
        declaration: SourceNominalId,
    },
    InvalidNominalDeclarationKind {
        declaration: SourceNominalId,
        actual: SourceDeclarationKind,
    },
    NominalKindMismatch {
        declaration: SourceNominalId,
        expected: PublicNominalKindV1,
        actual: PublicNominalKindV1,
    },
    NominalArityMismatch {
        declaration: SourceNominalId,
        expected: u32,
        actual: u32,
    },
    NestedExtension {
        entity: &'static str,
        owner_depth: usize,
    },
    InvalidDeclarationOwner {
        entity: &'static str,
        owner: DefinitionOwnerAtom,
    },
    GeneratedEnumVariant {
        variant: PersistentEnumVariantId,
    },
    NestedBindingTarget {
        binding: PersistentExportBindingId,
        source: Box<Self>,
    },
    NestedBindingExporterMismatch {
        binding: PersistentExportBindingId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    MissingNestedBindingRecord {
        binding: PersistentExportBindingId,
    },
    ReexportedNestedBinding {
        binding: PersistentExportBindingId,
    },
    ConstructorInMemberSet,
    VariantConstructorInMemberSet,
    MissingSourceDeclarationKey {
        declaration: CallableTemplateOrigin,
    },
    CallableDeclarationKindMismatch {
        declaration: CallableTemplateOrigin,
        actual: SourceDeclarationKind,
    },
    CallableSignatureKindMismatch {
        declaration: CallableTemplateOrigin,
        actual: DuplicateSignatureKey,
    },
    CallableNominalOwnerRequired {
        declaration: CallableTemplateOrigin,
        actual: PublicDeclarationOwnerV1,
    },
    PropertyDeclarationKindMismatch {
        declaration: PropertyOwner,
        actual: SourceDeclarationKind,
    },
    PropertySignatureKindMismatch {
        declaration: PropertyOwner,
        actual: DuplicateSignatureKey,
    },
    MissingPropertyInterface {
        declaration: PropertyOwner,
    },
    MissingDirectPublicTypeAliasBinding {
        alias: PersistentTypeAliasId,
    },
    MissingDefinitionOrigin {
        subject: DefinitionOriginSubject,
    },
    NominalSourceShapeNotEnum {
        declaration: SourceNominalId,
        actual: PublicNominalKindV1,
    },
    MissingEnumVariant {
        declaration: SourceNominalId,
        variant: PersistentEnumVariantId,
    },
}

impl fmt::Display for CrossConeHirNominalAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(formatter),
            Self::ForeignDeclaration {
                entity,
                expected,
                actual,
            } => write!(
                formatter,
                "{entity} belongs to Cone {actual}, expected current Cone {expected}"
            ),
            Self::UnreachableProvider { origin } => {
                write!(
                    formatter,
                    "Cone {origin} is outside the provider dependency closure"
                )
            }
            Self::MissingNominalInterface {
                origin,
                declaration,
            } => write!(
                formatter,
                "Cone {origin} has no public nominal interface for {declaration:?}"
            ),
            Self::InvalidNominalDeclarationKind {
                declaration,
                actual,
            } => write!(
                formatter,
                "nominal identity {declaration:?} has non-public declaration kind {actual:?}"
            ),
            Self::NominalKindMismatch {
                declaration,
                expected,
                actual,
            } => write!(
                formatter,
                "nominal interface {declaration:?} has kind {actual:?}, expected {expected:?}"
            ),
            Self::NominalArityMismatch {
                declaration,
                expected,
                actual,
            } => write!(
                formatter,
                "nominal interface {declaration:?} has type-parameter arity {actual}, expected {expected}"
            ),
            Self::NestedExtension {
                entity,
                owner_depth,
            } => write!(
                formatter,
                "{entity} is an extension with a non-empty owner chain of depth {owner_depth}"
            ),
            Self::InvalidDeclarationOwner { entity, owner } => {
                write!(formatter, "{entity} has non-nominal direct owner {owner:?}")
            }
            Self::GeneratedEnumVariant { variant } => write!(
                formatter,
                "enum variant {variant} is generated and has no public source owner"
            ),
            Self::NestedBindingTarget { binding, source } => {
                write!(
                    formatter,
                    "invalid nested binding {binding} target: {source}"
                )
            }
            Self::NestedBindingExporterMismatch {
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "nested binding {binding} is exported by Cone {actual}, expected {expected}"
            ),
            Self::MissingNestedBindingRecord { binding } => write!(
                formatter,
                "nested binding {binding} is absent from the current public binding surface"
            ),
            Self::ReexportedNestedBinding { binding } => write!(
                formatter,
                "nested binding {binding} is a re-export instead of a current declaration"
            ),
            Self::ConstructorInMemberSet => {
                formatter.write_str("constructor cannot appear in the nominal member set")
            }
            Self::VariantConstructorInMemberSet => formatter
                .write_str("enum variant constructor cannot appear in the nominal member set"),
            Self::MissingSourceDeclarationKey { declaration } => write!(
                formatter,
                "callable declaration {declaration:?} has no source declaration key"
            ),
            Self::CallableDeclarationKindMismatch {
                declaration,
                actual,
            } => write!(
                formatter,
                "callable declaration {declaration:?} has source declaration kind {actual:?}"
            ),
            Self::CallableSignatureKindMismatch {
                declaration,
                actual,
            } => write!(
                formatter,
                "callable declaration {declaration:?} has incompatible duplicate signature {actual:?}"
            ),
            Self::CallableNominalOwnerRequired {
                declaration,
                actual,
            } => write!(
                formatter,
                "callable declaration {declaration:?} requires a nominal owner, found {actual:?}"
            ),
            Self::PropertyDeclarationKindMismatch {
                declaration,
                actual,
            } => write!(
                formatter,
                "property declaration {declaration:?} has source declaration kind {actual:?}"
            ),
            Self::PropertySignatureKindMismatch {
                declaration,
                actual,
            } => write!(
                formatter,
                "property declaration {declaration:?} has incompatible duplicate signature {actual:?}"
            ),
            Self::MissingPropertyInterface { declaration } => write!(
                formatter,
                "property accessor owner {declaration:?} is absent from the current property surface"
            ),
            Self::MissingDirectPublicTypeAliasBinding { alias } => write!(
                formatter,
                "type alias {alias} has no declared-current public binding"
            ),
            Self::MissingDefinitionOrigin { subject } => write!(
                formatter,
                "definition origin for {subject:?} is absent from the current HIR foundation"
            ),
            Self::NominalSourceShapeNotEnum {
                declaration,
                actual,
            } => write!(
                formatter,
                "variant constructor owner {declaration:?} has {actual:?} source shape instead of enum"
            ),
            Self::MissingEnumVariant {
                declaration,
                variant,
            } => write!(
                formatter,
                "enum {declaration:?} has no source variant {variant}"
            ),
        }
    }
}

impl std::error::Error for CrossConeHirNominalAuthorityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::NestedBindingTarget { source, .. } => Some(source.as_ref()),
            Self::ForeignDeclaration { .. }
            | Self::UnreachableProvider { .. }
            | Self::MissingNominalInterface { .. }
            | Self::InvalidNominalDeclarationKind { .. }
            | Self::NominalKindMismatch { .. }
            | Self::NominalArityMismatch { .. }
            | Self::NestedExtension { .. }
            | Self::InvalidDeclarationOwner { .. }
            | Self::GeneratedEnumVariant { .. }
            | Self::NestedBindingExporterMismatch { .. }
            | Self::MissingNestedBindingRecord { .. }
            | Self::ReexportedNestedBinding { .. }
            | Self::ConstructorInMemberSet
            | Self::VariantConstructorInMemberSet
            | Self::MissingSourceDeclarationKey { .. }
            | Self::CallableDeclarationKindMismatch { .. }
            | Self::CallableSignatureKindMismatch { .. }
            | Self::CallableNominalOwnerRequired { .. }
            | Self::PropertyDeclarationKindMismatch { .. }
            | Self::PropertySignatureKindMismatch { .. }
            | Self::MissingPropertyInterface { .. }
            | Self::MissingDirectPublicTypeAliasBinding { .. }
            | Self::MissingDefinitionOrigin { .. }
            | Self::NominalSourceShapeNotEnum { .. }
            | Self::MissingEnumVariant { .. } => None,
        }
    }
}
