use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};

use scoop_identity::{
    NormalizedSourcePath, NormalizedSourcePathError, SourceContentDigest, SourceIdentity,
    SourceRole, TargetProfileId,
};

use crate::{ConeRelativePath, LoadedConeManifest, SourceSelection};

mod collect;
mod error;
mod read;
mod select;
mod selected;
mod walk;
pub use error::{DiscoveryIoOperation, SourceDiscoveryError, SourceDiscoveryErrorKind};
use select::selected_roots;
pub use selected::{SelectedManifestSources, SelectedSource};
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
    role: SourceRole,
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
            role: SourceRole::Production,
        }
    }

    pub const fn role(&self) -> SourceRole {
        self.role
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
        SourceRole,
    ) {
        (
            self.identity,
            self.display_locator,
            self.source_text,
            self.content_digest,
            self.role,
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
    metadata: std::fs::Metadata,
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
    select_manifest_sources(manifest, target)?.read()
}

/// Select production and test inputs without changing their Cone identity.
pub fn discover_manifest_sources_for_tests(
    manifest: &LoadedConeManifest,
    target: TargetProfileId,
) -> Result<DiscoveredManifestSources, SourceDiscoveryError> {
    select_manifest_sources_for_tests(manifest, target)?.read()
}

pub fn select_manifest_sources(
    manifest: &LoadedConeManifest,
    target: TargetProfileId,
) -> Result<SelectedManifestSources, SourceDiscoveryError> {
    select_cone_sources(
        manifest.real_root(),
        manifest.identity(),
        manifest.parsed().semantic().sources(),
        None,
        target,
    )
}

pub fn select_manifest_sources_for_tests(
    manifest: &LoadedConeManifest,
    target: TargetProfileId,
) -> Result<SelectedManifestSources, SourceDiscoveryError> {
    select_cone_sources(
        manifest.real_root(),
        manifest.identity(),
        manifest.parsed().semantic().sources(),
        Some(manifest.parsed().tests().sources()),
        target,
    )
}

/// Source selection also serves local editor members whose dependencies are unresolved.
pub fn select_cone_sources(
    root: &Path,
    identity: scoop_identity::ConeIdentity,
    sources: &SourceSelection,
    tests: Option<&SourceSelection>,
    target: TargetProfileId,
) -> Result<SelectedManifestSources, SourceDiscoveryError> {
    collect::select(
        root,
        identity,
        selected_roots(root, sources, tests, target)?,
    )
}

/// Select the union of all declared production and test paths for formatting.
/// Conditions are validated while loading the manifest, without selecting a target.
pub fn select_manifest_format_sources(
    manifest: &LoadedConeManifest,
) -> Result<Vec<PathBuf>, SourceDiscoveryError> {
    select_format_sources(
        manifest.real_root(),
        manifest.parsed().semantic().sources(),
        manifest.parsed().tests().sources(),
    )
}

/// Local formatting only needs a member directory and expanded selections.
pub fn select_format_sources(
    root: &Path,
    sources: &SourceSelection,
    tests: &SourceSelection,
) -> Result<Vec<PathBuf>, SourceDiscoveryError> {
    collect::formatting(root, select::formatting_roots(root, sources, tests)?)
}

#[cfg(test)]
mod tests;
