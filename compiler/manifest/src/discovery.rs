use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};

use scoop_identity::{
    NormalizedSourcePath, NormalizedSourcePathError, SourceContentDigest, SourceIdentity,
};

use crate::LoadedConeManifest;

const MAX_SYMLINK_DEPTH: usize = 64;

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredManifestSources(Vec<DiscoveredSource>);

impl DiscoveredManifestSources {
    pub fn as_slice(&self) -> &[DiscoveredSource] {
        &self.0
    }

    pub fn iter(&self) -> impl Iterator<Item = &DiscoveredSource> {
        self.0.iter()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryIoOperation {
    Inspect,
    Canonicalize,
    ListDirectory,
    ReadLink,
    ReadSource,
}

impl fmt::Display for DiscoveryIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Inspect => "inspect",
            Self::Canonicalize => "canonicalize",
            Self::ListDirectory => "list directory",
            Self::ReadLink => "read symlink",
            Self::ReadSource => "read source",
        })
    }
}

#[derive(Debug)]
pub struct SourceDiscoveryError {
    path: PathBuf,
    kind: SourceDiscoveryErrorKind,
}

impl SourceDiscoveryError {
    pub(crate) fn new(path: PathBuf, kind: SourceDiscoveryErrorKind) -> Self {
        Self { path, kind }
    }

    pub(crate) fn io(
        operation: DiscoveryIoOperation,
        path: PathBuf,
        source: std::io::Error,
    ) -> Self {
        Self::new(path, SourceDiscoveryErrorKind::Io { operation, source })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn kind(&self) -> &SourceDiscoveryErrorKind {
        &self.kind
    }
}

#[derive(Debug)]
pub enum SourceDiscoveryErrorKind {
    SourceRootNotDirectory,
    NonUtf8EntryName,
    EscapesSourceRoot,
    SymlinkDepthExceeded,
    DirectoryCycle,
    InvalidLogicalPath(NormalizedSourcePathError),
    DuplicateLogicalPath,
    InvalidUtf8Source,
    SourceChangedDuringDiscovery,
    EmptySourceSet,
    IdentityHash(String),
    InvalidSourceIdentity(String),
    Io {
        operation: DiscoveryIoOperation,
        source: std::io::Error,
    },
}

impl fmt::Display for SourceDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "source discovery failed at {}: ",
            self.path.display()
        )?;
        match &self.kind {
            SourceDiscoveryErrorKind::SourceRootNotDirectory => {
                formatter.write_str("resolved src root is not a directory")
            }
            SourceDiscoveryErrorKind::NonUtf8EntryName => {
                formatter.write_str("directory entry name is not valid UTF-8")
            }
            SourceDiscoveryErrorKind::EscapesSourceRoot => {
                formatter.write_str("symlink target escapes the resolved src root")
            }
            SourceDiscoveryErrorKind::SymlinkDepthExceeded => {
                formatter.write_str("symlink depth exceeds the limit of 64")
            }
            SourceDiscoveryErrorKind::DirectoryCycle => {
                formatter.write_str("symlink traversal creates a directory cycle")
            }
            SourceDiscoveryErrorKind::InvalidLogicalPath(error) => error.fmt(formatter),
            SourceDiscoveryErrorKind::DuplicateLogicalPath => {
                formatter.write_str("duplicate normalized source path")
            }
            SourceDiscoveryErrorKind::InvalidUtf8Source => {
                formatter.write_str("source file is not valid UTF-8")
            }
            SourceDiscoveryErrorKind::SourceChangedDuringDiscovery => {
                formatter.write_str("source target changed during discovery")
            }
            SourceDiscoveryErrorKind::EmptySourceSet => {
                formatter.write_str("manifest Cone must contain at least one .scoop source")
            }
            SourceDiscoveryErrorKind::IdentityHash(error) => {
                write!(formatter, "cannot derive Cone identity: {error}")
            }
            SourceDiscoveryErrorKind::InvalidSourceIdentity(error) => {
                write!(formatter, "cannot construct source identity: {error}")
            }
            SourceDiscoveryErrorKind::Io { operation, source } => {
                write!(formatter, "cannot {operation}: {source}")
            }
        }
    }
}

impl std::error::Error for SourceDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            SourceDiscoveryErrorKind::InvalidLogicalPath(error) => Some(error),
            SourceDiscoveryErrorKind::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

