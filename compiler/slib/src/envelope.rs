use std::fmt;

use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::cbor::EncodeError;
use scoop_wire::{
    BudgetMeter, DecodeLimits, DecodeUsage, WireError, decode_canonical_with_meter, encode,
};

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

/// Immutable `.slib` bytes whose canonical container, bootstrap manifest,
/// typed directory, member ranges, content hashes, and artifact fingerprint
/// have all been validated. It exposes no payload extraction or IR API.
#[derive(Debug, Eq, PartialEq)]
pub struct DecodedSlibEnvelope<'input> {
    archive: DecodedArchive<'input>,
    manifest: BootstrapManifest,
    decode_usage: DecodeUsage,
}

impl<'input> DecodedSlibEnvelope<'input> {
    pub fn open(
        input: &'input [u8],
        limits: DecodeLimits,
        selection: ValidatedLirTargetSelection,
    ) -> Result<Self, SlibReadError> {
        let archive = ManifestArchive::open(input).map_err(SlibReadError::Container)?;
        let mut meter = BudgetMeter::new(limits);
        let decoded =
            decode_canonical_with_meter::<DecodedBootstrapManifest>(archive.manifest(), &mut meter)
                .map_err(SlibReadError::CanonicalWire)?;
        let manifest = decoded
            .validate(selection)
            .map_err(|error| SlibReadError::Manifest(Box::new(error)))?;
        let archive = archive
            .validate_directory(manifest.members())
            .map_err(SlibReadError::Directory)?;
        Ok(Self {
            archive,
            manifest,
            decode_usage: meter.usage(),
        })
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.decode_usage
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SlibReadError {
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
            Self::Manifest(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SlibReadError {}

#[cfg(test)]
mod tests;
