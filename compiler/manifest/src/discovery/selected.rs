use std::path::{Path, PathBuf};
use std::time::SystemTime;

use scoop_identity::{SourceIdentity, SourceRole};

use super::{DiscoveredManifestSources, SourceDiscoveryError, SourceDisplayLocator};

/// A selected input and its update metadata, without reading source contents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedSource {
    identity: SourceIdentity,
    display_locator: SourceDisplayLocator,
    physical_path: PathBuf,
    byte_length: u64,
    modified: SystemTime,
    role: SourceRole,
}

impl SelectedSource {
    pub(crate) fn new(
        identity: SourceIdentity,
        display_locator: SourceDisplayLocator,
        physical_path: PathBuf,
        metadata: &std::fs::Metadata,
        role: SourceRole,
    ) -> std::io::Result<Self> {
        Ok(Self {
            identity,
            display_locator,
            physical_path,
            byte_length: metadata.len(),
            modified: metadata.modified()?,
            role,
        })
    }

    pub fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    pub fn display_locator(&self) -> &SourceDisplayLocator {
        &self.display_locator
    }

    pub fn physical_path(&self) -> &Path {
        &self.physical_path
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub const fn modified(&self) -> SystemTime {
        self.modified
    }

    pub const fn role(&self) -> SourceRole {
        self.role
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedManifestSources {
    pub(super) first: SelectedSource,
    pub(super) rest: Vec<SelectedSource>,
    pub(super) directories: Vec<crate::ConeRelativePath>,
}

impl SelectedManifestSources {
    pub fn iter(&self) -> impl Iterator<Item = &SelectedSource> {
        std::iter::once(&self.first).chain(&self.rest)
    }

    pub(super) fn read(self) -> Result<DiscoveredManifestSources, SourceDiscoveryError> {
        Ok(DiscoveredManifestSources {
            first: super::read::read_discovered_source(self.first)?,
            rest: self
                .rest
                .into_iter()
                .map(super::read::read_discovered_source)
                .collect::<Result<_, _>>()?,
            directories: self.directories,
        })
    }
}