struct SourceCandidate {
    logical_path: NormalizedSourcePath,
    physical_path: PathBuf,
    display_path: PathBuf,
}

struct ResolvedEntry {
    path: PathBuf,
    metadata: std::fs::Metadata,
    symlink_depth: usize,
}

pub fn discover_manifest_sources(
    manifest: &LoadedConeManifest,
) -> Result<DiscoveredManifestSources, SourceDiscoveryError> {
    let src_locator = manifest.real_root().join("src");
    let real_src = std::fs::canonicalize(&src_locator).map_err(|error| {
        SourceDiscoveryError::io(
            DiscoveryIoOperation::Canonicalize,
            src_locator.clone(),
            error,
        )
    })?;
    let src_metadata = std::fs::metadata(&real_src).map_err(|error| {
        SourceDiscoveryError::io(DiscoveryIoOperation::Inspect, real_src.clone(), error)
    })?;
    if !src_metadata.is_dir() {
        return Err(SourceDiscoveryError::new(
            src_locator,
            SourceDiscoveryErrorKind::SourceRootNotDirectory,
        ));
    }

    let mut active_directories = BTreeSet::new();
    let mut candidates = Vec::new();
    walk_directory(
        &real_src,
        Path::new("src"),
        &real_src,
        0,
        &mut active_directories,
        &mut candidates,
    )?;

    candidates.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
    for pair in candidates.windows(2) {
        if pair[0].logical_path == pair[1].logical_path {
            return Err(SourceDiscoveryError::new(
                pair[1].display_path.clone(),
                SourceDiscoveryErrorKind::DuplicateLogicalPath,
            ));
        }
    }
    if candidates.is_empty() {
        return Err(SourceDiscoveryError::new(
            src_locator,
            SourceDiscoveryErrorKind::EmptySourceSet,
        ));
    }

    let cone = manifest
        .parsed()
        .semantic()
        .coordinate()
        .identity()
        .map_err(|error| {
            SourceDiscoveryError::new(
                manifest.manifest_path().to_path_buf(),
                SourceDiscoveryErrorKind::IdentityHash(error.to_string()),
            )
        })?;
    let mut sources = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let bytes = std::fs::read(&candidate.physical_path).map_err(|error| {
            SourceDiscoveryError::io(
                DiscoveryIoOperation::ReadSource,
                candidate.display_path.clone(),
                error,
            )
        })?;
        let after_read = std::fs::canonicalize(&candidate.physical_path).map_err(|error| {
            SourceDiscoveryError::io(
                DiscoveryIoOperation::Canonicalize,
                candidate.display_path.clone(),
                error,
            )
        })?;
        if after_read != candidate.physical_path
            || !std::fs::metadata(&after_read)
                .map_err(|error| {
                    SourceDiscoveryError::io(
                        DiscoveryIoOperation::Inspect,
                        candidate.display_path.clone(),
                        error,
                    )
                })?
                .is_file()
        {
            return Err(SourceDiscoveryError::new(
                candidate.display_path,
                SourceDiscoveryErrorKind::SourceChangedDuringDiscovery,
            ));
        }
        let source_text = String::from_utf8(bytes).map_err(|_| {
            SourceDiscoveryError::new(
                candidate.display_path.clone(),
                SourceDiscoveryErrorKind::InvalidUtf8Source,
            )
        })?;
        let identity = SourceIdentity::new(cone, candidate.logical_path).map_err(|error| {
            SourceDiscoveryError::new(
                candidate.display_path.clone(),
                SourceDiscoveryErrorKind::InvalidSourceIdentity(error.to_string()),
            )
        })?;
        sources.push(DiscoveredSource::new(
            identity,
            SourceDisplayLocator::new(candidate.display_path),
            source_text,
        ));
    }

    Ok(DiscoveredManifestSources(sources))
}

fn walk_directory(
    real_directory: &Path,
    logical_directory: &Path,
    real_src: &Path,
    symlink_depth: usize,
    active_directories: &mut BTreeSet<PathBuf>,
    candidates: &mut Vec<SourceCandidate>,
) -> Result<(), SourceDiscoveryError> {
    if !active_directories.insert(real_directory.to_path_buf()) {
        return Err(SourceDiscoveryError::new(
            real_directory.to_path_buf(),
            SourceDiscoveryErrorKind::DirectoryCycle,
        ));
    }

    let result = walk_active_directory(
        real_directory,
        logical_directory,
        real_src,
        symlink_depth,
        active_directories,
        candidates,
    );
    active_directories.remove(real_directory);
    result
}

