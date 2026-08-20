use super::*;

/// Keeps the artifact position and the original reader or Link diagnostic.
#[derive(Debug)]
pub enum CrossConeLayoutArtifactValidationError {
    Envelope {
        slot: CrossConeClosureArtifactSlotV1,
        source: Box<crate::SlibReadError>,
    },
    Graph {
        slot: CrossConeClosureArtifactSlotV1,
        source: Box<crate::GraphValidationError>,
    },
    LinkSections {
        slot: CrossConeClosureArtifactSlotV1,
        source: Box<crate::CrossConeLayoutLinkSectionDecodeError>,
    },
    Semantic {
        source: Box<crate::CrossConeLayoutSemanticClosureError>,
    },
    Physical {
        source: Box<crate::CrossConeLayoutLirPhysicalError>,
    },
    Resource {
        source: scoop_wire::WireError,
    },
    MissingCurrentArtifact,
}

impl fmt::Display for CrossConeLayoutArtifactValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "layout artifact publication validation failed: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeLayoutArtifactValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Envelope { source, .. } => Some(source.as_ref()),
            Self::Graph { source, .. } => Some(source.as_ref()),
            Self::LinkSections { source, .. } => Some(source.as_ref()),
            Self::Semantic { source } => Some(source.as_ref()),
            Self::Physical { source } => Some(source.as_ref()),
            Self::Resource { source } => Some(source),
            Self::MissingCurrentArtifact => None,
        }
    }
}
