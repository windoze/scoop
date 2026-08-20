use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use scoop_identity::{
    NormalizedSourcePath, NormalizedSourcePathError, SourceContentDigest, SourceIdentity,
    TargetProfileId,
};

use crate::{ConeRelativePath, LoadedConeManifest, SourceSelection};

mod error;
mod read;
mod select;
mod walk;
use crate::stable_file::StableFileObservation;
pub use error::{DiscoveryIoOperation, SourceDiscoveryError, SourceDiscoveryErrorKind};
use read::read_discovered_source;
use select::selected_roots;
use walk::{resolve_entry, walk_directory};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDisplayLocator(PathBuf);

impl SourceDisplayLocator {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredSource {
    identity: SourceIdentity,
    display_locator: SourceDisplayLocator,
    source_text: String,
    content_digest: SourceContentDigest,
}

impl DiscoveredSource {
    pub(crate) fn new(
        identity: SourceIdentity,
        display_locator: SourceDisplayLocator,
        source_text: String,
    ) -> Self {
        let content_digest = SourceContentDigest::from_utf8(&source_text);
        Self {
            identity,
            display_locator,
            source_text,
            content_digest,
        }
    }

    pub fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    pub fn display_locator(&self) -> &SourceDisplayLocator {
        &self.display_locator
    }

    pub fn source_text(&self) -> &str {
        &self.source_text
    }

    pub const fn content_digest(&self) -> SourceContentDigest {
        self.content_digest
    }

    pub fn into_parts(
        self,
    ) -> (
        SourceIdentity,
        SourceDisplayLocator,
        String,
        SourceContentDigest,
    ) {
        (
            self.identity,
            self.display_locator,
            self.source_text,
            self.content_digest,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredManifestSources {
    first: DiscoveredSource,
    rest: Vec<DiscoveredSource>,
    directories: Vec<ConeRelativePath>,
}

impl DiscoveredManifestSources {
    pub fn selected_directories(&self) -> &[ConeRelativePath] {
        &self.directories
    }

    pub const fn first(&self) -> &DiscoveredSource {
        &self.first
    }

    pub fn iter(&self) -> impl Iterator<Item = &DiscoveredSource> {
        std::iter::once(&self.first).chain(&self.rest)
    }

    pub fn into_parts(self) -> (DiscoveredSource, Vec<DiscoveredSource>) {
        (self.first, self.rest)
    }
}

struct SourceCandidate {
    logical_path: NormalizedSourcePath,
    physical_path: PathBuf,
    display_path: PathBuf,
    selection: String,
}

#[derive(Debug)]
struct ResolvedEntry {
    path: PathBuf,
    metadata: std::fs::Metadata,
}

pub fn discover_manifest_sources(
    manifest: &LoadedConeManifest,
    target: TargetProfileId,
) -> Result<DiscoveredManifestSources, SourceDiscoveryError> {
    let roots = selected_roots(manifest, target)?;
    let mut candidates = Vec::new();
    let mut directories = Vec::new();
    for root in roots {
        if root.resolved.metadata.is_dir() {
            walk_directory(
                &root.resolved.path,
                root.path.as_path(),
                manifest.real_root(),
                &root.selection,
                &mut BTreeSet::new(),
                &mut candidates,
            )?;
            directories.push(root.path);
        } else {
            let logical_path = NormalizedSourcePath::new(root.path.as_str()).map_err(|error| {
                SourceDiscoveryError::new(
                    root.resolved.path.clone(),
                    SourceDiscoveryErrorKind::InvalidLogicalPath(error),
                )
            })?;
            candidates.push(SourceCandidate {
                logical_path,
                physical_path: root.resolved.path,
                display_path: manifest.real_root().join(root.path.as_path()),
                selection: root.selection,
            });
        }
    }
    candidates.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
    let mut physical_sources: BTreeMap<&Path, &SourceCandidate> = BTreeMap::new();
    for candidate in &candidates {
        if let Some(first) = physical_sources.insert(&candidate.physical_path, candidate) {
            return Err(SourceDiscoveryError::new(
                candidate.display_path.clone(),
                SourceDiscoveryErrorKind::ConflictingSelections {
                    first: format!("{}: {}", first.selection, first.logical_path),
                    second: format!("{}: {}", candidate.selection, candidate.logical_path),
                },
            ));
        }
    }
    let cone = manifest.identity();
    let mut candidates = candidates.into_iter();
    let first_candidate = candidates.next().ok_or_else(|| {
        SourceDiscoveryError::new(
            manifest.manifest_path().to_path_buf(),
            SourceDiscoveryErrorKind::EmptySourceSet,
        )
    })?;
    let first = read_discovered_source(first_candidate, cone)?;
    let rest = candidates
        .map(|candidate| read_discovered_source(candidate, cone))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DiscoveredManifestSources {
        first,
        rest,
        directories,
    })
}

#[cfg(test)]
mod tests;
