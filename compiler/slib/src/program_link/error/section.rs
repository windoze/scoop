use crate::{ArtifactProfileInventoryError, SingleConeLinkSectionDecodeError, SlibDiagnostic};

use super::{ProgramLinkReadError, error};

pub(in crate::program_link) fn section(
    source: SingleConeLinkSectionDecodeError,
) -> ProgramLinkReadError {
    let mut result = error(&source);
    result.semantic_path = path(&source);
    result
}

pub(super) fn path(source: &SingleConeLinkSectionDecodeError) -> String {
    use SingleConeLinkSectionDecodeError::*;
    match source {
        WrongProfile { .. } => "manifest:compatibility.artifact_profile".to_owned(),
        MissingMetadataMember { location } => format!("{location}:$"),
        MissingMetadataMemberPayload { member, .. } => format!("member/{member}:$"),
        OuterEnvelope { location, source } => format!("{location}:{}", source.diagnostic().path()),
        MissingSection {
            location,
            capability,
        } => format!("{}:$", name(*location, capability)),
        InnerSection {
            location,
            capability,
            source,
        } => format!("{}:{}", name(*location, capability), source.path()),
        Resource(source) => source.diagnostic().semantic_path(),
        SemanticFingerprints(_) => "manifest:semantic_fingerprints".to_owned(),
        SemanticFingerprintMismatch { location, .. } => format!("{location}:semantic_fingerprint"),
        Inventory(source) => match source {
            ArtifactProfileInventoryError::ConflictingCapabilityVersion {
                location, index, ..
            }
            | ArtifactProfileInventoryError::UnsupportedRequiredCapability {
                location,
                index,
                ..
            } => format!("{location}:sections[{index}]"),
            ArtifactProfileInventoryError::MissingRequiredCapability { location, .. } => {
                format!("{location}:sections")
            }
        },
    }
}

fn name(
    location: Option<crate::MetadataLocation>,
    capability: &scoop_identity::CapabilityId,
) -> String {
    let location = location.map_or_else(|| "manifest".to_owned(), |it| it.to_string());
    format!(
        "{location}/{}/{}/{}",
        capability.namespace(),
        capability.name(),
        capability.major_version()
    )
}
