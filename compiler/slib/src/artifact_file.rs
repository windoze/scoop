//! File-backed artifact input with one manifest and lazily retained members.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use scoop_lir::ValidatedLirTargetSelection;

use crate::archive::{MemberPosition, member_positions};
use crate::{
    ArchiveReadError, ArtifactManifestSummaryV1, BootstrapManifest, SlibMemberId, SlibReadError,
};

#[derive(Clone, Debug)]
pub struct ArtifactFile(Arc<ArtifactFileInner>);

#[derive(Debug)]
struct ArtifactFileInner {
    file: Mutex<File>,
    target: ValidatedLirTargetSelection,
    directory: OnceLock<ArtifactFileDirectory>,
}

#[derive(Debug)]
pub(crate) struct ArtifactFileDirectory {
    pub(crate) manifest: BootstrapManifest,
    summary: ArtifactManifestSummaryV1,
    members: Vec<(SlibMemberId, MemberPosition, OnceLock<Vec<u8>>)>,
}

impl ArtifactFile {
    pub fn open(path: &Path, target: ValidatedLirTargetSelection) -> io::Result<Self> {
        Ok(Self(Arc::new(ArtifactFileInner {
            file: Mutex::new(File::open(path)?),
            target,
            directory: OnceLock::new(),
        })))
    }

    pub fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.0.target
    }

    pub fn summary(&self) -> Result<&ArtifactManifestSummaryV1, SlibReadError> {
        Ok(&self.directory()?.summary)
    }

    pub(crate) fn directory(&self) -> Result<&ArtifactFileDirectory, SlibReadError> {
        if let Some(directory) = self.0.directory.get() {
            return Ok(directory);
        }
        let mut file = self.lock_file()?;
        if let Some(directory) = self.0.directory.get() {
            return Ok(directory);
        }
        file.seek(SeekFrom::Start(0)).map_err(SlibReadError::Io)?;
        let (manifest, next) = crate::manifest_summary::read_manifest(&mut *file, self.0.target)?;
        let summary = ArtifactManifestSummaryV1::from_manifest(&manifest, self.0.target)
            .map_err(SlibReadError::TestDirectory)?;
        let members = member_positions(manifest.members(), next)
            .map_err(SlibReadError::Directory)?
            .into_iter()
            .map(|(id, position)| (id, position, OnceLock::new()))
            .collect();
        Ok(self.0.directory.get_or_init(|| ArtifactFileDirectory {
            manifest,
            summary,
            members,
        }))
    }

    pub(crate) fn member<'input>(
        &'input self,
        directory: &'input ArtifactFileDirectory,
        id: SlibMemberId,
    ) -> Result<Option<&'input [u8]>, SlibReadError> {
        let Ok(index) = directory
            .members
            .binary_search_by_key(&id, |(id, _, _)| *id)
        else {
            return Ok(None);
        };
        let (_, position, cached) = &directory.members[index];
        if let Some(bytes) = cached.get() {
            return Ok(Some(bytes));
        }
        let mut file = self.lock_file()?;
        if let Some(bytes) = cached.get() {
            return Ok(Some(bytes));
        }
        file.seek(SeekFrom::Start(position.offset))
            .map_err(SlibReadError::Io)?;
        let length = usize::try_from(position.length)
            .map_err(|_| SlibReadError::Container(ArchiveReadError::LengthOverflow))?;
        let mut bytes = Vec::new();
        file.by_ref()
            .take(length as u64)
            .read_to_end(&mut bytes)
            .map_err(SlibReadError::Io)?;
        if bytes.len() != length {
            return Err(SlibReadError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete slib member",
            )));
        }
        let bytes = if position.compressed {
            crate::archive::decompress_member(&bytes, position.ordinal())
                .map_err(SlibReadError::Container)?
        } else {
            bytes
        };
        Ok(Some(cached.get_or_init(|| bytes)))
    }

    fn lock_file(&self) -> Result<std::sync::MutexGuard<'_, File>, SlibReadError> {
        self.0
            .file
            .lock()
            .map_err(|_| SlibReadError::Io(io::Error::other("artifact file lock was poisoned")))
    }
}

#[derive(Clone, Copy)]
pub enum ArtifactInput<'input> {
    Bytes(&'input [u8]),
    File(&'input ArtifactFile),
}

impl<'input> ArtifactInput<'input> {
    pub(crate) fn open(
        self,
        target: ValidatedLirTargetSelection,
    ) -> Result<crate::DecodedSlibEnvelope<'input>, SlibReadError> {
        match self {
            Self::Bytes(bytes) => crate::DecodedSlibEnvelope::open(bytes, target),
            Self::File(file) => crate::DecodedSlibEnvelope::from_file(file),
        }
    }
}

#[cfg(test)]
mod tests;
