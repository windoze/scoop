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
    CanonicalValueType {
        kind: CanonicalConstValueKindV1,
        error: E,
    },
    ValueKindTypeMismatch {
        kind: CanonicalConstValueKindV1,
        expected: Box<SignatureTypeKey>,
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
            Self::CanonicalValueType { kind, error } => {
                write!(
                    formatter,
                    "missing canonical core type for {kind:?}: {error}"
                )
            }
            Self::ValueKindTypeMismatch {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "const {kind:?} value type {actual:?} does not match canonical core type {expected:?}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportConstValueSemanticValidationError<E>
{
}
