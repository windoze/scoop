use std::fmt;

use scoop_identity::{
    ConeIdentity, DeclarationScope, DefinitionOrigin, PersistentPropertyId, PropertyOwner,
    SignatureTypeKey, SourceDeclarationIdentityError, SourceDeclarationKind,
};

use crate::{CanonicalConstValueKindV1, PropertyRepresentationV1};

#[derive(Debug, Eq, PartialEq)]
pub enum ExportConstValueSemanticValidationError<E> {
    Declaration(E),
    DeclarationKind {
        actual: SourceDeclarationKind,
    },
    DeclarationIdentity(SourceDeclarationIdentityError),
    DeclarationIdentityMismatch {
        expected: PersistentPropertyId,
        actual: PersistentPropertyId,
    },
    DeclarationCone {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    DeclarationScope {
        actual: DeclarationScope,
    },
    DefinitionOriginCone {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    DefinitionOriginMismatch {
        expected: Box<DefinitionOrigin>,
        actual: Box<DefinitionOrigin>,
    },
    PropertyInterface(E),
    PropertyInterfaceDeclaration {
        expected: PropertyOwner,
        actual: PropertyOwner,
    },
    PropertyRepresentation {
        actual: PropertyRepresentationV1,
    },
    PropertyValueTypeMismatch {
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    ValueType {
        kind: CanonicalConstValueKindV1,
        error: E,
    },
    NonNominalValueType {
        kind: CanonicalConstValueKindV1,
        actual: Box<SignatureTypeKey>,
    },
}

impl<E: fmt::Display> fmt::Display for ExportConstValueSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(error) => {
                write!(formatter, "invalid const property declaration: {error}")
            }
            Self::DeclarationKind { actual } => write!(
                formatter,
                "const property identity resolves to {actual:?}, expected Property"
            ),
            Self::DeclarationIdentity(error) => {
                write!(
                    formatter,
                    "invalid canonical const property identity: {error}"
                )
            }
            Self::DeclarationIdentityMismatch { expected, actual } => write!(
                formatter,
                "const property declaration identity {actual} does not match record identity {expected}"
            ),
            Self::DeclarationCone { expected, actual } => write!(
                formatter,
                "const property belongs to Cone {actual}, expected current Cone {expected}"
            ),
            Self::DeclarationScope { actual } => write!(
                formatter,
                "const property requires ConeWide declaration scope, found {actual:?}"
            ),
            Self::DefinitionOriginCone { expected, actual } => write!(
                formatter,
                "const definition origin belongs to Cone {actual}, expected {expected}"
            ),
            Self::DefinitionOriginMismatch { expected, actual } => write!(
                formatter,
                "const definition origin {actual:?} does not match foundation origin {expected:?}"
            ),
            Self::PropertyInterface(error) => {
                write!(formatter, "invalid const property interface: {error}")
            }
            Self::PropertyInterfaceDeclaration { expected, actual } => write!(
                formatter,
                "const property interface declaration {actual:?} does not match {expected:?}"
            ),
            Self::PropertyRepresentation { actual } => write!(
                formatter,
                "const property interface has representation {actual:?}, expected Const"
            ),
            Self::PropertyValueTypeMismatch { expected, actual } => write!(
                formatter,
                "const value type {actual:?} does not match property interface type {expected:?}"
            ),
            Self::ValueType { kind, error } => {
                write!(
                    formatter,
                    "invalid intrinsic nominal for const {kind:?}: {error}"
                )
            }
            Self::NonNominalValueType { kind, actual } => write!(
                formatter,
                "const {kind:?} requires a non-generic intrinsic nominal, found {actual:?}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportConstValueSemanticValidationError<E>
{
}
