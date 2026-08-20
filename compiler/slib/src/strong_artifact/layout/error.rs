use std::fmt;

use scoop_hir::CrossConeHirInterfaceIndexError;
use scoop_identity::ConeIdentity;
use scoop_wire::HashError;

use super::super::{StrongArtifactAssemblyError, StrongArtifactSectionV1};
use crate::{
    BootstrapManifestError, CrossConeLinkSemanticImportBuildError, LinkIdentityClosureBuildError,
    ManifestSectionError, MetadataEnvelopeError, MetadataLocation, MetadataSectionError,
    SemanticFingerprintError, SlibMemberId, SlibMemberRecordError, SlibWriteError,
};

#[derive(Debug)]
pub enum CrossConeLayoutArtifactWriteError {
    ComponentProducerMismatch {
        component: &'static str,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    TargetMismatch,
    CallableSemanticImports(CrossConeLinkSemanticImportBuildError),
    CallableSemanticImportMismatch,
    LayoutSemanticImportMismatch,
    LinkObjectProjectionMismatch,
    LinkObjectCount {
        expected: usize,
        actual: usize,
    },
    InvalidLinkObjectRole {
        index: usize,
        member: SlibMemberId,
    },
    LinkObjectMismatch {
        index: usize,
        expected: SlibMemberId,
        actual: SlibMemberId,
    },
    LinkObjectFingerprint(HashError),
    Compatibility(HashError),
    LinkIdentityClosure(LinkIdentityClosureBuildError),
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
    SemanticFingerprints(SemanticFingerprintError),
    ManifestSection(ManifestSectionError),
    Manifest(BootstrapManifestError),
    Archive(SlibWriteError),
}

impl From<StrongArtifactAssemblyError> for CrossConeLayoutArtifactWriteError {
    fn from(error: StrongArtifactAssemblyError) -> Self {
        match error {
            StrongArtifactAssemblyError::LinkObjectCount { expected, actual } => {
                Self::LinkObjectCount { expected, actual }
            }
            StrongArtifactAssemblyError::InvalidLinkObjectRole { index, member } => {
                Self::InvalidLinkObjectRole { index, member }
            }
            StrongArtifactAssemblyError::LinkObjectMismatch {
                index,
                expected,
                actual,
            } => Self::LinkObjectMismatch {
                index,
                expected,
                actual,
            },
            StrongArtifactAssemblyError::LinkObjectFingerprint(source) => {
                Self::LinkObjectFingerprint(source)
            }
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
            Self::CallableSemanticImports(source) => Some(source),
            Self::LinkObjectFingerprint(source) | Self::Compatibility(source) => Some(source),
            Self::LinkIdentityClosure(source) => Some(source),
            Self::HirInterfaceIndex(source) => Some(source),
            Self::Encoding { source, .. } => Some(source),
            Self::MetadataSection { source, .. } => Some(source),
            Self::MetadataEnvelope { source, .. } => Some(source),
            Self::MetadataEnvelopeEncoding { source, .. } => Some(source),
            Self::MetadataMember { source, .. } => Some(source),
            Self::SemanticFingerprints(source) => Some(source),
            Self::ManifestSection(source) => Some(source),
            Self::Manifest(source) => Some(source),
            Self::Archive(source) => Some(source),
            Self::ComponentProducerMismatch { .. }
            | Self::TargetMismatch
            | Self::CallableSemanticImportMismatch
            | Self::LayoutSemanticImportMismatch
            | Self::LinkObjectProjectionMismatch
            | Self::LinkObjectCount { .. }
            | Self::InvalidLinkObjectRole { .. }
            | Self::LinkObjectMismatch { .. } => None,
        }
    }
}
