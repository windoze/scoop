use std::fmt;

use scoop_hir::CrossConeHirInterfaceIndexError;
use scoop_wire::HashError;

use super::super::{StrongArtifactAssemblyError, StrongArtifactSectionV1};
use crate::{
    BootstrapManifestError, MetadataEnvelopeError, MetadataLocation, MetadataSectionError,
    SemanticFingerprintError, SlibMemberRecordError, SlibWriteError,
};

#[derive(Debug)]
pub enum CrossConeLayoutArtifactWriteError {
    TestDirectory(crate::TestDirectoryError),
    Compatibility(HashError),
    HirInterfaceIndex(CrossConeHirInterfaceIndexError),
    Encoding {
        section: StrongArtifactSectionV1,
        source: scoop_wire::cbor::EncodeError,
    },
    MetadataSection {
        section: StrongArtifactSectionV1,
        source: MetadataSectionError,
    },
    MetadataEnvelope {
        location: MetadataLocation,
        source: MetadataEnvelopeError,
    },
    MetadataEnvelopeEncoding {
        location: MetadataLocation,
        source: scoop_wire::cbor::EncodeError,
    },
    MetadataMember {
        location: MetadataLocation,
        source: SlibMemberRecordError,
    },
    ObjectMembers(SlibMemberRecordError),
    SemanticFingerprints(SemanticFingerprintError),
    Manifest(BootstrapManifestError),
    Archive(SlibWriteError),
}

impl From<StrongArtifactAssemblyError> for CrossConeLayoutArtifactWriteError {
    fn from(error: StrongArtifactAssemblyError) -> Self {
        match error {
            StrongArtifactAssemblyError::Encoding { section, source } => {
                Self::Encoding { section, source }
            }
            StrongArtifactAssemblyError::MetadataSection { section, source } => {
                Self::MetadataSection { section, source }
            }
            StrongArtifactAssemblyError::MetadataEnvelope { location, source } => {
                Self::MetadataEnvelope { location, source }
            }
            StrongArtifactAssemblyError::MetadataEnvelopeEncoding { location, source } => {
                Self::MetadataEnvelopeEncoding { location, source }
            }
            StrongArtifactAssemblyError::MetadataMember { location, source } => {
                Self::MetadataMember { location, source }
            }
        }
    }
}

impl fmt::Display for CrossConeLayoutArtifactWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot assemble cross-Cone layout strong artifact: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeLayoutArtifactWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compatibility(source) => Some(source),
            Self::HirInterfaceIndex(source) => Some(source),
            Self::Encoding { source, .. } => Some(source),
            Self::MetadataSection { source, .. } => Some(source),
            Self::MetadataEnvelope { source, .. } => Some(source),
            Self::MetadataEnvelopeEncoding { source, .. } => Some(source),
            Self::MetadataMember { source, .. } => Some(source),
            Self::SemanticFingerprints(source) => Some(source),
            Self::ObjectMembers(source) => Some(source),
            Self::Manifest(source) => Some(source),
            Self::Archive(source) => Some(source),
            Self::TestDirectory(source) => Some(source),
        }
    }
}
