use std::fmt;

use scoop_identity::{
    ConeIdentity, DeclarationScope, DefinitionOrigin, PersistentTypeAliasId,
    SourceDeclarationIdentityError, SourceDeclarationKind,
};

use crate::{PublicLookupAccessV1, SignatureTypeSemanticError};

#[derive(Debug, Eq, PartialEq)]
pub enum TypeAliasInterfaceSemanticValidationError<E> {
    Declaration(E),
    DeclarationKind {
        actual: SourceDeclarationKind,
    },
    DeclarationIdentity(SourceDeclarationIdentityError),
    DeclarationIdentityMismatch {
        expected: PersistentTypeAliasId,
        actual: PersistentTypeAliasId,
    },
    DeclarationCone {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    NestedDeclaration {
        owner_depth: usize,
    },
    DeclarationScope {
        actual: DeclarationScope,
    },
    Access {
        expected: PublicLookupAccessV1,
        actual: PublicLookupAccessV1,
    },
    DefinitionOriginCone {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    DefinitionOriginMismatch {
        expected: Box<DefinitionOrigin>,
        actual: Box<DefinitionOrigin>,
    },
    Target(SignatureTypeSemanticError<E>),
}

impl<E: fmt::Display> fmt::Display for TypeAliasInterfaceSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(error) => {
                write!(formatter, "invalid type-alias declaration: {error}")
            }
            Self::DeclarationKind { actual } => write!(
                formatter,
                "type-alias identity resolves to {actual:?}, expected TypeAlias"
            ),
            Self::DeclarationIdentity(error) => {
                write!(formatter, "invalid canonical type-alias identity: {error}")
            }
            Self::DeclarationIdentityMismatch { expected, actual } => write!(
                formatter,
                "type-alias declaration identity {actual} does not match record identity {expected}"
            ),
            Self::DeclarationCone { expected, actual } => write!(
                formatter,
                "type alias belongs to Cone {actual}, expected current Cone {expected}"
            ),
            Self::NestedDeclaration { owner_depth } => write!(
                formatter,
                "type alias is nested under {owner_depth} declaration owner(s)"
            ),
            Self::DeclarationScope { actual } => write!(
                formatter,
                "type alias requires ConeWide declaration scope, found {actual:?}"
            ),
            Self::Access { expected, actual } => write!(
                formatter,
                "type-alias access {actual:?} does not match source access {expected:?}"
            ),
            Self::DefinitionOriginCone { expected, actual } => write!(
                formatter,
                "type-alias definition origin belongs to Cone {actual}, expected {expected}"
            ),
            Self::DefinitionOriginMismatch { expected, actual } => write!(
                formatter,
                "type-alias definition origin {actual:?} does not match foundation origin {expected:?}"
            ),
            Self::Target(error) => write!(formatter, "invalid type-alias target: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for TypeAliasInterfaceSemanticValidationError<E>
{
}
