use scoop_hir::{ExportDefaultTemplateKeyV1, IntrinsicTypeKind, SourceNominalId};
use scoop_identity::ConeIdentity;
use scoop_wire::WireError;

use crate::cross_cone_hir_authority::CrossConeHirNominalAuthorityError;

#[derive(Debug)]
pub enum CrossConeHirDefaultTypeAccessError {
    Resource(WireError),
    Declaration(Box<CrossConeHirNominalAuthorityError>),
    Encoding(String),
    Arity {
        declaration: SourceNominalId,
        expected: u32,
        actual: usize,
    },
    MissingPointerProtocol,
    ConflictingPointerProtocols {
        first: ConeIdentity,
        second: ConeIdentity,
    },
    PointerRepresentation {
        declaration: SourceNominalId,
        expected: IntrinsicTypeKind,
    },
    WitnessDomain,
    Reference {
        template: ExportDefaultTemplateKeyV1,
        index: usize,
        source: Box<Self>,
    },
}

impl From<WireError> for CrossConeHirDefaultTypeAccessError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<CrossConeHirNominalAuthorityError> for CrossConeHirDefaultTypeAccessError {
    fn from(error: CrossConeHirNominalAuthorityError) -> Self {
        Self::Declaration(Box::new(error))
    }
}
impl std::fmt::Display for CrossConeHirDefaultTypeAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Declaration(error) => error.fmt(f),
            Self::Encoding(error) => write!(f, "cannot encode default type access domain: {error}"),
            Self::Arity {
                declaration,
                expected,
                actual,
            } => write!(
                f,
                "default type access for {declaration:?} requires {expected} arguments, got {actual}"
            ),
            Self::MissingPointerProtocol => {
                f.write_str("default pointer type has no reachable source declaration protocol")
            }
            Self::ConflictingPointerProtocols { first, second } => write!(
                f,
                "default pointer source roles conflict between providers {first} and {second}"
            ),
            Self::PointerRepresentation {
                declaration,
                expected,
            } => write!(
                f,
                "default pointer role {declaration:?} does not have intrinsic representation {expected:?}"
            ),
            Self::WitnessDomain => f.write_str(
                "default type access witness differs from the actual source declaration domain",
            ),
            Self::Reference {
                template,
                index,
                source,
            } => write!(
                f,
                "default {template:?} type reference[{index}] has invalid source access: {source}"
            ),
        }
    }
}
impl std::error::Error for CrossConeHirDefaultTypeAccessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Declaration(error) => Some(error.as_ref()),
            Self::Reference { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
