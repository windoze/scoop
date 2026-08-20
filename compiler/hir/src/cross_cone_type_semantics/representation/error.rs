use std::fmt;

use scoop_identity::{
    EnumVariantIdentityError, GeneratedNominalIdentityError, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentFieldId, PersistentIdMismatch, PersistentTypeId,
    SourceDeclarationIdentityError,
};

use crate::{
    DeclarationAccessSourceResolutionError, RepresentationFieldResolutionError,
    SignatureBinderScopeError,
};

#[derive(Debug)]
pub enum NominalRepresentationBuildError {
    GeneratedSourceVariant,
    VariantIdentity(EnumVariantIdentityError),
    OwnerIdentity(SourceDeclarationIdentityError),
    BackingIdentity(GeneratedNominalIdentityError),
    FieldOwner { index: usize },
    DuplicateField { index: usize },
    VariantOwner { index: usize },
    DuplicateVariant { index: usize },
    EmptyCLayout,
    Binder(SignatureBinderScopeError),
    NonNominalBase,
    ObjectBackingClass,
    GenericTemplate,
    GenericIntrinsicFamily,
    SourceKind,
    PublicSourceShape,
}

impl fmt::Display for NominalRepresentationBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeneratedSourceVariant => {
                f.write_str("source representation requires a source enum variant")
            }
            Self::VariantIdentity(error) => {
                write!(f, "invalid representation variant identity: {error}")
            }
            Self::OwnerIdentity(error) => {
                write!(f, "invalid representation owner identity: {error}")
            }
            Self::BackingIdentity(error) => write!(f, "invalid object backing identity: {error}"),
            Self::FieldOwner { index } => write!(
                f,
                "representation field at index {index} has a different owner"
            ),
            Self::DuplicateField { index } => {
                write!(f, "duplicate representation field at index {index}")
            }
            Self::VariantOwner { index } => write!(
                f,
                "representation variant at index {index} has a different owner"
            ),
            Self::DuplicateVariant { index } => {
                write!(f, "duplicate representation variant at index {index}")
            }
            Self::EmptyCLayout => f.write_str("CLayout representation cannot be empty"),
            Self::Binder(error) => write!(
                f,
                "source representation requires binder-free field/base types: {error}"
            ),
            Self::NonNominalBase => f.write_str("class representation base must be a nominal type"),
            Self::ObjectBackingClass => {
                f.write_str("object backing class differs from its generated identity")
            }
            Self::GenericTemplate => {
                f.write_str("generic templates cannot export concrete representation support")
            }
            Self::GenericIntrinsicFamily => f.write_str(
                "generic intrinsic families cannot use a param-free representation owner",
            ),
            Self::SourceKind => {
                f.write_str("representation kind differs from the source nominal kind")
            }
            Self::PublicSourceShape => f.write_str(
                "representation support differs from the public source field/variant sequence",
            ),
        }
    }
}
impl std::error::Error for NominalRepresentationBuildError {}

#[derive(Debug)]
pub enum NominalRepresentationResolutionError<E> {
    Allocation(scoop_wire::WireError),
    Reference(E),
    Access(DeclarationAccessSourceResolutionError<E>),
    Field(RepresentationFieldResolutionError<E, PersistentFieldId>),
    EnumField(RepresentationFieldResolutionError<E, PersistentEnumVariantFieldId>),
    OwnerIdentity(PersistentIdMismatch<PersistentTypeId>),
    VariantIdentity(PersistentIdMismatch<PersistentEnumVariantId>),
    Record(NominalRepresentationBuildError),
}

impl<E: fmt::Display> fmt::Display for NominalRepresentationResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation(error) => error.fmt(f),
            Self::Reference(error) => write!(f, "invalid representation reference: {error}"),
            Self::Access(error) => error.fmt(f),
            Self::Field(error) => error.fmt(f),
            Self::EnumField(error) => error.fmt(f),
            Self::OwnerIdentity(error) => error.fmt(f),
            Self::VariantIdentity(error) => error.fmt(f),
            Self::Record(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for NominalRepresentationResolutionError<E> {}
