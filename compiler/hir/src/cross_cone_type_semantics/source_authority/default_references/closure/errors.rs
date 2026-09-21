use super::*;
use std::fmt;

#[derive(Debug)]
pub enum DefaultSourceReferenceClosureError {
    Resource(WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    ReceiverOutsideBody,
    Missing {
        kind: ExportDefaultReferenceKindV1,
        index: u32,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Target {
        kind: ExportDefaultReferenceKindV1,
        index: u32,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Origin {
        kind: ExportDefaultReferenceKindV1,
        index: u32,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Extra {
        kind: ExportDefaultReferenceKindV1,
        index: u32,
    },
}
impl From<WireError> for DefaultSourceReferenceClosureError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for DefaultSourceReferenceClosureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::ReceiverOutsideBody => f.write_str(
                "default source receiver is absent from the complete expression traversal",
            ),
            Self::Missing { kind, index, site } => write!(
                f,
                "missing default source {kind} occurrence {index} at {site:?}"
            ),
            Self::Target { kind, index, site } => write!(
                f,
                "default source {kind} occurrence {index} target differs at {site:?}"
            ),
            Self::Origin { kind, index, site } => write!(
                f,
                "default source {kind} occurrence {index} origin differs at {site:?}"
            ),
            Self::Extra { kind, index } => {
                write!(f, "unconsumed default source {kind} occurrence {index}")
            }
        }
    }
}
impl std::error::Error for DefaultSourceReferenceClosureError {}
