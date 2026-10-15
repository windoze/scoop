use std::fmt;

use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::cbor::EncodeError;
use scoop_wire::{WireError, decode_canonical, encode};

use crate::{
    ArchiveReadError, ArchiveWriteError, BootstrapManifest, BootstrapManifestValidationError,
    CanonicalSlibArchive, DecodedArchive, DecodedBootstrapManifest, ManifestArchive, SlibMember,
    SlibMemberId,
};

impl CanonicalSlibArchive {
    /// Encodes a validated bootstrap manifest and writes only members whose
    /// records exactly match its typed directory.
    pub fn write_bootstrap(
        manifest: &BootstrapManifest,
        mut members: Vec<SlibMember>,
    ) -> Result<Self, SlibWriteError> {
        members.sort_unstable_by_key(|member| member.record().id());
        if manifest.members().len() != members.len() {
            return Err(SlibWriteError::DirectoryLengthMismatch {
                declared: manifest.members().len(),
                actual: members.len(),
            });
        }
        for (index, (declared, actual)) in manifest.members().iter().zip(&members).enumerate() {
            if declared != actual.record() {
                return Err(SlibWriteError::DirectoryRecordMismatch {
                    index,
                    declared: declared.id(),
                    actual: actual.record().id(),
                });
            }
        }

        let manifest_bytes = encode(manifest).map_err(SlibWriteError::ManifestEncoding)?;
        Self::write(&manifest_bytes, members).map_err(SlibWriteError::Archive)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlibWriteError {
    DirectoryLengthMismatch {
        declared: usize,
        actual: usize,
    },
    DirectoryRecordMismatch {
        index: usize,
        declared: SlibMemberId,
        actual: SlibMemberId,
    },
    ManifestEncoding(EncodeError),
    Archive(ArchiveWriteError),
}

impl fmt::Display for SlibWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectoryLengthMismatch { declared, actual } => write!(
                formatter,
                "manifest declares {declared} members, but writer received {actual}"
            ),
            Self::DirectoryRecordMismatch {
                index,
                declared,
                actual,
            } => write!(
                formatter,
                "writer member {index} does not match manifest directory record: declared {declared}, found {actual}"
            ),
            Self::ManifestEncoding(error) => error.fmt(formatter),
            Self::Archive(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SlibWriteError {}

/// Decoded bootstrap manifest and member ranges for one `.slib` input.
/// Stored content fingerprints are producer results, not reader checks.
#[derive(Clone, Debug)]
pub struct DecodedSlibEnvelope<'input> {
    source: EnvelopeSource<'input>,
    target_selection: ValidatedLirTargetSelection,
}

#[derive(Clone, Debug)]
enum EnvelopeSource<'input> {
    Memory {
        archive: DecodedArchive<'input>,
        manifest: Box<BootstrapManifest>,
    },
    File {
        file: &'input crate::ArtifactFile,
        directory: &'input crate::artifact_file::ArtifactFileDirectory,
    },
}

impl<'input> DecodedSlibEnvelope<'input> {
    pub fn open(
        input: &'input [u8],
        selection: ValidatedLirTargetSelection,
    ) -> Result<Self, SlibReadError> {
        let archive = ManifestArchive::open(input).map_err(SlibReadError::Container)?;
        let decoded = decode_canonical::<DecodedBootstrapManifest>(archive.manifest())
            .map_err(SlibReadError::CanonicalWire)?;
        let manifest = decoded
            .validate(selection)
            .map_err(|error| SlibReadError::Manifest(Box::new(error)))?;
        let archive = archive
            .validate_directory(manifest.members())
            .map_err(SlibReadError::Directory)?;
        Ok(Self {
            source: EnvelopeSource::Memory {
                archive,
                manifest: Box::new(manifest),
            },
            target_selection: selection,
        })
    }

    pub fn from_file(file: &'input crate::ArtifactFile) -> Result<Self, SlibReadError> {
        Ok(Self {
            source: EnvelopeSource::File {
                file,
                directory: file.directory()?,
            },
            target_selection: file.target_selection(),
        })
    }

    pub(crate) fn into_manifest(self) -> BootstrapManifest {
        match self.source {
            EnvelopeSource::Memory { manifest, .. } => *manifest,
            EnvelopeSource::File { directory, .. } => directory.manifest.clone(),
        }
    }

    pub(crate) const fn manifest(&self) -> &BootstrapManifest {
        match &self.source {
            EnvelopeSource::Memory { manifest, .. } => manifest,
            EnvelopeSource::File { directory, .. } => &directory.manifest,
        }
    }

    pub(crate) const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target_selection
    }

    pub(crate) fn member(&self, id: SlibMemberId) -> Result<Option<&'input [u8]>, SlibReadError> {
        match &self.source {
            EnvelopeSource::Memory { archive, .. } => {
                archive.member(id).map_err(SlibReadError::Container)
            }
            EnvelopeSource::File { file, directory } => file.member(directory, id),
        }
    }
}

#[derive(Debug)]
pub enum SlibReadError {
    Io(std::io::Error),
    TestDirectory(crate::TestDirectoryError),
    Container(ArchiveReadError),
    CanonicalWire(WireError),
    Manifest(Box<BootstrapManifestValidationError>),
    Directory(ArchiveReadError),
}

impl fmt::Display for SlibReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(error) | Self::Directory(error) => error.fmt(formatter),
            Self::CanonicalWire(error) => error.fmt(formatter),
            Self::Io(error) => error.fmt(formatter),
            Self::TestDirectory(error) => error.fmt(formatter),
            Self::Manifest(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SlibReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Io(error) => error,
            Self::TestDirectory(error) => error,
            Self::Container(error) | Self::Directory(error) => error,
            Self::CanonicalWire(error) => error,
            Self::Manifest(error) => error.as_ref(),
        })
    }
}

#[cfg(test)]
mod tests;
