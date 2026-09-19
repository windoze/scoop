use std::fmt;

use scoop_wire::WireError;

use super::super::ProtectedDefaultReferenceKindV1;
use crate::{ExportDefaultReferenceOccurrenceSiteV1, ProtectedDefaultExpressionUseV1};

#[derive(Debug, Eq, PartialEq)]
pub enum ProtectedDefaultBodyClosureError<E> {
    Resource(WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    Source(E),
    WitnessOwner {
        kind: ProtectedDefaultReferenceKindV1,
        index: usize,
    },
    Missing {
        kind: ProtectedDefaultReferenceKindV1,
        insertion_index: usize,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Extra {
        kind: ProtectedDefaultReferenceKindV1,
        index: usize,
    },
    MissingUse {
        kind: ProtectedDefaultReferenceKindV1,
        index: usize,
        expected: ProtectedDefaultExpressionUseV1,
    },
    ExtraUse {
        kind: ProtectedDefaultReferenceKindV1,
        index: usize,
        use_index: usize,
    },
    ReceiverOutsideBody,
}
impl<E> From<WireError> for ProtectedDefaultBodyClosureError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl<E: fmt::Display> fmt::Display for ProtectedDefaultBodyClosureError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::WitnessOwner { kind, index } => write!(
                f,
                "protected default {kind} reference {index} has a different witness owner"
            ),
            Self::Missing {
                kind,
                insertion_index,
                site,
            } => write!(
                f,
                "protected default {site:?} requires missing {kind} reference at {insertion_index}"
            ),
            Self::Extra { kind, index } => write!(
                f,
                "protected default {kind} reference {index} is absent from its body"
            ),
            Self::MissingUse {
                kind,
                index,
                expected,
            } => write!(
                f,
                "protected default {kind} reference {index} is missing body use {expected:?}"
            ),
            Self::ExtraUse {
                kind,
                index,
                use_index,
            } => write!(
                f,
                "protected default {kind} reference {index} has unused occurrence {use_index}"
            ),
            Self::ReceiverOutsideBody => f.write_str(
                "protected default receiver is absent from the complete expression traversal",
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedDefaultBodyClosureError<E> {}