fn walk_active_directory(
    real_directory: &Path,
    logical_directory: &Path,
    real_src: &Path,
    symlink_depth: usize,
    active_directories: &mut BTreeSet<PathBuf>,
    candidates: &mut Vec<SourceCandidate>,
) -> Result<(), SourceDiscoveryError> {
    let entries = std::fs::read_dir(real_directory).map_err(|error| {
        SourceDiscoveryError::io(
            DiscoveryIoOperation::ListDirectory,
            real_directory.to_path_buf(),
            error,
        )
    })?;
    let mut entries = entries
        .map(|entry| {
            entry.map_err(|error| {
                SourceDiscoveryError::io(
                    DiscoveryIoOperation::ListDirectory,
                    real_directory.to_path_buf(),
                    error,
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for entry in &entries {
        if validated_entry_name(&entry.file_name()).is_none() {
            return Err(SourceDiscoveryError::new(
                entry.path(),
                SourceDiscoveryErrorKind::NonUtf8EntryName,
            ));
        }
    }
    entries.sort_by(|left, right| {
        left.file_name()
            .to_str()
            .expect("entry names were validated as UTF-8")
            .as_bytes()
            .cmp(
                right
                    .file_name()
                    .to_str()
                    .expect("entry names were validated as UTF-8")
                    .as_bytes(),
            )
    });

    for entry in entries {
        let name = entry.file_name();
        let name = name.to_str().expect("entry names were validated as UTF-8");
        let logical_path = logical_directory.join(name);
        let resolved = resolve_entry(&entry.path(), symlink_depth)?;
        if !resolved.path.starts_with(real_src) {
            return Err(SourceDiscoveryError::new(
                entry.path(),
                SourceDiscoveryErrorKind::EscapesSourceRoot,
            ));
        }

        if resolved.metadata.is_dir() {
            walk_directory(
                &resolved.path,
                &logical_path,
                real_src,
                resolved.symlink_depth,
                active_directories,
                candidates,
            )?;
        } else if resolved.metadata.is_file()
            && logical_path.extension() == Some(OsStr::new("scoop"))
        {
            let normalized =
                NormalizedSourcePath::from_relative_path(&logical_path).map_err(|error| {
                    SourceDiscoveryError::new(
                        entry.path(),
                        SourceDiscoveryErrorKind::InvalidLogicalPath(error),
                    )
                })?;
            candidates.push(SourceCandidate {
                logical_path: normalized,
                physical_path: resolved.path,
                display_path: entry.path(),
            });
        }
    }
    Ok(())
}

fn validated_entry_name(name: &OsStr) -> Option<&str> {
    name.to_str()
}

fn resolve_entry(
    path: &Path,
    inherited_symlink_depth: usize,
) -> Result<ResolvedEntry, SourceDiscoveryError> {
    let mut current = path.to_path_buf();
    let mut depth = inherited_symlink_depth;
    loop {
        let metadata = std::fs::symlink_metadata(&current).map_err(|error| {
            SourceDiscoveryError::io(DiscoveryIoOperation::Inspect, current.clone(), error)
        })?;
        if !metadata.file_type().is_symlink() {
            let canonical = std::fs::canonicalize(&current).map_err(|error| {
                SourceDiscoveryError::io(DiscoveryIoOperation::Canonicalize, current.clone(), error)
            })?;
            let metadata = std::fs::metadata(&canonical).map_err(|error| {
                SourceDiscoveryError::io(DiscoveryIoOperation::Inspect, canonical.clone(), error)
            })?;
            return Ok(ResolvedEntry {
                path: canonical,
                metadata,
                symlink_depth: depth,
            });
        }
        depth = depth.checked_add(1).ok_or_else(|| {
            SourceDiscoveryError::new(
                path.to_path_buf(),
                SourceDiscoveryErrorKind::SymlinkDepthExceeded,
            )
        })?;
        if depth > MAX_SYMLINK_DEPTH {
            return Err(SourceDiscoveryError::new(
                path.to_path_buf(),
                SourceDiscoveryErrorKind::SymlinkDepthExceeded,
            ));
        }
        let target = std::fs::read_link(&current).map_err(|error| {
            SourceDiscoveryError::io(DiscoveryIoOperation::ReadLink, current.clone(), error)
        })?;
        current = if target.is_absolute() {
            target
        } else {
            current
                .parent()
                .expect("a directory entry always has a parent")
                .join(target)
        };
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::{ManifestRootLocator, load_cone_manifest};

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempCone(PathBuf);

    impl TempCone {
        fn new() -> Self {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "scoop-manifest-discovery-{}-{serial}",
                std::process::id()
            ));
            std::fs::create_dir_all(path.join("src")).unwrap();
            std::fs::write(
                path.join("Cone.toml"),
                "schema = 1\n[cone]\ngroup = \"dev.example\"\nname = \"lib\"\nversion = \"1.0.0\"\nkind = \"library\"\n",
            )
            .unwrap();
            Self(path)
        }

        fn load(&self) -> LoadedConeManifest {
            load_cone_manifest(&ManifestRootLocator::cone_directory(&self.0)).unwrap()
        }
    }

    impl Drop for TempCone {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn recursively_discovers_sources_in_logical_path_order() {
        let cone = TempCone::new();
        std::fs::create_dir(cone.0.join("src/z")).unwrap();
        std::fs::write(cone.0.join("src/z/b.scoop"), "package z\n").unwrap();
        std::fs::write(cone.0.join("src/a.scoop"), "package a\n").unwrap();
        std::fs::write(cone.0.join("src/ignored.txt"), "ignored").unwrap();

        let sources = discover_manifest_sources(&cone.load()).unwrap();
        let paths = sources
            .iter()
            .map(|source| source.identity().logical_path().as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, ["src/a.scoop", "src/z/b.scoop"]);
        assert_eq!(
            sources.as_slice()[0].content_digest(),
            SourceContentDigest::from_utf8("package a\n")
        );
    }

    #[test]
    fn rejects_an_empty_source_tree() {
        let cone = TempCone::new();
        assert!(matches!(
            discover_manifest_sources(&cone.load()).unwrap_err().kind(),
            SourceDiscoveryErrorKind::EmptySourceSet
        ));
    }

    #[test]
    fn rejects_invalid_utf8_source_text() {
        let cone = TempCone::new();
        std::fs::write(cone.0.join("src/bad.scoop"), [0xff]).unwrap();
        assert!(matches!(
            discover_manifest_sources(&cone.load()).unwrap_err().kind(),
            SourceDiscoveryErrorKind::InvalidUtf8Source
        ));
    }

    #[cfg(unix)]
    #[test]
    fn preserves_distinct_logical_paths_to_the_same_real_source() {
        let cone = TempCone::new();
        let target = cone.0.join("src/shared.scoop");
        std::fs::write(&target, "fun shared() {}\n").unwrap();
        std::os::unix::fs::symlink("shared.scoop", cone.0.join("src/alias.scoop")).unwrap();

        let sources = discover_manifest_sources(&cone.load()).unwrap();
        let paths = sources
            .iter()
            .map(|source| source.identity().logical_path().as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, ["src/alias.scoop", "src/shared.scoop"]);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_that_escapes_the_source_root() {
        let cone = TempCone::new();
        let outside = cone.0.join("outside.scoop");
        std::fs::write(&outside, "fun outside() {}\n").unwrap();
        std::os::unix::fs::symlink("../outside.scoop", cone.0.join("src/escape.scoop")).unwrap();

        assert!(matches!(
            discover_manifest_sources(&cone.load()).unwrap_err().kind(),
            SourceDiscoveryErrorKind::EscapesSourceRoot
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_directory_cycle() {
        let cone = TempCone::new();
        std::fs::write(cone.0.join("src/source.scoop"), "fun source() {}\n").unwrap();
        std::os::unix::fs::symlink(".", cone.0.join("src/again")).unwrap();

        assert!(matches!(
            discover_manifest_sources(&cone.load()).unwrap_err().kind(),
            SourceDiscoveryErrorKind::DirectoryCycle
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_directory_entries() {
        use std::os::unix::ffi::OsStringExt;

        let invalid = std::ffi::OsString::from_vec(vec![0xff]);
        assert!(validated_entry_name(&invalid).is_none());
    }
}
